use std::collections::HashMap;
use std::error::Error;
use std::fmt;
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

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(err) => write!(f, "{err}"),
            StoreError::KeyTooLarge => write!(f, "key too large"),
            StoreError::ValueTooLarge => write!(f, "value too large"),
            StoreError::CorruptLog => write!(f, "corrupt log"),
        }
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            StoreError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for StoreError {
    fn from(err: io::Error) -> Self {
        StoreError::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub trait KvStore {
    fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    fn set(&mut self, key: &[u8], value: &[u8]) -> Result<()>;
    fn del(&mut self, key: &[u8]) -> Result<usize>;
    fn exists(&self, key: &[u8]) -> bool;
}

#[derive(Default)]
pub struct MemoryStore {
    map: HashMap<Vec<u8>, Vec<u8>>,
}

impl KvStore for MemoryStore {
    fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.map.get(key).cloned())
    }

    fn set(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        check_sizes(key, value)?;
        self.map.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn del(&mut self, key: &[u8]) -> Result<usize> {
        Ok(usize::from(self.map.remove(key).is_some()))
    }

    fn exists(&self, key: &[u8]) -> bool {
        self.map.contains_key(key)
    }
}

fn check_sizes(key: &[u8], value: &[u8]) -> Result<()> {
    if key.len() > MAX_KEY_LEN {
        return Err(StoreError::KeyTooLarge);
    }
    if value.len() > MAX_VALUE_LEN {
        return Err(StoreError::ValueTooLarge);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct IndexEntry {
    value_offset: u64,
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
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        let index = replay(&mut log)?;
        Ok(Store { log, index })
    }

    pub fn set(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        check_sizes(key, value)?;
        self.log.seek(SeekFrom::End(0))?;
        write_record(&mut self.log, TAG_SET, key)?;
        self.log.write_all(&(value.len() as u32).to_le_bytes())?;
        let value_offset = self.log.stream_position()?;
        self.log.write_all(value)?;
        self.log.flush()?;
        self.index.insert(
            key.to_vec(),
            IndexEntry {
                value_offset,
                value_len: value.len() as u32,
            },
        );
        Ok(())
    }

    pub fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let Some(entry) = self.index.get(key).copied() else {
            return Ok(None);
        };
        self.log.seek(SeekFrom::Start(entry.value_offset))?;
        let mut buf = vec![0u8; entry.value_len as usize];
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

impl KvStore for Store {
    fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Store::get(self, key)
    }

    fn set(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        Store::set(self, key, value)
    }

    fn del(&mut self, key: &[u8]) -> Result<usize> {
        Store::del(self, key)
    }

    fn exists(&self, key: &[u8]) -> bool {
        Store::exists(self, key)
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

enum Record {
    Set {
        key: Vec<u8>,
        value_offset: u64,
        value_len: u32,
    },
    Delete {
        key: Vec<u8>,
    },
}

fn replay(log: &mut File) -> Result<HashMap<Vec<u8>, IndexEntry>> {
    log.seek(SeekFrom::Start(0))?;
    let mut index = HashMap::new();
    loop {
        let offset = log.stream_position()?;
        match read_record(log) {
            Ok(Record::Set {
                key,
                value_offset,
                value_len,
            }) => {
                index.insert(
                    key,
                    IndexEntry {
                        value_offset,
                        value_len,
                    },
                );
            }
            Ok(Record::Delete { key }) => {
                index.remove(&key);
            }
            Err(StoreError::Io(err)) if err.kind() == io::ErrorKind::UnexpectedEof => {
                log.set_len(offset)?;
                log.seek(SeekFrom::Start(offset))?;
                break;
            }
            Err(err) => return Err(err),
        }
    }
    Ok(index)
}

fn read_record(log: &mut File) -> Result<Record> {
    let mut tag = [0u8; 1];
    log.read_exact(&mut tag)?;
    match tag[0] {
        TAG_SET => {
            let key = read_bytes(log, MAX_KEY_LEN)?;
            let value_len = read_u32(log)?;
            if value_len as usize > MAX_VALUE_LEN {
                return Err(StoreError::CorruptLog);
            }
            let value_offset = log.stream_position()?;
            skip_bytes(log, u64::from(value_len))?;
            Ok(Record::Set {
                key,
                value_offset,
                value_len,
            })
        }
        TAG_DELETE => Ok(Record::Delete {
            key: read_bytes(log, MAX_KEY_LEN)?,
        }),
        _ => Err(StoreError::CorruptLog),
    }
}

fn read_u32(log: &mut File) -> Result<u32> {
    let mut buf = [0u8; 4];
    log.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_bytes(log: &mut File, max: usize) -> Result<Vec<u8>> {
    let len = read_u32(log)? as usize;
    if len > max {
        return Err(StoreError::CorruptLog);
    }
    let mut buf = vec![0u8; len];
    log.read_exact(&mut buf)?;
    Ok(buf)
}

fn skip_bytes(log: &mut File, len: u64) -> Result<()> {
    let pos = log.stream_position()?;
    let file_len = log.metadata()?.len();
    let new_pos = pos.checked_add(len).ok_or(StoreError::CorruptLog)?;
    if new_pos > file_len {
        return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
    }
    log.seek(SeekFrom::Start(new_pos))?;
    Ok(())
}
