//! **The programs the shell can spawn**: the `programs!` macro, the four `const fn` checks its
//! expansion names, and the table, one row per program. What each program is granted is the
//! `impl Program` in `program/manifests.rs`: one module in two files, so adding a program stays one
//! module's edit (notes/adding-a-program.md). calef expects the table to shrink once packages are
//! installed through jig rather than listed here (§272, fork 2, 2026-10-11).
//!
//! Moved out of the crate's root unchanged by milestone 842 (the grant-plan crate root is split
//! along its seams), as §272 (the grant-plan crate root splits into private modules) lays out; the
//! root re-exports every public item at its old path.
//!
//! Name: provisional (milestone 842, 2026-10-11), as are its sibling modules' names.

mod manifests;

#[cfg(test)]
mod tests;

mod catalog;

pub use catalog::*;
