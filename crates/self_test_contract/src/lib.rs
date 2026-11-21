//! **The boot self-test's hand-over**: what the kernel, the progenitor and `boot_self_test` agree
//! on so the self-test can run in two halves.
//!
//! The kernel runs the checks of its own internals and writes its tally and the facts the moved
//! checks need into one page ([`kernel_half`]). The progenitor maps that page into
//! `boot_self_test` and endows the program with the slots, images and pair pages
//! [`self_test_program`] fixes, and the program answers on its report endpoint with the two
//! messages that module names. The tally and the verdict's grammar are `boot_ladder::verdict`'s;
//! this crate holds only what passes between the three programs. See notes/self-test-checks.md,
//! "The two halves".
//!
//! # BUGS
//!
//! - The page carries no version beside its magic: a layout change re-mints
//!   [`kernel_half::MAGIC`], and a reader of the old page reads the new one as absent.
//!
//! Name: provisional, minted 2026-11-21 (UTC) by lane/boot-ladder-stands-alone, when the two
//! modules left `boot_ladder` so that crate holds only console lines. A wire contract between
//! programs of one system takes the `_contract` suffix. Considered `self_test_hand_over` (the
//! kernel file's word, but it names one step, not both modules) and `boot_self_test_contract`
//! (repeats the program's name, while the kernel's half is not that program's).

#![no_std]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

pub mod kernel_half;
pub mod self_test_program;
