mod common;

use std::fs;

use rizdb::store::{Store, MAX_KEY_LEN, MAX_VALUE_LEN};

use common::temp_data_dir;

#[test]
fn set_then_get_returns_value() {
    let dir = temp_data_dir("rizdb-store");
    let mut store = Store::open(&dir).unwrap();
    store.set(b"greeting", b"hello").unwrap();
    assert_eq!(store.get(b"greeting").unwrap(), Some(b"hello".to_vec()));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn get_missing_key_returns_none() {
    let dir = temp_data_dir("rizdb-store");
    let mut store = Store::open(&dir).unwrap();
    assert_eq!(store.get(b"missing").unwrap(), None);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn data_survives_reopen() {
    let dir = temp_data_dir("rizdb-store");
    {
        let mut store = Store::open(&dir).unwrap();
        store.set(b"k", b"v").unwrap();
    }
    let mut store = Store::open(&dir).unwrap();
    assert_eq!(store.get(b"k").unwrap(), Some(b"v".to_vec()));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn set_overwrites_existing_key() {
    let dir = temp_data_dir("rizdb-store");
    let mut store = Store::open(&dir).unwrap();
    store.set(b"k", b"one").unwrap();
    store.set(b"k", b"two").unwrap();
    assert_eq!(store.get(b"k").unwrap(), Some(b"two".to_vec()));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn del_removes_key() {
    let dir = temp_data_dir("rizdb-store");
    let mut store = Store::open(&dir).unwrap();
    store.set(b"k", b"v").unwrap();
    assert_eq!(store.del(b"k").unwrap(), 1);
    assert_eq!(store.get(b"k").unwrap(), None);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn del_missing_key_returns_zero() {
    let dir = temp_data_dir("rizdb-store");
    let mut store = Store::open(&dir).unwrap();
    assert_eq!(store.del(b"missing").unwrap(), 0);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn delete_survives_reopen() {
    let dir = temp_data_dir("rizdb-store");
    {
        let mut store = Store::open(&dir).unwrap();
        store.set(b"k", b"v").unwrap();
        store.del(b"k").unwrap();
    }
    let mut store = Store::open(&dir).unwrap();
    assert_eq!(store.get(b"k").unwrap(), None);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn exists_reports_presence() {
    let dir = temp_data_dir("rizdb-store");
    let mut store = Store::open(&dir).unwrap();
    assert!(!store.exists(b"k"));
    store.set(b"k", b"v").unwrap();
    assert!(store.exists(b"k"));
    store.del(b"k").unwrap();
    assert!(!store.exists(b"k"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rejects_key_larger_than_1_kib() {
    let dir = temp_data_dir("rizdb-store");
    let mut store = Store::open(&dir).unwrap();
    let key = vec![b'x'; MAX_KEY_LEN + 1];
    assert!(matches!(
        store.set(&key, b"v"),
        Err(rizdb::store::StoreError::KeyTooLarge)
    ));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rejects_value_larger_than_16_mib() {
    let dir = temp_data_dir("rizdb-store");
    let mut store = Store::open(&dir).unwrap();
    let value = vec![0u8; MAX_VALUE_LEN + 1];
    assert!(matches!(
        store.set(b"k", &value),
        Err(rizdb::store::StoreError::ValueTooLarge)
    ));
    let _ = fs::remove_dir_all(&dir);
}
