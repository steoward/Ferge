//! glTF 2.0 asset loading, including the binary container (GLB).
//!
//! Only a hand-written JSON reader is used — no `serde`, no external crate.
//! glTF documents are small (a few hundred kilobytes at most, and the heavy
//! data lives in the binary buffers), so the fact that this reader is O(n²) in
//! the number of object members is irrelevant in practice.
//!
//! The loader covers what an engine actually needs to render a skinned mesh:
//! accessors, bufferViews, buffers, meshes (positions, normals, uvs, joints,
//! weights), skins, and animation samplers/channels. Everything else in the
//! spec (cameras, extensions, sparse accessors) is reported as `GlbError::Unsupported`
//! rather than silently ignored.

pub mod glb;
pub mod json;
pub mod model;
pub mod loader;

pub use glb::{GlbHeader, GlbChunk, read_glb_chunks};
pub use json::{JsonValue, parse_json, JsonError};
pub use model::{
    GltfDocument, GltfAsset, GltfScene, GltfNode, GltfMesh, GltfPrimitive,
    GltfAccessor, GltfBufferView, GltfBuffer, GltfMaterial, GltfSkin,
    GltfAnimation, GltfAnimationSampler, GltfAnimationChannel, GltfTexture,
    ComponentType, AccessorType, Interpolation as GltfInterpolation, TargetPath,
};
pub use loader::{load_glb, load_gltf_from_bytes, LoadedModel};
pub use glb::GlbError;
