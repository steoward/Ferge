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
pub mod rpf;
pub mod rsc7;

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
pub use rpf::{
    RpfArchive, RpfEntry, RpfDirectoryEntry, RpfBinaryFileEntry, RpfResourceFileEntry,
    RpfEncryption, RpfError, create_synthetic_rpf, RPF7_MAGIC, RPF7_SECTOR_SIZE,
};
pub use rsc7::{
    Rsc7Header, Rsc7VirtualAllocation, Rsc7PrimitiveType, Rsc7VertexChannelType,
    Rsc7VertexChannelSemantic, Rsc7VertexChannel, Rsc7VertexDeclaration,
    Rsc7Vertex, Rsc7Geometry, Rsc7Mesh, Rsc7LodModel, Rsc7Shader, Rsc7ShaderGroup,
    Rsc7Drawable, Rsc7Error, Rsc7ArchiveExtractor,
    create_synthetic_rsc7_drawable,
    RSC7_MAGIC, RSC7_ALT_MAGIC, RSC7_VERSION_DRAWABLE, RSC7_SYSTEM_BASE, RSC7_GRAPHICS_BASE,
};
