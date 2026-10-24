//! `cargo xtask bench [--riscv|--x86] --restamp`: carry an icount floor across a nightly bump
//! without re-saving it, when an A/B on this machine proves the compiler did not move it.
//!
//! It builds and runs the bench leg twice on one machine and tree, under the nightly the floor's
//! `# toolchain:` line names and then under the pin in `rust-toolchain.toml`; icount is
//! deterministic per binary, so any difference is the compiler's. If every row moved less than
//! [`ROW_BOUND_PCT`] and every row's cumulative move since the last `--save` stays under
//! [`CUMULATIVE_BOUND_PCT`], it rewrites `# toolchain:` to the pin and adds one `# why:` line.
//! **It never writes a floor number.** Otherwise it changes nothing and exits non-zero, and the
//! table it prints (and writes to `--report <file>`) is where a person starts.

//! # The ruling
//!
//! calef, 2026-09-26 UTC, on #1336: option 4 of milestone 598 (a nightly bump restamps the floors
//! it proves it did not move), with both bounds as written here. A floor number still moves only by
//! a person's `--save`, so a slower nightly spends tripwire headroom against numbers somebody read,
//! and the cap stops a run of small moves being absorbed unseen. `# toolchain:` means "the nightly
//! these numbers were last proven valid for", not "the nightly that produced them".
//!
//! `--restamp` is a provisional name (this lane's), as is this module's.

//! # BUGS
//!
//! - The A/B covers what the bench leg builds with the pinned toolchain. The std programs built
//!   through the farm are packed identically into both runs, so a compiler term there is invisible;
//!   no bench row is known to depend on one.
//! - The term is measured on one machine (CI's `x86_64` runner, in the bump workflow) and applied
//!   to floors recorded on another (patagonia); that the ratio between nightlies travels is a
//!   claim, not a measurement.
//! - The cumulative term is read back from the last `# why:` line, rounded to three decimals of a
//!   percent, which across fifty restamps is far below the 2% cap.

use crate::bench::{pinned_nightly, take_last_run, today_utc};
use crate::host::{flag_value, workspace_root};

/// No row may move more than this, in percent, between the stamped nightly and the pin.
/// calef's number, 2026-09-26. #1335's largest compiler term was 0.21% (riscv64 `ipc_rtt`).
pub(crate) const ROW_BOUND_PCT: f64 = 0.5;

/// No row's compounded move across restamps since the last `--save` may reach this, in percent.
/// calef's number, 2026-09-26: a fifth of the 10% tripwire.
pub(crate) const CUMULATIVE_BOUND_PCT: f64 = 2.0;

/// The marker the cumulative ledger follows on a restamp's `# why:` line. Read back by
/// [`prior_cumulative`], so changing it orphans every restamp already committed.
const CUMULATIVE_MARKER: &str = "cumulative since the last --save:";

type Row = (String, u64, u64);

/// Run the restamp for the leg the command line names. `leg` builds and runs one bench pass and
/// returns whether it succeeded; its rows are read back through [`take_last_run`].
pub(crate) fn restamp(leg: impl Fn() -> bool) -> bool {
    let file = if std::env::args().any(|a| a == "--riscv") {
        "bench/baseline-riscv64.txt"
    } else if std::env::args().any(|a| a == "--x86") {
        "bench/baseline-x86_64.txt"
    } else {
        "bench/baseline-aarch64.txt"
    };
    let path = workspace_root().join(file);
    let report_path = flag_value("--report");
    let report = |body: &str| {
        eprintln!("{body}");
        if let Some(p) = &report_path
            && let Err(e) = std::fs::write(p, body)
        {
            eprintln!("bench: cannot write the report to {p}: {e}");
        }
    };

    let Ok(text) = std::fs::read_to_string(&path) else {
        report(&format!("**{file}**: unreadable, nothing restamped."));
        return false;
    };
    let Some(stamped) = stamped_toolchain(&text) else {
        report(&format!(
            "**{file}**: no `# toolchain:` line, so there is no old nightly to A/B against."
        ));
        return false;
    };
    let Some(pin) = pinned_nightly() else {
        report("rust-toolchain.toml has no channel line; nothing to restamp to.");
        return false;
    };
    if stamped == pin {
        report(&format!(
            "**{file}**: already names `{pin}`; nothing to restamp."
        ));
        return true;
    }

    // The two passes, old first. `RUSTUP_TOOLCHAIN` reaches every cargo this process spawns, so it
    // selects the compiler for the whole leg.
    let mut runs = Vec::new();
    for toolchain in [&stamped, &pin] {
        eprintln!("--- bench --restamp: {file} under {toolchain} ---");
        // SAFETY: xtask is single-threaded here; nothing reads the environment concurrently.
        unsafe { std::env::set_var("RUSTUP_TOOLCHAIN", toolchain) };
        if !leg() {
            report(&format!(
                "**{file}**: the bench leg failed under `{toolchain}`, so nothing was measured or restamped."
            ));
            return false;
        }
        runs.push(take_last_run());
    }

    let terms = match compiler_terms(&runs[0], &runs[1]) {
        Ok(t) => t,
        Err(e) => {
            report(&format!("**{file}**: {e}; nothing restamped."));
            return false;
        }
    };
    let cumulative = compose(&prior_cumulative(&text), &terms);
    let refusals = refusals(&terms, &cumulative);
    let table = table(
        file,
        &stamped,
        &pin,
        &runs[0],
        &runs[1],
        &terms,
        &cumulative,
    );

    if !refusals.is_empty() {
        report(&format!(
            "**{file}: not restamped.** A person re-records this floor with `--save --why`.\n\n{}\n\n{table}",
            refusals.join("\n")
        ));
        return false;
    }
    let why = restamp_reason(&today_utc(), &stamped, &pin, &terms, &cumulative);
    if let Err(e) = std::fs::write(&path, restamped(&text, &pin, &why)) {
        report(&format!("**{file}**: cannot write: {e}"));
        return false;
    }
    report(&format!(
        "**{file}: restamped** `{stamped}` to `{pin}`, rows unchanged.\n\n{table}"
    ));
    true
}

/// The nightly a floor's `# toolchain:` line names, as `script/lint` reads it.
fn stamped_toolchain(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.trim_start().strip_prefix("# toolchain:"))
        .and_then(|v| v.split_whitespace().next())
        .map(str::to_owned)
}

/// Per-row compiler term in percent, `new` against `old`, in `old`'s row order. Refused when the
/// two runs do not report the same rows, since a missing row cannot be proven unmoved.
fn compiler_terms(old: &[Row], new: &[Row]) -> Result<Vec<(String, f64)>, String> {
    if old.is_empty() {
        return Err("the old nightly's run reported no rows".into());
    }
    let mut names_old: Vec<&str> = old.iter().map(|r| r.0.as_str()).collect();
    let mut names_new: Vec<&str> = new.iter().map(|r| r.0.as_str()).collect();
    names_old.sort_unstable();
    names_new.sort_unstable();
    if names_old != names_new {
        return Err(format!(
            "the two nightlies reported different rows ({names_old:?} against {names_new:?})"
        ));
    }
    Ok(old
        .iter()
        .map(|(name, a, _)| {
            let b = new.iter().find(|r| &r.0 == name).map(|r| r.1).unwrap_or(0);
            let pct = match (*a, b) {
                (0, 0) => 0.0,
                (0, _) => f64::INFINITY,
                (a, b) => (b as f64 - a as f64) / a as f64 * 100.0,
            };
            (name.clone(), pct)
        })
        .collect())
}

/// The cumulative per-row term the last restamp recorded, or nothing if the floor has not been
/// restamped since its last `--save` (a save rewrites the header, so every restamp line in the file
/// is since the last save by construction).
fn prior_cumulative(text: &str) -> Vec<(String, f64)> {
    let Some(ledger) = text
        .lines()
        .filter_map(|l| l.trim_start().strip_prefix("# why:"))
        .filter_map(|w| w.rsplit_once(CUMULATIVE_MARKER).map(|(_, r)| r.trim()))
        .next_back()
    else {
        return Vec::new();
    };
    ledger
        .split(", ")
        .filter_map(|item| {
            let (name, pct) = item.trim().split_once(' ')?;
            let v: f64 = pct.trim().trim_end_matches('%').parse().ok()?;
            Some((name.to_owned(), v))
        })
        .collect()
}

/// Compound this bump's terms onto the prior cumulative, row by row. A row the prior ledger does
/// not name starts from zero; a row this run does not report is dropped, since it no longer gates.
fn compose(prior: &[(String, f64)], terms: &[(String, f64)]) -> Vec<(String, f64)> {
    terms
        .iter()
        .map(|(name, t)| {
            let p = prior.iter().find(|(n, _)| n == name).map_or(0.0, |r| r.1);
            (
                name.clone(),
                ((1.0 + p / 100.0) * (1.0 + t / 100.0) - 1.0) * 100.0,
            )
        })
        .collect()
}

/// Every reason the restamp is refused, empty when it may proceed. Strictly less than each bound.
fn refusals(terms: &[(String, f64)], cumulative: &[(String, f64)]) -> Vec<String> {
    let mut out = Vec::new();
    for (name, t) in terms {
        if t.abs().partial_cmp(&ROW_BOUND_PCT) != Some(std::cmp::Ordering::Less) {
            out.push(format!(
                "- `{name}` moved {t:+.3}% between the two nightlies (bound: under {ROW_BOUND_PCT}%)."
            ));
        }
    }
    for (name, c) in cumulative {
        if c.abs().partial_cmp(&CUMULATIVE_BOUND_PCT) != Some(std::cmp::Ordering::Less) {
            out.push(format!(
                "- `{name}` has moved {c:+.3}% across restamps since the last `--save` (bound: under {CUMULATIVE_BOUND_PCT}%)."
            ));
        }
    }
    out
}

/// `name +0.210%, name -0.093%`, largest magnitude first, rows that round to zero left out.
fn listing(rows: &[(String, f64)], limit: usize) -> String {
    let mut v: Vec<&(String, f64)> = rows
        .iter()
        .filter(|(_, p)| format!("{:.3}", p.abs()) != "0.000")
        .collect();
    v.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));
    let s: Vec<String> = v
        .iter()
        .take(limit)
        .map(|(n, p)| format!("{n} {p:+.3}%"))
        .collect();
    if s.is_empty() {
        "none".into()
    } else {
        s.join(", ")
    }
}

/// The `# why:` line a restamp adds. The cumulative ledger is last on the line, complete, and in
/// the form [`prior_cumulative`] reads back.
fn restamp_reason(
    date: &str,
    old: &str,
    new: &str,
    terms: &[(String, f64)],
    cumulative: &[(String, f64)],
) -> String {
    format!(
        "{date}: restamped from {old} to {new} by `cargo xtask bench --restamp`, rows unchanged; \
         compiler term on one machine, largest: {} (bounds {ROW_BOUND_PCT}% per row, \
         {CUMULATIVE_BOUND_PCT}% cumulative); {CUMULATIVE_MARKER} {}",
        listing(terms, 3),
        listing(cumulative, usize::MAX)
    )
}

/// The floor with its `# toolchain:` line pointed at `pin` and `why` added after the last `# why:`
/// line (or after `# date:`, or the stamp, whichever the file has). No other line changes.
fn restamped(text: &str, pin: &str, why: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let anchor = ["# why:", "# date:", "# toolchain:"]
        .iter()
        .find_map(|p| lines.iter().rposition(|l| l.trim_start().starts_with(p)));
    let mut out = String::with_capacity(text.len() + why.len() + 16);
    let mut stamped = false;
    for (i, l) in lines.iter().enumerate() {
        if !stamped && l.trim_start().starts_with("# toolchain:") {
            out.push_str(&format!("# toolchain: {pin}\n"));
            stamped = true;
        } else {
            out.push_str(l);
            out.push('\n');
        }
        if Some(i) == anchor {
            out.push_str(&format!("# why: {why}\n"));
        }
    }
    out
}

/// The A/B as a Markdown table, for the pull request body and the job summary.
fn table(
    file: &str,
    old: &str,
    new: &str,
    a: &[Row],
    b: &[Row],
    terms: &[(String, f64)],
    cumulative: &[(String, f64)],
) -> String {
    let mut s = format!(
        "`{file}`\n\n| row | {old} | {new} | this bump | since last save |\n|---|---:|---:|---:|---:|\n"
    );
    for (name, t) in terms {
        let x = a.iter().find(|r| &r.0 == name).map_or(0, |r| r.1);
        let y = b.iter().find(|r| &r.0 == name).map_or(0, |r| r.1);
        let c = cumulative
            .iter()
            .find(|r| &r.0 == name)
            .map_or(0.0, |r| r.1);
        s.push_str(&format!("| {name} | {x} | {y} | {t:+.3}% | {c:+.3}% |\n"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(v: &[(&str, u64)]) -> Vec<Row> {
        v.iter().map(|(n, t)| (n.to_string(), *t, 1)).collect()
    }

    const FLOOR: &str = "# toolchain: nightly-2026-09-26
# qemu: 11.1.1
# date: 2026-09-26
# why: nightly-2026-09-26: saved by a person
# bench/baseline-riscv64.txt: deterministic icount tick counts (cargo xtask bench --save).
             # indented header line
ipc_rtt 170436 1000
spawn_el0 1000000 100
";

    /// #1335's riscv64 `ipc_rtt`, both nightlies on one tree: 170436 then 170788.
    #[test]
    fn the_term_is_new_against_old_in_percent() {
        let t =
            compiler_terms(&rows(&[("ipc_rtt", 170436)]), &rows(&[("ipc_rtt", 170788)])).unwrap();
        assert!((t[0].1 - 0.2065).abs() < 0.001, "{t:?}");
    }

    #[test]
    fn a_row_only_one_nightly_reports_refuses() {
        let e = compiler_terms(&rows(&[("a", 1), ("b", 1)]), &rows(&[("a", 1)]));
        assert!(e.is_err());
    }

    /// The bounds are strict: a row at exactly 0.5% is refused, one at 0.499% is not.
    #[test]
    fn the_row_bound_is_strict() {
        let ok = vec![("a".to_string(), 0.499)];
        let at = vec![("a".to_string(), -0.5)];
        assert!(refusals(&ok, &ok).is_empty());
        assert_eq!(refusals(&at, &at).len(), 1);
    }

    /// Small terms that compound past 2% are refused even though no single bump reached 0.5%.
    #[test]
    fn the_cumulative_bound_catches_many_small_moves() {
        let mut cum = Vec::new();
        let term = vec![("ipc_rtt".to_string(), 0.45)];
        let mut bumps = 0;
        while refusals(&term, &compose(&cum, &term)).is_empty() {
            cum = compose(&cum, &term);
            bumps += 1;
            assert!(bumps < 10, "never refused");
        }
        assert_eq!(bumps, 4, "0.45% compounds past 2% on the fifth bump");
    }

    /// The restamp changes the stamp, adds one reason, and leaves every row and every other
    /// header line byte-identical; and the stamp still matches `script/lint`'s pattern.
    #[test]
    fn a_restamp_moves_the_stamp_and_nothing_else() {
        let terms = vec![
            ("ipc_rtt".to_string(), 0.2065),
            ("spawn_el0".to_string(), 0.0),
        ];
        let cum = compose(&[], &terms);
        let why = restamp_reason(
            "2026-09-27",
            "nightly-2026-09-26",
            "nightly-2026-09-27",
            &terms,
            &cum,
        );
        let out = restamped(FLOOR, "nightly-2026-09-27", &why);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "# toolchain: nightly-2026-09-27");
        assert_eq!(lines[3], "# why: nightly-2026-09-26: saved by a person");
        assert!(lines[4].starts_with("# why: 2026-09-27: restamped from nightly-2026-09-26"));
        assert_eq!(out.lines().count(), FLOOR.lines().count() + 1);
        let data = |s: &str| -> Vec<String> {
            s.lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .map(str::to_owned)
                .collect()
        };
        assert_eq!(data(&out), data(FLOOR));
        assert_eq!(
            stamped_toolchain(&out).as_deref(),
            Some("nightly-2026-09-27")
        );
    }

    /// What one restamp writes, the next one reads back, and composes onto.
    #[test]
    fn the_cumulative_ledger_round_trips() {
        let first = vec![
            ("ipc_rtt".to_string(), 0.2065),
            ("spawn_el0".to_string(), -0.093),
        ];
        let cum = compose(&[], &first);
        let why = restamp_reason("d", "o", "n", &first, &cum);
        let once = restamped(FLOOR, "n", &why);
        let back = prior_cumulative(&once);
        assert_eq!(back.len(), 2);
        let ipc = back.iter().find(|r| r.0 == "ipc_rtt").unwrap().1;
        assert!((ipc - 0.2065).abs() < 0.001, "{back:?}");
        let again = compose(&back, &[("ipc_rtt".to_string(), 0.2065)]);
        assert!((again[0].1 - 0.4139).abs() < 0.002, "{again:?}");
    }

    /// A floor a person just saved has no ledger, so the cumulative starts at zero.
    #[test]
    fn a_fresh_save_has_no_prior_cumulative() {
        assert!(prior_cumulative(FLOOR).is_empty());
    }
}
