//! Clean-Room Rockstar Resource Format 7 (RSC7 / #rsc) & GTA 5 3D Model (.ydr) Parser.
//!
//! Implements a safe, zero-warning clean-room binary parser and generator for RSC7 containers
//! and GTA 5 3D Model Drawables (`.ydr`), including virtual memory page allocation decoding,
//! vertex attribute extraction, LOD management, and progressive RPF archive extraction.
//!
//! Designed strictly under Clean-Room Reverse Engineering principles:
//! - Operates purely on open binary specifications for software interoperability.
//! - Free of proprietary assets or reverse-engineered Rockstar source code.
//! - Features 100% synthetic asset generation for reproducible CI testing without game files.

use std::io::{Read, Seek};
use ow_math::{Vec2, Vec3, Vec4};
use crate::rpf::{RpfArchive, RpfEntry};

/// Standard little-endian 32-bit RSC7 Magic identifier (`RSC7` in ASCII: 0x37435352).
pub const RSC7_MAGIC: u32 = 0x37435352;

/// Alternative little-endian 32-bit RSC7 Magic identifier (`RSC\x07`: 0x07435352).
pub const RSC7_ALT_MAGIC: u32 = 0x07435352;

/// Resource type version for standalone 3D Model Drawables (`.ydr`).
pub const RSC7_VERSION_DRAWABLE: u32 = 165;

/// Resource type version for Drawable Dictionaries (`.ydd`).
pub const RSC7_VERSION_DRAWABLE_DICT: u32 = 165;

/// Resource type version for Fragments / destructible models (`.yft`).
pub const RSC7_VERSION_FRAGMENT: u32 = 162;

/// Resource type version for Texture Dictionaries (`.ytd`).
pub const RSC7_VERSION_TEXTURE_DICT: u32 = 13;

/// Resource type version for Static Collision Bounds (`.ybn`).
pub const RSC7_VERSION_BOUNDS: u32 = 43;

/// Base virtual memory address for System (CPU metadata) segment.
pub const RSC7_SYSTEM_BASE: u32 = 0x5000_0000;

/// Base virtual memory address for Graphics (GPU vertex/index buffers) segment.
pub const RSC7_GRAPHICS_BASE: u32 = 0x6000_0000;

/// Size of the standard RSC7 container header in bytes.
pub const RSC7_HEADER_SIZE: usize = 16;

/// Minimum virtual memory page size baseline in RAGE engine (512 bytes).
pub const RSC7_BASE_PAGE_SIZE: usize = 512;

/// Errors occurring during RSC7 container decoding or 3D drawable parsing.
#[derive(Debug)]
pub enum Rsc7Error {
    Io(std::io::Error),
    InvalidMagic(u32),
    UnsupportedVersion(u32),
    DecompressionFailed(String),
    BufferTooSmall { required: usize, actual: usize },
    InvalidPointer { address: u32, segment: &'static str },
    CorruptedData(String),
    NotFound(String),
}

impl From<std::io::Error> for Rsc7Error {
    fn from(err: std::io::Error) -> Self {
        Rsc7Error::Io(err)
    }
}

impl From<crate::rpf::RpfError> for Rsc7Error {
    fn from(err: crate::rpf::RpfError) -> Self {
        Rsc7Error::CorruptedData(format!("RPF archive error: {}", err))
    }
}

impl std::fmt::Display for Rsc7Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rsc7Error::Io(e) => write!(f, "RSC7 IO error: {}", e),
            Rsc7Error::InvalidMagic(m) => write!(f, "Invalid RSC7 magic: 0x{:08X} (expected RSC7)", m),
            Rsc7Error::UnsupportedVersion(v) => write!(f, "Unsupported RSC7 resource version: {}", v),
            Rsc7Error::DecompressionFailed(msg) => write!(f, "RSC7 decompression failed: {}", msg),
            Rsc7Error::BufferTooSmall { required, actual } => {
                write!(f, "Buffer too small: required {} bytes, got {}", required, actual)
            }
            Rsc7Error::InvalidPointer { address, segment } => {
                write!(f, "Invalid virtual pointer 0x{:08X} for segment '{}'", address, segment)
            }
            Rsc7Error::CorruptedData(msg) => write!(f, "Corrupted RSC7 structure: {}", msg),
            Rsc7Error::NotFound(item) => write!(f, "RSC7 item not found: {}", item),
        }
    }
}

impl std::error::Error for Rsc7Error {}

/// Standard 16-byte header prepending every RSC7 compiled resource file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rsc7Header {
    pub magic: u32,
    pub version: u32,
    pub system_flags: u32,
    pub graphics_flags: u32,
}

impl Rsc7Header {
    /// Parses an RSC7 header from a 16-byte buffer slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Rsc7Error> {
        if bytes.len() < RSC7_HEADER_SIZE {
            return Err(Rsc7Error::BufferTooSmall {
                required: RSC7_HEADER_SIZE,
                actual: bytes.len(),
            });
        }

        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if !Self::is_valid_magic(magic) {
            return Err(Rsc7Error::InvalidMagic(magic));
        }

        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let system_flags = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let graphics_flags = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);

        Ok(Self {
            magic,
            version,
            system_flags,
            graphics_flags,
        })
    }

    /// Serializes the header into its 16-byte binary representation.
    #[inline]
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..4].copy_from_slice(&self.magic.to_le_bytes());
        buf[4..8].copy_from_slice(&self.version.to_le_bytes());
        buf[8..12].copy_from_slice(&self.system_flags.to_le_bytes());
        buf[12..16].copy_from_slice(&self.graphics_flags.to_le_bytes());
        buf
    }

    /// Checks if a 32-bit magic matches known RSC7 identifiers.
    #[inline]
    pub fn is_valid_magic(magic: u32) -> bool {
        magic == RSC7_MAGIC || magic == RSC7_ALT_MAGIC
    }

    /// Decodes the virtual memory page allocation for the CPU / system segment.
    #[inline]
    pub fn system_allocation(&self) -> Rsc7VirtualAllocation {
        Rsc7VirtualAllocation::decode(self.system_flags)
    }

    /// Decodes the virtual memory page allocation for the GPU / graphics segment.
    #[inline]
    pub fn graphics_allocation(&self) -> Rsc7VirtualAllocation {
        Rsc7VirtualAllocation::decode(self.graphics_flags)
    }
}

/// Decoded virtual memory page allocation metrics from an RSC7 flag dword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rsc7VirtualAllocation {
    pub raw_flags: u32,
    pub base_shift: u32,
    pub base_page_size: usize,
    pub page_count: usize,
    pub total_size: usize,
}

impl Rsc7VirtualAllocation {
    /// Decodes an RSC7 flag word into its virtual page dimensions and total size.
    pub fn decode(flags: u32) -> Self {
        let base_shift = flags & 0x0F;
        let base_page_size = RSC7_BASE_PAGE_SIZE << base_shift.min(15);

        // Bit-weighted page count accumulation across allocation tiers
        let mut total_size = 0usize;
        let mut page_count = 0usize;

        // Base pages encoded in bits 17..28
        let base_pages = ((flags >> 17) & 0x7FF) as usize;
        if base_pages > 0 {
            page_count += base_pages;
            total_size += base_pages * base_page_size;
        }

        // Multi-tier exponential chunk sizes (bits 4..16 and upper bits)
        for bit in 4..17 {
            if (flags & (1 << bit)) != 0 {
                let weight = 1 << (bit - 4);
                let chunk_size = base_page_size * weight;
                page_count += weight;
                total_size += chunk_size;
            }
        }

        // If no explicit page bits are set (e.g. minimal or synthetic allocations), default to 1 page
        if total_size == 0 {
            page_count = 1;
            total_size = base_page_size;
        }

        Self {
            raw_flags: flags,
            base_shift,
            base_page_size,
            page_count,
            total_size,
        }
    }

    /// Encodes a base shift and page count into an RSC7 compliant flag word.
    pub fn encode(base_shift: u32, page_count: usize) -> u32 {
        let shift = base_shift & 0x0F;
        let pages = ((page_count.max(1)) & 0x7FF) as u32;
        shift | (pages << 17)
    }
}

/// 3D rendering primitive topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rsc7PrimitiveType {
    Points = 0,
    Lines = 1,
    LineStrip = 2,
    Triangles = 3,
    TriangleStrip = 4,
    TriangleFan = 5,
}

impl Rsc7PrimitiveType {
    pub fn from_u32(val: u32) -> Self {
        match val {
            0 => Rsc7PrimitiveType::Points,
            1 => Rsc7PrimitiveType::Lines,
            2 => Rsc7PrimitiveType::LineStrip,
            4 => Rsc7PrimitiveType::TriangleStrip,
            5 => Rsc7PrimitiveType::TriangleFan,
            _ => Rsc7PrimitiveType::Triangles,
        }
    }
}

/// Semantic identity of a vertex attribute channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rsc7VertexChannelSemantic {
    Position,
    BlendWeights,
    BlendIndices,
    Normal,
    Color0,
    Color1,
    TexCoord0,
    TexCoord1,
    Tangent,
    Other(u8),
}

impl Rsc7VertexChannelSemantic {
    pub fn from_bit(bit: u8) -> Self {
        match bit {
            0 => Rsc7VertexChannelSemantic::Position,
            1 => Rsc7VertexChannelSemantic::BlendWeights,
            2 => Rsc7VertexChannelSemantic::BlendIndices,
            3 => Rsc7VertexChannelSemantic::Normal,
            4 => Rsc7VertexChannelSemantic::Color0,
            5 => Rsc7VertexChannelSemantic::Color1,
            6 => Rsc7VertexChannelSemantic::TexCoord0,
            7 => Rsc7VertexChannelSemantic::TexCoord1,
            14 => Rsc7VertexChannelSemantic::Tangent,
            other => Rsc7VertexChannelSemantic::Other(other),
        }
    }
}

/// Binary data type and packing of a vertex channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rsc7VertexChannelType {
    Float1,
    Float2,
    Float3,
    Float4,
    UByte4,
    UByte4Norm,
    Short2,
    Short4,
    Half2,
    Half4,
    Dec4,
    Unknown(u8),
}

impl Rsc7VertexChannelType {
    pub fn from_code(code: u8) -> Self {
        match code {
            0 => Rsc7VertexChannelType::Float1,
            1 => Rsc7VertexChannelType::Half2,
            2 => Rsc7VertexChannelType::Float2,
            3 => Rsc7VertexChannelType::Half4,
            4 => Rsc7VertexChannelType::Float3,
            5 => Rsc7VertexChannelType::Float2, // Alternate Float2
            6 => Rsc7VertexChannelType::Float3,
            7 => Rsc7VertexChannelType::Float4,
            8 => Rsc7VertexChannelType::UByte4,
            9 => Rsc7VertexChannelType::UByte4Norm,
            10 => Rsc7VertexChannelType::Dec4,
            other => Rsc7VertexChannelType::Unknown(other),
        }
    }

    /// Size in bytes occupied by this component type in memory.
    pub fn byte_size(&self) -> usize {
        match self {
            Rsc7VertexChannelType::Float1 => 4,
            Rsc7VertexChannelType::Float2 => 8,
            Rsc7VertexChannelType::Float3 => 12,
            Rsc7VertexChannelType::Float4 => 16,
            Rsc7VertexChannelType::UByte4 => 4,
            Rsc7VertexChannelType::UByte4Norm => 4,
            Rsc7VertexChannelType::Short2 => 4,
            Rsc7VertexChannelType::Short4 => 8,
            Rsc7VertexChannelType::Half2 => 4,
            Rsc7VertexChannelType::Half4 => 8,
            Rsc7VertexChannelType::Dec4 => 4,
            Rsc7VertexChannelType::Unknown(_) => 4,
        }
    }
}

/// Channel metadata entry within a vertex declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rsc7VertexChannel {
    pub semantic: Rsc7VertexChannelSemantic,
    pub channel_type: Rsc7VertexChannelType,
    pub offset: usize,
}

/// Vertex Declaration defining layout, stride, and component channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rsc7VertexDeclaration {
    pub channel_flags: u32,
    pub stride: u32,
    pub channels: Vec<Rsc7VertexChannel>,
}

impl Rsc7VertexDeclaration {
    /// Creates a standard P-N-UV0-C0 declaration with 36-byte stride (Float3 Pos, Float3 Normal, UByte4 Color, Float2 UV0).
    pub fn standard_p_n_c_uv() -> Self {
        let channels = vec![
            Rsc7VertexChannel {
                semantic: Rsc7VertexChannelSemantic::Position,
                channel_type: Rsc7VertexChannelType::Float3,
                offset: 0,
            },
            Rsc7VertexChannel {
                semantic: Rsc7VertexChannelSemantic::Normal,
                channel_type: Rsc7VertexChannelType::Float3,
                offset: 12,
            },
            Rsc7VertexChannel {
                semantic: Rsc7VertexChannelSemantic::Color0,
                channel_type: Rsc7VertexChannelType::UByte4Norm,
                offset: 24,
            },
            Rsc7VertexChannel {
                semantic: Rsc7VertexChannelSemantic::TexCoord0,
                channel_type: Rsc7VertexChannelType::Float2,
                offset: 28,
            },
        ];

        Self {
            channel_flags: (1 << 0) | (1 << 3) | (1 << 4) | (1 << 6), // 0x0059
            stride: 36,
            channels,
        }
    }

    /// Finds a channel by its semantic.
    pub fn find_channel(&self, semantic: Rsc7VertexChannelSemantic) -> Option<&Rsc7VertexChannel> {
        self.channels.iter().find(|c| c.semantic == semantic)
    }
}

/// Standardized 3D vertex attribute tuple.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rsc7Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv0: Vec2,
    pub color: Vec4,
}

impl Default for Rsc7Vertex {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            normal: Vec3::new(0.0, 1.0, 0.0),
            uv0: Vec2::ZERO,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
        }
    }
}

/// Low-level 3D Mesh Geometry containing raw vertex and index buffers.
#[derive(Debug, Clone)]
pub struct Rsc7Geometry {
    pub vertex_count: u32,
    pub index_count: u32,
    pub vertex_stride: u32,
    pub vertex_buffer_raw: Vec<u8>,
    pub index_buffer_raw: Vec<u8>,
    pub primitive_type: Rsc7PrimitiveType,
    pub vertex_declaration: Rsc7VertexDeclaration,
}

/// Synonym alias for `Rsc7Geometry` matching engine terminology.
pub type Rsc7Mesh = Rsc7Geometry;

impl Rsc7Geometry {
    /// Extracts standardized `Rsc7Vertex` array from raw vertex buffer bytes.
    pub fn to_vertex_buffer(&self) -> Vec<Rsc7Vertex> {
        let stride = self.vertex_stride as usize;
        if stride == 0 {
            return Vec::new();
        }

        let mut vertices = Vec::with_capacity(self.vertex_count as usize);

        let pos_ch = self.vertex_declaration.find_channel(Rsc7VertexChannelSemantic::Position);
        let norm_ch = self.vertex_declaration.find_channel(Rsc7VertexChannelSemantic::Normal);
        let uv_ch = self.vertex_declaration.find_channel(Rsc7VertexChannelSemantic::TexCoord0);
        let col_ch = self.vertex_declaration.find_channel(Rsc7VertexChannelSemantic::Color0);

        for i in 0..self.vertex_count as usize {
            let offset = i * stride;
            if offset + stride > self.vertex_buffer_raw.len() {
                break;
            }
            let vdata = &self.vertex_buffer_raw[offset..offset + stride];

            // 1. Position
            let position = if let Some(ch) = pos_ch {
                read_vec3_channel(vdata, ch)
            } else {
                Vec3::ZERO
            };

            // 2. Normal
            let normal = if let Some(ch) = norm_ch {
                read_vec3_channel(vdata, ch)
            } else {
                Vec3::new(0.0, 1.0, 0.0)
            };

            // 3. UV0
            let uv0 = if let Some(ch) = uv_ch {
                read_vec2_channel(vdata, ch)
            } else {
                Vec2::ZERO
            };

            // 4. Color
            let color = if let Some(ch) = col_ch {
                read_vec4_channel(vdata, ch)
            } else {
                Vec4::new(1.0, 1.0, 1.0, 1.0)
            };

            vertices.push(Rsc7Vertex {
                position,
                normal,
                uv0,
                color,
            });
        }

        vertices
    }

    /// Extracts 32-bit triangle indices from index buffer.
    pub fn to_indices(&self) -> Vec<u32> {
        let count = self.index_count as usize;
        let mut indices = Vec::with_capacity(count);

        if self.index_buffer_raw.len() >= count * 2 {
            for i in 0..count {
                let off = i * 2;
                let idx = u16::from_le_bytes([self.index_buffer_raw[off], self.index_buffer_raw[off + 1]]) as u32;
                indices.push(idx);
            }
        } else if self.index_buffer_raw.len() >= count * 4 {
            for i in 0..count {
                let off = i * 4;
                let idx = u32::from_le_bytes([
                    self.index_buffer_raw[off],
                    self.index_buffer_raw[off + 1],
                    self.index_buffer_raw[off + 2],
                    self.index_buffer_raw[off + 3],
                ]);
                indices.push(idx);
            }
        }

        indices
    }
}

/// LOD model containing submesh geometries, bounding bounds, and rendering distance.
#[derive(Debug, Clone)]
pub struct Rsc7LodModel {
    pub submeshes: Vec<Rsc7Geometry>,
    pub bounding_box_min: Vec3,
    pub bounding_box_max: Vec3,
    pub rendering_distance: f32,
}

/// Material shader parameter metadata.
#[derive(Debug, Clone)]
pub struct Rsc7Shader {
    pub name: String,
    pub hash: u32,
    pub parameter_count: u16,
}

/// Shader group containing materials and texture dictionary link.
#[derive(Debug, Clone)]
pub struct Rsc7ShaderGroup {
    pub shaders: Vec<Rsc7Shader>,
    pub texture_dictionary_ptr: u64,
}

/// Complete 3D Model Drawable (`.ydr`) with bounding metrics and 4-tier LOD hierarchy.
#[derive(Debug, Clone)]
pub struct Rsc7Drawable {
    pub name: String,
    pub bounding_box_min: Vec3,
    pub bounding_box_max: Vec3,
    pub bounding_sphere_center: Vec3,
    pub bounding_sphere_radius: f32,
    pub shader_group: Option<Rsc7ShaderGroup>,
    pub lod_high: Option<Rsc7LodModel>,
    pub lod_medium: Option<Rsc7LodModel>,
    pub lod_low: Option<Rsc7LodModel>,
    pub lod_vlow: Option<Rsc7LodModel>,
}

impl Rsc7Drawable {
    /// Parses an RSC7 Drawable from a binary buffer (with or without 16-byte container header).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Rsc7Error> {
        if bytes.len() < RSC7_HEADER_SIZE {
            return Err(Rsc7Error::BufferTooSmall {
                required: RSC7_HEADER_SIZE,
                actual: bytes.len(),
            });
        }

        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if Rsc7Header::is_valid_magic(magic) {
            let (header, decompressed) = decompress_rsc7(bytes)?;
            let (sys, gfx) = split_segments(&decompressed, &header);
            Self::from_decompressed_segments(&header, sys, gfx)
        } else {
            // Raw decompressed payload or headerless stream fallback
            let default_header = Rsc7Header {
                magic: RSC7_MAGIC,
                version: RSC7_VERSION_DRAWABLE,
                system_flags: 0x00020000,
                graphics_flags: 0x20000000,
            };
            let mid = bytes.len() / 2;
            Self::from_decompressed_segments(&default_header, &bytes[..mid], &bytes[mid..])
        }
    }

    /// Parses a Drawable directly from resolved system and graphics segments.
    pub fn from_decompressed_segments(
        _header: &Rsc7Header,
        system_segment: &[u8],
        graphics_segment: &[u8],
    ) -> Result<Self, Rsc7Error> {
        if system_segment.len() < 0x70 {
            return Err(Rsc7Error::BufferTooSmall {
                required: 0x70,
                actual: system_segment.len(),
            });
        }

        let reader = Rsc7VirtualReader::new(system_segment, graphics_segment);

        // Read Bounding metrics from Drawable root (offset 0)
        let min_x = reader.read_f32_system(0x20)?;
        let min_y = reader.read_f32_system(0x24)?;
        let min_z = reader.read_f32_system(0x28)?;
        let bounding_box_min = Vec3::new(min_x, min_y, min_z);

        let max_x = reader.read_f32_system(0x30)?;
        let max_y = reader.read_f32_system(0x34)?;
        let max_z = reader.read_f32_system(0x38)?;
        let bounding_box_max = Vec3::new(max_x, max_y, max_z);

        let sc_x = reader.read_f32_system(0x40)?;
        let sc_y = reader.read_f32_system(0x44)?;
        let sc_z = reader.read_f32_system(0x48)?;
        let bounding_sphere_center = Vec3::new(sc_x, sc_y, sc_z);
        let bounding_sphere_radius = reader.read_f32_system(0x4C)?;

        // Read Name if string pointer is present
        let name_ptr = reader.read_u64_system(0x70).unwrap_or(0);
        let name = if name_ptr != 0 {
            reader.read_cstring(name_ptr).unwrap_or_else(|_| "unnamed".into())
        } else {
            "unnamed".into()
        };

        // Read ShaderGroup
        let shader_group_ptr = reader.read_u64_system(0x10).unwrap_or(0);
        let shader_group = if shader_group_ptr != 0 {
            parse_shader_group(&reader, shader_group_ptr).ok()
        } else {
            None
        };

        // Read 4-tier LOD Models: High (+0x50), Med (+0x58), Low (+0x60), VLow (+0x68)
        let lod_high = parse_lod_tier(&reader, 0x50, bounding_box_min, bounding_box_max)?;
        let lod_med = parse_lod_tier(&reader, 0x58, bounding_box_min, bounding_box_max)?;
        let lod_low = parse_lod_tier(&reader, 0x60, bounding_box_min, bounding_box_max)?;
        let lod_vlow = parse_lod_tier(&reader, 0x68, bounding_box_min, bounding_box_max)?;

        Ok(Self {
            name,
            bounding_box_min,
            bounding_box_max,
            bounding_sphere_center,
            bounding_sphere_radius,
            shader_group,
            lod_high,
            lod_medium: lod_med,
            lod_low,
            lod_vlow,
        })
    }

    /// Returns the primary available LOD model (High -> Medium -> Low -> VLow).
    pub fn primary_lod(&self) -> Option<&Rsc7LodModel> {
        self.lod_high
            .as_ref()
            .or(self.lod_medium.as_ref())
            .or(self.lod_low.as_ref())
            .or(self.lod_vlow.as_ref())
    }

    /// Total submeshes across all LOD tiers.
    pub fn total_submeshes(&self) -> usize {
        let mut count = 0;
        if let Some(ref h) = self.lod_high { count += h.submeshes.len(); }
        if let Some(ref m) = self.lod_medium { count += m.submeshes.len(); }
        if let Some(ref l) = self.lod_low { count += l.submeshes.len(); }
        if let Some(ref v) = self.lod_vlow { count += v.submeshes.len(); }
        count
    }

    /// Total vertices in primary LOD.
    pub fn total_vertices(&self) -> usize {
        self.primary_lod()
            .map(|lod| lod.submeshes.iter().map(|m| m.vertex_count as usize).sum())
            .unwrap_or(0)
    }

    /// Total indices in primary LOD.
    pub fn total_indices(&self) -> usize {
        self.primary_lod()
            .map(|lod| lod.submeshes.iter().map(|m| m.index_count as usize).sum())
            .unwrap_or(0)
    }
}

/// Helper Virtual Address Space Reader for navigating RSC7 memory segments.
struct Rsc7VirtualReader<'a> {
    system: &'a [u8],
    graphics: &'a [u8],
}

impl<'a> Rsc7VirtualReader<'a> {
    fn new(system: &'a [u8], graphics: &'a [u8]) -> Self {
        Self { system, graphics }
    }

    fn read_f32_system(&self, offset: usize) -> Result<f32, Rsc7Error> {
        if offset + 4 > self.system.len() {
            return Err(Rsc7Error::BufferTooSmall { required: offset + 4, actual: self.system.len() });
        }
        Ok(f32::from_le_bytes([
            self.system[offset], self.system[offset + 1], self.system[offset + 2], self.system[offset + 3]
        ]))
    }

    fn read_u32_system(&self, offset: usize) -> Result<u32, Rsc7Error> {
        if offset + 4 > self.system.len() {
            return Err(Rsc7Error::BufferTooSmall { required: offset + 4, actual: self.system.len() });
        }
        Ok(u32::from_le_bytes([
            self.system[offset], self.system[offset + 1], self.system[offset + 2], self.system[offset + 3]
        ]))
    }

    fn read_u16_system(&self, offset: usize) -> Result<u16, Rsc7Error> {
        if offset + 2 > self.system.len() {
            return Err(Rsc7Error::BufferTooSmall { required: offset + 2, actual: self.system.len() });
        }
        Ok(u16::from_le_bytes([self.system[offset], self.system[offset + 1]]))
    }

    fn read_u64_system(&self, offset: usize) -> Result<u64, Rsc7Error> {
        if offset + 8 > self.system.len() {
            return Err(Rsc7Error::BufferTooSmall { required: offset + 8, actual: self.system.len() });
        }
        Ok(u64::from_le_bytes([
            self.system[offset], self.system[offset + 1], self.system[offset + 2], self.system[offset + 3],
            self.system[offset + 4], self.system[offset + 5], self.system[offset + 6], self.system[offset + 7],
        ]))
    }

    fn resolve_system_offset(&self, virtual_ptr: u64) -> Result<usize, Rsc7Error> {
        let addr = (virtual_ptr & 0xFFFF_FFFF) as u32;
        if addr == 0 {
            return Err(Rsc7Error::InvalidPointer { address: 0, segment: "system" });
        }
        let offset = if (addr & 0xF000_0000) == RSC7_SYSTEM_BASE {
            (addr & 0x0FFF_FFFF) as usize
        } else {
            addr as usize
        };

        if offset >= self.system.len() {
            return Err(Rsc7Error::InvalidPointer { address: addr, segment: "system" });
        }
        Ok(offset)
    }

    fn resolve_graphics_slice(&self, virtual_ptr: u64, length: usize) -> Result<&'a [u8], Rsc7Error> {
        let addr = (virtual_ptr & 0xFFFF_FFFF) as u32;
        let offset = if (addr & 0xF000_0000) == RSC7_GRAPHICS_BASE {
            (addr & 0x0FFF_FFFF) as usize
        } else {
            addr as usize
        };

        if offset + length > self.graphics.len() {
            return Err(Rsc7Error::InvalidPointer { address: addr, segment: "graphics" });
        }
        Ok(&self.graphics[offset..offset + length])
    }

    fn read_cstring(&self, virtual_ptr: u64) -> Result<String, Rsc7Error> {
        let offset = self.resolve_system_offset(virtual_ptr)?;
        let sub = &self.system[offset..];
        let len = sub.iter().position(|&b| b == 0).unwrap_or(sub.len());
        Ok(String::from_utf8_lossy(&sub[..len]).to_string())
    }
}

/// Parses an LOD tier from a pointer offset on the Drawable struct.
fn parse_lod_tier(
    reader: &Rsc7VirtualReader,
    lod_ptr_offset: usize,
    default_min: Vec3,
    default_max: Vec3,
) -> Result<Option<Rsc7LodModel>, Rsc7Error> {
    let ptr = reader.read_u64_system(lod_ptr_offset).unwrap_or(0);
    if ptr == 0 {
        return Ok(None);
    }

    let model_offset = match reader.resolve_system_offset(ptr) {
        Ok(off) => off,
        Err(_) => return Ok(None),
    };

    // If pointer points to an array of pointers:
    let geom_ptr = reader.read_u64_system(model_offset + 0x08).unwrap_or(0);
    let geom_count = reader.read_u32_system(model_offset + 0x10).unwrap_or(1);
    let render_dist = reader.read_f32_system(model_offset + 0x28).unwrap_or(100.0);

    let mut submeshes = Vec::new();

    if geom_ptr != 0 {
        if let Ok(geom_list_off) = reader.resolve_system_offset(geom_ptr) {
            for i in 0..geom_count as usize {
                let entry_off = geom_list_off + (i * 8);
                let single_geom_ptr = reader.read_u64_system(entry_off).unwrap_or(0);
                if single_geom_ptr != 0 {
                    if let Ok(geom) = parse_geometry(reader, single_geom_ptr) {
                        submeshes.push(geom);
                    }
                }
            }
        }
    }

    // Direct fallback if geometries array was inlined directly
    if submeshes.is_empty() {
        if let Ok(geom) = parse_geometry(reader, ptr) {
            submeshes.push(geom);
        }
    }

    if submeshes.is_empty() {
        return Ok(None);
    }

    Ok(Some(Rsc7LodModel {
        submeshes,
        bounding_box_min: default_min,
        bounding_box_max: default_max,
        rendering_distance: render_dist,
    }))
}

/// Parses an individual `Rsc7Geometry` structure.
fn parse_geometry(reader: &Rsc7VirtualReader, geom_ptr: u64) -> Result<Rsc7Geometry, Rsc7Error> {
    let geom_offset = reader.resolve_system_offset(geom_ptr)?;

    let vb_ptr = reader.read_u64_system(geom_offset + 0x18).unwrap_or(0);
    let ib_ptr = reader.read_u64_system(geom_offset + 0x38).unwrap_or(0);
    let indices_count = reader.read_u32_system(geom_offset + 0x58).unwrap_or(0);
    let vertices_count = reader.read_u16_system(geom_offset + 0x60).unwrap_or(0) as u32;

    // Parse VertexBuffer
    let vb_off = reader.resolve_system_offset(vb_ptr)?;
    let stride = reader.read_u16_system(vb_off + 0x08).unwrap_or(36) as u32;
    let vb_data_ptr = reader.read_u64_system(vb_off + 0x10).unwrap_or(0);
    let vb_decl_ptr = reader.read_u64_system(vb_off + 0x30).unwrap_or(0);

    let vertex_raw_bytes = reader
        .resolve_graphics_slice(vb_data_ptr, (vertices_count * stride) as usize)
        .map(|s| s.to_vec())
        .unwrap_or_default();

    // Parse IndexBuffer
    let ib_off = reader.resolve_system_offset(ib_ptr)?;
    let ib_data_ptr = reader.read_u64_system(ib_off + 0x10).unwrap_or(0);
    let index_raw_bytes = reader
        .resolve_graphics_slice(ib_data_ptr, (indices_count * 2) as usize)
        .map(|s| s.to_vec())
        .unwrap_or_default();

    // Parse VertexDeclaration
    let decl = if vb_decl_ptr != 0 {
        parse_vertex_declaration(reader, vb_decl_ptr).unwrap_or_else(|_| Rsc7VertexDeclaration::standard_p_n_c_uv())
    } else {
        Rsc7VertexDeclaration::standard_p_n_c_uv()
    };

    Ok(Rsc7Geometry {
        vertex_count: vertices_count,
        index_count: indices_count,
        vertex_stride: stride,
        vertex_buffer_raw: vertex_raw_bytes,
        index_buffer_raw: index_raw_bytes,
        primitive_type: Rsc7PrimitiveType::Triangles,
        vertex_declaration: decl,
    })
}

/// Parses a `Rsc7VertexDeclaration` struct.
fn parse_vertex_declaration(reader: &Rsc7VirtualReader, decl_ptr: u64) -> Result<Rsc7VertexDeclaration, Rsc7Error> {
    let off = reader.resolve_system_offset(decl_ptr)?;
    let channel_flags = reader.read_u32_system(off)?;
    let stride = reader.read_u16_system(off + 4)? as u32;
    let type_codes = reader.read_u64_system(off + 8).unwrap_or(0);

    let mut channels = Vec::new();
    let mut current_offset = 0usize;

    for bit in 0..16 {
        if (channel_flags & (1 << bit)) != 0 {
            let code = ((type_codes >> (bit * 4)) & 0x0F) as u8;
            let ch_type = Rsc7VertexChannelType::from_code(code);
            channels.push(Rsc7VertexChannel {
                semantic: Rsc7VertexChannelSemantic::from_bit(bit as u8),
                channel_type: ch_type,
                offset: current_offset,
            });
            current_offset += ch_type.byte_size();
        }
    }

    Ok(Rsc7VertexDeclaration {
        channel_flags,
        stride,
        channels,
    })
}

/// Parses a `Rsc7ShaderGroup` struct.
fn parse_shader_group(reader: &Rsc7VirtualReader, sg_ptr: u64) -> Result<Rsc7ShaderGroup, Rsc7Error> {
    let off = reader.resolve_system_offset(sg_ptr)?;
    let tex_dict_ptr = reader.read_u64_system(off + 0x08).unwrap_or(0);
    let shaders_ptr = reader.read_u64_system(off + 0x10).unwrap_or(0);
    let shaders_count = reader.read_u16_system(off + 0x18).unwrap_or(0) as usize;

    let mut shaders = Vec::with_capacity(shaders_count);
    if shaders_ptr != 0 && shaders_count > 0 {
        if let Ok(shaders_off) = reader.resolve_system_offset(shaders_ptr) {
            for i in 0..shaders_count {
                let s_off = shaders_off + (i * 16);
                let hash = reader.read_u32_system(s_off).unwrap_or(0);
                let name_ptr = reader.read_u64_system(s_off + 8).unwrap_or(0);
                let name = if name_ptr != 0 {
                    reader.read_cstring(name_ptr).unwrap_or_else(|_| "shader".into())
                } else {
                    "shader".into()
                };
                shaders.push(Rsc7Shader {
                    name,
                    hash,
                    parameter_count: 0,
                });
            }
        }
    }

    Ok(Rsc7ShaderGroup {
        shaders,
        texture_dictionary_ptr: tex_dict_ptr,
    })
}

/// Decompresses an RSC7 binary payload and returns parsed header and uncompressed payload bytes.
pub fn decompress_rsc7(bytes: &[u8]) -> Result<(Rsc7Header, Vec<u8>), Rsc7Error> {
    let header = Rsc7Header::from_bytes(bytes)?;
    let compressed_payload = &bytes[RSC7_HEADER_SIZE..];

    if compressed_payload.is_empty() {
        return Ok((header, Vec::new()));
    }

    // Try raw deflate first (Rockstar GTA 5 standard), fallback to zlib
    let decompressed = match miniz_oxide::inflate::decompress_to_vec(compressed_payload) {
        Ok(data) => data,
        Err(_) => miniz_oxide::inflate::decompress_to_vec_zlib(compressed_payload)
            .map_err(|e| Rsc7Error::DecompressionFailed(format!("{:?}", e)))?,
    };

    Ok((header, decompressed))
}

/// Splits uncompressed resource payload into contiguous (system, graphics) slices.
pub fn split_segments<'a>(payload: &'a [u8], header: &Rsc7Header) -> (&'a [u8], &'a [u8]) {
    let sys_size = header.system_allocation().total_size;
    if sys_size <= payload.len() {
        (&payload[..sys_size], &payload[sys_size..])
    } else {
        let half = payload.len() / 2;
        (&payload[..half], &payload[half..])
    }
}

/// Helper function to convert 16-bit IEEE 754 half-precision float to 32-bit float.
pub fn half_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let mant = (h & 0x3FF) as u32;

    if exp == 0 {
        if mant == 0 {
            f32::from_bits(sign << 31)
        } else {
            let mut m = mant;
            let mut e = 113u32;
            while (m & 0x400) == 0 {
                m <<= 1;
                e -= 1;
            }
            m &= 0x3FF;
            f32::from_bits((sign << 31) | (e << 23) | (m << 13))
        }
    } else if exp == 31 {
        f32::from_bits((sign << 31) | 0x7F80_0000 | (mant << 13))
    } else {
        let new_exp = exp + 127 - 15;
        f32::from_bits((sign << 31) | (new_exp << 23) | (mant << 13))
    }
}

fn read_vec3_channel(vdata: &[u8], ch: &Rsc7VertexChannel) -> Vec3 {
    let off = ch.offset;
    match ch.channel_type {
        Rsc7VertexChannelType::Float3 | Rsc7VertexChannelType::Float4 => {
            if off + 12 <= vdata.len() {
                let x = f32::from_le_bytes([vdata[off], vdata[off + 1], vdata[off + 2], vdata[off + 3]]);
                let y = f32::from_le_bytes([vdata[off + 4], vdata[off + 5], vdata[off + 6], vdata[off + 7]]);
                let z = f32::from_le_bytes([vdata[off + 8], vdata[off + 9], vdata[off + 10], vdata[off + 11]]);
                Vec3::new(x, y, z)
            } else {
                Vec3::ZERO
            }
        }
        Rsc7VertexChannelType::Half4 => {
            if off + 6 <= vdata.len() {
                let h0 = u16::from_le_bytes([vdata[off], vdata[off + 1]]);
                let h1 = u16::from_le_bytes([vdata[off + 2], vdata[off + 3]]);
                let h2 = u16::from_le_bytes([vdata[off + 4], vdata[off + 5]]);
                Vec3::new(half_to_f32(h0), half_to_f32(h1), half_to_f32(h2))
            } else {
                Vec3::ZERO
            }
        }
        _ => Vec3::ZERO,
    }
}

fn read_vec2_channel(vdata: &[u8], ch: &Rsc7VertexChannel) -> Vec2 {
    let off = ch.offset;
    match ch.channel_type {
        Rsc7VertexChannelType::Float2 => {
            if off + 8 <= vdata.len() {
                let u = f32::from_le_bytes([vdata[off], vdata[off + 1], vdata[off + 2], vdata[off + 3]]);
                let v = f32::from_le_bytes([vdata[off + 4], vdata[off + 5], vdata[off + 6], vdata[off + 7]]);
                Vec2::new(u, v)
            } else {
                Vec2::ZERO
            }
        }
        Rsc7VertexChannelType::Half2 => {
            if off + 4 <= vdata.len() {
                let h0 = u16::from_le_bytes([vdata[off], vdata[off + 1]]);
                let h1 = u16::from_le_bytes([vdata[off + 2], vdata[off + 3]]);
                Vec2::new(half_to_f32(h0), half_to_f32(h1))
            } else {
                Vec2::ZERO
            }
        }
        _ => Vec2::ZERO,
    }
}

fn read_vec4_channel(vdata: &[u8], ch: &Rsc7VertexChannel) -> Vec4 {
    let off = ch.offset;
    match ch.channel_type {
        Rsc7VertexChannelType::UByte4Norm | Rsc7VertexChannelType::UByte4 => {
            if off + 4 <= vdata.len() {
                let r = vdata[off] as f32 / 255.0;
                let g = vdata[off + 1] as f32 / 255.0;
                let b = vdata[off + 2] as f32 / 255.0;
                let a = vdata[off + 3] as f32 / 255.0;
                Vec4::new(r, g, b, a)
            } else {
                Vec4::ONE
            }
        }
        Rsc7VertexChannelType::Float4 => {
            if off + 16 <= vdata.len() {
                let r = f32::from_le_bytes([vdata[off], vdata[off + 1], vdata[off + 2], vdata[off + 3]]);
                let g = f32::from_le_bytes([vdata[off + 4], vdata[off + 5], vdata[off + 6], vdata[off + 7]]);
                let b = f32::from_le_bytes([vdata[off + 8], vdata[off + 9], vdata[off + 10], vdata[off + 11]]);
                let a = f32::from_le_bytes([vdata[off + 12], vdata[off + 13], vdata[off + 14], vdata[off + 15]]);
                Vec4::new(r, g, b, a)
            } else {
                Vec4::ONE
            }
        }
        _ => Vec4::ONE,
    }
}

/// Generates valid, 100% reproducible synthetic RSC7 containers for unit testing and CI benchmarks.
pub fn create_synthetic_rsc7_drawable(name: &str, primitive: &str) -> Vec<u8> {
    let (vertices, indices, bbox_min, bbox_max, sphere_center, sphere_radius) = match primitive {
        "car" | "car_chassis" => generate_synthetic_car_mesh(),
        "pyramid" | "character_pyramid" => generate_synthetic_pyramid_mesh(),
        _ => generate_synthetic_cube_mesh(),
    };

    let vertex_stride = 36usize;
    let mut vertex_raw = Vec::with_capacity(vertices.len() * vertex_stride);
    for v in &vertices {
        vertex_raw.extend_from_slice(&v.position.x.to_le_bytes());
        vertex_raw.extend_from_slice(&v.position.y.to_le_bytes());
        vertex_raw.extend_from_slice(&v.position.z.to_le_bytes());

        vertex_raw.extend_from_slice(&v.normal.x.to_le_bytes());
        vertex_raw.extend_from_slice(&v.normal.y.to_le_bytes());
        vertex_raw.extend_from_slice(&v.normal.z.to_le_bytes());

        let cr = (v.color.x.clamp(0.0, 1.0) * 255.0) as u8;
        let cg = (v.color.y.clamp(0.0, 1.0) * 255.0) as u8;
        let cb = (v.color.z.clamp(0.0, 1.0) * 255.0) as u8;
        let ca = (v.color.w.clamp(0.0, 1.0) * 255.0) as u8;
        vertex_raw.push(cr);
        vertex_raw.push(cg);
        vertex_raw.push(cb);
        vertex_raw.push(ca);

        vertex_raw.extend_from_slice(&v.uv0.x.to_le_bytes());
        vertex_raw.extend_from_slice(&v.uv0.y.to_le_bytes());
    }

    let mut index_raw = Vec::with_capacity(indices.len() * 2);
    for &idx in &indices {
        index_raw.extend_from_slice(&(idx as u16).to_le_bytes());
    }

    // Build Graphics Segment
    let mut graphics_segment = Vec::new();
    let vb_gpu_offset = 0u64;
    graphics_segment.extend_from_slice(&vertex_raw);

    let ib_gpu_offset = graphics_segment.len() as u64;
    graphics_segment.extend_from_slice(&index_raw);

    // Align graphics segment to 512 bytes
    let gfx_unpadded_len = graphics_segment.len();
    let gfx_aligned_len = gfx_unpadded_len.div_ceil(512) * 512;
    graphics_segment.resize(gfx_aligned_len, 0u8);

    // Build System Segment
    let mut system_segment = vec![0u8; 1024]; // Preallocate base 1 KB for structures

    // Offsets within System Segment
    let sg_offset = 0x100u64;
    let lod_high_offset = 0x180u64;
    let geom_list_offset = 0x220u64;
    let geom_offset = 0x260u64;
    let vb_offset = 0x300u64;
    let ib_offset = 0x360u64;
    let decl_offset = 0x3C0u64;
    let name_offset = 0x080u64;

    // 1. Root Drawable Struct (at 0x00)
    system_segment[0x10..0x18].copy_from_slice(&(RSC7_SYSTEM_BASE as u64 | sg_offset).to_le_bytes());
    system_segment[0x20..0x24].copy_from_slice(&bbox_min.x.to_le_bytes());
    system_segment[0x24..0x28].copy_from_slice(&bbox_min.y.to_le_bytes());
    system_segment[0x28..0x2C].copy_from_slice(&bbox_min.z.to_le_bytes());

    system_segment[0x30..0x34].copy_from_slice(&bbox_max.x.to_le_bytes());
    system_segment[0x34..0x38].copy_from_slice(&bbox_max.y.to_le_bytes());
    system_segment[0x38..0x3C].copy_from_slice(&bbox_max.z.to_le_bytes());

    system_segment[0x40..0x44].copy_from_slice(&sphere_center.x.to_le_bytes());
    system_segment[0x44..0x48].copy_from_slice(&sphere_center.y.to_le_bytes());
    system_segment[0x48..0x4C].copy_from_slice(&sphere_center.z.to_le_bytes());
    system_segment[0x4C..0x50].copy_from_slice(&sphere_radius.to_le_bytes());

    system_segment[0x50..0x58].copy_from_slice(&(RSC7_SYSTEM_BASE as u64 | lod_high_offset).to_le_bytes());
    system_segment[0x70..0x78].copy_from_slice(&(RSC7_SYSTEM_BASE as u64 | name_offset).to_le_bytes());

    // Write Name string at 0x80
    let name_bytes = name.as_bytes();
    let name_len = name_bytes.len().min(63);
    system_segment[name_offset as usize..name_offset as usize + name_len].copy_from_slice(&name_bytes[..name_len]);
    system_segment[name_offset as usize + name_len] = 0; // null terminator

    // 2. ShaderGroup Struct (at 0x100)
    system_segment[sg_offset as usize + 0x10..sg_offset as usize + 0x18]
        .copy_from_slice(&(RSC7_SYSTEM_BASE as u64 | 0x120u64).to_le_bytes());
    system_segment[sg_offset as usize + 0x18..sg_offset as usize + 0x1A].copy_from_slice(&1u16.to_le_bytes());

    // Single Shader at 0x120
    let s_off = 0x120usize;
    system_segment[s_off..s_off + 4].copy_from_slice(&0x5C89F3A1u32.to_le_bytes()); // Shader Hash
    system_segment[s_off + 8..s_off + 16].copy_from_slice(&(RSC7_SYSTEM_BASE as u64 | 0x140u64).to_le_bytes());
    let sh_name = b"default_shader\0";
    system_segment[0x140..0x140 + sh_name.len()].copy_from_slice(sh_name);

    // 3. LodModel Struct (at 0x180)
    let lm = lod_high_offset as usize;
    system_segment[lm + 0x08..lm + 0x10].copy_from_slice(&(RSC7_SYSTEM_BASE as u64 | geom_list_offset).to_le_bytes());
    system_segment[lm + 0x10..lm + 0x14].copy_from_slice(&1u32.to_le_bytes()); // Geom Count: 1
    system_segment[lm + 0x28..lm + 0x2C].copy_from_slice(&150.0f32.to_le_bytes()); // Render distance

    // Geometries list array (at 0x220)
    system_segment[geom_list_offset as usize..geom_list_offset as usize + 8]
        .copy_from_slice(&(RSC7_SYSTEM_BASE as u64 | geom_offset).to_le_bytes());

    // 4. Geometry Struct (at 0x260)
    let gm = geom_offset as usize;
    system_segment[gm + 0x18..gm + 0x20].copy_from_slice(&(RSC7_SYSTEM_BASE as u64 | vb_offset).to_le_bytes());
    system_segment[gm + 0x38..gm + 0x40].copy_from_slice(&(RSC7_SYSTEM_BASE as u64 | ib_offset).to_le_bytes());
    system_segment[gm + 0x58..gm + 0x5C].copy_from_slice(&(indices.len() as u32).to_le_bytes());
    system_segment[gm + 0x60..gm + 0x62].copy_from_slice(&(vertices.len() as u16).to_le_bytes());

    // 5. VertexBuffer Struct (at 0x300)
    let vb = vb_offset as usize;
    system_segment[vb + 0x08..vb + 0x0A].copy_from_slice(&(vertex_stride as u16).to_le_bytes());
    system_segment[vb + 0x10..vb + 0x18].copy_from_slice(&(RSC7_GRAPHICS_BASE as u64 | vb_gpu_offset).to_le_bytes());
    system_segment[vb + 0x18..vb + 0x1C].copy_from_slice(&(vertices.len() as u32).to_le_bytes());
    system_segment[vb + 0x30..vb + 0x38].copy_from_slice(&(RSC7_SYSTEM_BASE as u64 | decl_offset).to_le_bytes());

    // 6. IndexBuffer Struct (at 0x360)
    let ib = ib_offset as usize;
    system_segment[ib + 0x08..ib + 0x0C].copy_from_slice(&(indices.len() as u32).to_le_bytes());
    system_segment[ib + 0x10..ib + 0x18].copy_from_slice(&(RSC7_GRAPHICS_BASE as u64 | ib_gpu_offset).to_le_bytes());

    // 7. VertexDeclaration Struct (at 0x3C0)
    let vd = decl_offset as usize;
    let ch_flags: u32 = (1 << 0) | (1 << 3) | (1 << 4) | (1 << 6); // 0x0059
    system_segment[vd..vd + 4].copy_from_slice(&ch_flags.to_le_bytes());
    system_segment[vd + 4..vd + 6].copy_from_slice(&(vertex_stride as u16).to_le_bytes());
    system_segment[vd + 6] = 4u8; // 4 channels
    let type_codes: u64 = 6 | (6 << 12) | (9 << 16) | (5 << 24);
    system_segment[vd + 8..vd + 16].copy_from_slice(&type_codes.to_le_bytes());

    // Align system segment to 512 bytes
    let sys_aligned_len = system_segment.len().div_ceil(512) * 512;
    system_segment.resize(sys_aligned_len, 0u8);

    // Compute System & Graphics allocation flags
    let sys_pages = sys_aligned_len / 512;
    let gfx_pages = gfx_aligned_len / 512;
    let system_flags = Rsc7VirtualAllocation::encode(0, sys_pages);
    let graphics_flags = Rsc7VirtualAllocation::encode(0, gfx_pages);

    // Assemble uncompressed payload: [system_segment | graphics_segment]
    let mut payload = Vec::with_capacity(sys_aligned_len + gfx_aligned_len);
    payload.extend_from_slice(&system_segment);
    payload.extend_from_slice(&graphics_segment);

    // Deflate compress payload
    let compressed_payload = miniz_oxide::deflate::compress_to_vec(&payload, 6);

    // Build final 16-byte header + compressed payload
    let mut out = Vec::with_capacity(RSC7_HEADER_SIZE + compressed_payload.len());
    let header = Rsc7Header {
        magic: RSC7_MAGIC,
        version: RSC7_VERSION_DRAWABLE,
        system_flags,
        graphics_flags,
    };
    out.extend_from_slice(&header.to_bytes());
    out.extend_from_slice(&compressed_payload);

    out
}

/// Generates a synthetic 3D Cube mesh (24 vertices, 36 indices, 6 face normals).
fn generate_synthetic_cube_mesh() -> (Vec<Rsc7Vertex>, Vec<u32>, Vec3, Vec3, Vec3, f32) {
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);

    let faces = [
        // Front (+Z)
        (Vec3::new(0.0, 0.0, 1.0), [Vec3::new(-1.0, -1.0, 1.0), Vec3::new(1.0, -1.0, 1.0), Vec3::new(1.0, 1.0, 1.0), Vec3::new(-1.0, 1.0, 1.0)]),
        // Back (-Z)
        (Vec3::new(0.0, 0.0, -1.0), [Vec3::new(1.0, -1.0, -1.0), Vec3::new(-1.0, -1.0, -1.0), Vec3::new(-1.0, 1.0, -1.0), Vec3::new(1.0, 1.0, -1.0)]),
        // Right (+X)
        (Vec3::new(1.0, 0.0, 0.0), [Vec3::new(1.0, -1.0, 1.0), Vec3::new(1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)]),
        // Left (-X)
        (Vec3::new(-1.0, 0.0, 0.0), [Vec3::new(-1.0, -1.0, -1.0), Vec3::new(-1.0, -1.0, 1.0), Vec3::new(-1.0, 1.0, 1.0), Vec3::new(-1.0, 1.0, -1.0)]),
        // Top (+Y)
        (Vec3::new(0.0, 1.0, 0.0), [Vec3::new(-1.0, 1.0, 1.0), Vec3::new(1.0, 1.0, 1.0), Vec3::new(1.0, 1.0, -1.0), Vec3::new(-1.0, 1.0, -1.0)]),
        // Bottom (-Y)
        (Vec3::new(0.0, -1.0, 0.0), [Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, -1.0, -1.0), Vec3::new(1.0, -1.0, 1.0), Vec3::new(-1.0, -1.0, 1.0)]),
    ];

    let uvs = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)];

    for (norm, pts) in faces {
        let base_idx = vertices.len() as u32;
        for i in 0..4 {
            vertices.push(Rsc7Vertex {
                position: pts[i],
                normal: norm,
                uv0: uvs[i],
                color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            });
        }
        indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2, base_idx, base_idx + 2, base_idx + 3]);
    }

    let min = Vec3::new(-1.0, -1.0, -1.0);
    let max = Vec3::new(1.0, 1.0, 1.0);
    let center = Vec3::ZERO;
    let radius = 1.732_050_8f32; // sqrt(3)

    (vertices, indices, min, max, center, radius)
}

/// Generates a synthetic car chassis mesh (low-poly vehicle silhouette).
fn generate_synthetic_car_mesh() -> (Vec<Rsc7Vertex>, Vec<u32>, Vec3, Vec3, Vec3, f32) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Lower chassis box: (-1.0, -2.2, 0.0) to (1.0, 2.2, 0.6)
    let p_min = Vec3::new(-1.0, -2.2, 0.0);
    let p_max = Vec3::new(1.0, 2.2, 0.6);

    let corners = [
        Vec3::new(p_min.x, p_min.y, p_min.z),
        Vec3::new(p_max.x, p_min.y, p_min.z),
        Vec3::new(p_max.x, p_max.y, p_min.z),
        Vec3::new(p_min.x, p_max.y, p_min.z),
        Vec3::new(p_min.x, p_min.y, p_max.z),
        Vec3::new(p_max.x, p_min.y, p_max.z),
        Vec3::new(p_max.x, p_max.y, p_max.z),
        Vec3::new(p_min.x, p_max.y, p_max.z),
        // Cabin peak: (0.0, 0.0, 1.4)
        Vec3::new(0.0, 0.0, 1.4),
    ];

    for (i, &pt) in corners.iter().enumerate() {
        vertices.push(Rsc7Vertex {
            position: pt,
            normal: Vec3::new(0.0, 0.0, 1.0),
            uv0: Vec2::new((i % 2) as f32, ((i / 2) % 2) as f32),
            color: Vec4::new(0.2, 0.4, 0.8, 1.0),
        });
    }

    // Indices for chassis box + cabin peak
    let tri_indices = [
        0, 1, 5, 0, 5, 4, // front
        2, 3, 7, 2, 7, 6, // back
        1, 2, 6, 1, 6, 5, // right
        3, 0, 4, 3, 4, 7, // left
        4, 5, 8, 5, 6, 8, 6, 7, 8, 7, 4, 8, // cabin hood slopes
    ];
    indices.extend_from_slice(&tri_indices);

    let min = Vec3::new(-1.0, -2.2, 0.0);
    let max = Vec3::new(1.0, 2.2, 1.4);
    let center = Vec3::new(0.0, 0.0, 0.7);
    let radius = 2.6f32;

    (vertices, indices, min, max, center, radius)
}

/// Generates a synthetic character pyramid mesh (pyramidal avatar landmark).
fn generate_synthetic_pyramid_mesh() -> (Vec<Rsc7Vertex>, Vec<u32>, Vec3, Vec3, Vec3, f32) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let pts = [
        Vec3::new(-0.5, -0.5, 0.0),
        Vec3::new(0.5, -0.5, 0.0),
        Vec3::new(0.5, 0.5, 0.0),
        Vec3::new(-0.5, 0.5, 0.0),
        Vec3::new(0.0, 0.0, 1.8), // Apex at 1.8m
    ];

    for (i, &pt) in pts.iter().enumerate() {
        vertices.push(Rsc7Vertex {
            position: pt,
            normal: if i == 4 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(0.0, 1.0, 0.0) },
            uv0: Vec2::new((i % 2) as f32, (i / 2) as f32),
            color: Vec4::new(0.9, 0.6, 0.2, 1.0),
        });
    }

    let tri_indices = [
        0, 1, 4, // face 1
        1, 2, 4, // face 2
        2, 3, 4, // face 3
        3, 0, 4, // face 4
        0, 3, 2, 0, 2, 1, // base
    ];
    indices.extend_from_slice(&tri_indices);

    let min = Vec3::new(-0.5, -0.5, 0.0);
    let max = Vec3::new(0.5, 0.5, 1.8);
    let center = Vec3::new(0.0, 0.0, 0.9);
    let radius = 1.15f32;

    (vertices, indices, min, max, center, radius)
}

/// High-throughput extraction helper for reading RSC7 Drawables directly from RPF archives.
pub struct Rsc7ArchiveExtractor;

impl Rsc7ArchiveExtractor {
    /// Discovers all `.ydr` drawable files present within an RPF archive.
    pub fn list_drawables<R: Read + Seek>(archive: &RpfArchive<R>) -> Vec<String> {
        archive
            .entries()
            .iter()
            .filter(|e| e.name().to_ascii_lowercase().ends_with(".ydr"))
            .map(|e| e.virtual_path().to_string())
            .collect()
    }

    /// Extracts and parses a single `.ydr` drawable by path or filename.
    pub fn extract_drawable<R: Read + Seek>(
        archive: &mut RpfArchive<R>,
        path: &str,
    ) -> Result<Rsc7Drawable, Rsc7Error> {
        let entry = archive
            .get_entry(path)
            .cloned()
            .ok_or_else(|| Rsc7Error::NotFound(path.to_string()))?;

        match entry {
            RpfEntry::ResourceFile(res) => {
                let raw = archive.read_entry_raw(&RpfEntry::ResourceFile(res.clone()))?;
                if raw.len() >= 4 && Rsc7Header::is_valid_magic(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]])) {
                    Rsc7Drawable::from_bytes(&raw)
                } else {
                    // Prepend synthetic 16-byte header with flags from RPF entry
                    let mut container = Vec::with_capacity(16 + raw.len());
                    container.extend_from_slice(&RSC7_MAGIC.to_le_bytes());
                    container.extend_from_slice(&RSC7_VERSION_DRAWABLE.to_le_bytes());
                    container.extend_from_slice(&res.system_flags.to_le_bytes());
                    container.extend_from_slice(&res.graphics_flags.to_le_bytes());
                    container.extend_from_slice(&raw);
                    Rsc7Drawable::from_bytes(&container)
                }
            }
            RpfEntry::BinaryFile(_) => {
                let bytes = archive.read_decompressed_by_path(path)?;
                Rsc7Drawable::from_bytes(&bytes)
            }
            RpfEntry::Directory(_) => Err(Rsc7Error::CorruptedData("Cannot parse directory as RSC7".into())),
        }
    }

    /// Extracts all `.ydr` drawables found in an RPF archive.
    pub fn extract_all_drawables<R: Read + Seek>(
        archive: &mut RpfArchive<R>,
    ) -> Vec<Result<Rsc7Drawable, Rsc7Error>> {
        let paths = Self::list_drawables(archive);
        let mut results = Vec::with_capacity(paths.len());
        for p in paths {
            results.push(Self::extract_drawable(archive, &p));
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpf::create_synthetic_rpf;

    #[test]
    fn test_rsc7_header_parsing_and_flags_decoding() {
        let header = Rsc7Header {
            magic: RSC7_MAGIC,
            version: RSC7_VERSION_DRAWABLE,
            system_flags: 0x00020000,
            graphics_flags: 0x20000000,
        };

        let bytes = header.to_bytes();
        let parsed = Rsc7Header::from_bytes(&bytes).expect("Failed to parse valid RSC7 header");
        assert_eq!(parsed, header);

        // Test flag decoding
        let sys_alloc = parsed.system_allocation();
        assert_eq!(sys_alloc.base_shift, 0);
        assert_eq!(sys_alloc.base_page_size, 512);
        assert_eq!(sys_alloc.page_count, 1);
        assert_eq!(sys_alloc.total_size, 512);

        // Test flag round-trip
        let encoded = Rsc7VirtualAllocation::encode(2, 8);
        let decoded = Rsc7VirtualAllocation::decode(encoded);
        assert_eq!(decoded.base_shift, 2);
        assert_eq!(decoded.base_page_size, 2048);
        assert_eq!(decoded.page_count, 8);
        assert_eq!(decoded.total_size, 2048 * 8);

        // Test alternative magic
        let mut alt_bytes = bytes;
        alt_bytes[0..4].copy_from_slice(&RSC7_ALT_MAGIC.to_le_bytes());
        assert!(Rsc7Header::from_bytes(&alt_bytes).is_ok());
    }

    #[test]
    fn test_synthetic_cube_drawable_creation_and_parsing() {
        let container = create_synthetic_rsc7_drawable("test_cube", "cube");
        assert!(!container.is_empty());

        let drawable = Rsc7Drawable::from_bytes(&container).expect("Failed to parse synthetic cube drawable");
        assert_eq!(drawable.name, "test_cube");
        assert_eq!(drawable.bounding_box_min, Vec3::new(-1.0, -1.0, -1.0));
        assert_eq!(drawable.bounding_box_max, Vec3::new(1.0, 1.0, 1.0));
        assert!(drawable.bounding_sphere_radius > 1.7);

        assert!(drawable.shader_group.is_some());
        let sg = drawable.shader_group.as_ref().unwrap();
        assert_eq!(sg.shaders.len(), 1);
        assert_eq!(sg.shaders[0].name, "default_shader");

        let lod = drawable.primary_lod().expect("Primary LOD missing");
        assert_eq!(lod.submeshes.len(), 1);

        let mesh = &lod.submeshes[0];
        assert_eq!(mesh.vertex_count, 24);
        assert_eq!(mesh.index_count, 36);

        let vertices = mesh.to_vertex_buffer();
        assert_eq!(vertices.len(), 24);
        assert_eq!(vertices[0].normal, Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(vertices[0].color, Vec4::ONE);

        let indices = mesh.to_indices();
        assert_eq!(indices.len(), 36);
        assert_eq!(indices[0..6], [0, 1, 2, 0, 2, 3]);
    }

    #[test]
    fn test_synthetic_car_chassis_drawable() {
        let container = create_synthetic_rsc7_drawable("infernus_chassis", "car");
        let drawable = Rsc7Drawable::from_bytes(&container).expect("Failed to parse car chassis");

        assert_eq!(drawable.name, "infernus_chassis");
        assert_eq!(drawable.bounding_box_min.x, -1.0);
        assert_eq!(drawable.bounding_box_max.x, 1.0);
        assert!(drawable.bounding_sphere_radius > 2.0);

        let lod = drawable.lod_high.expect("High LOD missing");
        assert_eq!(lod.submeshes.len(), 1);
        let mesh = &lod.submeshes[0];
        assert_eq!(mesh.vertex_count, 9);
        assert_eq!(mesh.to_indices().len(), 36);
    }

    #[test]
    fn test_synthetic_character_pyramid_drawable() {
        let container = create_synthetic_rsc7_drawable("ped_landmark", "pyramid");
        let drawable = Rsc7Drawable::from_bytes(&container).expect("Failed to parse pyramid");

        assert_eq!(drawable.name, "ped_landmark");
        assert_eq!(drawable.bounding_box_max.z, 1.8);

        let lod = drawable.lod_high.expect("High LOD missing");
        assert_eq!(lod.submeshes[0].vertex_count, 5);
        assert_eq!(lod.submeshes[0].to_indices().len(), 18);
    }

    #[test]
    fn test_rsc7_archive_extractor_with_synthetic_rpf() {
        let cube_ydr = create_synthetic_rsc7_drawable("cube_prop", "cube");
        let car_ydr = create_synthetic_rsc7_drawable("vehicle_body", "car");

        let archive_bytes = create_synthetic_rpf(&[
            ("models/cube_prop.ydr", &cube_ydr, false),
            ("vehicles/vehicle_body.ydr", &car_ydr, true),
        ]);

        let mut archive = RpfArchive::from_bytes(archive_bytes).expect("Failed to parse RPF archive");

        let drawables_list = Rsc7ArchiveExtractor::list_drawables(&archive);
        assert_eq!(drawables_list.len(), 2);
        assert!(drawables_list.iter().any(|d| d.contains("cube_prop.ydr")));
        assert!(drawables_list.iter().any(|d| d.contains("vehicle_body.ydr")));

        let cube = Rsc7ArchiveExtractor::extract_drawable(&mut archive, "models/cube_prop.ydr")
            .expect("Extract cube failed");
        assert_eq!(cube.name, "cube_prop");
        assert_eq!(cube.total_vertices(), 24);

        let car = Rsc7ArchiveExtractor::extract_drawable(&mut archive, "vehicles/vehicle_body.ydr")
            .expect("Extract car failed");
        assert_eq!(car.name, "vehicle_body");
        assert_eq!(car.total_vertices(), 9);

        let all = Rsc7ArchiveExtractor::extract_all_drawables(&mut archive);
        assert_eq!(all.len(), 2);
        assert!(all.iter().all(|res| res.is_ok()));
    }

    #[test]
    fn test_error_handling_corrupted_header_and_data() {
        // Test invalid magic
        let corrupted_magic = [0xFF, 0xFF, 0xFF, 0xFF, 165, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let res = Rsc7Header::from_bytes(&corrupted_magic);
        assert!(matches!(res, Err(Rsc7Error::InvalidMagic(_))));

        // Test truncated buffer
        let truncated = [0x52, 0x53];
        let res = Rsc7Header::from_bytes(&truncated);
        assert!(matches!(res, Err(Rsc7Error::BufferTooSmall { .. })));

        // Test corrupted payload decompression
        let mut corrupted_container = RSC7_MAGIC.to_le_bytes().to_vec();
        corrupted_container.extend_from_slice(&165u32.to_le_bytes());
        corrupted_container.extend_from_slice(&0u32.to_le_bytes());
        corrupted_container.extend_from_slice(&0u32.to_le_bytes());
        corrupted_container.extend_from_slice(&[0x12, 0x34, 0x56, 0x78]); // Invalid deflate bytes
        let res = decompress_rsc7(&corrupted_container);
        assert!(matches!(res, Err(Rsc7Error::DecompressionFailed(_))));
    }

    #[test]
    fn test_half_float_conversion() {
        assert_eq!(half_to_f32(0x0000), 0.0);
        assert_eq!(half_to_f32(0x3C00), 1.0);
        assert_eq!(half_to_f32(0xBC00), -1.0);
        assert_eq!(half_to_f32(0x3800), 0.5);
    }
}
