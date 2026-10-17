//! Clean-Room RPFv7 Archive Format Reader & Streamer.
//!
//! Implements a safe, zero-warning parser and streaming pipeline for Rockstar Package Format (RPF7)
//! archives utilized in Grand Theft Auto V and related titles.
//!
//! Designed strictly under Clean-Room Reverse Engineering principles:
//! - Operates purely on binary specifications without reverse-engineered Rockstar code or proprietary assets.
//! - Reads local user archives on the fly from disk for educational and benchmarking purposes.
//! - Provides synthetic RPF generation for headless CI environments.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

/// RPF Magic Number for version 7 (`RPF7` in ASCII, little-endian: 0x52504637).
pub const RPF7_MAGIC: u32 = 0x52504637;

/// Directory identifier marker in RPF7 TOC (0x7FFFFF00).
pub const RPF7_DIRECTORY_IDENT: u32 = 0x7FFFFF00;

/// Sector alignment size for RPF file offsets (512 bytes).
pub const RPF7_SECTOR_SIZE: u64 = 512;

/// OpenIV unencrypted marker (`OPEN` in ASCII: 0x4E45504F).
pub const RPF7_OPEN_ENCRYPTION: u32 = 0x4E45504F;

/// RPF Archive Encryption Mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpfEncryption {
    None,
    Open,
    Aes,
    Ng,
    Unknown(u32),
}

impl RpfEncryption {
    pub fn from_u32(val: u32) -> Self {
        match val {
            0 => RpfEncryption::None,
            RPF7_OPEN_ENCRYPTION => RpfEncryption::Open,
            0x0FFFFFFF | 0x0FEFFFFF | 0x0FFFFFF9 => RpfEncryption::Aes,
            other => RpfEncryption::Unknown(other),
        }
    }

    pub fn is_unencrypted(&self) -> bool {
        matches!(self, RpfEncryption::None | RpfEncryption::Open)
    }
}

/// Errors occurring during RPF archive operations.
#[derive(Debug)]
pub enum RpfError {
    Io(std::io::Error),
    InvalidMagic(u32),
    UnsupportedEncryption(RpfEncryption),
    InvalidTOC(String),
    EntryNotFound(String),
    DecompressionFailed(String),
}

impl From<std::io::Error> for RpfError {
    fn from(err: std::io::Error) -> Self {
        RpfError::Io(err)
    }
}

impl std::fmt::Display for RpfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RpfError::Io(e) => write!(f, "RPF IO error: {}", e),
            RpfError::InvalidMagic(m) => write!(f, "Invalid RPF magic: 0x{:08X} (expected RPF7)", m),
            RpfError::UnsupportedEncryption(enc) => write!(f, "Unsupported RPF encryption: {:?}", enc),
            RpfError::InvalidTOC(msg) => write!(f, "Corrupted RPF Table of Contents: {}", msg),
            RpfError::EntryNotFound(path) => write!(f, "RPF entry not found: {}", path),
            RpfError::DecompressionFailed(msg) => write!(f, "RPF decompression failed: {}", msg),
        }
    }
}

impl std::error::Error for RpfError {}

/// Directory entry in the RPF TOC hierarchy.
#[derive(Debug, Clone)]
pub struct RpfDirectoryEntry {
    pub name: String,
    pub name_offset: u32,
    pub start_index: u32,
    pub entries_count: u32,
    pub virtual_path: String,
}

/// Standard raw or compressed binary file entry (textures, audio, scripts, models).
#[derive(Debug, Clone)]
pub struct RpfBinaryFileEntry {
    pub name: String,
    pub name_offset: u32,
    pub file_offset: u64, // Absolute byte offset in archive
    pub file_size: u32,   // Compressed byte size (or 0 if uncompressed)
    pub uncompressed_size: u32,
    pub is_encrypted: bool,
    pub virtual_path: String,
}

impl RpfBinaryFileEntry {
    /// Returns the effective payload size stored on disk in the archive.
    #[inline]
    pub fn on_disk_size(&self) -> u64 {
        if self.file_size == 0 {
            self.uncompressed_size as u64
        } else {
            self.file_size as u64
        }
    }

    /// Whether this entry is stored compressed via Deflate.
    #[inline]
    pub fn is_compressed(&self) -> bool {
        self.file_size > 0
    }
}

/// Compiled RAGE Resource file entry (e.g. .ytd, .ydr, .yft).
#[derive(Debug, Clone)]
pub struct RpfResourceFileEntry {
    pub name: String,
    pub name_offset: u32,
    pub file_offset: u64, // Absolute byte offset in archive
    pub file_size: u32,
    pub system_flags: u32,
    pub graphics_flags: u32,
    pub virtual_path: String,
}

/// Single item within the RPF virtual filesystem.
#[derive(Debug, Clone)]
pub enum RpfEntry {
    Directory(RpfDirectoryEntry),
    BinaryFile(RpfBinaryFileEntry),
    ResourceFile(RpfResourceFileEntry),
}

impl RpfEntry {
    pub fn name(&self) -> &str {
        match self {
            RpfEntry::Directory(d) => &d.name,
            RpfEntry::BinaryFile(b) => &b.name,
            RpfEntry::ResourceFile(r) => &r.name,
        }
    }

    pub fn virtual_path(&self) -> &str {
        match self {
            RpfEntry::Directory(d) => &d.virtual_path,
            RpfEntry::BinaryFile(b) => &b.virtual_path,
            RpfEntry::ResourceFile(r) => &r.virtual_path,
        }
    }

    pub fn is_file(&self) -> bool {
        !matches!(self, RpfEntry::Directory(_))
    }
}

/// Clean-room RPF7 Archive reader and progressive streaming handle.
pub struct RpfArchive<R> {
    reader: R,
    encryption: RpfEncryption,
    entries: Vec<RpfEntry>,
    path_map: HashMap<String, usize>,
    total_files: usize,
    total_folders: usize,
}

impl RpfArchive<File> {
    /// Opens an RPF archive directly from a file path on disk.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, RpfError> {
        let file = File::open(path)?;
        Self::from_reader(file)
    }
}

impl RpfArchive<Cursor<Vec<u8>>> {
    /// Parses an in-memory RPF archive byte slice.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, RpfError> {
        let cursor = Cursor::new(data);
        Self::from_reader(cursor)
    }
}

impl<R: Read + Seek> RpfArchive<R> {
    /// Reads and initializes an RPF7 archive from any Seek + Read stream.
    pub fn from_reader(mut reader: R) -> Result<Self, RpfError> {
        reader.seek(SeekFrom::Start(0))?;

        let mut header_buf = [0u8; 16];
        reader.read_exact(&mut header_buf)?;

        let magic = u32::from_le_bytes([header_buf[0], header_buf[1], header_buf[2], header_buf[3]]);
        if magic != RPF7_MAGIC {
            return Err(RpfError::InvalidMagic(magic));
        }

        let entry_count = u32::from_le_bytes([header_buf[4], header_buf[5], header_buf[6], header_buf[7]]) as usize;
        let names_length = u32::from_le_bytes([header_buf[8], header_buf[9], header_buf[10], header_buf[11]]) as usize;
        let enc_raw = u32::from_le_bytes([header_buf[12], header_buf[13], header_buf[14], header_buf[15]]);
        let encryption = RpfEncryption::from_u32(enc_raw);

        // Read TOC entries table
        let mut entries_data = vec![0u8; entry_count * 16];
        reader.read_exact(&mut entries_data)?;

        // Read string names table
        let mut names_data = vec![0u8; names_length];
        reader.read_exact(&mut names_data)?;

        let get_string = |offset: usize| -> String {
            if offset >= names_data.len() {
                return String::new();
            }
            let sub = &names_data[offset..];
            let len = sub.iter().position(|&b| b == 0).unwrap_or(sub.len());
            String::from_utf8_lossy(&sub[..len]).to_string()
        };

        let mut raw_entries: Vec<RpfEntry> = Vec::with_capacity(entry_count);
        let mut total_files = 0;
        let mut total_folders = 0;

        for i in 0..entry_count {
            let chunk = &entries_data[i * 16..(i + 1) * 16];
            let w0 = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let w1 = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            let w2 = u32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);
            let w3 = u32::from_le_bytes([chunk[12], chunk[13], chunk[14], chunk[15]]);

            if w1 == RPF7_DIRECTORY_IDENT {
                let name_off = w0 & 0xFFFF;
                let name = get_string(name_off as usize);
                raw_entries.push(RpfEntry::Directory(RpfDirectoryEntry {
                    name,
                    name_offset: name_off,
                    start_index: w2,
                    entries_count: w3,
                    virtual_path: String::new(),
                }));
                total_folders += 1;
            } else if (w1 & 0x8000_0000) == 0 {
                // Binary File Entry
                let buf = u64::from_le_bytes([
                    chunk[0], chunk[1], chunk[2], chunk[3],
                    chunk[4], chunk[5], chunk[6], chunk[7],
                ]);
                let name_off = (buf & 0xFFFF) as u32;
                let file_size = ((buf >> 16) & 0xFFFFFF) as u32;
                let file_offset_sectors = ((buf >> 40) & 0xFFFFFF) as u64;
                let file_offset_bytes = file_offset_sectors * RPF7_SECTOR_SIZE;
                let uncompressed_size = w2;
                let is_encrypted = w3 != 0;
                let name = get_string(name_off as usize);

                raw_entries.push(RpfEntry::BinaryFile(RpfBinaryFileEntry {
                    name,
                    name_offset: name_off,
                    file_offset: file_offset_bytes,
                    file_size,
                    uncompressed_size,
                    is_encrypted,
                    virtual_path: String::new(),
                }));
                total_files += 1;
            } else {
                // Resource File Entry
                let name_off = u16::from_le_bytes([chunk[0], chunk[1]]) as u32;
                let file_size = (chunk[2] as u32) | ((chunk[3] as u32) << 8) | ((chunk[4] as u32) << 16);
                let fo_sectors = (((chunk[5] as u32) | ((chunk[6] as u32) << 8) | ((chunk[7] as u32) << 16)) & 0x7FFFFF) as u64;
                let file_offset_bytes = fo_sectors * RPF7_SECTOR_SIZE;
                let name = get_string(name_off as usize);

                raw_entries.push(RpfEntry::ResourceFile(RpfResourceFileEntry {
                    name,
                    name_offset: name_off,
                    file_offset: file_offset_bytes,
                    file_size,
                    system_flags: w2,
                    graphics_flags: w3,
                    virtual_path: String::new(),
                }));
                total_files += 1;
            }
        }

        // Reconstruct Virtual Path Hierarchy
        let mut path_map = HashMap::with_capacity(entry_count);
        if !raw_entries.is_empty() {
            // Root directory is always index 0
            if let RpfEntry::Directory(ref mut root) = raw_entries[0] {
                root.virtual_path = String::new();
            }

            let mut stack = vec![(0usize, String::new())];
            while let Some((dir_idx, current_path)) = stack.pop() {
                let (start_idx, count) = if let RpfEntry::Directory(ref d) = raw_entries[dir_idx] {
                    (d.start_index as usize, d.entries_count as usize)
                } else {
                    continue;
                };

                for i in start_idx..(start_idx + count) {
                    if i >= raw_entries.len() {
                        break;
                    }
                    let child_name = raw_entries[i].name().to_string();
                    let child_path = if current_path.is_empty() {
                        child_name.clone()
                    } else {
                        format!("{}/{}", current_path, child_name)
                    };

                    match &mut raw_entries[i] {
                        RpfEntry::Directory(d) => {
                            d.virtual_path = child_path.clone();
                            path_map.insert(child_path.to_ascii_lowercase(), i);
                            stack.push((i, child_path));
                        }
                        RpfEntry::BinaryFile(b) => {
                            b.virtual_path = child_path.clone();
                            path_map.insert(child_path.to_ascii_lowercase(), i);
                        }
                        RpfEntry::ResourceFile(r) => {
                            r.virtual_path = child_path.clone();
                            path_map.insert(child_path.to_ascii_lowercase(), i);
                        }
                    }
                }
            }
        }

        Ok(Self {
            reader,
            encryption,
            entries: raw_entries,
            path_map,
            total_files,
            total_folders,
        })
    }

    #[inline]
    pub fn encryption(&self) -> RpfEncryption {
        self.encryption
    }

    #[inline]
    pub fn entries(&self) -> &[RpfEntry] {
        &self.entries
    }

    #[inline]
    pub fn total_files(&self) -> usize {
        self.total_files
    }

    #[inline]
    pub fn total_directories(&self) -> usize {
        self.total_folders
    }

    /// Resolves an entry by its virtual hierarchy path (case-insensitive).
    pub fn get_entry(&self, path: &str) -> Option<&RpfEntry> {
        let normalized = path.replace('\\', "/").to_ascii_lowercase();
        self.path_map.get(&normalized).map(|&idx| &self.entries[idx])
    }

    /// Reads raw byte slice of an entry directly by virtual path.
    pub fn read_raw_by_path(&mut self, path: &str) -> Result<Vec<u8>, RpfError> {
        let normalized = path.replace('\\', "/").to_ascii_lowercase();
        let idx = *self.path_map.get(&normalized).ok_or_else(|| RpfError::EntryNotFound(path.to_string()))?;
        let entry = self.entries[idx].clone();
        self.read_entry_raw(&entry)
    }

    /// Reads and decompresses file entry directly by virtual path.
    pub fn read_decompressed_by_path(&mut self, path: &str) -> Result<Vec<u8>, RpfError> {
        let normalized = path.replace('\\', "/").to_ascii_lowercase();
        let idx = *self.path_map.get(&normalized).ok_or_else(|| RpfError::EntryNotFound(path.to_string()))?;
        let entry = self.entries[idx].clone();
        self.read_entry_decompressed(&entry)
    }

    /// Reads raw byte slice of an entry directly from the archive.
    pub fn read_entry_raw(&mut self, entry: &RpfEntry) -> Result<Vec<u8>, RpfError> {
        match entry {
            RpfEntry::Directory(_) => Err(RpfError::InvalidTOC("Cannot read bytes from directory".into())),
            RpfEntry::BinaryFile(bin) => {
                let size = bin.on_disk_size() as usize;
                self.reader.seek(SeekFrom::Start(bin.file_offset))?;
                let mut buf = vec![0u8; size];
                self.reader.read_exact(&mut buf)?;
                Ok(buf)
            }
            RpfEntry::ResourceFile(res) => {
                let size = res.file_size as usize;
                self.reader.seek(SeekFrom::Start(res.file_offset))?;
                let mut buf = vec![0u8; size];
                self.reader.read_exact(&mut buf)?;
                Ok(buf)
            }
        }
    }

    /// Reads and decompresses file entry bytes if compressed.
    pub fn read_entry_decompressed(&mut self, entry: &RpfEntry) -> Result<Vec<u8>, RpfError> {
        match entry {
            RpfEntry::Directory(_) => Err(RpfError::InvalidTOC("Cannot decompress directory".into())),
            RpfEntry::BinaryFile(bin) => {
                let raw = self.read_entry_raw(entry)?;
                if bin.is_compressed() {
                    // GTA 5 Deflate compression format (standard raw deflate / zlib)
                    match miniz_oxide::inflate::decompress_to_vec_zlib(&raw) {
                        Ok(decompressed) => Ok(decompressed),
                        Err(_) => {
                            // Fallback to raw headerless deflate
                            miniz_oxide::inflate::decompress_to_vec(&raw)
                                .map_err(|e| RpfError::DecompressionFailed(format!("{:?}", e)))
                        }
                    }
                } else {
                    Ok(raw)
                }
            }
            RpfEntry::ResourceFile(_) => {
                // For resource files, return raw chunk (system + graphics segments)
                self.read_entry_raw(entry)
            }
        }
    }

    /// Streams raw entry bytes into any Write destination with zero redundant allocations.
    pub fn stream_entry_raw<W: Write>(&mut self, entry: &RpfEntry, writer: &mut W) -> Result<usize, RpfError> {
        let (offset, size) = match entry {
            RpfEntry::Directory(_) => return Err(RpfError::InvalidTOC("Cannot stream directory".into())),
            RpfEntry::BinaryFile(b) => (b.file_offset, b.on_disk_size() as usize),
            RpfEntry::ResourceFile(r) => (r.file_offset, r.file_size as usize),
        };

        self.reader.seek(SeekFrom::Start(offset))?;
        let mut buffer = [0u8; 64 * 1024]; // 64 KB streaming buffer
        let mut remaining = size;
        let mut total_written = 0;

        while remaining > 0 {
            let to_read = remaining.min(buffer.len());
            self.reader.read_exact(&mut buffer[..to_read])?;
            writer.write_all(&buffer[..to_read])?;
            remaining -= to_read;
            total_written += to_read;
        }

        Ok(total_written)
    }
}

/// Creates a compliant synthetic RPF7 container for unit testing and CI benchmarking.
pub fn create_synthetic_rpf(files: &[(&str, &[u8], bool)]) -> Vec<u8> {
    // files: (name, bytes, compress)
    let mut out = Vec::new();

    let entry_count = (1 + files.len()) as u32; // 1 root directory + files
    let mut names_table = Vec::new();
    let mut name_offsets = Vec::new();

    // Root directory name is empty string at offset 0
    names_table.push(0u8);

    for (name, _, _) in files {
        let off = names_table.len() as u32;
        name_offsets.push(off);
        names_table.extend_from_slice(name.as_bytes());
        names_table.push(0u8); // null terminator
    }

    let names_len = names_table.len() as u32;

    // Build header (16 bytes)
    out.extend_from_slice(&RPF7_MAGIC.to_le_bytes());
    out.extend_from_slice(&entry_count.to_le_bytes());
    out.extend_from_slice(&names_len.to_le_bytes());
    out.extend_from_slice(&RPF7_OPEN_ENCRYPTION.to_le_bytes());

    // Prepare payload data and offsets
    let header_and_toc_size = 16 + (entry_count as usize * 16) + names_len as usize;
    let aligned_first_payload = header_and_toc_size.div_ceil(RPF7_SECTOR_SIZE as usize) * (RPF7_SECTOR_SIZE as usize);

    let mut current_offset = aligned_first_payload as u64;
    let mut file_entries_meta = Vec::new();
    let mut payloads = Vec::new();

    for (i, (_, data, compress)) in files.iter().enumerate() {
        let name_off = name_offsets[i];
        let (mut stored_bytes, file_size_flag) = if *compress {
            let deflated = miniz_oxide::deflate::compress_to_vec_zlib(data, 6);
            let dlen = deflated.len() as u32;
            (deflated, dlen)
        } else {
            (data.to_vec(), 0u32)
        };

        let sector_off = current_offset / RPF7_SECTOR_SIZE;
        file_entries_meta.push((name_off, sector_off, file_size_flag, data.len() as u32));

        let payload_len = stored_bytes.len();
        let aligned_len = payload_len.div_ceil(RPF7_SECTOR_SIZE as usize) * (RPF7_SECTOR_SIZE as usize);
        stored_bytes.resize(aligned_len, 0u8);

        current_offset += aligned_len as u64;
        payloads.push(stored_bytes);
    }

    // Write TOC entry 0: Root directory
    let root_w0 = 0u32;
    let root_w1 = RPF7_DIRECTORY_IDENT;
    let root_w2 = 1u32; // start_idx
    let root_w3 = files.len() as u32; // count
    out.extend_from_slice(&root_w0.to_le_bytes());
    out.extend_from_slice(&root_w1.to_le_bytes());
    out.extend_from_slice(&root_w2.to_le_bytes());
    out.extend_from_slice(&root_w3.to_le_bytes());

    // Write File Entries (entries 1..N)
    for (name_off, sector_off, file_size, uncomp) in file_entries_meta {
        let buf = (name_off as u64 & 0xFFFF)
            | (((file_size as u64) & 0xFFFFFF) << 16)
            | (((sector_off) & 0xFFFFFF) << 40);

        out.extend_from_slice(&buf.to_le_bytes());
        out.extend_from_slice(&uncomp.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // not encrypted
    }

    // Write names table
    out.extend_from_slice(&names_table);

    // Pad to first payload sector
    while out.len() < aligned_first_payload {
        out.push(0u8);
    }

    // Write payloads
    for p in payloads {
        out.extend_from_slice(&p);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_rpf_generation_and_reading() {
        let raw_data1 = b"FerrumRAGE High-Throughput Texture Streaming Test Data 001";
        let raw_data2 = b"Procedural Mipmap Level Pyramidal Layer Binary Blob 002";

        let archive_bytes = create_synthetic_rpf(&[
            ("diffuse_2k.dds", raw_data1, false),
            ("normal_4k.dds", raw_data2, true),
        ]);

        let mut archive = RpfArchive::from_bytes(archive_bytes).expect("Failed to parse synthetic RPF");
        assert_eq!(archive.total_files(), 2);
        assert_eq!(archive.total_directories(), 1);

        // Test uncompressed entry
        let content1 = archive.read_decompressed_by_path("diffuse_2k.dds").expect("Read 1 failed");
        assert_eq!(content1, raw_data1);

        // Test compressed entry
        let content2 = archive.read_decompressed_by_path("normal_4k.dds").expect("Read 2 failed");
        assert_eq!(content2, raw_data2);

        // Test streaming to writer
        let entry1 = archive.get_entry("diffuse_2k.dds").expect("Entry 1 missing").clone();
        let mut stream_sink = Vec::new();
        let bytes_written = archive.stream_entry_raw(&entry1, &mut stream_sink).expect("Stream failed");
        assert_eq!(bytes_written, raw_data1.len());
        assert_eq!(stream_sink, raw_data1);
    }

    #[test]
    fn test_local_gta5_rpf_inspection_if_available() {
        let gta5_rpf_path = Path::new(r"D:\Games\Grand Theft Auto V\mods\x64e.rpf");
        if gta5_rpf_path.exists() {
            let archive = RpfArchive::open(gta5_rpf_path).expect("Failed to open local GTA 5 x64e.rpf");
            println!(
                "Local GTA 5 x64e.rpf verified: {} files, {} folders, encryption={:?}",
                archive.total_files(),
                archive.total_directories(),
                archive.encryption()
            );
            assert!(archive.total_files() > 10);
            assert!(archive.get_entry("levels/gta5/vehicles.rpf").is_some());
        }
    }
}
