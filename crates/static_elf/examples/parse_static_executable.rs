//! Parse a static ELF64 executable and print what a loader would map.
//!
//! `cargo run --example parse_static_executable -- <file>` reads the file. With no argument it
//! reads the crate's own seed for the machine it was built on, from `tests/fixtures/`.

use static_elf::{Elf, MACHINE_AARCH64, MACHINE_RISCV64, NATIVE_MACHINE, PageBytes};

fn main() {
    let bytes = match std::env::args_os().nth(1) {
        Some(path) => std::fs::read(&path).unwrap_or_else(|why| {
            eprintln!("{}: {why}", path.to_string_lossy());
            std::process::exit(2);
        }),
        None => match NATIVE_MACHINE {
            MACHINE_AARCH64 => include_bytes!("../tests/fixtures/minimal_rx_aarch64.elf").to_vec(),
            MACHINE_RISCV64 => include_bytes!("../tests/fixtures/minimal_rx_riscv64.elf").to_vec(),
            _ => include_bytes!("../tests/fixtures/minimal_rx_x86_64.elf").to_vec(),
        },
    };

    // A refusal prints as a sentence. Every check has run by the time `parse` returns.
    let elf = match Elf::parse(&bytes) {
        Ok(elf) => elf,
        Err(why) => {
            eprintln!("refused: {why}");
            std::process::exit(1);
        }
    };

    println!("entry {:#x}", elf.entry());
    for segment in elf.segments() {
        let (first, end) = segment.page_range(PageBytes::FOUR_KIBIBYTES);
        println!(
            "map {first:#x}..{end:#x} {}{}{} from {} file bytes, {} bytes in memory",
            if segment.is_readable() { 'r' } else { '-' },
            if segment.is_writable() { 'w' } else { '-' },
            if segment.is_executable() { 'x' } else { '-' },
            segment.data.len(),
            segment.memory_bytes,
        );
    }
}
