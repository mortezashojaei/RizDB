use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::session::{self, ParseError};
use crate::store::{self, Store};

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
    pub fsync_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 7379,
            data_dir: PathBuf::from("./data"),
            fsync_ms: 1000,
        }
    }
}

pub fn serve(config: Config) -> store::Result<()> {
    let store = Store::open(&config.data_dir)?;
    let store = Arc::new(Mutex::new(store));

    let fsync_store = Arc::clone(&store);
    let interval = Duration::from_millis(config.fsync_ms);
    thread::spawn(move || loop {
        thread::sleep(interval);
        if let Ok(mut guard) = fsync_store.lock() {
            let _ = guard.sync();
        }
    });

    let listener = TcpListener::bind((config.host.as_str(), config.port))?;
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };
        let store = Arc::clone(&store);
        thread::spawn(move || {
            let _ = handle_client(stream, store);
        });
    }
    Ok(())
}

fn handle_client(mut stream: TcpStream, store: Arc<Mutex<Store>>) -> store::Result<()> {
    let _ = stream.set_nodelay(true);
    let mut buf = Vec::new();
    let mut read_buf = [0u8; 4096];
    loop {
        let n = match stream.read(&mut read_buf) {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            Err(_) => return Ok(()),
        };
        buf.extend_from_slice(&read_buf[..n]);
        loop {
            match session::parse_command(&buf) {
                Ok((args, consumed)) => {
                    buf.drain(..consumed);
                    let reply = {
                        let mut guard = store.lock().expect("store lock");
                        session::dispatch(&mut *guard, &args)?
                    };
                    stream.write_all(&reply)?;
                }
                Err(ParseError::Incomplete) => break,
                Err(ParseError::Invalid(_)) => {
                    let _ = stream.write_all(b"-ERR protocol error\r\n");
                    return Ok(());
                }
            }
        }
    }
}
