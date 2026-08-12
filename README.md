# RizDB

Tiny disk-based key-value database server. Speaks **RESP2** over TCP.

See [SCOPE.md](./SCOPE.md) for the v1 product decisions and [issue #1](https://github.com/mortezashojaei/RizDB/issues/1) for the implementation spec.

## Run

```bash
cargo run -- --port 7379 --data-dir ./data --fsync-ms 1000
```

Defaults: host `127.0.0.1`, port `7379`, data dir `./data`, fsync interval `1000` ms.

Flags also accept env: `RIZDB_HOST`, `RIZDB_PORT`, `RIZDB_DATA_DIR`, `RIZDB_FSYNC_MS`.

`--fsync-ms` must be greater than 0.

### Durability

Writes are acknowledged after they reach the OS page cache (`flush`). Stable storage is updated on the fsync interval. **On power loss, writes from the last fsync window (up to `--fsync-ms`) may be lost.** Process crash after flush is usually fine; sudden power loss is not.

No auth or TLS — run only on a trusted local/private network.

### Wipe

Stop the server, delete the data directory, start again.

## Commands

`PING`, `GET`, `SET`, `DEL`, `EXISTS`

## Test

```bash
cargo test
```
