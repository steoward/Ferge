use ow_asset::glb::{bin_payload, json_payload, read_glb_chunks, GLB_MAGIC, CHUNK_TYPE_JSON, CHUNK_TYPE_BIN};
use ow_asset::json::{parse_json, JsonValue};
use ow_asset::loader::load_glb;
use ow_asset::model::{AccessorType, ComponentType, Interpolation, TargetPath};
use ow_math::{Quat, Vec3};

// ---------------------------------------------------------------------------
// JSON reader
// ---------------------------------------------------------------------------

#[test]
fn json_parses_all_scalar_types() {
    let text = r#"{"a":1,"b":-2.5,"c":"hi","d":true,"e":false,"f":null}"#;
    let v = parse_json(text).expect("valid json");

    assert_eq!(v.get("a").unwrap().as_f64(), Some(1.0));
    assert_eq!(v.get("b").unwrap().as_f64(), Some(-2.5));
    assert_eq!(v.get("c").unwrap().as_str(), Some("hi"));
    assert_eq!(v.get("d").unwrap().as_bool(), Some(true));
    assert_eq!(v.get("e").unwrap().as_bool(), Some(false));
    assert!(matches!(v.get("f"), Some(JsonValue::Null)));
}

#[test]
fn json_parses_nested_structures() {
    let text = r#"{"outer":{"inner":[1,2,{"deep":"value"}]}}"#;
    let v = parse_json(text).expect("valid json");

    let outer = v.get_object("outer").unwrap();
    let inner = outer.get_array("inner").unwrap();
    assert_eq!(inner.len(), 3);
    assert_eq!(inner[2].get_str("deep"), Some("value"));
}

#[test]
fn json_handles_escapes_and_unicode() {
    let text = r#"{"s":"line\nbreak \"quoted\" \u00e9 back\\slash"}"#;
    let v = parse_json(text).expect("valid json");
    assert_eq!(v.get_str("s"), Some("line\nbreak \"quoted\" é back\\slash"));
}

#[test]
fn json_handles_surrogate_pairs() {
    // A surrogate pair: 😀 is U+1F600.
    let text = r#"{"s":"\ud83d\ude00"}"#;
    let v = parse_json(text).expect("valid json");
    assert_eq!(v.get_str("s"), Some("😀"));
}

#[test]
fn json_rejects_malformed_input_with_offsets() {
    assert!(parse_json("{\"a\":}").is_err());
    assert!(parse_json("{\"a\":1").is_err());
    assert!(parse_json("[1,2").is_err());
    assert!(parse_json("").is_err());
    assert!(parse_json("{\"a\":1} extra").is_err(), "trailing content must be rejected");

    // The error carries a byte offset.
    let err = parse_json("{\"a\":1,}").unwrap_err();
    assert!(err.offset > 0);
}

#[test]
fn json_number_array_and_missing_keys() {
    let text = r#"{"n":[1,2,3]}"#;
    let v = parse_json(text).unwrap();
    assert_eq!(v.get("n").unwrap().number_array(), vec![1.0, 2.0, 3.0]);
    assert!(v.get("missing").is_none());
    assert_eq!(v.get_usize("missing", 7), 7, "default must apply for absent keys");
}

// ---------------------------------------------------------------------------
// GLB container
// ---------------------------------------------------------------------------

/// Builds a minimal valid GLB around a JSON document.
fn build_glb(json: &str, bin: &[u8]) -> Vec<u8> {
    let json_bytes = json.as_bytes();
    let json_padded = (json_bytes.len() + 3) & !3;
    let bin_padded = (bin.len() + 3) & !3;

    let mut out = Vec::new();
    let total = 12 + 8 + json_padded + 8 + bin_padded;

    out.extend_from_slice(&GLB_MAGIC.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());

    // JSON chunk
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_TYPE_JSON.to_le_bytes());
    out.extend_from_slice(json_bytes);
    out.extend(std::iter::repeat(b' ').take(json_padded - json_bytes.len()));

    // BIN chunk
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_TYPE_BIN.to_le_bytes());
    out.extend_from_slice(bin);
    out.extend(std::iter::repeat(0u8).take(bin_padded - bin.len()));

    out
}

#[test]
fn glb_round_trips_json_and_binary() {
    let json = r#"{"asset":{"version":"2.0"}}"#;
    let bin = [1u8, 2, 3, 4, 5, 6, 7, 8];

    let bytes = build_glb(json, &bin);
    let (header, chunks) = read_glb_chunks(&bytes).expect("valid glb");

    assert_eq!(header.magic, GLB_MAGIC);
    assert_eq!(header.version, 2);
    assert_eq!(json_payload(&bytes, &chunks).unwrap(), json);
    assert_eq!(bin_payload(&bytes, &chunks), &bin[..]);
}

#[test]
fn glb_rejects_bad_magic_and_version() {
    let mut bytes = build_glb(r#"{"asset":{"version":"2.0"}}"#, &[]);
    bytes[0] = b'X';
    assert!(read_glb_chunks(&bytes).is_err(), "bad magic must be rejected");

    let mut bytes = build_glb(r#"{"asset":{"version":"2.0"}}"#, &[]);
    bytes[4] = 1; // version 1
    assert!(read_glb_chunks(&bytes).is_err(), "version 1 must be rejected");
}

#[test]
fn glb_rejects_truncated_files() {
    assert!(read_glb_chunks(&[0u8; 4]).is_err(), "shorter than the header");
    assert!(read_glb_chunks(&[]).is_err());

    // A file whose declared length exceeds its actual size.
    let mut bytes = build_glb(r#"{"asset":{"version":"2.0"}}"#, &[]);
    let declared = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    bytes[8..12].copy_from_slice(&(declared + 1000).to_le_bytes());
    assert!(read_glb_chunks(&bytes).is_err());
}

// ---------------------------------------------------------------------------
// glTF document
// ---------------------------------------------------------------------------

/// A small glTF with one triangle mesh.
const TRIANGLE_GLTF: &str = r#"
{
  "asset": {"version": "2.0", "generator": "test"},
  "scene": 0,
  "scenes": [{"nodes": [0]}],
  "nodes": [{"mesh": 0}],
  "meshes": [{"primitives": [{"attributes": {"POSITION": 1}, "indices": 0}]}],
  "accessors": [
    {"bufferView": 0, "componentType": 5123, "count": 3, "type": "SCALAR"},
    {"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3",
     "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0]}
  ],
  "bufferViews": [
    {"buffer": 0, "byteOffset": 0, "byteLength": 6},
    {"buffer": 0, "byteOffset": 8, "byteLength": 36}
  ],
  "buffers": [{"byteLength": 64}]
}
"#;

#[test]
fn gltf_parses_document_structure() {
    let bin = vec![0u8; 64];
    let model = ow_asset::loader::load_gltf_from_bytes(TRIANGLE_GLTF, bin).expect("valid gltf");

    let doc = &model.document;
    assert_eq!(doc.asset.version, "2.0");
    assert_eq!(doc.asset.generator.as_deref(), Some("test"));
    assert_eq!(doc.nodes.len(), 1);
    assert_eq!(doc.meshes.len(), 1);
    assert_eq!(doc.accessors.len(), 2);

    assert_eq!(doc.meshes[0].primitives.len(), 1);
    assert_eq!(doc.meshes[0].primitives[0].positions(), Some(1));
    assert_eq!(doc.meshes[0].primitives[0].indices, Some(0));

    assert_eq!(doc.accessors[0].component_type, ComponentType::UnsignedShort);
    assert_eq!(doc.accessors[0].accessor_type, AccessorType::Scalar);
    assert_eq!(doc.accessors[1].accessor_type, AccessorType::Vec3);
    assert_eq!(doc.accessors[1].count, 3);
    assert_eq!(doc.accessors[1].byte_length(), 3 * 3 * 4);

    // Bounds are preserved: the renderer uses them for conservative culling.
    assert_eq!(doc.accessors[1].min, vec![0.0, 0.0, 0.0]);
    assert_eq!(doc.accessors[1].max, vec![1.0, 1.0, 0.0]);

    assert_eq!(model.scene_roots(), vec![0]);
    assert_eq!(model.triangle_count(), 1);
}

#[test]
fn gltf_reads_float_positions() {
    // Three vertices at (0,0,0), (1,0,0), (0,1,0) stored as f32 LE at offset 8.
    let mut bin = vec![0u8; 64];
    let verts: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let mut at = 8usize;
    for v in verts {
        for c in v {
            bin[at..at + 4].copy_from_slice(&c.to_le_bytes());
            at += 4;
        }
    }

    let model = ow_asset::loader::load_gltf_from_bytes(TRIANGLE_GLTF, bin).unwrap();
    let positions = model.accessor_f32(1).expect("positions");

    assert_eq!(positions.len(), 9);
    assert!((positions[3] - 1.0).abs() < 1e-6, "v1.x should be 1.0");
    assert!((positions[7] - 1.0).abs() < 1e-6, "v2.y should be 1.0");
}

#[test]
fn gltf_reads_unsigned_short_indices() {
    let mut bin = vec![0u8; 64];
    // Indices 0, 1, 2 at offset 0 as u16 LE.
    for (i, idx) in [0u16, 1, 2].iter().enumerate() {
        let bytes = idx.to_le_bytes();
        bin[i * 2..i * 2 + 2].copy_from_slice(&bytes);
    }

    let model = ow_asset::loader::load_gltf_from_bytes(TRIANGLE_GLTF, bin).unwrap();
    let indices = model.accessor_u32(0).expect("indices");
    assert_eq!(indices, vec![0, 1, 2]);
}

#[test]
fn gltf_rejects_old_versions() {
    let json = r#"{"asset": {"version": "1.0"}}"#;
    let err = ow_asset::loader::load_gltf_from_bytes(json, vec![]).unwrap_err();
    assert!(
        err.to_string().contains("unsupported") || err.to_string().contains("glTF version"),
        "expected an unsupported-version error, got {}",
        err
    );
}

// ---------------------------------------------------------------------------
// Skinned mesh: the full pipeline
// ---------------------------------------------------------------------------

/// A 3-bone skeleton with a 2-vertex mesh bound 50/50 to bones 0 and 1.
fn build_skinned_glb() -> Vec<u8> {
    // Binary layout:
    //   0..   8  indices (4 x u16)
    //   8..   44 positions (2 x VEC3)
    //   44..  76 weights (2 x VEC2, f32)
    //   76.. 84  joints (2 x VEC2, u8)
    //   84..  100 animation input times (2 x f32)
    //   100.. 116 animation output translations (2 x VEC3)
    let mut bin = vec![0u8; 256];

    for (i, idx) in [0u16, 1].iter().enumerate() {
        bin[i * 2..i * 2 + 2].copy_from_slice(&idx.to_le_bytes());
    }

    let positions: [[f32; 3]; 2] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
    let mut at = 8usize;
    for v in positions {
        for c in v {
            bin[at..at + 4].copy_from_slice(&c.to_le_bytes());
            at += 4;
        }
    }

    // 50/50 weights.
    at = 44;
    for _ in 0..2 {
        for w in [0.5f32, 0.5] {
            bin[at..at + 4].copy_from_slice(&w.to_le_bytes());
            at += 4;
        }
    }

    // Joints 0 and 1 for both vertices, as VEC2 of u8 (4 bytes each).
    bin[76] = 0;
    bin[77] = 1;
    bin[78] = 0;
    bin[79] = 1;

    // Animation times and translations.
    let times = [0.0f32, 1.0];
    at = 84;
    for t in times {
        bin[at..at + 4].copy_from_slice(&t.to_le_bytes());
        at += 4;
    }
    let translations: [[f32; 3]; 2] = [[0.0, 0.0, 0.0], [0.0, 5.0, 0.0]];
    at = 100;
    for v in translations {
        for c in v {
            bin[at..at + 4].copy_from_slice(&c.to_le_bytes());
            at += 4;
        }
    }

    let json = r#"
{
  "asset": {"version": "2.0"},
  "scene": 0,
  "scenes": [{"nodes": [0]}],
  "nodes": [
    {"name": "skinned-mesh", "mesh": 0, "skin": 0, "translation": [0, 0, 1]},
    {"name": "bone-a"},
    {"name": "bone-b"}
  ],
  "meshes": [{"name": "arm", "primitives": [{
      "attributes": {"POSITION": 1, "WEIGHTS_0": 2, "JOINTS_0": 3},
      "indices": 0
  }]}],
  "skins": [{
      "joints": [1, 2],
      "skeleton": 1
  }],
  "animations": [{
      "name": "lift",
      "samplers": [{"input": 4, "output": 5, "interpolation": "LINEAR"}],
      "channels": [{"sampler": 0, "target": {"node": 1, "path": "translation"}}]
  }],
  "accessors": [
    {"bufferView": 0, "componentType": 5123, "count": 2, "type": "SCALAR"},
    {"bufferView": 1, "componentType": 5126, "count": 2, "type": "VEC3",
     "min": [0,0,0], "max": [1,0,0]},
    {"bufferView": 2, "componentType": 5126, "count": 2, "type": "VEC2"},
    {"bufferView": 3, "componentType": 5121, "count": 2, "type": "VEC2"},
    {"bufferView": 4, "componentType": 5126, "count": 2, "type": "SCALAR"},
    {"bufferView": 5, "componentType": 5126, "count": 2, "type": "VEC3"}
  ],
  "bufferViews": [
    {"buffer": 0, "byteOffset": 0,   "byteLength": 4},
    {"buffer": 0, "byteOffset": 8,   "byteLength": 24},
    {"buffer": 0, "byteOffset": 44,  "byteLength": 16},
    {"buffer": 0, "byteOffset": 76,  "byteLength": 4},
    {"buffer": 0, "byteOffset": 84,  "byteLength": 8},
    {"buffer": 0, "byteOffset": 100, "byteLength": 24}
  ],
  "buffers": [{"byteLength": 256}]
}
"#;

    build_glb(json, &bin)
}

#[test]
fn glb_loads_skinned_mesh_end_to_end() {
    let bytes = build_skinned_glb();
    let model = load_glb(&bytes).expect("skinned glb");

    let doc = &model.document;

    // Structure.
    assert_eq!(doc.meshes[0].name.as_deref(), Some("arm"));
    assert_eq!(doc.skins.len(), 1);
    assert_eq!(doc.skins[0].joints, vec![1, 2], "joint references are node indices");
    assert_eq!(doc.animations.len(), 1);
    assert_eq!(doc.animations[0].name.as_deref(), Some("lift"));
    assert_eq!(doc.animations[0].channels[0].target_path, TargetPath::Translation);
    assert_eq!(doc.animations[0].samplers[0].interpolation, Interpolation::Linear);

    // The node that carries the mesh also carries the skin reference.
    assert_eq!(doc.nodes[0].skin, Some(0));
    assert_eq!(doc.nodes_using_skin(0), vec![0]);

    // Vertex attributes.
    let weights = model.accessor_f32(2).expect("weights");
    assert_eq!(weights, vec![0.5, 0.5, 0.5, 0.5], "50/50 split across two bones");

    let joints = model.accessor_joints(3).expect("joint indices");
    assert_eq!(joints, vec![0, 1, 0, 1]);

    // Animation data: linearly interpolate a lift from y=0 to y=5.
    let times = model.accessor_f32(4).expect("times");
    assert_eq!(times, vec![0.0, 1.0]);
    let translations = model.accessor_f32(5).expect("translations");
    assert!((translations[4] - 5.0).abs() < 1e-6, "second key lifts to y=5");
}

#[test]
fn glb_accessors_respect_buffer_view_offsets() {
    let bytes = build_skinned_glb();
    let model = load_glb(&bytes).unwrap();

    // Accessor 0 (indices) lives in bufferView 0 at byte 0; accessor 4 (times)
    // lives at byte 84; accessor 5 (translations) lives at byte 100.
    let indices = model.accessor_u32(0).unwrap();
    assert_eq!(indices, vec![0, 1]);
    let times = model.accessor_f32(4).unwrap();
    assert_eq!(times, vec![0.0, 1.0]);
    let translations = model.accessor_f32(5).unwrap();
    assert_eq!(translations[4], 5.0);
}

// ---------------------------------------------------------------------------
// Feeding loaded data into the animation system
// ---------------------------------------------------------------------------

#[test]
fn loaded_glb_drives_the_animation_system() {
    // The point of the loader: turning a file into a skeleton + clip the
    // animation crate can actually evaluate.
    use ow_animation::clip::{AnimationClip, WrapMode};
    use ow_animation::skeleton::Skeleton;
    use ow_animation::skinning::*;
    use ow_animation::track::{Interpolation as AInterp, Keyframe, TransformTrack};
    use ow_math::Transform;

    let bytes = build_skinned_glb();
    let model = load_glb(&bytes).unwrap();
    let doc = &model.document;

    // Build a skeleton from the skin's joint node references.
    let mut skeleton = Skeleton::new();
    for joint in &doc.skins[0].joints {
        let node = doc.node(*joint).unwrap();
        let index = skeleton.add_root(node.name.clone().unwrap_or_default());
        assert_eq!(index, *joint - 1);
    }
    skeleton.recompute_bind_pose();

    // Read the animation's keyframes out of the binary and build a clip.
    let anim = &doc.animations[0];
    let sampler = &anim.samplers[0];
    let times = model.accessor_f32(sampler.input).unwrap();
    let values = model.accessor_f32(sampler.output).unwrap();

    let track = TransformTrack::new(AInterp::Linear)
        .with_translation_keys((0..times.len())
            .map(|i| {
                Keyframe::new(
                    times[i],
                    Vec3::new(values[i * 3], values[i * 3 + 1], values[i * 3 + 2]),
                )
            })
            .collect());

    let mut clip = AnimationClip::new("lift").with_wrap_mode(WrapMode::Once);
    clip.add_channel(0, track);
    assert!((clip.duration - 1.0).abs() < 1e-6);

    // Sample halfway through and confirm the lift is in progress.
    let sample = clip.sample(0.5, 2);
    assert!(
        (sample.locals[0].position.y - 2.5).abs() < 1e-5,
        "halfway through the lift should be y=2.5, got {:?}",
        sample.locals[0].position
    );

    // Drive skinning from the same data and confirm the vertex moves.
    let animated = vec![sample.locals[0], Transform::identity()];
    let inverse_bind: Vec<Transform> = skeleton.bones.iter().map(|b| b.inverse_bind).collect();
    let matrices = compute_skinning_matrices(&animated, &inverse_bind);

    let base = model.accessor_f32(1).unwrap();
    let base_positions: Vec<Vec3> = (0..base.len() / 3)
        .map(|i| Vec3::new(base[i * 3], base[i * 3 + 1], base[i * 3 + 2]))
        .collect();

    let mut influences = VertexInfluences::default();
    influences.push(0, 0.5);
    influences.push(1, 0.5);
    influences.normalize();
    let skin = Skin::new(vec![influences, influences]);

    let (deformed, _) = apply_skinning(&skin, &base_positions, &[Vec3::UP; 2], &matrices);

    // The vertex is 50% on the moved bone, so it should lift 1.25 units.
    assert!(
        (deformed[0].y - 1.25).abs() < 1e-4,
        "vertex should lift half of 2.5, got {}",
        deformed[0].y
    );
    assert!(total_warp(&base_positions, &deformed) > 1.0);
}

#[test]
fn loaded_glb_rotation_quaternion_is_xyzw() {
    // glTF stores node rotations as (x, y, z, w); a mismatch would show up as a
    // wrong rotation here.
    let json = r#"
{"asset": {"version": "2.0"},
 "nodes": [{"rotation": [0.0, 0.7071068, 0.0, 0.7071068]}]}
"#;
    let model = ow_asset::loader::load_gltf_from_bytes(json, vec![]).unwrap();
    let r = &model.document.nodes[0].trs.rotation;
    assert!((r[1] - 0.7071068).abs() < 1e-6, "y should be sin(45°)");
    assert!((r[3] - 0.7071068).abs() < 1e-6, "w should be cos(45°)");
    assert!((r[0]).abs() < 1e-6);
    assert!((r[2]).abs() < 1e-6);

    // Converted to our Quat, a quarter turn about Y takes +Z to +X.
    let q = Quat::new(r[0], r[1], r[2], r[3]);
    assert!((r[3]).abs() > 0.5);
    let fwd = q.rotate_vector(Vec3::FORWARD);
    assert!(fwd.x.abs() > 0.5, "expected +X, got {:?}", fwd);
}
