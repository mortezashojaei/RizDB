use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

const LOG_NAME: &str = "rizdb.log";
pub const MAX_KEY_LEN: usize = 1024;
pub const MAX_VALUE_LEN: usize = 16 * 1024 * 1024;
const TAG_SET: u8 = 1;
const TAG_DELETE: u8 = 2;

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    KeyTooLarge,
    ValueTooLarge,
    CorruptLog,
}

impl From<io::Error> for StoreError {
    fn from(err: io::Error) -> Self {
        StoreError::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub fn check_sizes(key: &[u8], value: &[u8]) -> Result<()> {
    if key.len() > MAX_KEY_LEN {
        return Err(StoreError::KeyTooLarge);
    }
    if value.len() > MAX_VALUE_LEN {
        return Err(StoreError::ValueTooLarge);
    }
    Ok(())
}

struct IndexEntry {
    offset: u64,
    value_len: u32,
}

pub struct Store {
    log: File,
    index: HashMap<Vec<u8>, IndexEntry>,
}

impl Store {
    pub fn open(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join(LOG_NAME);
        let mut log = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path)?;
        let index = replay(&mut log)?;
        Ok(Store { log, index })
    }

    pub fn set(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        check_sizes(key, value)?;
        let offset = self.log.seek(SeekFrom::End(0))?;
        write_record(&mut self.log, TAG_SET, key)?;
        write_bytes(&mut self.log, value)?;
        self.log.flush()?;
        self.index.insert(
            key.to_vec(),
            IndexEntry {
                offset,
                value_len: value.len() as u32,
            },
        );
        Ok(())
    }

    pub fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let Some(entry) = self.index.get(key) else {
            return Ok(None);
        };
        let value_len = entry.value_len as usize;
        let value_offset = entry.offset + 1 + 4 + key.len() as u64 + 4;
        self.log.seek(SeekFrom::Start(value_offset))?;
        let mut buf = vec![0u8; value_len];
        self.log.read_exact(&mut buf)?;
        Ok(Some(buf))
    }

    pub fn del(&mut self, key: &[u8]) -> Result<usize> {
        if !self.index.contains_key(key) {
            return Ok(0);
        }
        self.log.seek(SeekFrom::End(0))?;
        write_record(&mut self.log, TAG_DELETE, key)?;
        self.log.flush()?;
        self.index.remove(key);
        Ok(1)
    }

    pub fn exists(&self, key: &[u8]) -> bool {
        self.index.contains_key(key)
    }

    pub fn sync(&mut self) -> Result<()> {
        self.log.sync_data()?;
        Ok(())
    }
}

fn write_record(log: &mut File, tag: u8, key: &[u8]) -> Result<()> {
    log.write_all(&[tag])?;
    write_bytes(log, key)
}

fn write_bytes(log: &mut File, data: &[u8]) -> Result<()> {
    log.write_all(&(data.len() as u32).to_le_bytes())?;
    log.write_all(data)?;
    Ok(())
}

fn replay(log: &mut File) -> Result<HashMap<Vec<u8>, IndexEntry>> {
    log.seek(SeekFrom::Start(0))?;
    let mut index = HashMap::new();
    loop {
        let offset = log.stream_position()?;
        let mut tag = [0u8; 1];
        match log.read_exact(&mut tag) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err.into()),
        }
        match tag[0] {
            TAG_SET => {
                let key = read_bytes(log)?;
                let value = read_bytes(log)?;
                index.insert(
                    key,
                    IndexEntry {
                        offset,
                        value_len: value.len() as u32,
                    },
                );
            }
            TAG_DELETE => {
                let key = read_bytes(log)?;
                index.remove(&key);
            }
            _ => return Err(StoreError::CorruptLog),
        }
    }
    Ok(index)
}

fn read_bytes(log: &mut File) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    log.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    log.read_exact(&mut buf)?;
    Ok(buf)
}
