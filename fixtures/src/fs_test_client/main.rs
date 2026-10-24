//! The FS-service client (milestone 32 (a real filesystem: RedoxFS behind a capability FS server)
//! phase 2): opens files through a granted directory
//! capability and proves the stack end to end.
//!
//! It holds an endpoint to the FS server and nothing that names the server, the block server or
//! the disk. That endpoint is its directory capability, and there is no global namespace to reach.
//! The proof role reads `motd`, writes a pattern to `scratch` and reads it back, and reports the
//! `motd` head and a sentinel; any failed check traps. The other roles are benchmarks, attackers and
//! witnesses, one module each, with shared requests in `channel` and the role numbers and `_start`
//! in `role` (module names provisional). See notes/fs-server/the-test-client.md.

//! # Capability contract (notes/fs-server.md, notes/abi.md §4)
//! - **slot 0**: the file-service endpoint, `WRITE` (this is the directory capability; the client
//!   `CALL`s here).
//! - **slot 1**: the report endpoint, `WRITE`.
//! - **`channel::FILE_VA`**: the base of the channel shared with the FS server (a name out, file bytes
//!   both ways). `filesystem_contract::fs::TRANSFER_PAGES` pages, all of them mapped by this client's wiring,
//!   because the throughput role asks for the whole of it in one request.

//! Name: ratified 2026-08-01 (calef, milestone 63), replacing `fsclient`. Refused `fsclient`
//! (squished) and `fs_client` (any program that wants files is an FS client, so that name belongs
//! to the real thing; it also predicts "a client of the FS service" rather than "the program the
//! kernel spawns to prove the FS contract holds").

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

mod bench;
mod channel;
mod crash;
mod directory_attacker;
mod extended_attributes;
mod file_attacker;
mod name_set_attributes;
mod proof;
mod role;
mod shared_frame;
mod throughput;
mod two_directories;

user_mode_runtime::panic_handler!();
