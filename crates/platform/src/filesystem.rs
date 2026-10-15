use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileMode {
    Read,
    Write,
    Append,
    ReadWrite,
    ReadAppend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SeekOrigin {
    Start,
    Current,
    End,
}

pub struct FileHandle {
    file: File,
    path: PathBuf,
}

impl FileHandle {
    pub fn open(path: impl AsRef<Path>, mode: FileMode) -> io::Result<Self> {
        let file = match mode {
            FileMode::Read => File::open(&path)?,
            FileMode::Write => File::create(&path)?,
            FileMode::Append => OpenOptions::new().append(true).create(true).open(&path)?,
            FileMode::ReadWrite => OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&path)?,
            FileMode::ReadAppend => OpenOptions::new().read(true).append(true).create(true).open(&path)?,
        };
        Ok(Self {
            file,
            path: path.as_ref().to_path_buf(),
        })
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }

    pub fn read_to_end(&mut self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.file.read_to_end(&mut buf)?;
        Ok(buf)
    }

    pub fn read_to_string(&mut self) -> io::Result<String> {
        let mut s = String::new();
        self.file.read_to_string(&mut s)?;
        Ok(s)
    }

    pub fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf)
    }

    pub fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.file.write_all(buf)
    }

    pub fn seek(&mut self, pos: SeekOrigin, offset: i64) -> io::Result<u64> {
        let seek_from = match pos {
            SeekOrigin::Start => SeekFrom::Start(offset as u64),
            SeekOrigin::Current => SeekFrom::Current(offset),
            SeekOrigin::End => SeekFrom::End(offset),
        };
        self.file.seek(seek_from)
    }

    pub fn position(&mut self) -> io::Result<u64> {
        self.file.stream_position()
    }

    pub fn size(&self) -> io::Result<u64> {
        Ok(self.file.metadata()?.len())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }

    pub fn close(self) -> io::Result<()> {
        drop(self.file);
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub struct FileSystem;

impl FileSystem {
    pub fn exists(path: impl AsRef<Path>) -> bool {
        path.as_ref().exists()
    }

    pub fn is_file(path: impl AsRef<Path>) -> bool {
        path.as_ref().is_file()
    }

    pub fn is_dir(path: impl AsRef<Path>) -> bool {
        path.as_ref().is_dir()
    }

    pub fn file_size(path: impl AsRef<Path>) -> io::Result<u64> {
        Ok(std::fs::metadata(path)?.len())
    }

    pub fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::create_dir(path)
    }

    pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    pub fn remove_dir(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::remove_dir(path)
    }

    pub fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    pub fn read_to_bytes(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    pub fn write_string(path: impl AsRef<Path>, contents: impl AsRef<str>) -> io::Result<()> {
        std::fs::write(path, contents.as_ref())
    }

    pub fn write_bytes(path: impl AsRef<Path>, contents: &[u8]) -> io::Result<()> {
        std::fs::write(path, contents)
    }

    pub fn list_dir(path: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            entries.push(entry.path());
        }
        Ok(entries)
    }

    pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
        std::fs::copy(from, to)
    }

    pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
        std::fs::rename(from, to)
    }
}
