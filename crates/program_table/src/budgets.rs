//! **The region budgets the shell and the progenitor both count**, in pages: what a job, a `std`
//! program and a file run by its path are built in, and the pool the shell holds jobs in. The crate
//! root re-exports every item.
//!
//! Name: provisional.

use crate::{Manifest, StartupContract, spawnproto};

/// **The region the progenitor builds a `std` program in, and then hands it as its heap**, in pages
/// (milestone 595 (provisional)). The shell prints it in `caps` and the progenitor spends it, so it
/// is here, where both read it.
///
/// One region, so one reclaim returns all of it: the child is built from it first, and the same
/// region, narrowed to `WRITE`, is its allocator's budget at
/// `std_runtime_contract::MEMORY_REGION_SLOT`. 256 pages of heap, the test harness's
/// `BUDGET_PAGES`, plus 128 for the build; not a measured high-water mark. See
/// notes/grant-expression/grant-plan-crate.md ("The `std` region").
///
/// Name: provisional.
pub const STD_REGION_PAGES: u64 = 256 + 128;

/// **The region a named `std` program is built in**, in pages: [`STD_REGION_PAGES`], plus one
/// package's worth ([`spawnproto::IMAGE_MAXIMUM_PAGES`]) for a program that declares
/// [`Manifest::installer`] (milestone 809 (the package client becomes a program)).
///
/// A package manager reads a whole package into one run of its own pages before it sends it a page
/// at a time, so the run is its buffer and its frames. It comes from the progenitor's image pool,
/// which is sized for a package's bytes. See notes/grant-expression/grant-plan-crate.md ("The `std`
/// region").
///
/// Name: provisional.
pub const fn named_std_region_pages(m: &Manifest) -> u64 {
    STD_REGION_PAGES
        + if m.installer {
            spawnproto::IMAGE_MAXIMUM_PAGES
        } else {
            0
        }
}

/// **One native job's region**, in pages: everything a spawned program is made of, so a single
/// reclaim frees all of it (milestone 933 (every job is built the way a supervised job is)). The
/// shell splits it off its own budget for a job it holds, and the progenitor used to split it off
/// its job pool, so both read the number here. Counted from the largest debug image the prompt
/// spawns, its stack, a thread control block, an address-space root, the tables for the windows a
/// child touches and the mapping records of §13 (capability revocation and untyped reclamation);
/// see notes/system-initializer/budgets.md.
///
/// Name: provisional, moved here from `system_initializer` by milestone 933's lane.
pub const JOB_REGION_PAGES: u64 = 60;

/// **One directory-granted native job's region**: the program, the `fs_subtree_caretaker` that
/// carries its grant and the two endpoints between them, out of one carve (§92 (a caretaker is
/// supervised by the client it serves)). See notes/system-initializer/budgets.md.
///
/// Name: provisional, moved here from `system_initializer` by milestone 933's lane.
pub const DIRECTORY_JOB_REGION_PAGES: u64 = 98;

/// **The jobs a pool holds at once**: six native regions and one `std` region. The progenitor's job
/// pool was this size, and the shell's budget carries the same mix for the jobs it holds (milestone
/// 933). Small on purpose, as the pool was: `script/swish-check` runs more jobs through it than it
/// could hold without the regions coming back, so widening it silently retires that gate. See
/// notes/system-initializer/budgets.md.
///
/// Name: provisional (milestone 933's lane).
pub const JOB_POOL_PAGES: u64 = JOB_REGION_PAGES * 6 + STD_REGION_PAGES;

/// **The region a job from the program table is built in**, in pages: the `std` region for a `std`
/// program, else [`DIRECTORY_JOB_REGION_PAGES`] behind a directory grant and [`JOB_REGION_PAGES`]
/// otherwise. `m` is the manifest after the line's verb narrowed it. The shell splits exactly this
/// many pages for a job it holds, and the progenitor refuses a job whose region is short.
///
/// Name: provisional (milestone 933's lane).
pub const fn job_region_pages(m: &Manifest, directory_grant: bool) -> u64 {
    if matches!(m.startup, StartupContract::Std) {
        named_std_region_pages(m)
    } else if directory_grant {
        DIRECTORY_JOB_REGION_PAGES
    } else {
        JOB_REGION_PAGES
    }
}

/// **The region a file run by its path is built in**, in pages: what a named program of the same
/// runtime gets ([`STD_REGION_PAGES`], or `native` for the progenitor's native job), plus the
/// image's own pages (milestone 595 (the shell runs a `std` program), 2026-09-27).
///
/// A named program's region was sized around its image, and every one in the archive is small. A
/// file is not: `rg`'s image is nearly twice a `std` region by itself, so a fixed size would either
/// refuse it or leave it no heap. Adding the image keeps what a program has left after its build
/// the same whichever way it arrived, at the price of counting a small image's pages twice.
///
/// Name: provisional.
pub const fn image_region_pages(image_pages: u64, startup: StartupContract, native: u64) -> u64 {
    image_pages
        + match startup {
            StartupContract::Std => STD_REGION_PAGES,
            StartupContract::Native => native,
        }
}
