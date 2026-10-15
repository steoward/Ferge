//! The binary glTF (GLB) container: a 12-byte header followed by a JSON chunk
//! and a BIN chunk.
//!
//! Layout:
//! ```text
//! offset  size  contents
//! 0       4     magic   "glTF" (0x46546C67 little-endian)
//! 4       4     version 2
//! 8       4     total file length
//! 12      4     chunk[0] length (JSON)
//! 16      4     chunk[0] type 0x4E4F534A ("JSON")
//! 20      len   JSON, padded with spaces to a multiple of 4
//! 20+len  4     chunk[1] length (BIN)
//! 24+len  4     chunk[1] type 0x004E4942 ("BIN\0")
//! 28+len  len   binary buffer, padded with zeros to a multiple of 4
//! ```

/// GLB magic: `glTF` read as a little-endian word.
pub const GLB_MAGIC: u32 = 0x46546C67;

/// Chunk type for the JSON payload.
pub const CHUNK_TYPE_JSON: u32 = 0x4E4F534A;

/// Chunk type for the binary payload.
pub const CHUNK_TYPE_BIN: u32 = 0x004E4942;

/// The 12-byte GLB file header.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlbHeader {
    pub magic: u32,
    pub version: u32,
    pub length: u32,
}

/// A chunk found in the GLB body.
#[derive(Debug, Clone, PartialEq)]
pub struct GlbChunk {
    pub chunk_type: u32,
    /// Byte offset of the chunk's payload relative to the start of the file.
    pub offset: usize,
    pub length: usize,
}

/// Where a GLB failed to parse.
#[derive(Debug, Clone, PartialEq)]
pub enum GlbError {
    TooShort,
    BadMagic(u32),
    UnsupportedVersion(u32),
    LengthMismatch { declared: u32, actual: usize },
    TruncatedChunk { at: usize, need: usize, have: usize },
    UnknownChunkType(u32),
}

impl std::fmt::Display for GlbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GlbError::TooShort => write!(f, "GLB file is shorter than its 12-byte header"),
            GlbError::BadMagic(m) => write!(f, "GLB magic mismatch: 0x{:08X}", m),
            GlbError::UnsupportedVersion(v) => {
                write!(f, "unsupported GLB version {} (only 2 is supported)", v)
            }
            GlbError::LengthMismatch { declared, actual } => write!(
                f,
                "GLB header declares {} bytes but the file holds {}",
                declared, actual
            ),
            GlbError::TruncatedChunk { at, need, have } => write!(
                f,
                "chunk at byte {} needs {} bytes but only {} remain",
                at, need, have
            ),
            GlbError::UnknownChunkType(t) => {
                write!(f, "unknown GLB chunk type 0x{:08X}", t)
            }
        }
    }
}

impl std::error::Error for GlbError {}

fn read_u32_le(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

/// Describes the chunks in a GLB body without copying the payload.
///
/// The returned offsets point into the original byte slice, so the caller can
/// borrow the JSON and BIN payloads directly instead of allocating copies of
/// what may be tens of megabytes of geometry.
pub fn read_glb_chunks(bytes: &[u8]) -> Result<(GlbHeader, Vec<GlbChunk>), GlbError> {
    if bytes.len() < 12 {
        return Err(GlbError::TooShort);
    }

    let magic = read_u32_le(bytes, 0).ok_or(GlbError::TooShort)?;
    if magic != GLB_MAGIC {
        return Err(GlbError::BadMagic(magic));
    }

    let version = read_u32_le(bytes, 4).ok_or(GlbError::TooShort)?;
    if version != 2 {
        return Err(GlbError::UnsupportedVersion(version));
    }

    let length = read_u32_le(bytes, 8).ok_or(GlbError::TooShort)?;
    if length as usize > bytes.len() {
        return Err(GlbError::LengthMismatch {
            declared: length,
            actual: bytes.len(),
        });
    }

    let mut chunks = Vec::new();
    let mut cursor = 12usize;

    while cursor < bytes.len() {
        let declared = read_u32_le(bytes, cursor).ok_or(GlbError::TruncatedChunk {
            at: cursor,
            need: 4,
            have: bytes.len() - cursor,
        })?;
        let chunk_type = read_u32_le(bytes, cursor + 4).ok_or(GlbError::TruncatedChunk {
            at: cursor + 4,
            need: 4,
            have: bytes.len() - cursor - 4,
        })?;

        if chunk_type != CHUNK_TYPE_JSON && chunk_type != CHUNK_TYPE_BIN {
            return Err(GlbError::UnknownChunkType(chunk_type));
        }

        let payload_offset = cursor + 8;
        let payload_length = declared as usize;
        if payload_offset + payload_length > bytes.len() {
            return Err(GlbError::TruncatedChunk {
                at: payload_offset,
                need: payload_length,
                have: bytes.len() - payload_offset,
            });
        }

        chunks.push(GlbChunk {
            chunk_type,
            offset: payload_offset,
            length: payload_length,
        });

        // Chunks are padded to 4-byte alignment; skip any padding.
        let padded = 8 + pad4(payload_length);
        cursor += padded;
    }

    Ok((GlbHeader { magic, version, length }, chunks))
}

/// Rounds `n` up to the next multiple of four.
pub fn pad4(n: usize) -> usize {
    (n + 3) & !3
}

/// Extracts the JSON payload as a string.
pub fn json_payload<'a>(bytes: &'a [u8], chunks: &[GlbChunk]) -> Result<&'a str, GlbError> {
    for chunk in chunks {
        if chunk.chunk_type == CHUNK_TYPE_JSON {
            let slice = bytes
                .get(chunk.offset..chunk.offset + chunk.length)
                .ok_or(GlbError::TruncatedChunk {
                    at: chunk.offset,
                    need: chunk.length,
                    have: bytes.len().saturating_sub(chunk.offset),
                })?;
            return std::str::from_utf8(slice).map_err(|_| GlbError::UnknownChunkType(CHUNK_TYPE_JSON));
        }
    }
    Err(GlbError::UnknownChunkType(CHUNK_TYPE_JSON))
}

/// Extracts the binary payload.
pub fn bin_payload<'a>(bytes: &'a [u8], chunks: &[GlbChunk]) -> &'a [u8] {
    for chunk in chunks {
        if chunk.chunk_type == CHUNK_TYPE_BIN {
            if let Some(slice) = bytes.get(chunk.offset..chunk.offset + chunk.length) {
                return slice;
            }
        }
    }
    &[]
}
