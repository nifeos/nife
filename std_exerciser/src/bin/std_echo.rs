//! **`std_echo`: a `std` program that prints the words it was run with, and nothing else**
//! (milestone 205 (how a foreign program is told what to do)). It is the smallest `std` program
//! that carries a manifest note saying it hears words, which is what an installed foreign program
//! run by path is: `std_exerciser` is too big to travel as an image (256 KiB,
//! `spawnproto::IMAGE_MAX_PAGES`), and `ripgrep` is ten times that.
//!
//! `script/swish-check` copies it to the disk unvouched and runs it by path, so the line proves
//! three things at once: the shell read the note and sent the argv, the progenitor sized a `std`
//! region from the argv bit and built the `std` layout for an image, and `std::env::args_os()`
//! read the page. It prints one line, `words [...]`, in `{:?}` form so a byte that is not text is
//! still visible.
//!
//! Name: provisional (2026-09-27).

// The manifest the note carries: an unvouched program's grants are §219 (how the shell names an
// installed program to the spawner)'s whatever this says, so it declares the least that is true.
capability_manifest_note::carry!(grant_plan::Manifest {
    arg: grant_plan::ArgSpec::Words(grant_plan::WordGrant::ReadOnly),
    runtime: grant_plan::Runtime::Std,
    ..grant_plan::UNVOUCHED_MANIFEST
});

fn main() {
    println!("words {:?}", std::env::args_os().collect::<Vec<_>>());
}
