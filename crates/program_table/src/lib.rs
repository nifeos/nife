//! **`program_table`: the programs nife's shell can spawn, and how a spawn is carried** (milestone
//! 1094 (an outside review of `grant_plan`), its first decision). The planner in `grant_plan` knows
//! only manifests; this crate is nife's side of it. [`Program`] is the 28-program catalogue and what
//! each row declares, [`spawnproto`] the spawn protocol's word layout, [`job_windows`] the shell's
//! window pool, and [`budgets`] the region sizes the shell splits and the progenitor counts.
//! [`plan`] and [`plan_stage`] resolve a typed name against the catalogue and plan the line against
//! its manifest, returning the row beside the endowment, because the shell and the progenitor speak
//! about a program by its row. See notes/grant-expression.md and notes/program-manifest.md.
//!
//! Name: provisional (it waits on the naming-layers decision), minted 2026-11-14 (UTC) by lane
//! milestone/1094-grant-plan-split.

#![no_std]
// The items moved here from `grant_plan`, which carried the same opt-out for its worklist
// (notes/doc-coverage.md).
#![allow(missing_docs)]

pub mod budgets;
pub mod image;
pub mod job_page_frame;
pub mod job_windows;
pub mod spawnproto;

mod planning;
mod program;
#[cfg(test)]
mod tests;

pub use budgets::{
    DIRECTORY_JOB_REGION_PAGES, JOB_POOL_PAGES, JOB_REGION_PAGES, STD_REGION_PAGES,
    image_region_pages, job_region_pages, named_std_region_pages,
};
// The vocabulary the rows are written in, in scope under the names the modules here always used.
// Not re-exported: each item has one public path, in the crate that defines it.
pub(crate) use grant_contract::*;
pub(crate) use grant_plan::{Endowment, Holdings, ParsedInvocation, Refusal, Streams, expand};
pub use image::{
    IMAGE_ROW, ImageRefusal, NO_NOTE_MANIFEST, UNVOUCHED_MANIFEST, UNVOUCHED_STD_MANIFEST, Vouch,
    image_can_carry, image_hears_words, image_manifest, image_request_fits,
};
pub use planning::{plan, plan_stage};
pub use program::{PROGRAM_COUNT, Program};
