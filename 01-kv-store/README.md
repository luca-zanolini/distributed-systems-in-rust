# 01 — Key-Value Store

A persistent key-value store with an interactive command-line REPL. This is the first
project in the repo and the foundation the later ones build on: nearly every distributed
store (Redis, etcd, TiKV, DynamoDB) is a key-value store with replication and consensus
layered on top.

## What it does

Start it and type commands at the prompt:

```
set name Luca
get name              -> Luca
set country South Korea
get country           -> South Korea
remove name           -> Removed: Luca
get name              -> Key not found
exit
```

State is written to `store.db` (JSON) on exit and reloaded on startup, so your data
survives a restart.

## Run

```bash
cargo run     # start the REPL
cargo test    # run the unit tests
```

## Design

- **`Store`** — a thin wrapper over `HashMap<String, String>` with `set` / `get` / `remove`.
- **REPL** — reads a line from stdin, splits it into words, and dispatches with a `match`
  on a slice pattern (`["set", key, rest @ ..]`, `["get", key]`, …). Handles end-of-input
  and unknown commands.
- **Persistence** — `serde` + `serde_json` serialize the store to JSON in `store.db`;
  `load` reads it back on startup, treating a missing file on the first run as an empty store.

## Notable details

- **Ownership by intent:** `get` borrows and lends back (`Option<&String>`); `remove`
  returns an *owned* `String`, because the map gives that value up.
- **Multi-word values:** a slice-rest pattern (`rest @ ..`) plus `join(" ")` lets values
  contain spaces (`set country South Korea`).
- **Errors:** file and JSON operations return `Result`; `?` propagates them to `main`,
  whose `Box<dyn Error>` return type can hold either an I/O or a JSON error.

## What I learned

Rust: structs and methods, ownership and borrowing (owned vs. borrowed parameters *and*
returns), `Option` / `Result` / `match`, the `?` operator, slice patterns, file I/O, `serde`
derive macros, and unit tests. Distributed-systems seed: **durability** — persisting state so
it survives a restart — and the fact that changing a serialization format breaks old data
files, which is a real migration problem in production systems.

---
Part of [distributed-systems-in-rust](../).
