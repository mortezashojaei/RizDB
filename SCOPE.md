# RizDB — project scope

A tiny **disk-based key-value database server** that apps connect to over the network.

## Goals

- **Usable**: a real app can point a RESP client at it, store data, and get it back after restart
- **Disk-primary**: disk is the source of truth; the working set of *values* may exceed RAM (keys + offsets stay in memory)
- **Small**: easy to run on low-resource machines (e.g. Raspberry Pi); one human can read the whole codebase
- **Rust**: idiomatic Rust throughout

## v1 wire protocol

- **RESP2** over TCP (not RESP3)
- Default listen port **7379** (not 6379)
- Docs lead with **RESP**; mention other products only as example clients when needed
- Commands only: `PING`, `GET`, `SET`, `DEL`, `EXISTS`
- Semantics: `GET` miss → null bulk; `DEL`/`EXISTS` miss → `0`; `SET` always overwrites
- Keys and values are **binary-safe** (opaque bytes)
- Hard caps: **key ≤ 1 KiB**, **value ≤ 16 MiB**
- Wipe the DB: stop the process, delete the data directory, restart (no `FLUSHDB`)

## v1 runtime

- Many concurrent connections; **one serialized command executor**
- Durability: acknowledge after the write reaches the OS; **fsync every 1000 ms** by default (configurable); document the small loss window on power failure
- Config via **flags / env only**: listen address, data directory, fsync interval

## v1 storage

- **Append-only log** + **in-memory key → file-offset index** (Bitcask-style)
- On startup: **replay the full log** to rebuild the index
- **No compaction in v1** — overwrites and deletes leave garbage on disk until a later milestone
- Single node only

## Security

No auth or TLS in v1 — run only on trusted local/private networks.

## Explicitly later (not v1)

- Compaction / log GC
- TTL, richer value types, batch commands (`MGET` / `MSET`)
- Auth, TLS, replication / clustering
- RESP3, config files, explicit `SYNC` / `FLUSHDB` protocol commands
