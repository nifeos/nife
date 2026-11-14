//! **`grant_contract`: what a program declares it needs, and where it finds what it was given.**
//!
//! A program's [`Manifest`] says what it may be granted: an integer argument, memory, a file, a
//! directory, its words, its streams, and the services it is endowed with rather than given on the
//! line. The capability slot numbers ([`DOMAIN_SLOT`], [`DIAGNOSTICS_SLOT`] and their neighbors)
//! say where a program finds each of those services, and [`narrow_by_verb`] narrows a manifest to
//! what one verb of the program needs. The spawner endows from a manifest, a planner binds a
//! command line against one, and a program reads its slots from the same numbers, so none of the
//! three keeps a copy. A program declares the pair [`INTERFACE_ID`] and [`VERSION`].
//!
//! `no_std`, no allocation, no input or output.
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane `milestone/1094-grant-plan-split`. The name
//! it is published under waits on the decision about naming layers.

#![no_std]
// The `missing_docs` ratchet of milestone 68 (code-quality gates: one lint policy) is
// workspace-wide, opt-out rather than opt-in (§107 (`missing_docs` moves to
// `workspace.lints.rust`)); this crate opts out with the planner it came from until its worklist
// (notes/doc-coverage.md) is burned down.
#![allow(missing_docs)]

/// The version of the capability slot numbers the progenitor grants into and a program reads: 1. A
/// break bumps it, with the dated ledger entry the interface-stability discipline owes; an addition
/// an older peer ignores does not.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane lane/956-interface-ids-for-the-owed-formats.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 17, naming the capability slot
/// numbers the progenitor grants into and a program reads. The number is this interface's for good:
/// no other interface takes it, and a format change is a new version, never a new number. Milestone
/// 956 (the linked interfaces carry checkable versions) holds the table of numbers, and
/// `script/lint` holds the tree to it. A program declares the pair of this id and [`VERSION`].
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane lane/956-interface-ids-for-the-owed-formats.
pub const INTERFACE_ID: u64 = 17;

pub mod verbs;

mod manifest;

pub use manifest::{
    CATALOG_SLOT, DIAGNOSTICS_SLOT, DOMAIN_SLOT, DirectoryGrantDeclaration, ENTROPY_SLOT,
    FileGrantDeclaration, INDEX_SOURCES_SLOT, INSTALLER_REPLY_SLOT, INSTALLER_SLOT,
    IndexStoreDeclaration, InputDeclaration, MACHINE_SLOT, MAXIMUM_DECLARED_SHORT_OPTIONS,
    Manifest, MemoryGrantDeclaration, NETWORK_SLOT, NO_SHORT_OPTIONS, OutputDeclaration,
    POWER_MANAGEMENT_CHIP_ROUTE_SLOT, ProgramArgumentDeclaration, REBOOT_SLOT, SHARE_SLOT,
    SHELL_CONFIGURATION_SLOT, SHELL_CONFIGURATION_VIRTUAL_ADDRESS, SYNC_SLOT, ShortOptions,
    StartupContract, WordGrant,
};
pub use verbs::{
    MAXIMUM_VERB, ManifestAuthorities, VerbGrant, delegated_within, narrow_by_verb, verb_key,
    verb_of_page,
};
