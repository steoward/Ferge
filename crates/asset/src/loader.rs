//! Parses glTF 2.0 documents (both the `.gltf` JSON form and the binary `.glb`
//! container) into [`GltfDocument`], and reads typed data out of the binary
//! bufferViews.

use crate::glb::{
    bin_payload, json_payload, read_glb_chunks, GlbError,
};
use crate::json::{parse_json, JsonValue};
use crate::model::*;

/// Where loading failed.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadError {
    Container(String),
    Json(String),
    MissingMember { member: &'static str, index: usize },
    #[allow(dead_code)]
    InvalidValue { member: &'static str, index: usize, reason: String },
    Unsupported(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Container(m) => write!(f, "container error: {}", m),
            LoadError::Json(m) => write!(f, "JSON error: {}", m),
            LoadError::MissingMember { member, index } => {
                write!(f, "element {} is missing required member '{}'", index, member)
            }
            LoadError::InvalidValue { member, index, reason } => write!(
                f,
                "element {} has an invalid '{}': {}",
                index, member, reason
            ),
            LoadError::Unsupported(m) => write!(f, "unsupported: {}", m),
        }
    }
}

impl std::error::Error for LoadError {}

impl From<GlbError> for LoadError {
    fn from(e: GlbError) -> Self {
        LoadError::Container(e.to_string())
    }
}

impl From<crate::json::JsonError> for LoadError {
    fn from(e: crate::json::JsonError) -> Self {
        LoadError::Json(e.to_string())
    }
}

/// A loaded model: the parsed document plus the binary payload it indexes into.
#[derive(Debug, Clone)]
pub struct LoadedModel {
    pub document: GltfDocument,
    /// The GLB BIN chunk, or empty for a `.gltf` file with external buffers.
    pub binary: Vec<u8>,
}

/// Loads a GLB container.
pub fn load_glb(bytes: &[u8]) -> Result<LoadedModel, LoadError> {
    let (_, chunks) = read_glb_chunks(bytes)?;
    let json = json_payload(bytes, &chunks)?;
    let binary = bin_payload(bytes, &chunks).to_vec();

    let document = parse_document(json, &binary)?;
    Ok(LoadedModel { document, binary })
}

/// Loads the JSON side of a `.gltf` file. The binary payload must be supplied
/// separately; external buffer URIs are not fetched.
pub fn load_gltf_from_bytes(
    json: &str,
    binary: Vec<u8>,
) -> Result<LoadedModel, LoadError> {
    let document = parse_document(json, &binary)?;
    Ok(LoadedModel { document, binary })
}

fn parse_document(json: &str, binary: &[u8]) -> Result<GltfDocument, LoadError> {
    let root = parse_json(json)?;

    let asset = parse_asset(root.get_object("asset"))?;

    let mut buffers = Vec::new();
    if let Some(items) = root.get_array("buffers") {
        for b in items {
            buffers.push(GltfBuffer {
                byte_length: b.get_usize("byteLength", 0),
                uri: b.get_str("uri").map(|s| s.to_string()),
            });
        }
    }

    let mut buffer_views = Vec::new();
    if let Some(items) = root.get_array("bufferViews") {
        for (i, v) in items.iter().enumerate() {
            buffer_views.push(GltfBufferView {
                buffer: v
                    .get("buffer")
                    .and_then(JsonValue::as_usize)
                    .ok_or(LoadError::MissingMember { member: "buffer", index: i })?,
                byte_offset: v.get_usize("byteOffset", 0),
                byte_length: v
                    .get("byteLength")
                    .and_then(JsonValue::as_usize)
                    .ok_or(LoadError::MissingMember { member: "byteLength", index: i })?,
                byte_stride: v.get("byteStride").and_then(JsonValue::as_usize),
            });
        }
    }

    let mut accessors = Vec::new();
    if let Some(items) = root.get_array("accessors") {
        for (i, a) in items.iter().enumerate() {
            let component_type = a
                .get("componentType")
                .and_then(JsonValue::as_u32)
                .and_then(ComponentType::from_code)
                .ok_or(LoadError::MissingMember {
                    member: "componentType",
                    index: i,
                })?;
            let type_name = a
                .get_str("type")
                .ok_or(LoadError::MissingMember { member: "type", index: i })?;
            let accessor_type = AccessorType::from_name(type_name).ok_or_else(|| {
                LoadError::InvalidValue {
                    member: "type",
                    index: i,
                    reason: format!("unknown accessor type '{}'", type_name),
                }
            })?;

            accessors.push(GltfAccessor {
                buffer_view: a.get("bufferView").and_then(JsonValue::as_usize),
                byte_offset: a.get_usize("byteOffset", 0),
                component_type,
                normalized: a.get("normalized").and_then(JsonValue::as_bool).unwrap_or(false),
                count: a
                    .get("count")
                    .and_then(JsonValue::as_usize)
                    .ok_or(LoadError::MissingMember { member: "count", index: i })?,
                accessor_type,
                min: a.get_array("min").map(|v| v.to_vec()).unwrap_or_default().iter().filter_map(JsonValue::as_f64).collect(),
                max: a.get_array("max").map(|v| v.to_vec()).unwrap_or_default().iter().filter_map(JsonValue::as_f64).collect(),
            });
        }
    }

    let mut materials = Vec::new();
    if let Some(items) = root.get_array("materials") {
        for m in items {
            let mut material = GltfMaterial {
                name: m.get_str("name").map(|s| s.to_string()),
                double_sided: m.get("doubleSided").and_then(JsonValue::as_bool).unwrap_or(false),
                ..Default::default()
            };

            if let Some(base_color) = m.get_array("baseColorFactor") {
                if base_color.len() == 4 {
                    material.base_color_factor = [
                        base_color[0].as_f32().unwrap_or(1.0),
                        base_color[1].as_f32().unwrap_or(1.0),
                        base_color[2].as_f32().unwrap_or(1.0),
                        base_color[3].as_f32().unwrap_or(1.0),
                    ];
                }
            }
            if let Some(mr) = m.get_object("pbrMetallicRoughness") {
                material.metallic_factor = mr.get("metallicFactor").and_then(JsonValue::as_f32).unwrap_or(1.0);
                material.roughness_factor = mr.get("roughnessFactor").and_then(JsonValue::as_f32).unwrap_or(1.0);
                if let Some(bc) = mr.get_array("baseColorFactor") {
                    if bc.len() == 4 {
                        material.base_color_factor = [
                            bc[0].as_f32().unwrap_or(1.0),
                            bc[1].as_f32().unwrap_or(1.0),
                            bc[2].as_f32().unwrap_or(1.0),
                            bc[3].as_f32().unwrap_or(1.0),
                        ];
                    }
                }
                material.base_color_texture = mr
                    .get_object("baseColorTexture")
                    .and_then(|t| t.get("index"))
                    .and_then(JsonValue::as_usize);
            }
            materials.push(material);
        }
    }

    let mut textures = Vec::new();
    if let Some(items) = root.get_array("textures") {
        for t in items {
            textures.push(GltfTexture {
                source: t.get("source").and_then(JsonValue::as_usize).unwrap_or(0),
                name: t.get_str("name").map(|s| s.to_string()),
            });
        }
    }

    let mut meshes = Vec::new();
    if let Some(items) = root.get_array("meshes") {
        for m in items {
            let mut primitives = Vec::new();
            if let Some(prim_items) = m.get_array("primitives") {
                for (i, p) in prim_items.iter().enumerate() {
                    let attrs = p
                        .get("attributes")
                        .and_then(JsonValue::object_pairs)
                        .map(|pairs| {
                            pairs
                                .iter()
                                .filter_map(|(k, v)| v.as_usize().map(|idx| (k.clone(), idx)))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();

                    let positions_accessor = attrs
                        .iter()
                        .find(|(k, _)| k == "POSITION")
                        .map(|(_, v)| *v)
                        .ok_or(LoadError::MissingMember {
                            member: "attributes.POSITION",
                            index: i,
                        })?;
                    let _ = positions_accessor;

                    primitives.push(GltfPrimitive {
                        attributes: attrs,
                        indices: p.get("indices").and_then(JsonValue::as_usize),
                        material: p.get("material").and_then(JsonValue::as_usize),
                        mode: p.get("mode").and_then(JsonValue::as_u32).unwrap_or(4),
                    });
                }
            }
            meshes.push(GltfMesh {
                name: m.get_str("name").map(|s| s.to_string()),
                primitives,
            });
        }
    }

    let mut skins = Vec::new();
    if let Some(items) = root.get_array("skins") {
        for s in items {
            skins.push(GltfSkin {
                name: s.get_str("name").map(|s| s.to_string()),
                inverse_bind_matrices: s.get("inverseBindMatrices").and_then(JsonValue::as_usize),
                joints: s
                    .get_array("joints")
                    .map(|v| v.iter().filter_map(JsonValue::as_usize).collect())
                    .unwrap_or_default(),
                skeleton_root: s.get("skeleton").and_then(JsonValue::as_usize),
            });
        }
    }

    let mut nodes = Vec::new();
    if let Some(items) = root.get_array("nodes") {
        for n in items {
            let mut trs = NodeTrs {
                translation: n
                    .get_array("translation")
                    .map(read_vec3)
                    .unwrap_or([0.0, 0.0, 0.0]),
                rotation: n
                    .get_array("rotation")
                    .map(read_vec4)
                    .unwrap_or([0.0, 0.0, 0.0, 1.0]),
                scale: n
                    .get_array("scale")
                    .map(read_vec3)
                    .unwrap_or([1.0, 1.0, 1.0]),
            };
            // glTF's quaternion convention is (x, y, z, w), which matches.
            if trs.rotation == [0.0, 0.0, 0.0, 1.0] {
                trs.rotation = [0.0, 0.0, 0.0, 1.0];
            }

            let matrix = n.get_array("matrix").map(read_mat4);

            nodes.push(GltfNode {
                name: n.get_str("name").map(|s| s.to_string()),
                mesh: n.get("mesh").and_then(JsonValue::as_usize),
                skin: n.get("skin").and_then(JsonValue::as_usize),
                camera: n.get("camera").and_then(JsonValue::as_usize),
                children: n
                    .get_array("children")
                    .map(|v| v.iter().filter_map(JsonValue::as_usize).collect())
                    .unwrap_or_default(),
                trs,
                matrix,
            });
        }
    }

    let mut animations = Vec::new();
    if let Some(items) = root.get_array("animations") {
        for a in items {
            let mut samplers = Vec::new();
            if let Some(sampler_items) = a.get_array("samplers") {
                for (i, s) in sampler_items.iter().enumerate() {
                    let interp_name = s.get_str("interpolation").unwrap_or("LINEAR");
                    let interpolation = Interpolation::from_name(interp_name).ok_or_else(|| {
                        LoadError::InvalidValue {
                            member: "interpolation",
                            index: i,
                            reason: interp_name.to_string(),
                        }
                    })?;
                    samplers.push(GltfAnimationSampler {
                        input: s
                            .get("input")
                            .and_then(JsonValue::as_usize)
                            .ok_or(LoadError::MissingMember { member: "input", index: i })?,
                        output: s
                            .get("output")
                            .and_then(JsonValue::as_usize)
                            .ok_or(LoadError::MissingMember { member: "output", index: i })?,
                        interpolation,
                    });
                }
            }

            let mut channels = Vec::new();
            if let Some(channel_items) = a.get_array("channels") {
                for (i, c) in channel_items.iter().enumerate() {
                    let path_name = c
                        .get_object("target")
                        .and_then(|t| t.get_str("path"))
                        .ok_or(LoadError::MissingMember {
                            member: "channels[].target.path",
                            index: i,
                        })?;
                    let target_path = TargetPath::from_name(path_name).ok_or_else(|| {
                        LoadError::InvalidValue {
                            member: "target.path",
                            index: i,
                            reason: path_name.to_string(),
                        }
                    })?;
                    channels.push(GltfAnimationChannel {
                        sampler: c
                            .get("sampler")
                            .and_then(JsonValue::as_usize)
                            .ok_or(LoadError::MissingMember { member: "sampler", index: i })?,
                        target_node: c
                            .get_object("target")
                            .and_then(|t| t.get("node"))
                            .and_then(JsonValue::as_usize),
                        target_path,
                    });
                }
            }

            animations.push(GltfAnimation {
                name: a.get_str("name").map(|s| s.to_string()),
                samplers,
                channels,
            });
        }
    }

    let mut scenes = Vec::new();
    if let Some(items) = root.get_array("scenes") {
        for s in items {
            scenes.push(GltfScene {
                nodes: s
                    .get_array("nodes")
                    .map(|v| v.iter().filter_map(JsonValue::as_usize).collect())
                    .unwrap_or_default(),
            });
        }
    }

    let _ = binary;

    Ok(GltfDocument {
        asset,
        scene: root.get("scene").and_then(JsonValue::as_usize),
        scenes,
        nodes,
        meshes,
        accessors,
        buffer_views,
        buffers,
        materials,
        textures,
        skins,
        animations,
    })
}

fn parse_asset(asset: Option<&JsonValue>) -> Result<GltfAsset, LoadError> {
    let asset = asset.ok_or(LoadError::MissingMember {
        member: "asset",
        index: 0,
    })?;
    let version = asset
        .get_str("version")
        .ok_or(LoadError::MissingMember {
            member: "asset.version",
            index: 0,
        })?
        .to_string();

    // Only glTF 2.0 is supported; 1.0 uses a completely different layout.
    if !version.starts_with("2.") {
        return Err(LoadError::Unsupported(format!(
            "glTF version {} (only 2.x is supported)",
            version
        )));
    }

    Ok(GltfAsset {
        version,
        generator: asset.get_str("generator").map(|s| s.to_string()),
        copyright: asset.get_str("copyright").map(|s| s.to_string()),
    })
}

fn read_vec3(array: &[JsonValue]) -> [f32; 3] {
    [
        array.first().and_then(JsonValue::as_f32).unwrap_or(0.0),
        array.get(1).and_then(JsonValue::as_f32).unwrap_or(0.0),
        array.get(2).and_then(JsonValue::as_f32).unwrap_or(0.0),
    ]
}

fn read_vec4(array: &[JsonValue]) -> [f32; 4] {
    [
        array.first().and_then(JsonValue::as_f32).unwrap_or(0.0),
        array.get(1).and_then(JsonValue::as_f32).unwrap_or(0.0),
        array.get(2).and_then(JsonValue::as_f32).unwrap_or(0.0),
        array.get(3).and_then(JsonValue::as_f32).unwrap_or(0.0),
    ]
}

fn read_mat4(array: &[JsonValue]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = array.get(i).and_then(JsonValue::as_f32).unwrap_or(0.0);
    }
    if out[15] == 0.0 {
        out[15] = 1.0;
    }
    out
}

impl LoadedModel {
    /// Reads raw bytes for accessor `index`, honouring its bufferView offset
    /// and its own byteOffset.
    pub fn accessor_bytes(&self, index: usize) -> Option<&[u8]> {
        let accessor = self.document.accessors.get(index)?;
        let view = self.document.buffer_views.get(accessor.buffer_view?)?;
        let buffer = self.binary.as_slice();

        let start = view.byte_offset + accessor.byte_offset;
        let length = accessor.byte_length();
        buffer.get(start..start + length)
    }

    /// Decodes accessor `index` into `f32` components.
    ///
    /// Integer component types are converted to floats. `normalized` accessors
    /// are scaled into the unit range, which is what the glTF spec demands for
    /// `POSITION`, `NORMAL` and `JOINTS_0` data.
    pub fn accessor_f32(&self, index: usize) -> Option<Vec<f32>> {
        let accessor = self.document.accessors.get(index)?;
        let raw = self.accessor_bytes(index)?;

        let components = accessor.total_components();
        let mut out = Vec::with_capacity(components);

        match accessor.component_type {
            ComponentType::Float => {
                for c in 0..components {
                    let at = c * 4;
                    let bytes = raw.get(at..at + 4)?;
                    out.push(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
                }
            }
            ComponentType::UnsignedByte => {
                let divisor = if accessor.normalized { 255.0 } else { 1.0 };
                for c in 0..components {
                    out.push(*raw.get(c)? as f32 / divisor);
                }
            }
            ComponentType::Byte => {
                let divisor = if accessor.normalized { 127.0 } else { 1.0 };
                for c in 0..components {
                    out.push(*raw.get(c)? as i8 as f32 / divisor);
                }
            }
            ComponentType::UnsignedShort => {
                let divisor = if accessor.normalized { 65535.0 } else { 1.0 };
                for c in 0..components {
                    let at = c * 2;
                    out.push(u16::from_le_bytes([*raw.get(at)?, *raw.get(at + 1)?]) as f32 / divisor);
                }
            }
            ComponentType::Short => {
                let divisor = if accessor.normalized { 32767.0 } else { 1.0 };
                for c in 0..components {
                    let at = c * 2;
                    out.push(i16::from_le_bytes([*raw.get(at)?, *raw.get(at + 1)?]) as f32 / divisor);
                }
            }
            ComponentType::UnsignedInt => {
                for c in 0..components {
                    let at = c * 4;
                    out.push(u32::from_le_bytes([
                        *raw.get(at)?,
                        *raw.get(at + 1)?,
                        *raw.get(at + 2)?,
                        *raw.get(at + 3)?,
                    ]) as f32);
                }
            }
        }

        Some(out)
    }

    /// Decodes accessor `index` as `u32` indices.
    pub fn accessor_u32(&self, index: usize) -> Option<Vec<u32>> {
        let accessor = self.document.accessors.get(index)?;
        let raw = self.accessor_bytes(index)?;
        let count = accessor.count;

        match accessor.component_type {
            ComponentType::UnsignedByte => Some((0..count).filter_map(|c| raw.get(c).map(|b| *b as u32)).collect()),
            ComponentType::UnsignedShort => Some(
                (0..count)
                    .filter_map(|c| {
                        let at = c * 2;
                        raw.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as u32)
                    })
                    .collect(),
            ),
            ComponentType::UnsignedInt => Some(
                (0..count)
                    .filter_map(|c| {
                        let at = c * 4;
                        raw.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    })
                    .collect(),
            ),
            _ => None,
        }
    }

    /// Decodes accessor `index` as `u16` joint indices.
    pub fn accessor_joints(&self, index: usize) -> Option<Vec<u16>> {
        let accessor = self.document.accessors.get(index)?;
        let raw = self.accessor_bytes(index)?;
        let total = accessor.total_components();

        match accessor.component_type {
            ComponentType::UnsignedByte => Some((0..total).filter_map(|c| raw.get(c).map(|b| *b as u16)).collect()),
            ComponentType::UnsignedShort => Some(
                (0..total)
                    .filter_map(|c| {
                        let at = c * 2;
                        raw.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
                    })
                    .collect(),
            ),
            _ => None,
        }
    }

    /// The document's default scene roots, or all nodes if no scene is declared.
    pub fn scene_roots(&self) -> Vec<usize> {
        let roots = self.document.scene_roots();
        if roots.is_empty() {
            (0..self.document.nodes.len()).collect()
        } else {
            roots.to_vec()
        }
    }

    /// Total number of triangles across every primitive.
    pub fn triangle_count(&self) -> usize {
        self.document.total_triangle_count()
    }
}
