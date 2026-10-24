//! **The host side of the search measurement in milestone 121 (`ripgrep` on nife)**: stage the
//! priced tree, then run unmodified `ripgrep` over it as nife's kernel harness and bench boot do.
//!
//! ```text
//! cargo run --release -p walk_pricing --example rg_host -- <path to rg> [runs]
//! ```
//!
//! The words are `filesystem_fixture::walk::RG_SEARCH` split by nife's argv builder,
//! `grant_plan::each_word`, run in the staged tree with output to a drained pipe (a host's nearest
//! thing to nife's byte sink). It prints ripgrep's own `seconds` line and spawn-to-exit beside it.

//! # BUGS
//!
//! - The tree is staged on whatever filesystem `DIR` is on (tmpfs under the Linux script), and
//!   every timed run after the first is warm; see notes/ripgrep-on-nife.md,
//!   "The search against Linux".

use std::process::Command;
use std::time::Instant;

use filesystem_fixture::walk as tree;

/// `rg --stats`'s last line, `<seconds> seconds`, in nanoseconds.
fn rg_seconds(text: &str) -> Option<u128> {
    let line = text.lines().rev().find(|l| l.ends_with(" seconds"))?;
    let secs = line.strip_suffix(" seconds")?;
    let (whole, frac) = secs.split_once('.')?;
    let frac = format!("{frac:0<9}");
    Some(whole.parse::<u128>().ok()? * 1_000_000_000 + frac[..9].parse::<u128>().ok()?)
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    let rg = args
        .next()
        .expect("usage: rg_host <path to rg> [runs] [dir]");
    let runs: usize = args
        .next()
        .and_then(|r| r.to_str().and_then(|r| r.parse().ok()))
        .unwrap_or(5);
    let dir = args
        .next()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("rg-walk-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&dir);
    walk_pricing::stage(&dir).expect("could not stage the priced tree");
    let mut words: Vec<Vec<u8>> = Vec::new();
    grant_plan::each_word(tree::RG_SEARCH.as_bytes(), &mut |w| {
        words.push(w.to_vec());
        Ok(())
    })
    .expect("RG_SEARCH is a line the shell takes");
    let words: Vec<String> = words
        .into_iter()
        .map(|w| String::from_utf8(w).expect("ASCII"))
        .collect();

    // One untimed run first, so every timed one is warm, as nife's bench row is.
    for run in 0..=runs {
        let t = Instant::now();
        // stdin is inherited rather than `output()`'s default of /dev/null, for two reasons. The
        // Linux run is PID 1 on an initramfs with no /dev, so opening /dev/null fails the spawn.
        // And `rg` given no path searches stdin when stdin is a file or a pipe; inheriting the
        // console (or a terminal) keeps it walking its working directory, as nife's does.
        let out = Command::new(&rg)
            .args(&words[1..])
            .current_dir(&dir)
            .stdin(std::process::Stdio::inherit())
            .output()
            .unwrap_or_else(|e| {
                panic!(
                    "could not run {rg:?} in {dir:?} (it exists: {}, the directory: {}): {e}",
                    std::path::Path::new(&rg).exists(),
                    dir.exists()
                )
            });
        let wall = t.elapsed();
        let text = String::from_utf8_lossy(&out.stdout);
        if run == 0 {
            // The untimed run's whole transcript tail, so a reader sees the counts nife asserts.
            println!(
                "rg exit {:?}, {} bytes out",
                out.status.code(),
                out.stdout.len()
            );
            for line in text
                .lines()
                .rev()
                .take(9)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
            {
                println!("rg {line}");
            }
            continue;
        }
        let Some(ns) = rg_seconds(&text) else {
            println!("rg printed no stats");
            continue;
        };
        println!(
            "rg search {ns} ns (rg's own figure), spawn to exit {} ns",
            wall.as_nanos()
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
