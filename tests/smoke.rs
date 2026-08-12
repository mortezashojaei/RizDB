mod common;

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use common::temp_data_dir;

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn wait_for_port(port: u16) {
    for _ in 0..50 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("server did not start on port {port}");
}

fn resp_array(args: &[&[u8]]) -> Vec<u8> {
    let mut out = format!("*{}\r\n", args.len()).into_bytes();
    for arg in args {
        out.extend_from_slice(format!("${}\r\n", arg.len()).as_bytes());
        out.extend_from_slice(arg);
        out.extend_from_slice(b"\r\n");
    }
    out
}

fn roundtrip(port: u16, request: &[u8]) -> Vec<u8> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(request).unwrap();
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).unwrap();
    buf[..n].to_vec()
}

fn start_server(port: u16, data_dir: &str) -> Child {
    let bin = env!("CARGO_BIN_EXE_rizdb");
    Command::new(bin)
        .args([
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--data-dir",
            data_dir,
            "--fsync-ms",
            "50",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

#[test]
fn smoke_set_get_survives_restart() {
    let dir = temp_data_dir("rizdb-smoke");
    let port = free_port();
    let dir_str = dir.to_str().unwrap();

    let mut child = start_server(port, dir_str);
    wait_for_port(port);

    let set = roundtrip(port, &resp_array(&[b"SET", b"greeting", b"hello"]));
    assert_eq!(set, b"+OK\r\n");

    let get = roundtrip(port, &resp_array(&[b"GET", b"greeting"]));
    assert_eq!(get, b"$5\r\nhello\r\n");

    thread::sleep(Duration::from_millis(120));
    let _ = child.kill();
    let _ = child.wait();

    let mut child = start_server(port, dir_str);
    wait_for_port(port);
    let get = roundtrip(port, &resp_array(&[b"GET", b"greeting"]));
    assert_eq!(get, b"$5\r\nhello\r\n");

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn concurrent_clients_see_consistent_values() {
    let dir = temp_data_dir("rizdb-smoke");
    let port = free_port();
    let mut child = start_server(port, dir.to_str().unwrap());
    wait_for_port(port);

    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();
    for i in 0..2 {
        let barrier = Arc::clone(&barrier);
        let key = format!("k{i}");
        let value = format!("v{i}");
        handles.push(thread::spawn(move || {
            barrier.wait();
            let set = roundtrip(
                port,
                &resp_array(&[b"SET", key.as_bytes(), value.as_bytes()]),
            );
            assert_eq!(set, b"+OK\r\n");
            let get = roundtrip(port, &resp_array(&[b"GET", key.as_bytes()]));
            let expected = format!("${}\r\n{}\r\n", value.len(), value);
            assert_eq!(get, expected.as_bytes());
        }));
    }

    barrier.wait();
    for handle in handles {
        handle.join().unwrap();
    }

    // Same key contended: last writer wins, no protocol corruption.
    let barrier = Arc::new(Barrier::new(9));
    let mut handles = Vec::new();
    for i in 0..8 {
        let barrier = Arc::clone(&barrier);
        let value = format!("w{i}");
        handles.push(thread::spawn(move || {
            barrier.wait();
            let set = roundtrip(port, &resp_array(&[b"SET", b"race", value.as_bytes()]));
            assert_eq!(set, b"+OK\r\n");
        }));
    }
    barrier.wait();
    for handle in handles {
        handle.join().unwrap();
    }
    let get = roundtrip(port, &resp_array(&[b"GET", b"race"]));
    assert!(get.starts_with(b"$2\r\nw"), "{get:?}");
    assert!(get.ends_with(b"\r\n"));

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(&dir);
}
