//! **`std_grep`: the smallest `std` program that reads what its words name** (milestone 205 (how
//! a foreign program is told what to do), §170 (how a foreign program is told what to do) clauses
//! 2 to 5). `std_grep NEEDLE [PATH...]` prints every line of every file under the paths that
//! contains NEEDLE, as `path:line`, walking a directory the way `rg` does. With no path it searches `.`, as `rg` does.
//!
//! It exists because `rg` cannot travel as an image yet (256 KiB, `spawnproto::IMAGE_MAX_PAGES`)
//! and is not built in CI, and the designation half needs a program that behaves like it at the
//! prompt. `script/swish-check` copies it to the disk unvouched and runs it by path, so every
//! name it opens was designated by a word on the line and granted read-only, and nothing else
//! exists for it (calef's N1 ruling, 2026-09-27T06:27Z: `std_grep needle` names nothing and is
//! granted nothing, so its search of `.` fails, loudly).
//!
//! Exit status is `grep`'s: 0 for a match, 1 for none, 2 for an error. Name: provisional
//! (2026-09-27).

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

// Unvouched bytes are granted what §219 (how the shell names an installed program to the spawner)
// and §170 say whatever this declares; it declares what is
// true: it hears words, reads what they name, and runs on `std`.
manifest_note::carry!(grant_plan::Manifest {
    program_argument: grant_plan::ArgSpec::Words(grant_plan::WordGrant::ReadOnly),
    runtime: grant_plan::Runtime::Std,
    ..grant_plan::UNVOUCHED_MANIFEST
});

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(needle) = args.next() else {
        eprintln!("usage: std_grep NEEDLE [PATH...]");
        return ExitCode::from(2);
    };
    let needle = needle.to_string_lossy().into_owned();
    let mut paths: Vec<PathBuf> = args.map(PathBuf::from).collect();
    if paths.is_empty() {
        paths.push(PathBuf::from("."));
    }
    let (mut found, mut failed) = (false, false);
    for p in &paths {
        search(p, &needle, &mut found, &mut failed);
    }
    ExitCode::from(if failed {
        2
    } else if found {
        0
    } else {
        1
    })
}

fn search(path: &Path, needle: &str, found: &mut bool, failed: &mut bool) {
    let mut complain = |e: std::io::Error| {
        // The kind rather than the whole error, so the line a person reads says why in one word:
        // `Unsupported` is "no directory was granted", `NotFound` "no such name here".
        let why = match e.kind() {
            ErrorKind::Unsupported => "no directory was granted to search".to_string(),
            k => format!("{k:?}"),
        };
        eprintln!("std_grep: {}: {why}", path.display());
        *failed = true;
    };
    match std::fs::read_dir(path) {
        Ok(entries) => {
            let mut names: Vec<PathBuf> =
                entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
            names.sort();
            for n in names {
                search(&n, needle, found, failed);
            }
            return;
        }
        Err(e) if e.kind() == ErrorKind::Unsupported => return complain(e),
        Err(_) => {}
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            for line in String::from_utf8_lossy(&bytes).lines() {
                if line.contains(needle) {
                    println!("{}:{line}", path.display());
                    *found = true;
                }
            }
        }
        Err(e) => complain(e),
    }
}
