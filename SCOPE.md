# RizDB — project scope

A tiny **disk-based database server** that apps connect to over the network.

- Speaks a **Redis-compatible connection protocol** so existing clients can talk to it
- Stores data **on disk**, not as an in-memory database
- Stays **very small and simple** — easy to run on low-resource machines (e.g. Raspberry Pi) and easy for a human to understand
- Goal: learn how a real database works end-to-end (connections + durable storage), and ship something someone could actually use

Security note for early versions: no auth assumed — run only on trusted local/private networks unless auth and related protections are added later.
