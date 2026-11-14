//! **`grant_plan`: the grant-expression logic of the capability shell** (milestone 31, phase 1).
//!
//! On a nife command line *designation is authorization* (Mark Miller's principle): naming a
//! resource in a command is how you grant it. The crate performs no IO and makes no syscalls.
//! [`parse`] turns a line of bytes into a [`Command`]. For a program invocation, [`plan_against`]
//! places its tokens into the slots a program's [`Manifest`] declares and yields an [`Endowment`],
//! exactly what to grant the child, or a typed [`Refusal`] the shell prints at the prompt. The
//! manifest is the caller's to supply: this crate knows no program, and the declarations it plans
//! against are `grant_contract`'s. See notes/grant-expression.md, its appendix
//! grant-expression/grant-plan-crate.md, and notes/program-manifest.md.
//!
//! Name: ratified 2026-08-01 (calef, milestone 63), replacing `capsh`. Refused `capsh` (Linux's
//! `capsh(1)` is a different tool). Refused `designation`, `designate` and `designator` (the user
//! designates; this crate's work starts after that). Refused `endow`, `award`, `confer`, `bestow`,
//! `allot` and `furnish` (grant is already this tree's word, and `endow` is taken by
//! `supervision_contract::ChildEndowment`). Not named for `swish`: seven things use it.

#![no_std]
// milestone 68's ratchet is workspace-wide (§107); this crate opts out until its 22-item
// worklist (notes/doc-coverage.md) is burned down.
#![allow(missing_docs)]

pub mod expand;
pub mod line;
pub mod nav;
pub mod word;

mod command;
mod escalation;
mod parsing;
mod planning;
mod refusal;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub use command::{
    Command, DirectoryGrant, Endowment, FileGrant, Holdings, MAXIMUM_SHORT_OPTIONS, MountedTree,
    TouchOperands, rmopt,
};
pub use escalation::{Action, COOP_GRACE_TICKS, Escalation};
// The declarations a line is planned against, in scope under the names the modules here always
// used. Not re-exported: each item has one public path, in `grant_contract`.
pub(crate) use grant_contract::*;
pub use parsing::{
    BUILTINS, MAXIMUM_POSITIONALS, ParsedInvocation, UserVerb, argv, check_words, each_word,
    job_operand, kill_operands, parse, parse_run, tokenize, user_verb, verb_of, vouch_verb,
    vouched_name,
};
pub(crate) use planning::nav_refusal;
pub use planning::{
    ISOLATION_WORD, Streams, check_chain, plan_against, plan_against_with, redirect_target,
};
// The rows the tests plan against, as a caller would pass them (see the dev dependency's comment).
#[cfg(test)]
use program_table::Program;
pub use refusal::{CapabilityKind, Refusal};

mod lexing;

use lexing::*;
pub use lexing::{MAXIMUM_FILE_NAME, WORDS_NAME, file_name_fits, parse_u64, trim};

// ---- small byte helpers (no_std, no alloc) ----
