#![no_std]
//! The one library target of the `components` package: the logic of programs that has host tests
//! and proofs, kept out of the program's own binary because a binary of this package builds with
//! `test = false` and cannot carry them.
//!
//! Name: provisional.

pub mod block_driver_roles;
