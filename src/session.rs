use std::collections::HashMap;

use crate::store::{self, check_sizes, Store, StoreError};

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    Incomplete,
    Invalid(&'static str),
}

pub trait KvStore {
    fn get(&mut self, key: &[u8]) -> store::Result<Option<Vec<u8>>>;
    fn set(&mut self, key: &[u8], value: &[u8]) -> store::Result<()>;
    fn del(&mut self, key: &[u8]) -> store::Result<usize>;
    fn exists(&self, key: &[u8]) -> bool;
}

#[derive(Default)]
pub struct MemoryStore {
    map: HashMap<Vec<u8>, Vec<u8>>,
}

impl KvStore for MemoryStore {
    fn get(&mut self, key: &[u8]) -> store::Result<Option<Vec<u8>>> {
        Ok(self.map.get(key).cloned())
    }

    fn set(&mut self, key: &[u8], value: &[u8]) -> store::Result<()> {
        check_sizes(key, value)?;
        self.map.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn del(&mut self, key: &[u8]) -> store::Result<usize> {
        Ok(usize::from(self.map.remove(key).is_some()))
    }

    fn exists(&self, key: &[u8]) -> bool {
        self.map.contains_key(key)
    }
}

impl KvStore for Store {
    fn get(&mut self, key: &[u8]) -> store::Result<Option<Vec<u8>>> {
        Store::get(self, key)
    }

    fn set(&mut self, key: &[u8], value: &[u8]) -> store::Result<()> {
        Store::set(self, key, value)
    }

    fn del(&mut self, key: &[u8]) -> store::Result<usize> {
        Store::del(self, key)
    }

    fn exists(&self, key: &[u8]) -> bool {
        Store::exists(self, key)
    }
}

/// Parse one RESP2 Array-of-Bulk-Strings command.
/// Returns `(args, bytes_consumed)`.
pub fn parse_command(input: &[u8]) -> Result<(Vec<Vec<u8>>, usize), ParseError> {
    if input.is_empty() {
        return Err(ParseError::Incomplete);
    }
    if input[0] != b'*' {
        return Err(ParseError::Invalid("expected array"));
    }
    let (argc, mut pos) = parse_line_int(input, 1)?;
    if argc < 0 {
        return Err(ParseError::Invalid("negative argc"));
    }
    let mut args = Vec::with_capacity(argc as usize);
    for _ in 0..argc {
        if pos >= input.len() {
            return Err(ParseError::Incomplete);
        }
        if input[pos] != b'$' {
            return Err(ParseError::Invalid("expected bulk string"));
        }
        let (len, next) = parse_line_int(input, pos + 1)?;
        pos = next;
        if len < 0 {
            return Err(ParseError::Invalid("negative bulk length"));
        }
        let len = len as usize;
        if pos + len + 2 > input.len() {
            return Err(ParseError::Incomplete);
        }
        let data = input[pos..pos + len].to_vec();
        pos += len;
        if &input[pos..pos + 2] != b"\r\n" {
            return Err(ParseError::Invalid("bulk missing CRLF"));
        }
        pos += 2;
        args.push(data);
    }
    Ok((args, pos))
}

fn parse_line_int(input: &[u8], start: usize) -> Result<(i64, usize), ParseError> {
    let mut end = start;
    while end < input.len() && input[end] != b'\r' {
        end += 1;
    }
    if end + 1 >= input.len() || input[end] != b'\r' || input[end + 1] != b'\n' {
        return Err(ParseError::Incomplete);
    }
    let text = std::str::from_utf8(&input[start..end])
        .map_err(|_| ParseError::Invalid("non-utf8 integer"))?;
    let value = text
        .parse::<i64>()
        .map_err(|_| ParseError::Invalid("bad integer"))?;
    Ok((value, end + 2))
}

pub fn dispatch(store: &mut impl KvStore, args: &[Vec<u8>]) -> store::Result<Vec<u8>> {
    let Some(cmd) = args.first() else {
        return Ok(error("wrong number of arguments"));
    };
    let cmd = cmd.to_ascii_uppercase();
    match cmd.as_slice() {
        b"PING" => Ok(simple("PONG")),
        b"GET" => {
            if args.len() != 2 {
                return Ok(error("wrong number of arguments for 'GET'"));
            }
            match store.get(&args[1])? {
                Some(value) => Ok(bulk(&value)),
                None => Ok(null_bulk()),
            }
        }
        b"SET" => {
            if args.len() != 3 {
                return Ok(error("wrong number of arguments for 'SET'"));
            }
            match store.set(&args[1], &args[2]) {
                Ok(()) => Ok(simple("OK")),
                Err(StoreError::KeyTooLarge) => Ok(error("key too large")),
                Err(StoreError::ValueTooLarge) => Ok(error("value too large")),
                Err(err) => Err(err),
            }
        }
        b"DEL" => {
            if args.len() != 2 {
                return Ok(error("wrong number of arguments for 'DEL'"));
            }
            let n = store.del(&args[1])?;
            Ok(integer(n as i64))
        }
        b"EXISTS" => {
            if args.len() != 2 {
                return Ok(error("wrong number of arguments for 'EXISTS'"));
            }
            Ok(integer(i64::from(store.exists(&args[1]))))
        }
        _ => Ok(error("unknown command")),
    }
}

fn simple(s: &str) -> Vec<u8> {
    format!("+{s}\r\n").into_bytes()
}

fn error(s: &str) -> Vec<u8> {
    format!("-ERR {s}\r\n").into_bytes()
}

fn integer(n: i64) -> Vec<u8> {
    format!(":{n}\r\n").into_bytes()
}

fn bulk(data: &[u8]) -> Vec<u8> {
    let mut out = format!("${}\r\n", data.len()).into_bytes();
    out.extend_from_slice(data);
    out.extend_from_slice(b"\r\n");
    out
}

fn null_bulk() -> Vec<u8> {
    b"$-1\r\n".to_vec()
}
