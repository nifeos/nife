//! Replay every committed seed through the target's body, so `cargo test` (and `cargo miri test`)
//! checks the seeds without libFuzzer.

use std::path::Path;

#[test]
fn every_seed_passes() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("seeds/memory_region_heap_sequence");
    let mut entries: Vec<_> = std::fs::read_dir(&directory)
        .expect("the seed directory")
        .map(|entry| entry.expect("a seed entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "bin"))
        .collect();
    entries.sort();
    assert!(!entries.is_empty(), "no seeds in {}", directory.display());
    for path in entries {
        fuzz::run(&std::fs::read(&path).expect("a seed"));
    }
}
