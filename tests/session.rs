use rizdb::session::{dispatch, MemoryStore, parse_command};
use rizdb::store::{MAX_KEY_LEN, MAX_VALUE_LEN};

#[test]
fn ping_returns_pong() {
    let mut store = MemoryStore::default();
    let reply = dispatch(&mut store, &[b"PING".to_vec()]).unwrap();
    assert_eq!(reply, b"+PONG\r\n");
}

#[test]
fn parse_ping_array() {
    let input = b"*1\r\n$4\r\nPING\r\n";
    let (args, consumed) = parse_command(input).unwrap();
    assert_eq!(consumed, input.len());
    assert_eq!(args, vec![b"PING".to_vec()]);
}

#[test]
fn set_then_get_round_trip() {
    let mut store = MemoryStore::default();
    let set = dispatch(
        &mut store,
        &[b"SET".to_vec(), b"k".to_vec(), b"hello".to_vec()],
    )
    .unwrap();
    assert_eq!(set, b"+OK\r\n");

    let get = dispatch(&mut store, &[b"GET".to_vec(), b"k".to_vec()]).unwrap();
    assert_eq!(get, b"$5\r\nhello\r\n");
}

#[test]
fn set_overwrites_existing_key() {
    let mut store = MemoryStore::default();
    dispatch(
        &mut store,
        &[b"SET".to_vec(), b"k".to_vec(), b"one".to_vec()],
    )
    .unwrap();
    let set = dispatch(
        &mut store,
        &[b"SET".to_vec(), b"k".to_vec(), b"two".to_vec()],
    )
    .unwrap();
    assert_eq!(set, b"+OK\r\n");
    let get = dispatch(&mut store, &[b"GET".to_vec(), b"k".to_vec()]).unwrap();
    assert_eq!(get, b"$3\r\ntwo\r\n");
}

#[test]
fn get_missing_returns_null_bulk() {
    let mut store = MemoryStore::default();
    let get = dispatch(&mut store, &[b"GET".to_vec(), b"missing".to_vec()]).unwrap();
    assert_eq!(get, b"$-1\r\n");
}

#[test]
fn del_and_exists_semantics() {
    let mut store = MemoryStore::default();
    dispatch(
        &mut store,
        &[b"SET".to_vec(), b"k".to_vec(), b"v".to_vec()],
    )
    .unwrap();

    let exists = dispatch(&mut store, &[b"EXISTS".to_vec(), b"k".to_vec()]).unwrap();
    assert_eq!(exists, b":1\r\n");

    let del = dispatch(&mut store, &[b"DEL".to_vec(), b"k".to_vec()]).unwrap();
    assert_eq!(del, b":1\r\n");

    let missing = dispatch(&mut store, &[b"DEL".to_vec(), b"k".to_vec()]).unwrap();
    assert_eq!(missing, b":0\r\n");

    let exists = dispatch(&mut store, &[b"EXISTS".to_vec(), b"k".to_vec()]).unwrap();
    assert_eq!(exists, b":0\r\n");
}

#[test]
fn set_rejects_oversized_key() {
    let mut store = MemoryStore::default();
    let key = vec![b'x'; MAX_KEY_LEN + 1];
    let reply = dispatch(&mut store, &[b"SET".to_vec(), key, b"v".to_vec()]).unwrap();
    assert_eq!(reply, b"-ERR key too large\r\n");
}

#[test]
fn set_rejects_oversized_value() {
    let mut store = MemoryStore::default();
    let value = vec![0u8; MAX_VALUE_LEN + 1];
    let reply = dispatch(&mut store, &[b"SET".to_vec(), b"k".to_vec(), value]).unwrap();
    assert_eq!(reply, b"-ERR value too large\r\n");
}
