//! **`top`: the same domain `ps` lists, ordered by what it is costing** (§150 (how does a thread's
//! CPU time reach userspace?), notes/process-view.md). The program's logic, lifted out to run on
//! the host; `components/src/top.rs` is the syscalls and the output streams. It ranks by CPU time
//! through `ps` ([`ps::Survey::rank_by_cpu_time`]) and adds the lines above the table,
//! [`write_summary`] and [`write_machine_line`]. See notes/process-view/top.md.

//! # BUGS
//!
//! - No live refresh: an interval spin would rank `top` itself as the busiest thread.
//! - No `%CPU`, which needs two samples; `TIME(ms)` is cumulative, as Unix's `TIME+` is.
//! - No top *N*: `ProgramArgumentDeclaration` cannot declare an optional integer.
//! - The summary counts this domain, not the machine, because nothing here is ambient.
//! - Ties break by tid, so a quiet machine ranks in slot order.
//!
//! Name: provisional here; calef ratified the program's name on 2026-10-06 and kept it a program
//! of its own rather than a flag on `ps`.

#![cfg_attr(not(test), no_std)]

/// **The line above the table**: how long the machine has been counting, how many threads the
/// domain holds, and what they are doing, ending in a newline, for the output stream.
///
/// `uptime_nanos` is `user_mode_runtime::monotonic_nanos()`, passed in so this stays a pure
/// function a host test can assert on. A refused caller prints neither this nor the table. See
/// notes/process-view/top.md, "The summary line".
///
/// # Examples
///
/// ```
/// use ps::Row;
///
/// let rows = [
///     Row { tid: 5, state: abi::survey::RUNNING, cpu_millis: Some(310) },
///     Row { tid: 9, state: abi::survey::DEAD, cpu_millis: Some(10) },
/// ];
/// let mut text = Vec::new();
/// top::write_summary(&rows, 72_000_000_000, &mut |b| text.extend_from_slice(b));
/// let line = String::from_utf8(text).unwrap();
/// assert!(line.starts_with("up 00:01:12, 2 threads: "), "{line}");
/// assert!(line.contains("1 running"), "{line}");
/// ```
pub fn write_summary(rows: &[ps::Row], uptime_nanos: u64, out: &mut dyn FnMut(&[u8])) {
    // `uptime::format` is the same formatter `uptime` prints, reused rather than restated so the
    // two programs cannot come to disagree about what "up" means. It ends in a newline and this
    // line does not end here, so the newline is dropped.
    let up = uptime::format(uptime_nanos);
    let up = up.as_bytes();
    out(up.strip_suffix(b"\n").unwrap_or(up));
    out(b", ");
    write_u64(rows.len() as u64, out);
    out(b" threads: ");

    let mut first = true;
    for state in [
        abi::survey::RUNNING,
        abi::survey::READY,
        abi::survey::BLOCKED,
        abi::survey::DEAD,
    ] {
        if !first {
            out(b", ");
        }
        first = false;
        write_u64(rows.iter().filter(|r| r.state == state).count() as u64, out);
        out(b" ");
        out(ps::state_name(state).as_bytes());
    }
    out(b"\n");
}

/// **The machine line under the summary, which is what became of `tload`** (milestone 126,
/// DECISIONS §225 (`free` sees the machine and your share)): the threads runnable at each core's
/// last tick, on how many cores, and the share of ticks since boot not spent idle. `None` prints the
/// sentence a withheld page earns rather than a machine of zeroes. See notes/process-view/top.md.
pub fn write_machine_line(
    machine: Option<&machine_statistics_contract::Snapshot>,
    out: &mut dyn FnMut(&[u8]),
) {
    let Some(m) = machine else {
        out(b"machine: not shown, the owner has not granted the machine statistics page\n");
        return;
    };
    let ticks = m.busy_ticks() + m.idle_ticks();
    out(b"machine: ");
    write_u64(m.runnable(), out);
    out(b" runnable on ");
    write_u64(m.online_cpus() as u64, out);
    out(b" cores, ");
    write_u64((m.busy_ticks() * 100).checked_div(ticks).unwrap_or(0), out);
    out(b"% busy since boot\n");
}

/// A `u64` in decimal, no padding. The summary line is prose rather than a table, so its numbers
/// are not in columns.
fn write_u64(v: u64, out: &mut dyn FnMut(&[u8])) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut v = v;
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    out(&buf[i..]);
}

#[cfg(test)]
mod tests {
    use ps::Row;

    fn row(tid: u64, state: u64, ms: u64) -> Row {
        Row {
            tid,
            state,
            cpu_millis: Some(ms),
        }
    }

    fn shown(rows: &[Row], nanos: u64) -> String {
        let mut v = Vec::new();
        super::write_summary(rows, nanos, &mut |b| v.extend_from_slice(b));
        String::from_utf8(v).unwrap()
    }

    /// The whole line, asserted verbatim once. Every other test here checks one property; this one
    /// checks the shape, because a status line is read by a person and its punctuation is the
    /// difference between a sentence and a debug dump.
    #[test]
    fn the_machine_line_is_what_became_of_tload() {
        use machine_statistics_contract::{Snapshot, WORDS, build_header, word};
        let mut w = [0u64; WORDS];
        w[..word::LINE].copy_from_slice(&build_header(4096, 100));
        for (cpu, busy, idle, r) in [(1, 30, 70, 2), (3, 10, 90, 1)] {
            w[word::cpu(cpu) + word::ONLINE] = 1;
            w[word::cpu(cpu) + word::BUSY_TICKS] = busy;
            w[word::cpu(cpu) + word::IDLE_TICKS] = idle;
            w[word::cpu(cpu) + word::RUNNABLE] = r;
        }
        let m = Snapshot::from_words(&w).unwrap();
        let mut v = Vec::new();
        super::write_machine_line(Some(&m), &mut |b| v.extend_from_slice(b));
        assert_eq!(
            String::from_utf8(v).unwrap(),
            "machine: 3 runnable on 2 cores, 20% busy since boot\n"
        );
        let mut v = Vec::new();
        super::write_machine_line(None, &mut |b| v.extend_from_slice(b));
        assert!(String::from_utf8(v).unwrap().contains("not granted"));
    }

    #[test]
    fn the_summary_reads_as_a_sentence() {
        let rows = [
            row(3, abi::survey::RUNNING, 300),
            row(5, abi::survey::BLOCKED, 20),
            row(7, abi::survey::DEAD, 40),
            row(9, abi::survey::READY, 0),
        ];
        assert_eq!(
            shown(&rows, 72_000_000_000),
            "up 00:01:12, 4 threads: 1 running, 1 ready, 1 blocked, 1 dead\n",
        );
    }

    /// **A zero is printed rather than elided.** A status line that drops its empty categories
    /// reads as one that did not look, and the reader cannot tell "no corpses" from "corpses were
    /// not counted".
    #[test]
    fn a_state_with_nobody_in_it_still_reports_zero() {
        let line = shown(&[row(3, abi::survey::RUNNING, 10)], 0);
        assert!(line.contains("0 dead"), "{line}");
        assert!(line.contains("0 blocked"), "{line}");
    }

    /// An empty domain still produces a well-formed line. `ps` says "this domain holds no
    /// processes" on diagnostics and prints no table; this line is the output stream's half, and a
    /// caller that prints it is printing a truthful zero rather than nothing.
    #[test]
    fn an_empty_domain_summarises_to_zero_of_everything() {
        assert_eq!(
            shown(&[], 0),
            "up 00:00:00, 0 threads: 0 running, 0 ready, 0 blocked, 0 dead\n",
        );
    }

    /// The counts are of the survey, and a state this program does not know (a kernel newer than
    /// it) is counted in none of the four rather than miscounted into one of them. The total still
    /// names it, which is the pair that makes the discrepancy visible instead of silent.
    #[test]
    fn an_unknown_state_is_in_the_total_and_in_no_category() {
        let line = shown(&[row(3, 99, 0)], 0);
        assert!(line.contains("1 threads: "), "{line}");
        assert!(
            line.contains("0 running, 0 ready, 0 blocked, 0 dead"),
            "{line}"
        );
    }
}
