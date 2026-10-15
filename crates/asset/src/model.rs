//! The subset of the glTF 2.0 document an engine needs to render skinned
//! geometry.
//!
//! Every field is an index into a document array or an option, so a value of
//! `0` can still mean "the first element" rather than "absent". This is the
//! glTF idiom and it is preserved here — a missing member is `None`, not a
//! sentinel index.

/// Numeric component type of an accessor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentType {
    Byte = 5120,
    UnsignedByte = 5121,
    Short = 5122,
    UnsignedShort = 5123,
    UnsignedInt = 5125,
    Float = 5126,
}

impl ComponentType {
    pub fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            5120 => Self::Byte,
            5121 => Self::UnsignedByte,
            5122 => Self::Short,
            5123 => Self::UnsignedShort,
            5125 => Self::UnsignedInt,
            5126 => Self::Float,
            _ => return None,
        })
    }

    /// Size of one component in bytes.
    pub fn size_in_bytes(self) -> usize {
        match self {
            Self::Byte | Self::UnsignedByte => 1,
            Self::Short | Self::UnsignedShort => 2,
            Self::UnsignedInt | Self::Float => 4,
        }
    }

    /// Number of components the type holds (always 1; matrices and vectors are
    /// described by [`AccessorType`]).
    pub fn component_count(self) -> usize {
        1
    }
}

/// Element layout of an accessor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessorType {
    Scalar,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat3,
    Mat4,
}

impl AccessorType {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "SCALAR" => Self::Scalar,
            "VEC2" => Self::Vec2,
            "VEC3" => Self::Vec3,
            "VEC4" => Self::Vec4,
            "MAT2" => Self::Mat2,
            "MAT3" => Self::Mat3,
            "MAT4" => Self::Mat4,
            _ => return None,
        })
    }

    pub fn component_count(self) -> usize {
        match self {
            Self::Scalar => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Vec4 | Self::Mat2 => 4,
            Self::Mat3 => 9,
            Self::Mat4 => 16,
        }
    }
}

/// How animation sampling interpolates between keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Interpolation {
    Step,
    Linear,
    CubicSpline,
}

impl Interpolation {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "STEP" => Self::Step,
            "LINEAR" => Self::Linear,
            "CUBICSPLINE" => Self::CubicSpline,
            _ => return None,
        })
    }
}

/// Which property of a node an animation channel drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetPath {
    Translation,
    Rotation,
    Scale,
    Weights,
}

impl TargetPath {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "translation" => Self::Translation,
            "rotation" => Self::Rotation,
            "scale" => Self::Scale,
            "weights" => Self::Weights,
            _ => return None,
        })
    }
}

/// The glTF asset header (`asset` member).
#[derive(Debug, Clone, Default)]
pub struct GltfAsset {
    pub version: String,
    pub generator: Option<String>,
    pub copyright: Option<String>,
}

/// An accessor: a typed view into a bufferView.
#[derive(Debug, Clone)]
pub struct GltfAccessor {
    pub buffer_view: Option<usize>,
    pub byte_offset: usize,
    pub component_type: ComponentType,
    pub normalized: bool,
    pub count: usize,
    pub accessor_type: AccessorType,
    /// Minimum values, one per component, when the source declares them.
    pub min: Vec<f64>,
    pub max: Vec<f64>,
}

impl GltfAccessor {
    /// Total number of scalar components in this accessor.
    pub fn total_components(&self) -> usize {
        self.count * self.accessor_type.component_count()
    }

    /// Byte length of the accessor's payload.
    pub fn byte_length(&self) -> usize {
        self.total_components() * self.component_type.size_in_bytes()
    }
}

/// A bufferView: a slice of a buffer.
#[derive(Debug, Clone)]
pub struct GltfBufferView {
    pub buffer: usize,
    pub byte_offset: usize,
    pub byte_length: usize,
    pub byte_stride: Option<usize>,
}

/// A buffer: either the GLB BIN chunk or an external file (unsupported here).
#[derive(Debug, Clone)]
pub struct GltfBuffer {
    pub byte_length: usize,
    pub uri: Option<String>,
}

/// A material, reduced to the metallic-roughness inputs the renderer uses.
#[derive(Debug, Clone)]
pub struct GltfMaterial {
    pub name: Option<String>,
    pub base_color_factor: [f32; 4],
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub base_color_texture: Option<usize>,
    pub double_sided: bool,
}

impl Default for GltfMaterial {
    fn default() -> Self {
        Self {
            name: None,
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            base_color_texture: None,
            double_sided: false,
        }
    }
}

/// An image-backed texture.
#[derive(Debug, Clone)]
pub struct GltfTexture {
    pub source: usize,
    pub name: Option<String>,
}

/// One draw primitive of a mesh.
#[derive(Debug, Clone, Default)]
pub struct GltfPrimitive {
    /// Attribute name → accessor index. Standard names are POSITION, NORMAL,
    /// TEXCOORD_0, JOINTS_0, WEIGHTS_0, COLOR_0, TANGENT.
    pub attributes: Vec<(String, usize)>,
    pub indices: Option<usize>,
    pub material: Option<usize>,
    /// 4 is triangles, the only mode this loader supports.
    pub mode: u32,
}

impl GltfPrimitive {
    /// Reads a named attribute's accessor index.
    pub fn attribute(&self, name: &str) -> Option<usize> {
        self.attributes
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| *v)
    }

    pub fn positions(&self) -> Option<usize> {
        self.attribute("POSITION")
    }

    pub fn normals(&self) -> Option<usize> {
        self.attribute("NORMAL")
    }

    pub fn uvs(&self) -> Option<usize> {
        self.attribute("TEXCOORD_0")
    }

    pub fn joints(&self) -> Option<usize> {
        self.attribute("JOINTS_0")
    }

    pub fn weights(&self) -> Option<usize> {
        self.attribute("WEIGHTS_0")
    }

    pub fn tangents(&self) -> Option<usize> {
        self.attribute("TANGENT")
    }
}

/// A mesh: a named list of primitives.
#[derive(Debug, Clone)]
pub struct GltfMesh {
    pub name: Option<String>,
    pub primitives: Vec<GltfPrimitive>,
}

/// TRS of a node, either decomposed or as a single matrix.
#[derive(Debug, Clone, Default)]
pub struct NodeTrs {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

/// A node in the scene graph.
#[derive(Debug, Clone)]
pub struct GltfNode {
    pub name: Option<String>,
    pub mesh: Option<usize>,
    pub skin: Option<usize>,
    pub children: Vec<usize>,
    pub trs: NodeTrs,
    /// When set, the node's transform is this matrix instead of `trs`.
    pub matrix: Option<[f32; 16]>,
    pub camera: Option<usize>,
}

/// A skin: a skeleton plus per-joint inverse bind matrices.
#[derive(Debug, Clone)]
pub struct GltfSkin {
    pub name: Option<String>,
    /// Index into the accessor array of inverse bind matrices (MAT4).
    pub inverse_bind_matrices: Option<usize>,
    pub joints: Vec<usize>,
    pub skeleton_root: Option<usize>,
}

/// A keyframe sampler: input times and output values.
#[derive(Debug, Clone)]
pub struct GltfAnimationSampler {
    pub input: usize,
    pub output: usize,
    pub interpolation: Interpolation,
}

/// A channel: one sampled property of one node.
#[derive(Debug, Clone)]
pub struct GltfAnimationChannel {
    pub sampler: usize,
    pub target_node: Option<usize>,
    pub target_path: TargetPath,
}

/// An animation clip.
#[derive(Debug, Clone)]
pub struct GltfAnimation {
    pub name: Option<String>,
    pub samplers: Vec<GltfAnimationSampler>,
    pub channels: Vec<GltfAnimationChannel>,
}

/// A scene: a set of root nodes.
#[derive(Debug, Clone)]
pub struct GltfScene {
    pub nodes: Vec<usize>,
}

/// A complete parsed glTF document.
#[derive(Debug, Clone)]
pub struct GltfDocument {
    pub asset: GltfAsset,
    pub scene: Option<usize>,
    pub scenes: Vec<GltfScene>,
    pub nodes: Vec<GltfNode>,
    pub meshes: Vec<GltfMesh>,
    pub accessors: Vec<GltfAccessor>,
    pub buffer_views: Vec<GltfBufferView>,
    pub buffers: Vec<GltfBuffer>,
    pub materials: Vec<GltfMaterial>,
    pub textures: Vec<GltfTexture>,
    pub skins: Vec<GltfSkin>,
    pub animations: Vec<GltfAnimation>,
}

impl GltfDocument {
    pub fn mesh(&self, index: usize) -> Option<&GltfMesh> {
        self.meshes.get(index)
    }

    pub fn node(&self, index: usize) -> Option<&GltfNode> {
        self.nodes.get(index)
    }

    pub fn accessor(&self, index: usize) -> Option<&GltfAccessor> {
        self.accessors.get(index)
    }

    pub fn buffer_view(&self, index: usize) -> Option<&GltfBufferView> {
        self.buffer_views.get(index)
    }

    pub fn skin(&self, index: usize) -> Option<&GltfSkin> {
        self.skins.get(index)
    }

    pub fn material(&self, index: usize) -> Option<&GltfMaterial> {
        self.materials.get(index)
    }

    pub fn scene_roots(&self) -> &[usize] {
        self.scene
            .and_then(|i| self.scenes.get(i))
            .map(|s| s.nodes.as_slice())
            .unwrap_or(&[])
    }

    /// Nodes that reference skin `index`.
    pub fn nodes_using_skin(&self, index: usize) -> Vec<usize> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.skin == Some(index))
            .map(|(i, _)| i)
            .collect()
    }

    /// Nodes that reference mesh `index`.
    pub fn nodes_using_mesh(&self, index: usize) -> Vec<usize> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.mesh == Some(index))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn total_triangle_count(&self) -> usize {
        self.meshes
            .iter()
            .flat_map(|m| m.primitives.iter())
            .map(|p| {
                let verts = p
                    .positions()
                    .and_then(|a| self.accessors.get(a))
                    .map(|a| a.count)
                    .unwrap_or(0);
                verts / 3
            })
            .sum()
    }
}
