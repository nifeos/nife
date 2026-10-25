//! **`grant_plan`: the grant-expression logic of the capability shell** (milestone 31, phase 1).
//!
//! On a nife command line *designation is authorization* (Mark Miller's principle): naming a
//! resource in a command is how you grant it. The crate performs no IO and makes no syscalls.
//! [`parse`] turns a line of bytes into a [`Command`]. For a program invocation, [`plan`] places
//! its tokens into the slots the program's [`Manifest`] declares ([`Program::manifest`]) and yields
//! an [`Endowment`], exactly what to grant the child, or a typed [`Refusal`] the shell prints at the
//! prompt. [`spawnproto`] is the spawn protocol's word layout. See notes/grant-expression.md, its
//! appendix grant-expression/grant-plan-crate.md, and notes/program-manifest.md.
//!
//! Name: ratified 2026-08-01 (calef, milestone 63), replacing `capsh`. Refused `capsh` (Linux's
//! `capsh(1)` is a different tool). Refused `designation`, `designate` and `designator` (the user
//! designates; this crate's work starts after that). Refused `endow`, `award`, `confer`, `bestow`,
//! `allot` and `furnish` (grant is already this tree's word, and `endow` is taken by
//! `supervision_protocol::ChildEndowment`). Not named for `swish`: seven things use it.

#![no_std]
// milestone 68's ratchet is workspace-wide (§107); this crate opts out until its 22-item
// worklist (notes/doc-coverage.md) is burned down.
#![allow(missing_docs)]

pub mod expand;
pub mod image;
pub mod job_page_frame;
pub mod job_windows;
pub mod line;
pub mod nav;
pub mod spawnproto;
pub mod verbs;
pub mod word;

mod command;
mod escalation;
mod manifest;
mod parsing;
mod planning;
mod program;
mod refusal;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub use command::{
    Command, DirectoryGrant, Endowment, FileGrant, Holdings, MAX_SHORT_OPTIONS, MountedTree,
    TouchOperands, rmopt,
};
pub use escalation::{Action, COOP_GRACE_TICKS, Escalation};
pub use image::{
    IMAGE_ROW, ImageRefusal, NO_NOTE_MANIFEST, Vouch, image_can_carry, image_hears_words,
    image_manifest, image_request_fits,
};
pub use manifest::{
    CATALOG_SLOT, DIAGNOSTICS_SLOT, DOMAIN_SLOT, DirectoryGrantDeclaration, ENTROPY_SLOT,
    FileGrantDeclaration, INDEX_SOURCES_SLOT, INSTALLER_REPLY_SLOT, INSTALLER_SLOT,
    IndexStoreDeclaration, InputDeclaration, MACHINE_SLOT, MAX_DECLARED_SHORT_OPTIONS, Manifest,
    MemoryGrantDeclaration, NETWORK_SLOT, NO_SHORT_OPTIONS, OutputDeclaration,
    ProgramArgumentDeclaration, REBOOT_SLOT, SHARE_SLOT, SHELL_CONFIGURATION_SLOT,
    SHELL_CONFIGURATION_VIRTUAL_ADDRESS, STD_REGION_PAGES, SYNC_SLOT, ShortOptions,
    StartupContract, UNVOUCHED_MANIFEST, UNVOUCHED_STD_MANIFEST, WordGrant, image_region_pages,
    named_std_region_pages,
};
pub use parsing::{
    BUILTINS, MAX_POSITIONALS, ParsedInvocation, UserVerb, argv, check_words, each_word,
    job_operand, kill_operands, parse, parse_run, tokenize, user_verb, vouch_verb, vouched_name,
};
pub(crate) use planning::nav_refusal;
pub use planning::{
    Streams, check_chain, plan, plan_against, plan_against_with, plan_stage, redirect_target,
};
pub use program::{PROGRAM_COUNT, Program};
pub use refusal::{CapabilityKind, Refusal};
pub use verbs::{MAX_VERB, ManifestAuthorities, VerbGrant, narrow_by_verb, verb_of, verb_of_page};

mod lexing;

use lexing::*;
pub use lexing::{MAX_FILE_NAME, WORDS_NAME, file_name_fits, parse_u64, trim};

// ---- small byte helpers (no_std, no alloc) ----
