//! **`grant_plan`: the grant-expression logic of a capability shell**.
//!
//! On this shell's command line *designation is authorization* (Mark Miller's principle): naming
//! a resource in a command is how you grant it. The crate performs no input or output and makes no
//! system call.
//! [`parse`] turns a line of bytes into a [`CommandLine`]. For a program invocation, [`plan_against`]
//! places its tokens into the slots a program's [`Manifest`] declares and yields an [`Endowment`],
//! exactly what to grant the child, or a typed [`Refusal`] the shell prints at the prompt. The
//! manifest is the caller's to supply: this crate knows no program, and the declarations it plans
//! against are `grant_contract`'s.
//!
//! Name: ratified 2026-08-01 (calef, milestone 63), replacing `capsh`. Refused `capsh` (Linux's
//! `capsh(1)` is a different tool). Refused `designation`, `designate` and `designator` (the user
//! designates; this crate's work starts after that). Refused `endow`, `award`, `confer`, `bestow`,
//! `allot` and `furnish` (grant is already this tree's word, and `endow` is taken by
//! `supervision_contract::ChildEndowment`). Not named for `swish`: seven things use it.

#![no_std]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

pub mod expansion;
pub mod line;
pub mod navigation;
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
    CommandLine, DirectoryGrant, Endowment, FileGrant, MAXIMUM_SHORT_OPTIONS, MountedTree,
    ShellHoldings, TouchOperands, rm_options,
};
pub use escalation::{COOPERATIVE_GRACE_POLLS, Escalation, InterruptAction};
// The declarations a line is planned against, in scope under the names the modules here always
// used. Not re-exported: each item has one public path, in `grant_contract`.
pub(crate) use grant_contract::*;
pub use parsing::{
    BUILTINS, MAXIMUM_POSITIONALS, NoJobNamed, ParsedInvocation, UserVerb, check_words,
    for_each_word, job_operand, kill_operands, line_verb, parse, parse_invocation, tokenize,
    user_verb, vouch_operand, vouched_name, write_argument_page,
};
pub(crate) use planning::nav_refusal;
pub use planning::{
    Streams, check_chain, input_file_grant, output_file_grant, plan_against,
    plan_against_with_streams,
};
pub use refusal::{CapabilityKind, Refusal};
// The rows the tests plan against, as a caller would pass them.
#[cfg(test)]
use test_support::rows;

mod lexing;

use lexing::*;
pub use lexing::{MAXIMUM_FILE_NAME_BYTES, MAXIMUM_WORDS_NAME_BYTES, is_valid_file_name};

// ---- small byte helpers (no_std, no alloc) ----
