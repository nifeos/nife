//! **`installed_programs`**: what is installed, as a table that is never edited, only succeeded (§208
//! (installing a package is granting it, and the installed programs are versioned)). The progenitor reads
//! the live generation on every image spawn and looks the executable's digest up ([`lookup_digest`]);
//! the shell reads it to resolve a bare word and preview `caps`. Both take the location from here
//! (rule 7): a directory [`DIRECTORY`] holding [`CURRENT`] and one file per generation
//! ([`generation_name`]). Install and remove write the next generation ([`with_entries`],
//! [`with_entry`], [`without_package`]); a one-line `current` names the live one
//! ([`parse_current`], [`format_current`]). Any malformed line makes the whole table
//! [`Error::Malformed`]: a half-parsed table would vouch for whatever survived. The line format,
//! rows keyed on digest, pointers, `grant` lines and the `answered` line, is in
//! notes/packages/installed-programs.md. [`approval`] is the compiled approval: what `jig approve`
//! sends, what the progenitor stores beside the generations, and what a delegated holder's
//! requests are checked against (milestone 911 (scripted installs grant only what the owner's
//! approval file allows)).

//! # BUGS
//!
//! - Nothing collects old generations; a retention rule is owed before frequent installs.
//! - A table is read whole into the caller's buffer, about 140 bytes a row.
//! - The progenitor uses one page per generation; a row with its pointer, about 170 bytes, lets it
//!   hold about two dozen installed programs.
//! - Nothing makes the `current` write atomic; a torn write on RedoxFS was not checked.
//!
//! Name: ratified 2026-10-18 (calef, "`installed_programs`"). Each row is a program, keyed by its
//! executable's digest, that may run, and vouching goes through the same installer as install and
//! remove, so "installed" covers every row.

#![no_std]

pub use measured_boot::Digest;

mod answered;
pub mod approval;
mod generation;
mod grants;
pub use answered::{Answered, answered, record, same_record};
pub use generation::*;
pub use grants::{DirRights, Grant, Granted, ROOT_ZONE, covers, extras, grants, network_granted};
