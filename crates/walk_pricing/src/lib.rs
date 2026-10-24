//! **What a directory walk costs, separated into the parts that cost it** (milestone 121 (`ripgrep` on
//! nife: enumeration as a capability), and what the walk costs).
//!
//! [`price`] times slopes over [`filesystem_fixture::walk`]'s tree, each over one variable: per
//! path component, per directory entry and per byte read, and the whole walk split by call
//! ([`Split`]) to check them against. Points are medians of [`REPS`] or [`WHOLE_REPS`] warm runs.
//! Plain `std::fs`, so nife and a host run the same code; see notes/walk-pricing.md.
//!
//! # Name
//!
//! Name: provisional 2026-09-26 (milestone 121's lane). A noun for what the crate does to a walk,
//! in the `_pricing` shape nothing else in the tree uses yet; calef names crates.

//! # BUGS
//!
//! - A median of three is not statistics: the figures are magnitudes and directions.
//! - Warm, not cold: these are the costs of a second walk.
//! - The per-entry slope is inside one `READDIR` page; see `fixture::walk::WIDE_COUNT`.
//! - Path-shaped only: a walker holding `std::fs::Dir` handles is not measured.
//!
//! notes/walk-pricing.md, "What the instrument does not see", has each in full.

use std::path::Path;
use std::time::Instant;
use std::{fs, io};

use filesystem_fixture::walk as tree;

/// Timed runs per slope point. Odd, so the median is a run rather than an average of two. Three
/// rather than five because the kernel suite runs this under TCG on three architectures inside a
/// CI job with a time limit; the figures to quote are the totals anyway (notes/walk-pricing.md).
pub const REPS: usize = 3;

/// Timed runs of the whole walk. Five since milestone 606 (a directory walk costs what it does on
/// Linux) made a warm walk about seven times cheaper on nife, which paid for
/// them under TCG; before that the walk was timed once.
pub const WHOLE_REPS: usize = 5;

/// **Build the priced tree under `root`**, which becomes [`tree::ROOT`]'s contents. Used by the
/// image builder (`xtask`), by this crate's own test, and by a host that wants to price the same
/// shape; nife never calls it, because its copy is on the disk image.
pub fn stage(root: &Path) -> io::Result<()> {
    let mut level = root.join(tree::CHAIN);
    for i in 0..=tree::DEPTH {
        if i > 0 {
            level = level.join(tree::LEVEL);
        }
        fs::create_dir_all(&level)?;
        fs::write(level.join(tree::LEVEL_FILE), tree::SMALL_BODY)?;
    }
    let wide = root.join(tree::WIDE);
    fs::create_dir_all(&wide)?;
    for i in 0..tree::WIDE_COUNT {
        fs::write(wide.join(name(i)), tree::SMALL_BODY)?;
    }
    let narrow = root.join(tree::NARROW);
    fs::create_dir_all(&narrow)?;
    fs::write(narrow.join(name(0)), tree::SMALL_BODY)?;
    let sizes = root.join(tree::SIZES);
    fs::create_dir_all(&sizes)?;
    for (file, len) in tree::SIZE_FILES {
        let body: Vec<u8> = (0..len).map(tree::sized_byte).collect();
        fs::write(sizes.join(file), body)?;
    }
    Ok(())
}

/// [`tree::wide_name`] as a string.
fn name(i: usize) -> String {
    tree::wide_name(i).iter().map(|&b| b as char).collect()
}

/// What one full walk touched. Deterministic for a given tree, which is what makes it assertable
/// where the timings are not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    /// Entries `read_dir` handed back, directories and files alike.
    pub entries: usize,
    /// Files opened and read to the end.
    pub files: usize,
    /// Bytes those reads returned.
    pub bytes: usize,
    /// Path components resolved, summed over every `read_dir` and every open: the number of hops
    /// a component-at-a-time resolver pays, and the number a multi-component one would save.
    pub components: usize,
}

/// **Walk `root` recursively and read every file**, the way `walkdir` does: list, and for each
/// entry either descend by the path the listing handed back or open that path.
///
/// An error anywhere is returned rather than skipped, because a walk that silently finds nothing
/// where it could not look is the failure a search tool must never have. That is the property the
/// refusal half of milestone 121 asserts through this function.
pub fn walk(root: &Path) -> io::Result<Totals> {
    let mut t = Totals::default();
    walk_from(root, 0, &mut t)?;
    Ok(t)
}

fn walk_from(dir: &Path, depth: usize, t: &mut Totals) -> io::Result<()> {
    walk_split(dir, depth, t, &mut Split::default())
}

/// **Where a walk's time went**, by the `std::fs` call that spent it (milestone 606 (a directory
/// walk costs what it does on Linux), provisional number). The four are the whole of what a walk
/// asks of the platform, so they sum to the walk less the walker's own bookkeeping, and each maps
/// onto a distinct part of nife's path: `list` is `OPENDIR` per component, `READDIR` pages and a
/// `CLOSE` per hop; `open` is the same descent plus `OPEN`; `read` is `FSTAT` and the `READ`s;
/// `close` is one `CLOSE`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Split {
    /// `read_dir` of a directory and draining its entries, with each entry's `file_type`.
    pub list: u64,
    /// `File::open` of each file, by the path the listing handed back.
    pub open: u64,
    /// `read_to_end` on each open file: the size hint and every read, to end of file.
    pub read: u64,
    /// Dropping each file, which closes it.
    pub close: u64,
}

impl Split {
    /// The four summed.
    pub fn total(&self) -> u64 {
        self.list + self.open + self.read + self.close
    }
}

/// Nanoseconds since `t0`, saturating.
fn since(t0: Instant) -> u64 {
    u64::try_from(t0.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// [`walk_from`], timing each platform call into `s`. One walker rather than a timed copy of it,
/// so the walk that is split is the walk that is counted: `Instant` on nife is one counter read,
/// cheap next to any of the calls it brackets.
fn walk_split(dir: &Path, depth: usize, t: &mut Totals, s: &mut Split) -> io::Result<()> {
    use std::io::Read;
    let t0 = Instant::now();
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let is_dir = entry.file_type()?.is_dir();
        entries.push((entry.path(), is_dir));
    }
    s.list += since(t0);
    for (path, is_dir) in entries {
        t.entries += 1;
        // The entry's own depth below the root is the component count of the path about to be
        // resolved, for a descent and an open alike.
        t.components += depth + 1;
        if is_dir {
            walk_split(&path, depth + 1, t, s)?;
        } else {
            let t0 = Instant::now();
            let mut file = fs::File::open(&path)?;
            s.open += since(t0);
            // `fs::read` is exactly these two calls with the open in front: `read_to_end` on a
            // `File` asks for the size first, as `fs::read` does.
            let mut body = Vec::new();
            let t0 = Instant::now();
            file.read_to_end(&mut body)?;
            s.read += since(t0);
            let t0 = Instant::now();
            drop(file);
            s.close += since(t0);
            t.bytes += body.len();
            t.files += 1;
        }
    }
    Ok(())
}

/// The median of [`REPS`] timed runs of `f`, in nanoseconds, after one untimed run.
fn median_ns(mut f: impl FnMut() -> io::Result<()>) -> io::Result<u64> {
    f()?;
    let mut runs = [0u64; REPS];
    for run in &mut runs {
        let t0 = Instant::now();
        f()?;
        *run = u64::try_from(t0.elapsed().as_nanos()).unwrap_or(u64::MAX);
    }
    runs.sort_unstable();
    Ok(*middle(&runs))
}

/// The middle element of a sorted, odd-length slice. Both medians here take it, so the index is
/// written once and tested on values a test controls, which a real clock is not.
fn middle<T>(sorted: &[T]) -> &T {
    &sorted[sorted.len() / 2]
}

/// Least-squares slope of `ys` over `xs`, in the units of `ys` per unit of `xs`.
fn slope(xs: &[f64], ys: &[f64]) -> f64 {
    let n = xs.len() as f64;
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let num: f64 = xs.iter().zip(ys).map(|(x, y)| (x - mx) * (y - my)).sum();
    let den: f64 = xs.iter().map(|x| (x - mx) * (x - mx)).sum();
    num / den
}

/// Everything [`price`] measured. Timings are nanoseconds as the platform's `Instant` reports
/// them, so under an emulator they are the emulator's time and only their ratios mean anything.
#[derive(Clone, Debug)]
pub struct Price {
    /// What the whole walk touched.
    pub totals: Totals,
    /// Reading the chain's file at each level, shallowest first: `[k]` is `k + 2` components below
    /// the root (`chain`, `k` levels, the file).
    pub by_depth: Vec<u64>,
    /// The fitted cost of one more component, from [`Self::by_depth`].
    pub per_component: f64,
    /// Listing the one-entry directory, with `file_type` on each entry.
    pub list_narrow: u64,
    /// Listing the wide one.
    pub list_wide: u64,
    /// Reading the smallest and largest sized file.
    pub read_small: u64,
    /// See [`Self::read_small`].
    pub read_large: u64,
    /// The whole walk, warm: the median of [`WHOLE_REPS`] runs.
    pub whole: u64,
    /// The fastest of those runs.
    pub whole_min: u64,
    /// The slowest.
    pub whole_max: u64,
    /// [`Self::whole`], by the call that spent it.
    pub split: Split,
}

impl Price {
    /// What one more entry in a listing costs.
    pub fn per_entry(&self) -> f64 {
        (self.list_wide as f64 - self.list_narrow as f64) / (tree::WIDE_COUNT - 1) as f64
    }

    /// What one more KiB read costs.
    pub fn per_kib(&self) -> f64 {
        let (small, large) = (tree::SIZE_FILES[0].1, tree::SIZE_FILES[2].1);
        (self.read_large as f64 - self.read_small as f64) / ((large - small) as f64 / 1024.0)
    }
}

/// **Price a walk over the tree at `root`.** See the crate header for what each figure isolates.
pub fn price(root: &Path) -> io::Result<Price> {
    let totals = walk(root)?;

    let mut by_depth = Vec::with_capacity(tree::DEPTH + 1);
    let mut level = root.join(tree::CHAIN);
    for i in 0..=tree::DEPTH {
        if i > 0 {
            level = level.join(tree::LEVEL);
        }
        let file = level.join(tree::LEVEL_FILE);
        by_depth.push(median_ns(|| fs::read(&file).map(drop))?);
    }
    let xs: Vec<f64> = (0..by_depth.len()).map(|k| k as f64).collect();
    let ys: Vec<f64> = by_depth.iter().map(|&y| y as f64).collect();
    let per_component = slope(&xs, &ys);

    let list = |dir: &Path| {
        let dir = dir.to_path_buf();
        median_ns(move || {
            for e in fs::read_dir(&dir)? {
                e?.file_type()?;
            }
            Ok(())
        })
    };
    let list_narrow = list(&root.join(tree::NARROW))?;
    let list_wide = list(&root.join(tree::WIDE))?;

    let sizes = root.join(tree::SIZES);
    let small = sizes.join(tree::SIZE_FILES[0].0);
    let large = sizes.join(tree::SIZE_FILES[2].0);
    let read_small = median_ns(|| fs::read(&small).map(drop))?;
    let read_large = median_ns(|| fs::read(&large).map(drop))?;

    // Warm: the walk that counted above is the untimed run. Each timed run is also split by call,
    // and the split reported is the median run's.
    let mut wholes = Vec::with_capacity(WHOLE_REPS);
    for _ in 0..WHOLE_REPS {
        let mut split = Split::default();
        let t0 = Instant::now();
        walk_split(root, 0, &mut Totals::default(), &mut split)?;
        wholes.push((since(t0), split));
    }
    wholes.sort_unstable_by_key(|&(ns, _)| ns);
    let (whole, split) = *middle(&wholes);
    let (whole_min, whole_max) = (wholes[0].0, wholes[WHOLE_REPS - 1].0);

    Ok(Price {
        totals,
        by_depth,
        per_component,
        list_narrow,
        list_wide,
        read_small,
        read_large,
        whole,
        whole_min,
        whole_max,
        split,
    })
}

/// **The first line [`report`] prints**, and the only one a gate may assert on: the counts, which
/// are a fact about the tree and the walk and not about the machine.
pub fn totals_line(t: &Totals) -> String {
    format!(
        "walk visited {} entries, {} files, {} bytes, {} components",
        t.entries, t.files, t.bytes, t.components
    )
}

/// The priced figures as the lines a person reads, [`totals_line`] first.
pub fn report(p: &Price) -> Vec<String> {
    vec![
        totals_line(&p.totals),
        format!(
            "walk per component {:.0} ns (depth 2: {} ns, depth {}: {} ns)",
            p.per_component,
            p.by_depth[0],
            tree::DEPTH + 2,
            p.by_depth[tree::DEPTH],
        ),
        format!(
            "walk per entry {:.0} ns (list 1: {} ns, list {}: {} ns)",
            p.per_entry(),
            p.list_narrow,
            tree::WIDE_COUNT,
            p.list_wide,
        ),
        format!(
            "walk per KiB {:.0} ns (read 4 KiB: {} ns, read 256 KiB: {} ns)",
            p.per_kib(),
            p.read_small,
            p.read_large,
        ),
        format!(
            "walk whole {} ns (min {} ns, max {} ns, {} runs)",
            p.whole, p.whole_min, p.whole_max, WHOLE_REPS,
        ),
        format!(
            "walk split list {} ns, open {} ns, read {} ns, close {} ns",
            p.split.list, p.split.open, p.split.read, p.split.close,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory in the host temp dir, unique per test and process.
    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("walk_pricing-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// **The walker visits exactly the fixture**, and the fixture's own arithmetic agrees. This is
    /// the claim the nife gate asserts from inside a confined process; here it is proven against a
    /// filesystem nobody disputes, so a disagreement there is nife's and not the walker's.
    #[test]
    fn the_walk_visits_what_the_fixture_says_it_holds() {
        let root = scratch("walk");
        stage(&root).expect("stage the tree");
        let t = walk(&root).expect("walk the tree");
        assert_eq!(t.entries, tree::WALK_ENTRIES);
        assert_eq!(t.files, tree::WALK_FILES);
        assert_eq!(t.bytes, tree::WALK_BYTES);
        assert_eq!(t.components, tree::WALK_COMPONENTS);
        let _ = fs::remove_dir_all(&root);
    }

    /// The lines [`tree::rg_match_path`] says `rg` prints over the tree, sorted: `path:body`.
    fn fixture_rg_lines() -> Vec<String> {
        let body = std::str::from_utf8(tree::SMALL_BODY).expect("ASCII");
        let mut lines: Vec<String> = (0..tree::RG_MATCHES)
            .map(|i| {
                let mut buf = [0u8; 32];
                let len = tree::rg_match_path(i, &mut buf);
                format!(
                    "{}:{}",
                    std::str::from_utf8(&buf[..len]).expect("ASCII"),
                    body.trim_end()
                )
            })
            .collect();
        lines.sort();
        lines
    }

    /// **[`tree::rg_match_path`] names exactly the staged files that hold the small body**, each
    /// once. The kernel harness holds `rg`'s parallel walk to that list, so the list must be the
    /// tree's and not a guess about it.
    #[test]
    fn the_rg_match_list_is_the_files_holding_the_body() {
        let root = scratch("rg-list");
        stage(&root).expect("stage the tree");
        let mut paths: Vec<String> = (0..tree::RG_MATCHES)
            .map(|i| {
                let mut buf = [0u8; 32];
                let len = tree::rg_match_path(i, &mut buf);
                String::from_utf8(buf[..len].to_vec()).expect("ASCII")
            })
            .collect();
        for p in &paths {
            assert_eq!(fs::read(root.join(p)).expect(p), tree::SMALL_BODY, "{p}");
        }
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), tree::RG_MATCHES, "a path is listed twice");
        let _ = fs::remove_dir_all(&root);
    }

    /// **The host's `rg`, walking in parallel, prints exactly the fixture's lines** (milestone 121,
    /// lane milestone/121-rg-threads). The same two lines the kernel harness types on nife, run by
    /// ripgrep's own 14.1.1 build for the host, which `cargo xtask basalt-fetch` fetches by SHA-256:
    /// the version basalt's package carries, with upstream `ignore`, whose walker basalt's patch
    /// leaves alone on a `unix` host. Its match lines, sorted, must be [`fixture_rg_lines`]; nife's
    /// are held to the same list, so the two agree byte for byte up to order. Sorted because a
    /// parallel walk prints in the order its threads finish, on any system.
    ///
    /// Skips when no host `rg` was fetched, unless `NIFE_RIPGREP_REQUIRED=1`, which the CI row that
    /// fetches it sets.
    #[test]
    fn the_hosts_parallel_rg_prints_the_fixture_lines() {
        let rg = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ripgrep/host/rg");
        if !rg.exists() {
            assert!(
                std::env::var_os("NIFE_RIPGREP_REQUIRED").is_none(),
                "no host rg at {} and NIFE_RIPGREP_REQUIRED is set",
                rg.display()
            );
            eprintln!(
                "no host rg at {}; `cargo xtask basalt-fetch` fetches it",
                rg.display()
            );
            return;
        }
        let root = scratch("rg-host");
        stage(&root).expect("stage the tree");
        for line in [tree::RG_SEARCH_THREADED, tree::RG_SEARCH_FOUR_THREADS] {
            let mut words: Vec<String> = Vec::new();
            grant_plan::each_word(line.as_bytes(), &mut |w| {
                words.push(String::from_utf8(w.to_vec()).expect("ASCII"));
                Ok(())
            })
            .expect("a line the shell takes");
            let out = std::process::Command::new(&rg)
                .args(&words[1..])
                .arg("--no-config")
                .current_dir(&root)
                .output()
                .expect("run the host's rg");
            assert!(out.status.success(), "{line}: {out:?}");
            let text = String::from_utf8(out.stdout).expect("rg printed UTF-8");
            let (matches, stats) = text.split_once("\n\n").expect("rg printed its stats");
            let mut got: Vec<String> = matches.lines().map(str::to_string).collect();
            got.sort();
            assert_eq!(got, fixture_rg_lines(), "{line}");
            assert!(
                stats.contains(&format!("{} files searched", tree::WALK_FILES)),
                "{line}: {stats}"
            );
        }
        let _ = fs::remove_dir_all(&root);
    }

    /// **The report a gate asserts on leads with the counts**, and every figure the crate header
    /// promises is printed. The timings are the host's and are not checked; the shape is, because
    /// the nife gate finds its lines by these prefixes.
    #[test]
    fn a_priced_walk_reports_the_counts_first_and_every_figure() {
        let root = scratch("price");
        stage(&root).expect("stage the tree");
        let p = price(&root).expect("price the walk");
        assert_eq!(p.by_depth.len(), tree::DEPTH + 1);
        assert!(p.whole > 0 && p.list_wide > 0 && p.read_large > 0);
        // The split is the whole walk less the walker's own bookkeeping, so it cannot exceed it.
        assert!(p.split.total() <= p.whole && p.split.list > 0 && p.split.read > 0);
        assert!(p.per_entry().is_finite() && p.per_kib().is_finite());
        let lines = report(&p);
        assert_eq!(lines[0], totals_line(&p.totals));
        for (line, prefix) in lines[1..].iter().zip([
            "walk per component",
            "walk per entry",
            "walk per KiB",
            "walk whole",
            "walk split",
        ]) {
            assert!(
                line.starts_with(prefix),
                "{line:?} does not start {prefix:?}"
            );
        }
        let _ = fs::remove_dir_all(&root);
    }

    /// **A walk that cannot look is an error, never an empty result.** The one property a search
    /// tool must have, checked against the host's own refusal: a missing root is `NotFound`, not a
    /// walk of zero entries.
    #[test]
    fn a_walk_that_cannot_look_fails_rather_than_finding_nothing() {
        let root = scratch("missing").join("not-there");
        let err = walk(&root).expect_err("walking a missing root succeeded");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    /// The slope is a least-squares fit and not an endpoint difference: a straight line comes back
    /// exactly, and one outlier moves it less than it moves the endpoints.
    #[test]
    fn slope_fits_a_line() {
        let xs = [0.0, 1.0, 2.0, 3.0];
        assert!((slope(&xs, &[5.0, 7.0, 9.0, 11.0]) - 2.0).abs() < 1e-9);
    }

    // ---- survivor triage of the 2026-10-03 census ---------------------------------------------

    #[test]
    fn a_split_totals_its_four_parts() {
        let s = Split {
            list: 1,
            open: 2,
            read: 4,
            close: 8,
        };
        assert_eq!(s.total(), 15);
    }

    /// The first line is the one a gate asserts on, so its words and order are fixed.
    #[test]
    fn the_totals_line_is_the_four_counts_in_order() {
        let t = Totals {
            entries: 1,
            files: 2,
            bytes: 3,
            components: 4,
        };
        assert_eq!(
            totals_line(&t),
            "walk visited 1 entries, 2 files, 3 bytes, 4 components"
        );
    }

    fn price_with(list_narrow: u64, list_wide: u64, read_small: u64, read_large: u64) -> Price {
        Price {
            totals: Totals::default(),
            by_depth: Vec::new(),
            per_component: 0.0,
            list_narrow,
            list_wide,
            read_small,
            read_large,
            whole: 0,
            whole_min: 0,
            whole_max: 0,
            split: Split::default(),
        }
    }

    /// The per-entry and per-KiB figures are differences of two timings over the difference in what
    /// they measured: 127 more entries, and 252 more KiB (the largest sized file less the smallest).
    #[test]
    fn a_price_per_entry_and_per_kib_divide_the_difference_by_what_differs() {
        let kib = (tree::SIZE_FILES[2].1 - tree::SIZE_FILES[0].1) / 1024;
        let p = price_with(
            100,
            100 + 7 * (tree::WIDE_COUNT as u64 - 1),
            50,
            50 + 3 * kib as u64,
        );
        assert!((p.per_entry() - 7.0).abs() < 1e-9, "{}", p.per_entry());
        assert!((p.per_kib() - 3.0).abs() < 1e-9, "{}", p.per_kib());
    }

    /// The middle of five is the third. Real timings tie too often to tell it from the second, so
    /// the index is checked here on values that cannot.
    #[test]
    fn the_middle_of_five_sorted_values_is_the_third() {
        assert_eq!(*middle(&[10, 20, 30, 40, 50]), 30);
    }

    /// `median_ns` runs its closure once untimed and then `REPS` times, and answers what the middle
    /// run took.
    #[test]
    fn the_median_is_of_timed_runs_after_one_that_is_not() {
        let mut calls = 0;
        let ns = median_ns(|| {
            calls += 1;
            std::thread::sleep(std::time::Duration::from_millis(2));
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, REPS + 1);
        assert!(ns >= 2_000_000, "{ns} ns for a 2 ms sleep");
        assert!(median_ns(|| Err(io::Error::other("x"))).is_err());
    }

    /// Every level of the chain is read. A tree missing its deepest file is a pricing error and not
    /// a shallower chain read over again.
    #[test]
    fn pricing_reads_every_level_of_the_chain() {
        let root = scratch("deepest");
        stage(&root).expect("stage the tree");
        let mut deepest = root.join(tree::CHAIN);
        for _ in 0..tree::DEPTH {
            deepest = deepest.join(tree::LEVEL);
        }
        fs::remove_file(deepest.join(tree::LEVEL_FILE)).expect("the deepest file exists");
        assert!(price(&root).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    /// A split walk accumulates time into every part it times: more than one file and more than
    /// one directory, so an accumulator that restarted from zero would show as nothing.
    #[test]
    fn a_split_walk_accumulates_open_and_close_time_over_all_its_files() {
        let root = scratch("split");
        stage(&root).expect("stage the tree");
        let mut s = Split::default();
        walk_split(&root, 0, &mut Totals::default(), &mut s).expect("walk");
        assert!(
            s.list > 0 && s.open > 0 && s.read > 0 && s.close > 0,
            "{s:?}"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
