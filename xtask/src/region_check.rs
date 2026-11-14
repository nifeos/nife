//! **A spawned program must fit the region it is spawned into** (`cargo xtask region-check`,
//! `script/region-check`).
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane program-region-size-check.
//!
//! The shell splits a fixed region off its budget for every native job
//! (`program_table::job_region_pages`: `JOB_REGION_PAGES`, or `DIRECTORY_JOB_REGION_PAGES` behind a
//! directory grant), and the progenitor builds the program, its stack, its tables and its records
//! out of that one carve. A program whose image grows past the carve fails at the first spawn with
//! "could not spawn", which the prompt cannot tell from an empty pool, and only a QEMU suite
//! notices. This reads each program's built ELF and says so on the host, naming the program, its
//! pages and the region.
//!
//! **What it measures.** The pages the image occupies once loaded (every `PT_LOAD` segment's
//! `p_memsz`, so `.bss` counts, merged where segments share a page), as the loader maps them, plus
//! the fixed costs in [`fixed_pages`], against the region. The region numbers are
//! `grant_plan`'s; the stack numbers are read out of `crates/system_initializer/src/layout.rs`, so
//! no number is copied here. The one number that is this file's own is [`TABLES_AND_RECORDS`], the
//! tables, thread control block, address-space root and mapping records of one address space, and
//! the margin [`MARGIN_PAGES`]; `notes/system-initializer/budgets.md` says how each was chosen.
//!
//! Debug images only, because a debug image is the largest and is what the suites spawn. A `std`
//! program is built elsewhere and with its own region, so it is skipped.

use std::collections::BTreeMap;

use grant_contract::{DirectoryGrantDeclaration, StartupContract};
use object::Endianness;
use object::read::elf::{ElfFile64, ProgramHeader};
use program_table::{Program, job_region_pages};

use crate::host::{flag_value, workspace_root};
use crate::{RISCV_TARGET, TARGET, X86_TARGET, profile_dir};

/// Pages one address space costs beyond its image and stack: the thread control block, the
/// address-space root, the intermediate tables for the windows a child touches (two
/// gigabyte-level tables, and on `x86_64` three more for the timebase page) and the mapping
/// records of §13 (capability revocation and untyped reclamation). Provisional: `top` (31 image pages) passes at 60 and cannot be built at 48, so
/// the true figure lies between 18 and 29 less its twelve stack pages.
const TABLES_AND_RECORDS: u64 = 8;

/// Pages that must stay free under the region after every cost above, so a program that is close
/// to the wall fails here rather than on the next small change. Provisional.
const MARGIN_PAGES: u64 = 4;

/// The program a directory grant puts beside the program (`fs_subtree_caretaker`); the
/// `fs_nameset_caretaker` that replaces it for a rename is no larger.
const CARETAKER: &str = "fs_subtree_caretaker";

/// One program's row: what it needs, what it has.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) program: String,
    pub(crate) image_pages: u64,
    pub(crate) need_pages: u64,
    pub(crate) region_pages: u64,
}

impl Row {
    /// The sentence a failure prints: the program, its size and the region it must fit.
    pub(crate) fn failure(&self) -> Option<String> {
        (self.need_pages + MARGIN_PAGES > self.region_pages).then(|| {
            format!(
                "{}: image {} pages, {} with its stack and tables, in a region of {} pages \
                 (margin {}): over by {}",
                self.program,
                self.image_pages,
                self.need_pages,
                self.region_pages,
                MARGIN_PAGES,
                (self.need_pages + MARGIN_PAGES).saturating_sub(self.region_pages)
            )
        })
    }
}

/// Pages a loaded image spans: the union of every `PT_LOAD` segment's `[vaddr, vaddr + memsz)`
/// rounded out to pages.
pub(crate) fn loaded_pages(elf: &[u8]) -> Result<u64, String> {
    let file = ElfFile64::<Endianness>::parse(elf).map_err(|e| format!("not an ELF64: {e}"))?;
    let endian = file.endian();
    let mut pages = std::collections::BTreeSet::new();
    for header in file.elf_program_headers() {
        if header.p_type(endian) != object::elf::PT_LOAD {
            continue;
        }
        let start = header.p_vaddr(endian);
        let end = start + header.p_memsz(endian);
        if end == start {
            continue;
        }
        for page in start / 4096..end.div_ceil(4096) {
            pages.insert(page);
        }
    }
    Ok(pages.len() as u64)
}

/// The constant `name` from `layout.rs`'s `pub const NAME: u64 = N;`.
fn layout_constant(layout: &str, name: &str) -> Result<u64, String> {
    let needle = format!("const {name}: u64 = ");
    layout
        .lines()
        .find_map(|l| l.split_once(&needle).map(|(_, rest)| rest))
        .and_then(|rest| rest.trim_end_matches(';').trim().parse().ok())
        .ok_or_else(|| format!("layout.rs has no `{needle}<number>;` line"))
}

/// Pages one address space costs besides its image: the stack and [`TABLES_AND_RECORDS`].
fn fixed_pages(stack_pages: u64) -> u64 {
    stack_pages + TABLES_AND_RECORDS
}

/// Rows for every native program in `program_table::Program::ALL`, given each built program's loaded
/// pages by archive name. A program with no entry in `sizes` is reported as an error: the check
/// would otherwise pass for a program it never saw.
pub(crate) fn rows(
    sizes: &BTreeMap<String, u64>,
    child_stack: u64,
    caretaker_stack: u64,
) -> Result<Vec<Row>, String> {
    let size = |name: &str| {
        sizes
            .get(name)
            .copied()
            .ok_or_else(|| format!("{name}: no built image to measure"))
    };
    let mut out = Vec::new();
    for program in Program::ALL {
        let manifest = program.manifest();
        if matches!(manifest.startup, StartupContract::Std) {
            continue;
        }
        let directory = !matches!(manifest.directory, DirectoryGrantDeclaration::Forbidden);
        let image = size(program.name())?;
        let mut need = image + fixed_pages(child_stack);
        if directory {
            // The caretaker is a second address space in the same carve, with its own stack (one
            // page `build_child` maps, and `CARETAKER_STACK_PAGES` more) and tables.
            need += size(CARETAKER)? + fixed_pages(caretaker_stack + 1);
        }
        out.push(Row {
            program: program.name().to_string(),
            image_pages: image,
            need_pages: need,
            region_pages: job_region_pages(&manifest, directory),
        });
    }
    Ok(out)
}

/// `cargo xtask region-check [--arch aarch64|riscv64|x86_64] [--no-build]`.
pub(crate) fn region_check() -> bool {
    let (target, arch) = match flag_value("--arch").as_deref() {
        None | Some("aarch64") => (TARGET, "aarch64"),
        Some("riscv64") => (RISCV_TARGET, "riscv64"),
        Some("x86_64") => (X86_TARGET, "x86_64"),
        Some(other) => {
            eprintln!("region-check: --arch {other} is not aarch64, riscv64 or x86_64");
            return false;
        }
    };
    if !std::env::args().any(|a| a == "--no-build") && !crate::archive::build_programs(target) {
        eprintln!("region-check: building the programs failed");
        return false;
    }
    let layout_path = workspace_root().join("crates/system_initializer/src/layout.rs");
    let constants = std::fs::read_to_string(&layout_path)
        .map_err(|e| format!("cannot read {}: {e}", layout_path.display()))
        .and_then(|text| {
            Ok((
                layout_constant(&text, "CHILD_STACK_PAGES")?,
                layout_constant(&text, "CARETAKER_STACK_PAGES")?,
            ))
        });
    let (child_stack, caretaker_stack) = match constants {
        Ok(c) => c,
        Err(e) => {
            eprintln!("region-check: {e}");
            return false;
        }
    };
    let mut sizes = BTreeMap::new();
    let mut names: Vec<&str> = Program::ALL.iter().map(|p| p.name()).collect();
    names.push(CARETAKER);
    for name in names {
        let path = workspace_root().join(format!("target/{target}/{}/{name}", profile_dir()));
        let measured = std::fs::read(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))
            .and_then(|bytes| loaded_pages(&bytes));
        // A `std` program is built elsewhere and has no image here; `rows` skips it, and
        // reports any native program that is missing.
        if let Ok(pages) = measured {
            sizes.insert(name.to_string(), pages);
        }
    }
    let table = match rows(&sizes, child_stack, caretaker_stack) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("region-check: {e}");
            return false;
        }
    };
    let mut failed = false;
    for row in &table {
        println!(
            "region-check: {arch} {:<28} image {:>4} need {:>4} region {:>4}",
            row.program, row.image_pages, row.need_pages, row.region_pages
        );
        if let Some(why) = row.failure() {
            eprintln!("region-check: FAIL {arch} {why}");
            failed = true;
        }
    }
    if failed {
        eprintln!(
            "region-check: a program outgrew its job region; shrink it, or raise the region in \
             grant_plan with a measured reason (notes/system-initializer/budgets.md)"
        );
    } else {
        println!(
            "region-check: PASS ({arch}, {} programs, margin {MARGIN_PAGES} pages)",
            table.len()
        );
    }
    !failed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(need: u64, region: u64) -> Row {
        Row {
            program: "date".into(),
            image_pages: need,
            need_pages: need,
            region_pages: region,
        }
    }

    #[test]
    fn a_program_under_the_region_less_the_margin_passes() {
        assert_eq!(row(60 - MARGIN_PAGES, 60).failure(), None);
    }

    #[test]
    fn a_program_inside_the_margin_fails_naming_program_pages_and_region() {
        let why = row(60 - MARGIN_PAGES + 1, 60).failure().expect("fails");
        assert!(why.starts_with("date: image 57 pages"), "{why}");
        assert!(why.contains("region of 60 pages"), "{why}");
        assert!(why.ends_with("over by 1"), "{why}");
    }

    #[test]
    fn the_layout_constants_are_read_from_the_source() {
        let text = "/// doc\npub const CHILD_STACK_PAGES: u64 = 12;\n";
        assert_eq!(layout_constant(text, "CHILD_STACK_PAGES"), Ok(12));
        assert!(layout_constant(text, "CARETAKER_STACK_PAGES").is_err());
    }

    #[test]
    fn a_missing_image_is_an_error_not_a_pass() {
        assert!(rows(&BTreeMap::new(), 12, 4).is_err());
    }
}
