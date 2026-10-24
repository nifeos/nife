//! **Many boots in a log, and what the placement lottery drew each time.**
//!
//! [`progress`](crate::progress) reads one boot; [`tally`] reads a capture of many and reports how
//! often each arrangement came up, with the rate beside every draw. notes/soak/rebooting-soak.md
//! says why the distribution is wanted, and notes/soak/boot-lottery.md holds the draws so far.
//!
//! A core is **clean** when it carries at least one responder or caller and no grinder: a grinder
//! starves the IPC threads beside it, and a core with no IPC thread is idle to the round-trip rate
//! (notes/soak/radon-2026-09-03.md). A draw is judged on the **last** census it printed, because
//! the spawn census is the lottery's ticket and the settled one is what the machine ran.

//! # BUGS
//!
//! - **A draw with no census is still a draw** and reports `clean=?` (a kernel without the census,
//!   or a boot cut off before its first). It is counted in the series and kept out of the
//!   distribution, so forty good boots and ten wedged ones do not read as forty boots.
//! - **A boot that never reaches `soak-test: started` is only an attempt**, so `attempts` above
//!   `draws` is the one sign of a series that wedged unattended.
//! - **The attempt count is U-Boot SPL's banner**: boots of the board, and zero on a QEMU capture.
//! - **Nothing reads the rate as a function of anything.** It prints the pairs.

use core::fmt::Write as _;

/// How one draw ended, which is the difference between a series that ran and one that stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ending {
    /// The kernel announced the reset. The next draw in the log is the boot it produced.
    Rebooted,
    /// Somebody typed on the console and the loop disarmed itself. A deliberate end to a series.
    Disarmed,
    /// The firmware refused the cold reboot. **This is the one that means the mechanism is not
    /// available on this board**, and it is why the kernel prints `sbiret.error` rather than
    /// halting quietly.
    Refused,
    /// The soak failed and the kernel panicked, carrying the reason. A series ends here on purpose:
    /// the failure is the result, and rebooting over it would destroy the evidence.
    Failed(String),
    /// The log ended during this draw. The last draw of a healthy capture is always this one; an
    /// earlier one is a board that stopped speaking.
    Truncated,
}

/// One boot's draw: the arrangement it settled into, the rate it ran at, and how it ended.
#[derive(Debug, Clone)]
pub struct Draw {
    /// Cores holding an IPC thread and no grinder, from the last census this boot printed. `None`
    /// when the boot printed no census.
    pub clean_cpus: Option<usize>,
    /// Cores the last census listed, which is the online count. `None` with no census.
    pub cpus: Option<usize>,
    /// The `rate=` of the last heartbeat, in round trips per second.
    pub rate: Option<u64>,
    /// The `beat=` of the last heartbeat, so a truncated draw is visible as a short one.
    pub beats: u64,
    /// How it ended.
    pub ending: Ending,
}

/// Every draw in one capture, plus what the log says about boots that produced none.
#[derive(Debug, Clone, Default)]
pub struct Series {
    /// U-Boot SPL banners seen: boots of the board, whether or not they reached the workload.
    pub attempts: usize,
    /// One per `soak-test: started`, in the order they were captured.
    pub draws: Vec<Draw>,
}

/// **Read a capture into a series of draws.**
///
/// Takes the whole log as text rather than a reader, because the callers are a test with a fixture
/// and a subcommand handed a file that has already finished being written. A live watch is
/// [`watch`](crate::watch)'s job and is a different shape.
pub fn tally(log: &str) -> Series {
    let mut series = Series::default();
    // The census being accumulated: one entry per `core=` line since the last census header.
    let mut census: Vec<CpuLine> = Vec::new();
    // The last complete census this draw printed, which is the one it is judged on.
    let mut settled: Option<Vec<CpuLine>> = None;

    for raw in log.lines() {
        let line = raw.trim_end_matches(['\r', '\n']);

        if line.contains("U-Boot SPL") {
            series.attempts += 1;
        }

        if line.contains("soak-test: started") {
            // Close the previous draw before opening this one. A draw that reached here without an
            // ending was cut off by whatever produced this boot, which from inside the log is
            // indistinguishable from a truncation, and is reported as one.
            finish(&mut series, &mut settled, &mut census);
            series.draws.push(Draw {
                clean_cpus: None,
                cpus: None,
                rate: None,
                beats: 0,
                ending: Ending::Truncated,
            });
            continue;
        }

        let Some(draw) = series.draws.last_mut() else {
            // Anything before the first `soak-test: started` is a boot that has not announced a workload
            // yet: U-Boot, the tour, or a boot that failed before either.
            continue;
        };

        // A census header opens a fresh block. Matching on the legend rather than on the sentence
        // in front of it, because the sentence differs between the spawn census, the re-census and
        // the failure census, and the legend is the same words in all three.
        if line.contains("R=responder") {
            if !census.is_empty() {
                settled = Some(core::mem::take(&mut census));
            }
            census.clear();
            continue;
        }
        if let Some(parsed) = parse_cpu_line(line) {
            census.push(parsed);
            continue;
        }

        if line.contains("soak-test: t=") {
            if let Some(beat) = field(line, "beat=") {
                draw.beats = beat;
            }
            if let Some(rate) = field(line, "rate=") {
                draw.rate = Some(rate);
            }
            continue;
        }

        // The endings are ranked, not first-seen: `rebooting now` is printed before the `ecall`,
        // since the UART stops draining once a reset starts, so a refusal always follows it. A
        // finding outranks the loop working.
        if let Some(at) = line.find("soak-test: FAILED") {
            draw.ending = Ending::Failed(line[at..].to_string());
        } else if line.contains("soak-test-reboot: FAILED") {
            draw.ending = Ending::Refused;
        } else if draw.ending == Ending::Truncated {
            if line.contains("soak-test-reboot: DISARMED") {
                draw.ending = Ending::Disarmed;
            } else if line.contains("soak-test-reboot: rebooting now") {
                draw.ending = Ending::Rebooted;
            }
        }
    }
    finish(&mut series, &mut settled, &mut census);
    series
}

/// One `soak-test-census: core=N threads=M ...` line, reduced to the two questions asked of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CpuLine {
    grinder: bool,
    ipc: bool,
}

/// Attach the census in hand to the draw in hand, and clear both for the next boot.
fn finish(series: &mut Series, settled: &mut Option<Vec<CpuLine>>, census: &mut Vec<CpuLine>) {
    // The block still being accumulated when the draw ended is the last one printed, so it wins
    // over the one before it. This is what makes a re-census the judgement rather than the spawn
    // census, and it falls out of the ordering instead of needing a rule.
    if !census.is_empty() {
        *settled = Some(core::mem::take(census));
    }
    if let (Some(draw), Some(block)) = (series.draws.last_mut(), settled.take()) {
        draw.cpus = Some(block.len());
        draw.clean_cpus = Some(block.iter().filter(|c| c.ipc && !c.grinder).count());
    }
    census.clear();
    *settled = None;
}

/// Parse `soak-test-census: core=1 threads=5 C0 W1 C2 R3 W3` into the two facts the count needs.
///
/// Returns `None` for every other `soak-test-census:` line, of which there are several: the legend, the
/// three explanatory sentences, and the `unplaced=` line. Keyed on `core=` and `threads=` together
/// rather than on either alone, so a sentence that happens to contain one of the words is not
/// mistaken for a core.
fn parse_cpu_line(line: &str) -> Option<CpuLine> {
    if !line.contains("soak-test-census:") {
        return None;
    }
    let at = line.find(" threads=")?;
    line.find("core=")?;
    let rest = &line[at + " threads=".len()..];
    let tokens = rest.split_whitespace().skip(1);
    let mut grinder = false;
    let mut ipc = false;
    for token in tokens {
        // A token is a role letter and a group number. Anything else on the line is not a worker,
        // and a token whose tail is not a number is not one either.
        let mut chars = token.chars();
        let role = chars.next()?;
        if !chars.as_str().chars().all(|c| c.is_ascii_digit()) || chars.as_str().is_empty() {
            continue;
        }
        match role {
            'G' => grinder = true,
            'R' | 'C' => ipc = true,
            _ => {}
        }
    }
    Some(CpuLine { grinder, ipc })
}

/// Read `name=<digits>` out of a heartbeat line.
fn field(line: &str, name: &str) -> Option<u64> {
    let at = line.find(name)?;
    let rest = &line[at + name.len()..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

impl Series {
    /// **The distribution, and the two lines of arithmetic that are the milestone's whole result.**
    ///
    /// Plain text on purpose: this goes into notes/soak.md beside the nine hand-drawn boots, and a
    /// table a person can paste is worth more than a format a program would have to re-read.
    pub fn report(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "boot lottery: {} draw{} from {} board boot{} (SPL banners)",
            self.draws.len(),
            plural(self.draws.len()),
            self.attempts,
            plural(self.attempts),
        );
        if self.attempts > self.draws.len() {
            let _ = writeln!(
                out,
                "  {} boot(s) never reached the workload: read the log around them, because an \
                 unattended board can fail unattended",
                self.attempts - self.draws.len()
            );
        }
        let _ = writeln!(out);
        let _ = writeln!(out, "  draw  clean/cores  rate      beats  ended");
        for (i, d) in self.draws.iter().enumerate() {
            let clean = match (d.clean_cpus, d.cpus) {
                (Some(c), Some(n)) => format!("{c}/{n}"),
                _ => "?".to_string(),
            };
            let rate = d.rate.map_or("?".to_string(), |r| format!("{r}/s"));
            let _ = writeln!(
                out,
                "  {:>4}  {:>11}  {:>8}  {:>5}  {}",
                i + 1,
                clean,
                rate,
                d.beats,
                describe(&d.ending)
            );
        }

        let judged: Vec<&Draw> = self
            .draws
            .iter()
            .filter(|d| d.clean_cpus.is_some())
            .collect();
        let widest = judged
            .iter()
            .filter_map(|d| d.cpus)
            .max()
            .unwrap_or_default();
        let _ = writeln!(out);
        // No header when there is nothing under it. A table of zeroes reads as a measurement of
        // zero, and what it actually means is that no boot in this log printed a census.
        if !judged.is_empty() {
            let _ = writeln!(out, "  clean cores  draws  rates seen");
        }
        for clean in 0..=widest {
            let here: Vec<&&Draw> = judged
                .iter()
                .filter(|d| d.clean_cpus == Some(clean))
                .collect();
            let rates: Vec<u64> = here.iter().filter_map(|d| d.rate).collect();
            let span = match (rates.iter().min(), rates.iter().max()) {
                (Some(lo), Some(hi)) if lo == hi => format!("{lo}/s"),
                (Some(lo), Some(hi)) => format!("{lo}-{hi}/s"),
                _ => "-".to_string(),
            };
            let _ = writeln!(out, "  {clean:>11}  {:>5}  {span}", here.len());
        }
        if judged.len() < self.draws.len() {
            let _ = writeln!(
                out,
                "  ({} draw(s) printed no census and are excluded from the distribution)",
                self.draws.len() - judged.len()
            );
        }
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "  A count of draws is not a probability. It is this board, this firmware and this \
             build, and DECISIONS 138 is where the mechanism lives; see notes/soak.md."
        );
        out
    }
}

/// The `s` in "3 draws". One draw is not "1 draws", and a report a person pastes into a note is
/// read by people.
fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn describe(ending: &Ending) -> String {
    match ending {
        Ending::Rebooted => "rebooted".to_string(),
        Ending::Disarmed => "DISARMED (a key was pressed; the series ends here)".to_string(),
        Ending::Refused => {
            "REFUSED (the firmware does not do SRST reset type 1; see the log)".to_string()
        }
        Ending::Failed(why) => format!("FAILED: {why}"),
        Ending::Truncated => "log ends here".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The clean-core count, against the arrangement radon actually printed.**
    ///
    /// This census is quoted from notes/soak.md's record of the 17:06 run's settled arrangement,
    /// which is the only one this project has off a board. The placement is that run's, verbatim;
    /// the marker prefix is today's, because this is a parser input rather than a record and a
    /// board printed `soak-census:` before milestone 297 renamed it. Three of the four grinders are on core
    /// 3 and core 4 holds no grinder at all, so the count is: core 1 clean, core 2 has a grinder,
    /// core 3 has grinders, core 4 has callers and a responder and no grinder. Two clean cores,
    /// which is what a 188,687/s run is expected to look like.
    #[test]
    fn the_settled_arrangement_off_radon_counts_two_clean_cpus() {
        let log = concat!(
            "soak-test: started 4 groups\n",
            "soak-test-census: where the workers are NOW: R=responder, C=caller, G=grinder, W=tick waiter\n",
            "soak-test-census: core=1 threads=7 C0 C1 W1 R2 C3 C3 W3\n",
            "soak-test-census: core=2 threads=7 R0 C0 R1 C1 C2 C2 G3\n",
            "soak-test-census: core=3 threads=6 C0 G0 C1 G1 G2 C3\n",
            "soak-test-census: core=4 threads=4 W0 C2 W2 R3\n",
            "soak-test: t=5s beat=1 rounds=943435 rate=188687/s workers=24\n",
        );
        let series = tally(log);
        assert_eq!(series.draws.len(), 1);
        assert_eq!(series.draws[0].cpus, Some(4));
        assert_eq!(series.draws[0].clean_cpus, Some(2));
        assert_eq!(series.draws[0].rate, Some(188_687));
        assert_eq!(series.draws[0].ending, Ending::Truncated);
    }

    /// **A core holding only waiters is not clean.** It carries no grinder, and counting it would
    /// report three clean cores on an arrangement whose IPC lives on two.
    #[test]
    fn a_cpu_with_only_tick_waiters_is_not_clean() {
        let log = concat!(
            "soak-test: started\n",
            "soak-test-census: R=responder, C=caller, G=grinder, W=tick waiter\n",
            "soak-test-census: core=0 threads=2 W0 W1\n",
            "soak-test-census: core=1 threads=3 R0 C0 C0\n",
            "soak-test-census: core=2 threads=1 G0\n",
        );
        let series = tally(log);
        assert_eq!(series.draws[0].clean_cpus, Some(1));
        assert_eq!(series.draws[0].cpus, Some(3));
    }

    /// **The last census wins, because the spawn one is the ticket and not the result.**
    ///
    /// The spawn block here has two clean cores and the re-census has none. notes/soak.md records
    /// that on radon the arrangement converges once, about twenty-five seconds in, and then holds;
    /// judging on the spawn block would misattribute every run.
    #[test]
    fn a_re_census_replaces_the_spawn_census() {
        let log = concat!(
            "soak-test: started\n",
            "soak-test-census: where the kernel placed each worker at spawn: R=responder, C=caller, G=grinder, W=tick waiter\n",
            "soak-test-census: core=0 threads=2 R0 C0\n",
            "soak-test-census: core=1 threads=2 R1 C1\n",
            "soak-test: t=5s beat=1 rate=300000/s drifted=4\n",
            "soak-test-census: where the workers are NOW: R=responder, C=caller, G=grinder, W=tick waiter\n",
            "soak-test-census: core=0 threads=3 R0 C0 G0\n",
            "soak-test-census: core=1 threads=3 R1 C1 G1\n",
            "soak-test: t=10s beat=2 rate=23000/s drifted=0\n",
        );
        let series = tally(log);
        assert_eq!(series.draws[0].clean_cpus, Some(0));
        assert_eq!(series.draws[0].rate, Some(23_000));
        assert_eq!(series.draws[0].beats, 2);
    }

    /// **Three boots in one capture, each ending its own way**, which is the shape of the log a
    /// bench run produces and the thing no single-boot reader can report.
    #[test]
    fn a_series_separates_boots_and_records_how_each_ended() {
        let log = concat!(
            "U-Boot SPL 2021.10\n",
            "soak-test: started\n",
            "soak-test-census: R=responder, C=caller, G=grinder, W=tick waiter\n",
            "soak-test-census: core=0 threads=2 R0 C0\n",
            "soak-test-census: core=1 threads=1 G0\n",
            "soak-test: t=120s beat=24 rate=342000/s\n",
            "soak-test-reboot: rebooting now (SBI SRST system_reset, reset type 1, cold reboot).\n",
            "U-Boot SPL 2021.10\n",
            "soak-test: started\n",
            "soak-test-census: R=responder, C=caller, G=grinder, W=tick waiter\n",
            "soak-test-census: core=0 threads=3 R0 C0 G0\n",
            "soak-test-census: core=1 threads=1 C1\n",
            "soak-test: t=120s beat=24 rate=184000/s\n",
            "soak-test-reboot: rebooting now (SBI SRST system_reset, reset type 1, cold reboot).\n",
            "U-Boot SPL 2021.10\n",
            "soak-test: started\n",
            "soak-test-census: R=responder, C=caller, G=grinder, W=tick waiter\n",
            "soak-test-census: core=0 threads=2 R0 C0\n",
            "soak-test-census: core=1 threads=2 R1 C1\n",
            "soak-test: t=15s beat=3 rate=347000/s\n",
            "soak-test-reboot: DISARMED at t=15s: a byte arrived on this console.\n",
        );
        let series = tally(log);
        assert_eq!(series.attempts, 3);
        assert_eq!(series.draws.len(), 3);
        assert_eq!(series.draws[0].clean_cpus, Some(1));
        assert_eq!(series.draws[0].ending, Ending::Rebooted);
        assert_eq!(series.draws[1].clean_cpus, Some(1));
        assert_eq!(series.draws[2].clean_cpus, Some(2));
        assert_eq!(series.draws[2].ending, Ending::Disarmed);

        let report = series.report();
        assert!(report.contains("3 draws from 3 board boots"));
        assert!(report.contains("DISARMED"));
        // Two draws at one clean core, spanning the two rates; one at two.
        assert!(report.contains("184000-342000/s"), "{report}");
    }

    /// **A boot that never announced a workload is counted as an attempt and nothing else**, which
    /// is the only way a wedge at 2am shows up in a tally at all.
    #[test]
    fn a_boot_that_never_soaked_is_visible_as_a_missing_draw() {
        let log = concat!(
            "U-Boot SPL 2021.10\n",
            "soak-test: started\n",
            "soak-test-census: R=responder, C=caller, G=grinder, W=tick waiter\n",
            "soak-test-census: core=0 threads=2 R0 C0\n",
            "soak-test: t=120s beat=24 rate=342000/s\n",
            "soak-test-reboot: rebooting now\n",
            "U-Boot SPL 2021.10\n",
            "### ERROR ### Please RESET the board ###\n",
        );
        let series = tally(log);
        assert_eq!(series.attempts, 2);
        assert_eq!(series.draws.len(), 1);
        assert!(
            series
                .report()
                .contains("1 boot(s) never reached the workload")
        );
    }

    /// **A firmware that refuses the reset ends the series and says which**, which is the outcome
    /// this milestone could not check without the board and the reason the kernel prints
    /// `sbiret.error` rather than halting quietly.
    #[test]
    fn a_refused_reset_is_its_own_ending() {
        let log = concat!(
            "soak-test: started\n",
            "soak-test-reboot: rebooting now (SBI SRST system_reset, reset type 1, cold reboot).\n",
            "soak-test-reboot: FAILED: the firmware refused a cold reboot and returned sbiret.error=-2\n",
        );
        let series = tally(log);
        // `rebooting now` is printed first and the refusal follows it, so the first ending seen is
        // the wrong one. The refusal is the finding, so it must win.
        assert_eq!(series.draws[0].ending, Ending::Refused);
    }

    /// **A draw with no census is counted and not judged**, so a log of good boots and truncated
    /// ones does not read as a log of good boots.
    #[test]
    fn a_draw_with_no_census_is_excluded_from_the_distribution_and_not_from_the_series() {
        let log = concat!(
            "soak-test: started\n",
            "soak-test: t=5s beat=1 rate=100/s\n",
            "soak-test-reboot: rebooting now\n",
            "soak-test: started\n",
            "soak-test-census: R=responder, C=caller, G=grinder, W=tick waiter\n",
            "soak-test-census: core=0 threads=2 R0 C0\n",
            "soak-test: t=5s beat=1 rate=200/s\n",
        );
        let series = tally(log);
        assert_eq!(series.draws.len(), 2);
        assert_eq!(series.draws[0].clean_cpus, None);
        assert_eq!(series.draws[1].clean_cpus, Some(1));
        assert!(
            series
                .report()
                .contains("1 draw(s) printed no census and are excluded")
        );
    }

    /// **The real QEMU capture parses**, so the recogniser is exercised against text a machine
    /// printed rather than only against text this test wrote. That capture predates the census, so
    /// its draw is unjudged, which is itself the case the bug above describes.
    #[test]
    fn the_captured_qemu_soak_reads_as_one_unjudged_draw() {
        // A pre-297 capture, respelled at read time and not on disk; see
        // `crate::respell_pre_297_markers`.
        let log = crate::respell_pre_297_markers(include_str!(
            "../tests/fixtures/captured/qemu-2026-09-01-riscv64-soak.log"
        ));
        let log = log.as_str();
        let series = tally(log);
        assert_eq!(series.draws.len(), 1, "one boot, one draw");
        assert!(series.draws[0].rate.is_some(), "its beats were read");
        assert_eq!(series.draws[0].ending, Ending::Truncated);
    }

    /// **A capture with a real census in it**, so the clean-core count is not proved solely against
    /// text this file wrote.
    ///
    /// `script/soak --arch riscv64 --for 30s` on patagonia, 2026-09-03, under the command's name at
    /// the time. Three of the four grinders
    /// land on one core and a fourth shares with the last group, so the settled arrangement has one
    /// clean core; the rate at the last beat is 18,963/s, which is the low end of the spread radon
    /// shows. The assertion is on the count rather than on the rate, because the rate is a property
    /// of a busy laptop and the count is a property of the log.
    #[test]
    fn the_captured_census_run_is_judged_and_reads_one_clean_cpu() {
        // A pre-297 capture, respelled at read time and not on disk; see
        // `crate::respell_pre_297_markers`.
        let log = crate::respell_pre_297_markers(include_str!(
            "../tests/fixtures/captured/qemu-2026-09-03-riscv64-soak-census.log"
        ));
        let log = log.as_str();
        let series = tally(log);
        assert_eq!(series.draws.len(), 1);
        assert_eq!(series.draws[0].cpus, Some(4), "four online cores");
        assert_eq!(series.draws[0].clean_cpus, Some(1));
        assert_eq!(series.draws[0].rate, Some(18_963));
        assert!(series.report().contains("1/4"), "{}", series.report());
    }

    /// **The same judgement against a real census in the marker vocabulary the kernel prints
    /// today** (milestone 297). `script/soak-test --arch riscv64 --for 30s`, taken the day the
    /// rename landed, unedited. The spawn lottery and the settled arrangement differ in this one,
    /// which the pre-221 capture above cannot show, so it is also the only real sample of a census
    /// block being *replaced*.
    #[test]
    fn the_renamed_census_marker_is_tallied_from_a_real_run() {
        let log = include_str!("../tests/fixtures/captured/qemu-2026-09-14-riscv64-soak-test.log");
        let series = tally(log);
        assert_eq!(series.draws.len(), 1, "one boot, one draw");
        assert_eq!(series.draws[0].cpus, Some(4), "four online cores");
        assert!(
            series.draws[0].clean_cpus.is_some(),
            "a census block was read: {}",
            series.report()
        );
        assert_eq!(series.draws[0].ending, Ending::Truncated);
    }

    /// **`parse_cpu_line` on its own**, rather than only through `tally`, because a shifted
    /// slice start can still land on the real role token by coincidence: the offset error this
    /// catches only shows up when the bytes it wrongly includes look like a *second* worker.
    #[test]
    fn parse_cpu_line_reads_the_tokens_after_threads_and_nothing_before_it() {
        // `threads=1 R2` is the real content; `core=0 G9` sits before it on purpose, so that an
        // off-by-nine slice start (the `+`-to-`-` mutant) would land inside `G9` and misread it
        // as a second worker, a grinder this time, on top of the responder that is actually there.
        assert_eq!(
            parse_cpu_line("soak-test-census: core=0 G9 threads=1 R2"),
            Some(CpuLine {
                grinder: false,
                ipc: true
            })
        );
    }

    /// A token whose tail is not a role's group number must be skipped, not guessed at. This is
    /// the one distinguishing case between "skip when malformed or empty" (`||`) and "skip only
    /// when malformed and empty at once" (`&&`, which nothing can ever satisfy: an empty tail
    /// always reads as `all(digit)` vacuously true, so the `&&` guard never fires and every
    /// token, however garbled, is read as a worker).
    #[test]
    fn a_token_whose_tail_is_not_a_group_number_is_not_a_worker() {
        assert_eq!(
            parse_cpu_line("soak-test-census: core=0 threads=1 Grinder R2"),
            Some(CpuLine {
                grinder: false,
                ipc: true
            }),
            "'Grinder' starts with G but is not a group number and must not set grinder"
        );
    }

    /// **A census legend with no core lines under it before the next boot still settles the
    /// block that came before it.** `finish()`'s own "still accumulating" fallback only saves
    /// the day when a boot ends *mid*-census; this is the other place `settled` is assigned, and
    /// nothing else exercises it in isolation.
    #[test]
    fn a_second_legend_with_nothing_under_it_still_settles_the_first_block() {
        let log = concat!(
            "soak-test: started\n",
            "soak-test-census: R=responder, C=caller, G=grinder, W=tick waiter\n",
            "soak-test-census: core=0 threads=2 R0 C0\n",
            // The re-census legend arrives, but no core line follows it before the next boot:
            // `settled` has to come from the census caught above the legend line, not from
            // whatever `finish()` finds in `census` (which is empty here).
            "soak-test-census: R=responder, C=caller, G=grinder, W=tick waiter\n",
            "soak-test: started\n",
        );
        let series = tally(log);
        assert_eq!(series.draws[0].cpus, Some(1));
        assert_eq!(series.draws[0].clean_cpus, Some(1));
    }

    /// A series with exactly as many attempts as draws has nothing that failed to reach the
    /// workload, and the report must not say otherwise.
    #[test]
    fn a_series_with_no_missing_draws_says_nothing_about_missing_ones() {
        let series = Series {
            attempts: 2,
            draws: vec![
                Draw {
                    clean_cpus: Some(1),
                    cpus: Some(4),
                    rate: Some(100),
                    beats: 3,
                    ending: Ending::Rebooted,
                },
                Draw {
                    clean_cpus: Some(1),
                    cpus: Some(4),
                    rate: Some(100),
                    beats: 3,
                    ending: Ending::Truncated,
                },
            ],
        };
        assert!(!series.report().contains("never reached the workload"));
    }

    /// The distribution table's header and its per-row draw count and span, checked against a
    /// series built directly rather than through `tally`, so each of the report's three moving
    /// parts (whether the header prints at all, which row a draw lands in, and whether a single
    /// rate renders as itself rather than as a range) can be pinned down exactly.
    #[test]
    fn the_distribution_table_puts_each_draw_in_its_own_row_and_no_other() {
        let series = Series {
            attempts: 2,
            draws: vec![
                Draw {
                    clean_cpus: Some(0),
                    cpus: Some(4),
                    rate: Some(100),
                    beats: 1,
                    ending: Ending::Truncated,
                },
                Draw {
                    clean_cpus: Some(2),
                    cpus: Some(4),
                    rate: Some(300),
                    beats: 1,
                    ending: Ending::Truncated,
                },
            ],
        };
        let report = series.report();
        assert!(
            report.contains("clean cores  draws  rates seen"),
            "a non-empty distribution must print its header:\n{report}"
        );
        // clean=1 has no draws at all: swapping `==` for `!=` when filtering each row would put
        // *other* rows' draws here instead of leaving it empty.
        assert!(
            report.contains(&format!("  {:>11}  {:>5}  -", 1, 0)),
            "clean=1 must show zero draws with no rate span:\n{report}"
        );
        // A single draw's rate must render as itself, not as a degenerate one-point range: that
        // is the `lo == hi` guard, and disabling it always takes the two-value arm.
        assert!(
            !report.contains("300-300/s"),
            "a single rate must not render as a range:\n{report}"
        );
        assert!(report.contains("300/s"));
    }

    /// When every draw carries a census, nothing was excluded from the distribution, and the
    /// report must not claim otherwise.
    #[test]
    fn a_fully_judged_series_reports_no_exclusions() {
        let series = Series {
            attempts: 1,
            draws: vec![Draw {
                clean_cpus: Some(1),
                cpus: Some(4),
                rate: Some(100),
                beats: 1,
                ending: Ending::Truncated,
            }],
        };
        assert!(!series.report().contains("excluded from the distribution"));
    }
}
