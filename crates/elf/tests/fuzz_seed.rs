//! The committed fuzz seeds must still be valid ELFs.
//!
//! `fuzz/fuzz_targets/elf_parse` returns at once on anything `Elf::parse` rejects, so a seed that
//! stops parsing leaves the fuzzer an empty corpus while it still reports no crashes. There is one
//! seed per machine nife runs, differing only at offset 18 (`e_machine`). See notes/elf/parser.md
//! ("The fuzz seeds").
//!
//! Regenerate them with the script in [fuzz/seeds/README.md](../../../fuzz/seeds/README.md) if a
//! deliberate tightening of `parse` makes them invalid.

use elf::Elf;

/// Every seed under `fuzz/seeds/elf_parse/`.
///
/// No `#[cfg(target_arch)]`: all three are compiled in and the right one is chosen by the machine
/// number in its own header, so the choice cannot disagree with the bytes.
const SEEDS: [&[u8]; 3] = [
    include_bytes!("../../../fuzz/seeds/elf_parse/minimal_rx_aarch64.elf"),
    include_bytes!("../../../fuzz/seeds/elf_parse/minimal_rx_riscv64.elf"),
    include_bytes!("../../../fuzz/seeds/elf_parse/minimal_rx_x86_64.elf"),
];

/// The `e_machine` a seed's header claims.
fn machine_of(seed: &[u8]) -> u16 {
    u16::from_le_bytes([seed[18], seed[19]])
}

/// The seed this build's fuzzer will actually get past `Elf::parse`.
fn seed_for_this_build() -> &'static [u8] {
    SEEDS
        .iter()
        .copied()
        .find(|seed| machine_of(seed) == elf::NATIVE_MACHINE)
        .expect("fuzz/seeds/elf_parse/ must hold a seed for the machine this build accepts")
}

#[test]
fn the_fuzz_seed_is_a_valid_executable() {
    let seed = seed_for_this_build();
    let elf = Elf::parse(seed).expect("the seed must parse, or the fuzzer starts from nothing");

    assert_eq!(elf.entry(), 0x4000_0000);

    let segments: Vec<_> = elf.segments().collect();
    assert_eq!(
        segments.len(),
        1,
        "one PT_LOAD, which is the point of minimal"
    );

    // Read and execute, not write: the seed has to get past the W^X refusal as well as the header
    // checks, or it teaches the fuzzer nothing about the paths after validation.
    let seg = segments[0];
    assert!(seg.is_readable());
    assert!(seg.is_executable());
    assert!(!seg.is_writable());
    assert!(
        seg.vaddr <= elf.entry() && elf.entry() < seg.vaddr + seg.memsz,
        "the entry point must land inside the executable segment"
    );
}

/// Every machine nife runs has a seed, checked against `elf::KNOWN_MACHINES` so a missing seed
/// fails on every host, not only the one it is missing for.
#[test]
fn every_machine_nife_runs_has_a_seed() {
    for machine in elf::KNOWN_MACHINES {
        assert!(
            SEEDS.iter().any(|seed| machine_of(seed) == machine),
            "no seed in fuzz/seeds/elf_parse/ carries e_machine {machine}; \
             see fuzz/seeds/README.md for the generator (milestone 288)"
        );
    }
    assert_eq!(
        SEEDS.len(),
        elf::KNOWN_MACHINES.len(),
        "a seed for a machine no nife build accepts is dead weight in every fuzz run"
    );
}
