//! Unmodified `ripgrep` runs its parallel walker on more than one thread (milestone 121
//! (`ripgrep` on nife), fatal risk 1's third gap).
//!
//! calef, 2026-10-12 (UTC): *"the path to green on risk 1 requires a foreign program running a
//! second thread."* `rg` runs as a `std` job holding the priced tree through a caretaker. When it
//! ends, [`sched::process_threads_started`] must be the main thread plus the workers it asked for,
//! and its output the fixture's 138 match lines, each once, in any order (the host's `rg` is held
//! to the same list by `walk_pricing`). They skip without `rg`; CI's `rg-threads` rows run them at
//! four cores on all three ISAs. See notes/ripgrep-on-nife.md, "Threads: it asks, and since
//! milestone 812 it gets more than one".

//! Name: provisional, lane milestone/121-rg-threads (#2016), 2026-10-12 (UTC), for the module and
//! the file. The pull request records no other name considered.

use super::*;

/// `rg`'s heap budget here, in pages: 512, not the 256 `ripgrep_tests` gives the serial walk. Four
/// workers spent 269 on 2026-10-12 (UTC) against the serial walk's 65, about 51 pages a worker (its
/// 64 KiB stack, its thread block and its searcher's buffers), and at 256 the fourth spawn was
/// refused for want of a page for its TCB.
const RG_HEAP_PAGES: u64 = 512;

/// **Run `rg` as a process over the priced tree, told `line`**, holding `rights` through a
/// caretaker, and hand back its transcript's length, how many threads its process ever started,
/// and how many heap pages it spent. The process is `std_threads_tests::run_as_process_with`'s,
/// with what a walk adds: the caretaker's endpoint at the filesystem slot, the file page it shares
/// mapped where the PAL reads it, and the argument page. Everything comes back before this
/// returns. `None` when this boot has no file service.
fn run_rg_as_process(rights: u64, line: &str, out: &mut [u8]) -> Option<(usize, u32, u64)> {
    use filesystem_fixture::walk as tree;

    use super::std_threads_tests::{Setup, run_as_process_with};
    let image = program("rg")?;
    let (dir_ep, file_page, report, caretaker) = fs_service::narrow_dir_held(
        program("block_driver")?,
        fs_service::fs_server_image()?,
        program("fs_subtree_caretaker")?,
        tree::ROOT,
        rights,
    )?;
    // The argument page: `line` as the shell would send it, read-only at `ARGS_PAGE`.
    let args_region = crate::memory_region::create(1).expect("no argument page region");
    let args = crate::memory_region::retype_page(args_region).expect("no argument page");
    {
        // SAFETY: a frame just retyped for this page alone, direct-mapped, which nothing else
        // holds until the program is given it read-only.
        let page = unsafe { &mut *(mmu::phys_to_virt(args) as *mut [u8; FRAME_SIZE as usize]) };
        page.fill(0);
        grant_plan::argv(line.as_bytes(), page).expect("the test's line is not an argv");
    }
    let ran = run_as_process_with(
        image,
        "rg",
        out,
        &Setup {
            maps: &[
                (std_runtime_contract::FS_PAGE, file_page, true),
                (std_runtime_contract::ARGS_PAGE, args, false),
            ],
            endpoints: &[(std_runtime_contract::FS_DIR_SLOT, dir_ep)],
            read_pages: &[(std_runtime_contract::ARGS_SLOT, args)],
            report: Some(report),
            heap_pages: RG_HEAP_PAGES,
        },
    );
    assert!(
        super::wait_for(|| crate::sched::reclaim_region(args_region).is_ok()),
        "rg's argument page did not come back"
    );
    assert!(
        caretaker.release(),
        "rg's caretaker outlived its holding: a service this test cannot give back",
    );
    Some((ran.len, ran.started, ran.heap_spent))
}

/// **`text` is exactly the fixture's match lines, each once, then `rg`'s stats.** Order is not
/// checked: the parallel walker prints each file as its worker finishes it.
fn assert_the_fixture_lines(text: &str, line: &str) {
    use filesystem_fixture::walk as tree;
    let body = core::str::from_utf8(tree::SMALL_BODY)
        .expect("ASCII")
        .trim_end();
    let (matches, stats) = text
        .split_once("\n\n")
        .unwrap_or_else(|| panic!("`{line}` printed no stats block: {text}"));
    let mut seen = [false; tree::RG_MATCHES];
    for l in matches.lines() {
        let i = (0..tree::RG_MATCHES)
            .find(|&i| {
                let mut buf = [0u8; 32];
                let len = tree::rg_match_path(i, &mut buf);
                l.strip_suffix(body)
                    .and_then(|p| p.strip_suffix(':'))
                    .is_some_and(|p| p.as_bytes() == &buf[..len])
            })
            .unwrap_or_else(|| panic!("`{line}` printed a line the tree does not hold: {l:?}"));
        assert!(!seen[i], "`{line}` printed {l:?} twice");
        seen[i] = true;
    }
    assert!(
        seen.iter().all(|&s| s),
        "`{line}` missed {} of the tree's {} matching lines",
        seen.iter().filter(|&&s| !s).count(),
        tree::RG_MATCHES,
    );
    let mut searched = Line {
        buf: [0; 64],
        len: 0,
    };
    {
        use core::fmt::Write;
        write!(searched, "\n{} files searched\n", tree::WALK_FILES).expect("fits");
    }
    assert!(
        stats.contains(searched.as_str()),
        "`{line}` did not search every file: {stats}",
    );
}

/// A line of at most 64 bytes, without an allocator.
struct Line {
    buf: [u8; 64],
    len: usize,
}

impl core::fmt::Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();
        self.buf
            .get_mut(self.len..end)
            .ok_or(core::fmt::Error)?
            .copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

impl Line {
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).expect("ASCII")
    }
}

/// One threaded run and its verdict: the lines, and `1 + workers` threads started.
fn threaded_run(line: &str, workers: usize, before_counting: impl FnOnce()) {
    use filesystem_contract::dir;
    let mut got = [0u8; 8192];
    let Some((len, started, spent)) =
        run_rg_as_process(dir::ENUMERATE | dir::READ | dir::DESCEND, line, &mut got)
    else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    let text = core::str::from_utf8(&got[..len]).unwrap_or("<not utf-8>");
    crate::println!(
        "    `{line}`: {started} threads started ({workers} workers and the main thread), \
         {len} bytes, {spent} heap pages"
    );
    assert_the_fixture_lines(text, line);
    before_counting();
    assert_eq!(
        started as usize,
        1 + workers,
        "`{line}` started {started} threads, not its main thread and {workers} workers"
    );
}

/// Skip when this archive has no `rg` or this boot no file service.
macro_rules! skip_without_rg {
    () => {
        if program("rg").is_none() {
            // CI's rows fetch `rg` (milestone 810 (`ripgrep` is packaged in basalt and installed
            // with `jig`)) and say so at build time; there, a missing `rg` is a broken fetch and
            // must not read as a skip.
            assert!(
                env!("NIFE_RIPGREP_REQUIRED") != "1",
                "no rg in this archive, and NIFE_RIPGREP_REQUIRED=1: `cargo xtask basalt-fetch` \
                 ran and its rg did not reach the archive"
            );
            crate::testing::skip!(
                "no rg in this archive: `cargo xtask basalt-fetch` fetches it (milestone 810)"
            );
        }
        if fs_service::fs_server_image().is_none() {
            crate::testing::skip!(fs_service::NO_FS_SERVER);
        }
    };
}

/// **`rg` with no thread flag walks on one thread per online core.** It asks
/// `available_parallelism`, nife answers the online count, and `ignore`'s parallel walker starts
/// that many workers (`ripgrep` caps it at 12). The kernel counts every thread the process started,
/// and the count must be the main thread plus one per core. Booted at one core this has nothing to
/// prove, so it fails rather than pass on a serial walk: CI boots four.
///
/// Falsification: unfalsified. A replay needs `rg` in the archive, which only the `rg-threads`
/// rows build.
#[test_case]
fn ripgrep_walks_on_every_online_core_by_default() {
    use filesystem_fixture::walk as tree;
    skip_without_rg!();
    let online = crate::smp::online_harts_mask().count_ones() as usize;
    // Asked after the run, so a boot with no disk (x86_64's one-core OVMF leg) skips first.
    threaded_run(tree::RG_SEARCH_THREADED, online.min(12), || {
        assert!(
            online > 1,
            "booted with {online} core: rg's default is one thread here, and this test is about more"
        );
    });
}

/// **`rg --threads 4` starts four workers**, whatever the core count, and they find exactly what
/// the serial walk finds.
///
/// Falsification: unfalsified, for [`ripgrep_walks_on_every_online_core_by_default`]'s reason.
#[test_case]
fn ripgrep_told_four_threads_starts_four_workers() {
    use filesystem_fixture::walk as tree;
    skip_without_rg!();
    threaded_run(tree::RG_SEARCH_FOUR_THREADS, tree::RG_FOUR, || {});
}
