//! **`std_echo`: a `std` program that prints the words it was run with, and nothing else**
//! (milestone 205 (how a foreign program is told what to do)). It is the smallest `std` program
//! whose manifest note says it hears words; `std_exerciser` is too big to travel as an image (256
//! KiB, `spawnproto::IMAGE_MAXIMUM_PAGES`). `script/swish-check` runs it unvouched by path, so its
//! one line, `words [...]` in `{:?}` form, proves the shell sent the argv, the progenitor built the
//! `std` layout for an image, and `std::env::args_os()` read the page. See notes/std/the-proof.md.
//!
//! Name: provisional (2026-09-27).

// The manifest the note carries: an unvouched program's grants are §219 (how the shell names an
// installed program to the spawner)'s whatever this says, so it declares the least that is true.
capability_manifest_note::carry!(grant_contract::Manifest {
    program_argument: grant_contract::ProgramArgumentDeclaration::Words(
        grant_contract::WordGrant::ReadOnly
    ),
    startup: grant_contract::StartupContract::Std,
    ..program_table::UNVOUCHED_MANIFEST
});

fn main() {
    println!("words {:?}", std::env::args_os().collect::<Vec<_>>());
}
