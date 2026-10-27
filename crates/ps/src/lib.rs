//! **`ps`: what a process listing is when there is no `/proc`** (notes/process-view.md). The
//! program's logic, lifted out to run on the host; `components/src/ps.rs` is the syscall and the
//! two output streams. The listing is a capability, not ambient `/proc`: this crate is handed a
//! function that reads one entry of a domain, backed by `abi::rendezvous::SURVEY` on an endowed
//! supervision endpoint, so it cannot widen the domain or learn that another exists. Rows, an
//! empty domain and a refusal stay three distinct answers. [`collect`] takes the whole domain
//! into the caller's buffer before anything is written, then the caller emits
//! [`Survey::write_diagnostics`] and [`Survey::write_report`] in that order (§67 (a program's
//! second stream is a declaration)). notes/process-view/the-ps-crate.md has the reasons.

//! # BUGS
//!
//! - No `CMD` column: there is no display name, and no design for who may see one.
//! - **`THR` counts the threads of the viewer's domain, never the process's true total** (§313 (`ENUMERATE` on a domain covers which of its threads share a process)): a
//!   sibling supervised elsewhere is invisible, and so is a sibling nobody supervises (only a
//!   process's first thread is supervised unless its starter supervises each one).
//! - `PROC` is an ordinal for one walk, not a process identity: it can renumber if a process ends
//!   between two runs, and the label walk and the state walk are joined on the tid.
//! - The tid prints whole (`(generation << 32) | slot`); split, it would expose the table's shape.
//! - `ps` does not sort; [`Survey::rank_by_cpu_time`] is for `components/src/top.rs`.
//! - `TIME` is a second walk ([`Survey::join_cpu_time`]); a thread that dies between prints `-`.
//! - Each row is its own snapshot, so an early row may be stale when the table prints.
//! - Nothing automated spawns `ps` or `pgrep`; only a static stack measurement covers the buffer
//!   (host tests and `kernel::user::survey_tests` cover the logic and the syscall).
//!
//! Name: ratified 2026-08-23 (calef): the name every reader knows, shared with
//! `components/src/ps.rs` as one thing split at the IO boundary.

#![cfg_attr(not(test), no_std)]

/// The widest domain a survey can produce: the kernel's entire thread table (`MAXIMUM_THREADS` in
/// `kernel/src/sched.rs`). **A buffer this size makes truncation unreachable rather than handled**,
/// and a compile-time assertion in `kernel::user::survey_tests` keeps the two in step. The buffer
/// is a stack allocation in `ps` and `pgrep`; notes/process-view/the-ps-crate.md has its measured
/// cost against the child stack.
///
/// Name: ratified 2026-10-27 (#2797).
pub const MAXIMUM_ROWS: usize = 256;

/// One line of the listing: a thread and what it is doing.
///
/// Two fields, because two facts is what a supervision domain honestly knows about its members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Row {
    /// The kernel-stamped thread id, the same name `abi::rendezvous::REAP` accepts.
    pub tid: u64,
    /// One of `abi::survey`'s state codes.
    pub state: u64,
    /// **Scheduled on-CPU time in milliseconds**, or `None` when nobody asked for it.
    ///
    /// `None` rather than `0`: [`collect`] walks only for the state record, and the figure arrives
    /// only if a caller then runs [`Survey::join_cpu_time`]. A zero is a thread that has never been
    /// on a CPU at a tick, a different fact from "not asked". It also stays `None` for a thread
    /// that died between the two walks.
    pub cpu_ms: Option<u64>,
    /// **Which process of the domain the thread belongs to**, as `abi::survey::record::PROCESS`
    /// labeled it in one walk (§313), or `None` when nobody asked or the thread died between
    /// walks.
    ///
    /// Two rows with the same label share an address space and a capability table. The label means
    /// nothing outside the walk that returned it, and a program compares it only against other
    /// rows of the same [`Survey`]. The thread count of a process is the number of rows that carry
    /// its label ([`Survey::threads_with_label`]): threads of the viewer's domain, never the
    /// process's true total.
    pub process: Option<u64>,
}

/// A finished walk of one domain: the rows, and whatever went wrong.
///
/// Built by [`collect`], which is the only constructor, so a `Survey` always describes a walk that
/// really happened.
pub struct Survey<'a> {
    /// Mutable because a survey is written to after it is collected: a second walk fills
    /// [`Row::cpu_ms`] in ([`Survey::join_cpu_time`]) and a ranking view reorders it
    /// ([`Survey::rank_by_cpu_time`]). The borrow is still the caller's buffer; nothing here
    /// allocates.
    rows: &'a mut [Row],
    /// The negated `abi::Error` that ended the walk, if one did. `Some` here means **the listing is
    /// not the domain**: it is however far the walk got, which is why the caller prints the reason
    /// rather than the partial table.
    refused: Option<i64>,
    /// The kernel handed back a cursor that did not advance. Not reachable from the shipped kernel
    /// and checked anyway, because a `collect` that could loop forever on a bad reader would not be
    /// a total function and could not be fuzzed or property-tested.
    stalled: bool,
    /// The buffer filled and the domain had more in it. Unreachable for a caller that sized its
    /// buffer at [`MAXIMUM_ROWS`], which `ps` does; reported rather than silent for one that did
    /// not.
    truncated: bool,
}

/// **Walk a domain to its end and keep what it says.**
///
/// `read(cursor)` is one `abi::rendezvous::SURVEY`: it answers `(next_cursor, tid, state)`, where a
/// negative first word is a refusal and `abi::survey::DONE` means the walk is over. Start at 0,
/// feed each `next_cursor` back; this function is that loop. A closure rather than a syscall, so
/// every case is reachable from a host test. The walk stops at the first refusal and reports it
/// rather than skipping the entry.
///
/// # Examples
///
/// Driving it from a host test is the whole contract, and it needs no kernel:
///
/// ```
/// use ps::{MAXIMUM_ROWS, Row, collect};
///
/// // A domain of one running thread: cursor 0 yields it, cursor 1 says done.
/// let mut reader = |cursor: u64| match cursor {
///     0 => (1, 7, abi::survey::RUNNING),
///     _ => (abi::survey::DONE as i64, 0, 0),
/// };
/// let mut rows = [Row::default(); MAXIMUM_ROWS];
/// let survey = collect(&mut rows, &mut reader);
/// assert_eq!(survey.rows().len(), 1);
/// assert_eq!(survey.rows()[0].tid, 7);
///
/// let mut text = Vec::new();
/// survey.write_report(&mut |b| text.extend_from_slice(b));
/// assert!(String::from_utf8(text).unwrap().contains("running"));
/// ```
pub fn collect<'a>(
    rows: &'a mut [Row],
    read: &mut dyn FnMut(u64) -> (i64, u64, u64),
) -> Survey<'a> {
    let mut n = 0usize;
    let mut refused = None;
    let mut stalled = false;
    let mut truncated = false;
    let mut cursor = 0u64;
    loop {
        let (next, tid, state) = read(cursor);
        if next < 0 {
            refused = Some(next);
            break;
        }
        let next = next as u64;
        if next == abi::survey::DONE {
            break;
        }
        // A cursor that did not advance would spin here forever. The kernel is inside the trusted
        // base and does not do this; the check exists so that this function terminates for *every*
        // reader, including a test's.
        if next <= cursor {
            stalled = true;
            break;
        }
        if n == rows.len() {
            truncated = true;
            break;
        }
        rows[n] = Row {
            tid,
            state,
            cpu_ms: None,
            process: None,
        };
        n += 1;
        cursor = next;
    }
    Survey {
        rows: &mut rows[..n],
        refused,
        stalled,
        truncated,
    }
}

impl Survey<'_> {
    /// The rows, in the order the kernel reported them unless [`rank_by_cpu_time`](Self::rank_by_cpu_time)
    /// has reordered them.
    pub fn rows(&self) -> &[Row] {
        self.rows
    }

    /// **Fill in [`Row::cpu_ms`] from a second walk of the same domain** (milestone 282 (a thread's CPU time, and the `top` it makes possible),
    /// `abi::survey::record::CPU_TIME`).
    ///
    /// `read(cursor)` is one `SURVEY` asking for the CPU-time record, bound by the caller, as
    /// [`collect`]'s closure is for the state record. Two walks joined on the tid, rather than one
    /// walk carrying two facts, is DECISIONS §204 (how userspace asks where a thread runs)'s shape.
    /// A tid in the first walk and not the second died in between and keeps `None`; one only in the
    /// second is ignored. A refusal in the second walk is recorded as one in the first is, so
    /// [`is_refused`](Self::is_refused) tells the caller. notes/process-view/the-ps-crate.md has
    /// the cost and the bargain.
    pub fn join_cpu_time(&mut self, read: &mut dyn FnMut(u64) -> (i64, u64, u64)) {
        self.join(read, |row, ms| row.cpu_ms = Some(ms));
    }

    /// **Fill in [`Row::process`] from a walk of the same domain** (milestone 942 (`ps` and `top`
    /// show which process each thread belongs to), `abi::survey::record::PROCESS`).
    ///
    /// `read(cursor)` is one `SURVEY` asking for the process record. It joins on the tid exactly as
    /// [`join_cpu_time`](Self::join_cpu_time) does, and a thread that died between the walks keeps
    /// `None`. A label of 0 is "no label" and is kept as `None`. The count per process is then
    /// computed here from the rows, so the kernel exposes no count.
    ///
    /// Name: provisional, minted 2026-10-27 (UTC) by lane milestone/942-ps-top-process-grouping.
    pub fn join_process(&mut self, read: &mut dyn FnMut(u64) -> (i64, u64, u64)) {
        self.join(read, |row, label| {
            row.process = (label != 0).then_some(label);
        });
    }

    /// One more walk of the domain, `apply`ing each answer to the row with the same tid. The loop
    /// both joins share.
    fn join(
        &mut self,
        read: &mut dyn FnMut(u64) -> (i64, u64, u64),
        apply: impl Fn(&mut Row, u64),
    ) {
        let mut cursor = 0u64;
        loop {
            let (next, tid, word) = read(cursor);
            if next < 0 {
                self.refused = Some(next);
                return;
            }
            let next = next as u64;
            if next == abi::survey::DONE {
                return;
            }
            // The same non-advancing-cursor guard `collect` carries, and for the same reason: this
            // loop must terminate for every reader, including a test's.
            if next <= cursor {
                self.stalled = true;
                return;
            }
            if let Some(row) = self.rows.iter_mut().find(|r| r.tid == tid) {
                apply(row, word);
            }
            cursor = next;
        }
    }

    /// **How many rows carry `label`**: the threads of that process that the viewer's domain
    /// holds (§313). Never the process's true member count, which this program cannot learn.
    ///
    /// Name: provisional, minted 2026-10-27 (UTC) by lane milestone/942-ps-top-process-grouping.
    pub fn threads_with_label(&self, label: u64) -> usize {
        self.rows
            .iter()
            .filter(|r| r.process == Some(label))
            .count()
    }

    /// **Order the rows by their process's total CPU time, most first**, then by label, then by
    /// each thread's own time: the per-process ranking `top` offers beside
    /// [`rank_by_cpu_time`](Self::rank_by_cpu_time).
    ///
    /// A process's total is the sum of its rows' [`Row::cpu_ms`], the unknown ones counting as
    /// zero. The threads of one process end up adjacent. A row with no label is its own group and
    /// sorts after every labeled process, for the reason an unknown figure sorts last in the
    /// per-thread ranking. The order is total (ties break on label, then tid), so a redraw of an
    /// idle machine does not shuffle.
    ///
    /// Name: provisional, minted 2026-10-27 (UTC) by lane milestone/942-ps-top-process-grouping.
    pub fn rank_by_process_cpu_time(&mut self) {
        // Each process total is summed once into a table indexed by label, so the comparator reads
        // a figure instead of rescanning rows it is in the middle of moving. The kernel's labels
        // are ordinals of at most `MAXIMUM_ROWS` processes; a label beyond that (a kernel that
        // answers otherwise) totals 0 and sorts after the processes it can account for.
        let mut total = [0u64; MAXIMUM_ROWS + 1];
        for r in self.rows.iter() {
            if let Some(slot) = r.process.and_then(|l| total.get_mut(l as usize)) {
                *slot += r.cpu_ms.unwrap_or(0);
            }
        }
        let of = |r: &Row| {
            r.process
                .and_then(|l| total.get(l as usize))
                .copied()
                .unwrap_or(0)
        };
        self.rows.sort_unstable_by(|a, b| {
            a.process
                .is_none()
                .cmp(&b.process.is_none())
                .then(of(b).cmp(&of(a)))
                .then(a.process.cmp(&b.process))
                .then(b.cpu_ms.unwrap_or(0).cmp(&a.cpu_ms.unwrap_or(0)))
                .then(a.tid.cmp(&b.tid))
        });
    }

    /// **Order the rows by CPU time, most first**, which is what makes a listing a `top`.
    ///
    /// Ties break on the tid, ascending, so the order is total and a redraw of an idle machine does
    /// not shuffle. A row with no figure (see [`Row::cpu_ms`]) sorts last, because "not known"
    /// is not "none used" and putting it at the head would be the strongest possible claim about
    /// the one row that has nothing to say.
    ///
    /// `sort_unstable_by` allocates nothing, which is why this can happen in a `no_std` program on
    /// a twelve-page stack.
    pub fn rank_by_cpu_time(&mut self) {
        self.rows.sort_unstable_by(|a, b| {
            b.cpu_ms
                .unwrap_or(0)
                .cmp(&a.cpu_ms.unwrap_or(0))
                .then(a.cpu_ms.is_none().cmp(&b.cpu_ms.is_none()))
                .then(a.tid.cmp(&b.tid))
        });
    }

    /// **Was this a refusal?** True when the domain could not be read at all or not to its end. A
    /// caller must not print an empty table for this: see the crate docs.
    pub fn is_refused(&self) -> bool {
        self.refused.is_some() || self.stalled
    }

    /// **Is this listing the whole domain?** False when the walk was refused, stalled, or ran out of
    /// buffer. A caller that prints the table anyway is printing something it cannot vouch for.
    pub fn is_complete(&self) -> bool {
        !self.is_refused() && !self.truncated
    }

    /// **What there is to complain about, as a clause a program name prefixes**, or `None` when the
    /// walk succeeded and found processes.
    ///
    /// Split out of [`write_diagnostics`](Survey::write_diagnostics) so `crates/pgrep` describes a
    /// refusal in the same words; the clause carries no program name and no newline for that
    /// reason. The order is [`write_diagnostics`](Survey::write_diagnostics)'s, and the empty
    /// domain is last because it is the only one of the four that is not a failure.
    pub fn complaint(&self) -> Option<&'static str> {
        if self.stalled {
            return Some("the survey did not advance; the listing is incomplete");
        }
        if let Some(code) = self.refused {
            return Some(refusal(code));
        }
        if self.truncated {
            return Some("the listing buffer filled; this domain has more in it");
        }
        if self.rows.is_empty() {
            // **Not an error, and it must not read like one.** The caller held the domain and was
            // allowed to look; there was nothing in it. This is the sentence that distinguishes an
            // empty answer from a refused one, which is the distinction Linux's `ps` cannot draw.
            return Some("this domain holds no processes");
        }
        None
    }

    /// **Everything to complain about, said before a byte of output** (DECISIONS §67).
    ///
    /// Writes nothing at all when the walk succeeded and found processes, which is the common case
    /// and the one where a second stream should stay silent.
    pub fn write_diagnostics(&self, out: &mut dyn FnMut(&[u8])) {
        if let Some(clause) = self.complaint() {
            out(b"ps: ");
            out(clause.as_bytes());
            out(b"\n");
        }
    }

    /// The table itself, on the output stream. **Nothing at all on a refusal**, so a `ps > out.txt`
    /// that was refused leaves an empty file rather than a plausible-looking listing of nothing.
    pub fn write_report(&self, out: &mut dyn FnMut(&[u8])) {
        if !self.is_complete() || self.rows.is_empty() {
            return;
        }
        // The TIME column appears only when somebody asked for it (`Survey::join_cpu_time`), so a
        // `pgrep`-shaped caller that did not is not handed a column of dashes to explain.
        let timed = self.rows().iter().any(|r| r.cpu_ms.is_some());
        // The same for the process columns (milestone 942): `PROC` and `THR` appear only when the
        // process walk ran, so a caller that did not ask is not handed a column of dashes.
        let grouped = self.rows().iter().any(|r| r.process.is_some());
        if grouped {
            out(b"PROC  THR");
        }
        if timed {
            out(b"         TID  STATE     TIME(ms)\n");
        } else {
            out(b"         TID  STATE\n");
        }
        for row in self.rows() {
            if grouped {
                // The count is the rows that carry this row's label, so it equals the group's size
                // on the table by construction; `-` for a row the walk could not label.
                write_cell(row.process, 4, out);
                out(b"  ");
                write_cell(
                    row.process.map(|l| self.threads_with_label(l) as u64),
                    3,
                    out,
                );
            }
            write_thread_id(row.tid, out);
            out(b"  ");
            let name = state_name(row.state);
            out(name.as_bytes());
            if timed {
                // Pad the state to the width of the widest one so the figures line up; a column of
                // numbers that does not is a column a reader has to add up by hand.
                for _ in name.len()..STATE_WIDTH {
                    out(b" ");
                }
                out(b"  ");
                write_ms(row.cpu_ms, out);
            }
            out(b"\n");
        }
    }
}

/// What an `abi::survey` state code is called in the listing.
///
/// Lowercase, because these are states rather than headings and the tree's other programs do not
/// shout. An unknown code prints as `?` rather than panicking: this program is reading a number
/// from a kernel that may be newer than it is, and a listing with one honest `?` in it beats a
/// crash.
pub const fn state_name(state: u64) -> &'static str {
    match state {
        abi::survey::READY => "ready",
        abi::survey::RUNNING => "running",
        abi::survey::BLOCKED => "blocked",
        abi::survey::DEAD => "dead",
        _ => "?",
    }
}

/// What a negated `abi::Error` means to somebody who typed `ps`.
///
/// **Written as facts about authority, not as errno.** A refusal here must read like the capability
/// model, which is the same standard `grant_plan::Refusal::message` holds itself to: the reader
/// should learn what they do not hold, not which branch of a `match` they landed in.
pub fn refusal(code: i64) -> &'static str {
    match abi::Error::from_ret(code) {
        Some(abi::Error::NoSuchSlot) => "this process holds no process-domain capability",
        Some(abi::Error::NotPermitted) => {
            "this endpoint may be sent to, but not looked at: no READ on the domain"
        }
        Some(abi::Error::WrongObject) => {
            "the capability granted here is not a supervision endpoint"
        }
        Some(abi::Error::Gone) => "the domain this named has been destroyed",
        _ => "the domain could not be read",
    }
}

/// The width the `STATE` column is padded to: the longest name [`state_name`] can return
/// (`running`, seven characters), so the column after it starts in the same place on every line.
const STATE_WIDTH: usize = 7;

/// A number right-aligned in `width` columns, or `-` when there is none. A wider number pushes the
/// column rather than losing a digit.
fn write_cell(value: Option<u64>, width: usize, out: &mut dyn FnMut(&[u8])) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    match value {
        None => {
            i -= 1;
            buf[i] = b'-';
        }
        Some(mut v) => loop {
            i -= 1;
            buf[i] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        },
    }
    for _ in (buf.len() - i)..width {
        out(b" ");
    }
    out(&buf[i..]);
}

/// A CPU-time figure right-aligned in eight columns, or `-` when the row has none.
///
/// Eight columns holds 27 hours of CPU time before the column widens, and a figure that outgrows it
/// pushes the column rather than losing a digit, for the same reason [`write_thread_id`] does.
///
/// **`-` rather than `0` for an unknown figure**, which is the whole reason [`Row::cpu_ms`] is
/// an `Option`: a thread that was never on a CPU at a tick really does read `0`, and a reader who
/// cannot tell that from "this row was not asked about" has been handed the plausible wrong number
/// this tree keeps refusing to print.
fn write_ms(ms: Option<u64>, out: &mut dyn FnMut(&[u8])) {
    let Some(ms) = ms else {
        out(b"       -");
        return;
    };
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut v = ms;
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    for _ in (buf.len() - i)..8 {
        out(b" ");
    }
    out(&buf[i..]);
}

/// A tid right-aligned in twelve columns, which is wide enough for a generational name whose
/// generation has moved (`(1 << 32) | 6` is ten digits). A wider one is not truncated, it just
/// pushes the column; losing digits from a name that `REAP` has to accept would be worse than an
/// uneven table.
fn write_thread_id(tid: u64, out: &mut dyn FnMut(&[u8])) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut v = tid;
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    let digits = buf.len() - i;
    for _ in digits..12 {
        out(b" ");
    }
    out(&buf[i..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reader over a canned domain: `entries` in slot order, then done.
    fn domain(entries: &'static [(u64, u64)]) -> impl FnMut(u64) -> (i64, u64, u64) {
        move |cursor: u64| match entries.get(cursor as usize) {
            Some(&(tid, state)) => (cursor as i64 + 1, tid, state),
            None => (abi::survey::DONE as i64, 0, 0),
        }
    }

    fn shown(f: impl FnOnce(&mut dyn FnMut(&[u8]))) -> String {
        let mut v = Vec::new();
        f(&mut |b| v.extend_from_slice(b));
        String::from_utf8(v).unwrap()
    }

    #[test]
    fn a_domain_walks_to_its_end() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let s = collect(
            &mut rows,
            &mut domain(&[
                (3, abi::survey::RUNNING),
                (5, abi::survey::BLOCKED),
                (9, abi::survey::DEAD),
            ]),
        );
        assert!(!s.is_refused());
        assert_eq!(s.rows().len(), 3);
        assert_eq!(
            s.rows()[2],
            Row {
                tid: 9,
                state: abi::survey::DEAD,
                cpu_ms: None,
                process: None,
            }
        );
    }

    /// **The claim the whole program exists to make.** An empty domain and a refused one must not
    /// produce the same thing on any stream, because a monitor that reports nothing when it could
    /// not look reads exactly like a quiet machine.
    #[test]
    fn an_empty_domain_and_a_refusal_are_different_answers() {
        let mut rows_a = [Row::default(); MAXIMUM_ROWS];
        let empty = collect(&mut rows_a, &mut domain(&[]));
        let mut rows_b = [Row::default(); MAXIMUM_ROWS];
        let refused = collect(&mut rows_b, &mut |_| {
            (abi::Error::NotPermitted as i64, 0, 0)
        });

        assert!(!empty.is_refused(), "an empty domain is not a refusal");
        assert!(refused.is_refused());

        let empty_diag = shown(|o| empty.write_diagnostics(o));
        let refused_diag = shown(|o| refused.write_diagnostics(o));
        assert_ne!(empty_diag, refused_diag);
        assert!(empty_diag.contains("no processes"), "{empty_diag}");
        assert!(refused_diag.contains("not looked at"), "{refused_diag}");

        // And neither of them puts a single byte on the output stream, so a redirect of either
        // produces an empty file rather than a listing that says nothing happened.
        assert_eq!(shown(|o| empty.write_report(o)), "");
        assert_eq!(shown(|o| refused.write_report(o)), "");
    }

    /// A refusal arriving **mid-walk** is still a refusal, not a short table. Stopping and saying
    /// so is the point; a listing missing a row it could not read is the failure mode.
    #[test]
    fn a_refusal_halfway_through_discards_the_partial_table() {
        let mut calls = 0;
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let s = collect(&mut rows, &mut |cursor| {
            calls += 1;
            match cursor {
                0 => (1, 3, abi::survey::RUNNING),
                _ => (abi::Error::Gone as i64, 0, 0),
            }
        });
        assert!(s.is_refused());
        assert_eq!(
            s.rows().len(),
            1,
            "the rows it did read are kept for the record"
        );
        assert_eq!(
            shown(|o| s.write_report(o)),
            "",
            "but none of them are printed"
        );
        assert!(shown(|o| s.write_diagnostics(o)).contains("destroyed"));
    }

    /// **`collect` terminates for every reader**, including one that never advances its cursor. The
    /// shipped kernel cannot do this; the property is what makes this function safe to hand an
    /// arbitrary closure, which is what every test above does.
    #[test]
    fn a_cursor_that_does_not_advance_ends_the_walk() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let s = collect(&mut rows, &mut |_| (1, 4, abi::survey::READY));
        assert!(s.is_refused());
        assert!(shown(|o| s.write_diagnostics(o)).contains("did not advance"));
    }

    /// **A short buffer is not a short listing, it is a stated one.** The whole design refuses to
    /// let a monitor report less than it saw without saying so, and running out of room is one more
    /// way that could happen. `ps` cannot reach this, because it sizes its buffer at
    /// [`MAXIMUM_ROWS`]; a future caller with a smaller one gets told rather than getting a
    /// plausible table.
    #[test]
    fn a_buffer_too_small_for_the_domain_says_so_and_prints_nothing() {
        let mut rows = [Row::default(); 2];
        let s = collect(&mut rows, &mut |cursor| {
            (cursor as i64 + 1, cursor + 100, abi::survey::READY)
        });
        assert_eq!(s.rows().len(), 2);
        assert!(
            !s.is_complete(),
            "a truncated listing claimed to be the whole domain"
        );
        assert!(!s.is_refused(), "running out of room is not a refusal");
        assert!(shown(|o| s.write_diagnostics(o)).contains("more in it"));
        assert_eq!(
            shown(|o| s.write_report(o)),
            "",
            "a partial table must not be printed"
        );
    }

    /// A domain as wide as the kernel's whole thread table fills the buffer exactly and needs no
    /// truncation case.
    #[test]
    fn the_widest_possible_domain_fits() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let s = collect(&mut rows, &mut |cursor| {
            if (cursor as usize) < MAXIMUM_ROWS {
                (cursor as i64 + 1, cursor + 100, abi::survey::READY)
            } else {
                (abi::survey::DONE as i64, 0, 0)
            }
        });
        assert!(!s.is_refused());
        assert_eq!(s.rows().len(), MAXIMUM_ROWS);
    }

    #[test]
    fn the_table_has_a_header_and_one_line_per_thread() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let s = collect(
            &mut rows,
            &mut domain(&[(3, abi::survey::RUNNING), (5, abi::survey::DEAD)]),
        );
        let out = shown(|o| s.write_report(o));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3, "header plus two rows: {out}");
        assert!(lines[0].contains("TID") && lines[0].contains("STATE"));
        assert!(lines[1].ends_with("3  running"), "{}", lines[1]);
        assert!(lines[2].ends_with("5  dead"), "{}", lines[2]);
        // Nothing to say when the answer is the table itself.
        assert_eq!(shown(|o| s.write_diagnostics(o)), "");
    }

    /// A generational tid is a ten-digit number and must survive the column intact: it is the name
    /// `REAP` accepts, so a truncated one would be a name that no longer works.
    #[test]
    fn a_wide_thread_id_keeps_every_digit() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let s = collect(
            &mut rows,
            &mut domain(&[((1u64 << 32) | 6, abi::survey::DEAD)]),
        );
        let out = shown(|o| s.write_report(o));
        assert!(out.contains("4294967302"), "{out}");
    }

    /// Every state code a kernel can report has its own name, and an unknown one prints as `?`.
    /// Only `running` and `dead` were pinned by the table tests, so deleting the `ready` or
    /// `blocked` arm left every test green.
    #[test]
    fn every_state_has_its_name() {
        assert_eq!(state_name(abi::survey::READY), "ready");
        assert_eq!(state_name(abi::survey::RUNNING), "running");
        assert_eq!(state_name(abi::survey::BLOCKED), "blocked");
        assert_eq!(state_name(abi::survey::DEAD), "dead");
        assert_eq!(state_name(u64::MAX), "?");
    }

    /// Each refusal a person can meet is explained in its own words. The two this pins were the
    /// ones whose arms could be deleted into the generic "could not be read" line unnoticed.
    #[test]
    fn each_refusal_is_explained_in_its_own_words() {
        let say = |e: abi::Error| refusal(e as i64);
        assert!(say(abi::Error::NoSuchSlot).contains("holds no process-domain capability"));
        assert!(say(abi::Error::WrongObject).contains("not a supervision endpoint"));
        assert!(say(abi::Error::NotPermitted).contains("no READ"));
        assert!(say(abi::Error::Gone).contains("destroyed"));
        assert!(refusal(0).contains("could not be read"));
    }

    /// The two right-aligned columns pad by the width of the digits written: a tid in twelve
    /// columns and a CPU figure in eight. A one-digit value cannot tell the digit count from its
    /// quotient or sum with the buffer offset, so these use several widths.
    #[test]
    fn the_columns_pad_to_their_widths() {
        let tid = |t| shown(|o| write_thread_id(t, o));
        assert_eq!(tid(7), "           7");
        assert_eq!(tid(1234), "        1234");
        assert_eq!(tid(123_456_789_012), "123456789012");
        assert_eq!(tid(1_234_567_890_123), "1234567890123");
        let ms = |m| shown(|o| write_ms(Some(m), o));
        assert_eq!(ms(0), "       0");
        assert_eq!(ms(12_345), "   12345");
        assert_eq!(ms(12_345_678), "12345678");
        assert_eq!(ms(123_456_789), "123456789");
        assert_eq!(shown(|o| write_ms(None, o)), "       -");
        let cell = |v, w| shown(|o| write_cell(v, w, o));
        assert_eq!(cell(Some(7), 4), "   7");
        assert_eq!(cell(Some(12), 4), "  12");
        assert_eq!(cell(Some(123), 4), " 123");
        assert_eq!(cell(Some(12_345), 4), "12345");
        assert_eq!(cell(None, 3), "  -");
    }

    /// A CPU-time reader over a canned domain: `entries` in slot order, then done.
    fn times(entries: &'static [(u64, u64)]) -> impl FnMut(u64) -> (i64, u64, u64) {
        move |cursor: u64| match entries.get(cursor as usize) {
            Some(&(tid, ms)) => (cursor as i64 + 1, tid, ms),
            None => (abi::survey::DONE as i64, 0, 0),
        }
    }

    /// **The second walk fills the column, joined on the tid rather than on the position.**
    ///
    /// The two walks are given the domain in *different orders* on purpose. Joining on position
    /// would pass on a canned reader that returned them in the same order and would silently
    /// mis-attribute every figure the first time a slot was reused between two calls.
    #[test]
    fn a_second_walk_fills_the_time_column_by_tid() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = collect(
            &mut rows,
            &mut domain(&[(3, abi::survey::RUNNING), (5, abi::survey::BLOCKED)]),
        );
        s.join_cpu_time(&mut times(&[(5, 20), (3, 990)]));

        assert_eq!(s.rows()[0].tid, 3);
        assert_eq!(s.rows()[0].cpu_ms, Some(990));
        assert_eq!(s.rows()[1].cpu_ms, Some(20));
    }

    /// **A thread that died between the two walks has no figure, and that is printed as `-`.**
    ///
    /// The alternative is a zero, which reads as "this thread has used no CPU" and is a claim
    /// nobody made. It is the same distinction the empty domain and the refusal are kept apart by,
    /// one column over.
    #[test]
    fn a_thread_missing_from_the_second_walk_keeps_no_figure() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = collect(
            &mut rows,
            &mut domain(&[(3, abi::survey::RUNNING), (5, abi::survey::RUNNING)]),
        );
        s.join_cpu_time(&mut times(&[(3, 40)]));

        assert_eq!(s.rows()[1].cpu_ms, None);
        let out = shown(|o| s.write_report(o));
        assert!(out.lines().nth(2).unwrap().ends_with('-'), "{out}");
    }

    /// A refusal in the **second** walk is a refusal, exactly as one in the first is. A table with
    /// a blank column and no complaint would be the failure this crate exists to avoid, wearing a
    /// new hat.
    #[test]
    fn a_refusal_in_the_second_walk_is_still_a_refusal() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = collect(&mut rows, &mut domain(&[(3, abi::survey::RUNNING)]));
        assert!(!s.is_refused());
        s.join_cpu_time(&mut |_| (abi::Error::Gone as i64, 0, 0));
        assert!(s.is_refused());
        assert_eq!(shown(|o| s.write_report(o)), "");
        assert!(shown(|o| s.write_diagnostics(o)).contains("destroyed"));
    }

    /// A second walk whose cursor does not advance terminates, the same guard `collect` carries.
    #[test]
    fn a_second_walk_that_does_not_advance_ends() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = collect(&mut rows, &mut domain(&[(3, abi::survey::RUNNING)]));
        s.join_cpu_time(&mut |_| (1, 3, 10));
        assert!(s.is_refused());
    }

    /// **The ranking is what makes a listing a `top`**: most CPU first, ties on the tid, and a row
    /// with no figure last rather than first.
    #[test]
    fn ranking_puts_the_busiest_first_and_the_unknown_last() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = collect(
            &mut rows,
            &mut domain(&[
                (3, abi::survey::RUNNING),
                (5, abi::survey::RUNNING),
                (7, abi::survey::RUNNING),
                (9, abi::survey::RUNNING),
            ]),
        );
        // 9 gets no figure at all; 3 and 7 tie at 100.
        s.join_cpu_time(&mut times(&[(3, 100), (5, 900), (7, 100)]));
        s.rank_by_cpu_time();

        let order: Vec<u64> = s.rows().iter().map(|r| r.tid).collect();
        assert_eq!(order, vec![5, 3, 7, 9], "ranked: {order:?}");
    }

    /// The `TIME` column appears only when a caller asked for it, so a listing that ran one walk is
    /// not handed a column of dashes to explain.
    #[test]
    fn the_time_column_appears_only_when_it_was_asked_for() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = collect(&mut rows, &mut domain(&[(3, abi::survey::RUNNING)]));
        assert!(!shown(|o| s.write_report(o)).contains("TIME"));
        s.join_cpu_time(&mut times(&[(3, 250)]));
        let out = shown(|o| s.write_report(o));
        assert!(out.contains("TIME(ms)"), "{out}");
        assert!(out.lines().nth(1).unwrap().ends_with("250"), "{out}");
    }

    /// A state code from a kernel newer than this program prints as `?` rather than panicking or
    /// being silently dropped.
    #[test]
    fn an_unknown_state_is_shown_as_unknown() {
        assert_eq!(state_name(99), "?");
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let s = collect(&mut rows, &mut domain(&[(3, 99)]));
        assert!(shown(|o| s.write_report(o)).contains('?'));
    }

    /// Every refusal reads as a fact about authority. The catalogue is checked so the wording
    /// cannot drift into errno.
    #[test]
    fn every_refusal_names_what_is_not_held() {
        for code in [
            abi::Error::NoSuchSlot as i64,
            abi::Error::NotPermitted as i64,
            abi::Error::WrongObject as i64,
            abi::Error::Gone as i64,
            -99,
        ] {
            let m = refusal(code);
            assert!(!m.is_empty());
            assert!(
                m.chars().next().is_some_and(|c| c.is_lowercase()),
                "a refusal is a clause the program name prefixes: {m}",
            );
        }
    }

    /// A process-label reader over a canned domain: `(tid, label)` in slot order, then done.
    fn labels(entries: &'static [(u64, u64)]) -> impl FnMut(u64) -> (i64, u64, u64) {
        times(entries)
    }

    /// Three threads in one process and one in another, in the order the kernel meets them.
    const FOUR: &[(u64, u64)] = &[(3, 1), (4, 2), (5, 1), (6, 1)];

    fn four_threads<'a>(rows: &'a mut [Row; MAXIMUM_ROWS]) -> Survey<'a> {
        collect(
            rows,
            &mut domain(&[
                (3, abi::survey::RUNNING),
                (4, abi::survey::BLOCKED),
                (5, abi::survey::BLOCKED),
                (6, abi::survey::READY),
            ]),
        )
    }

    /// **Threads of one process share a label, and the count is the rows that carry it.**
    ///
    /// Joined on the tid like `TIME`, with the process walk in a different order from the state
    /// walk, so a join on position would mislabel.
    #[test]
    fn a_process_walk_labels_threads_and_counts_them_from_the_rows() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = four_threads(&mut rows);
        s.join_process(&mut labels(&[(6, 1), (5, 1), (4, 2), (3, 1)]));

        let label: Vec<_> = s.rows().iter().map(|r| r.process).collect();
        assert_eq!(label, [Some(1), Some(2), Some(1), Some(1)]);
        assert_eq!(s.threads_with_label(1), 3);
        assert_eq!(s.threads_with_label(2), 1);
        assert_eq!(s.threads_with_label(9), 0);
    }

    /// **The table shows the label and the count on every row**, in front of the columns it
    /// already had, so a line reading `TID  STATE` is still a substring of the header.
    #[test]
    fn the_table_prints_a_label_and_a_thread_count() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = four_threads(&mut rows);
        s.join_process(&mut labels(FOUR));
        let text = shown(|o| s.write_report(o));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "PROC  THR         TID  STATE");
        assert_eq!(lines[1], "   1    3           3  running");
        assert_eq!(lines[2], "   2    1           4  blocked");
        assert_eq!(lines[3], "   1    3           5  blocked");
        assert_eq!(lines[4], "   1    3           6  ready");
        assert_eq!(lines.len(), 5);
    }

    /// **A thread that died between the walks prints `-` and is not counted into a process it
    /// was never seen in**, the way a missing `TIME` figure degrades. The survivors' count is
    /// still the number of rows that carry the label.
    #[test]
    fn a_thread_that_dies_between_the_walks_degrades_to_a_dash() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = four_threads(&mut rows);
        s.join_process(&mut labels(&[(3, 1), (4, 2), (6, 1)]));
        assert_eq!(s.rows()[2].process, None);
        assert_eq!(s.threads_with_label(1), 2);
        let text = shown(|o| s.write_report(o));
        assert!(text.contains("   -    -           5  blocked"), "{text}");
        assert!(text.contains("   1    2           3  running"), "{text}");
    }

    /// A label of 0 is the kernel saying it has none, and prints as a dash rather than a group.
    #[test]
    fn a_zero_label_is_no_label() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = four_threads(&mut rows);
        s.join_process(&mut labels(&[(3, 0)]));
        assert_eq!(s.rows()[0].process, None);
    }

    /// A refusal on the process walk is a refusal of the listing, as it is on the `TIME` walk.
    #[test]
    fn a_refusal_in_the_process_walk_is_still_a_refusal() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = four_threads(&mut rows);
        s.join_process(&mut |_| (abi::Error::NotPermitted as i64, 0, 0));
        assert!(s.is_refused());
    }

    /// **No process column appears when nobody asked.**
    #[test]
    fn the_process_columns_appear_only_when_they_were_asked_for() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let s = four_threads(&mut rows);
        let text = shown(|o| s.write_report(o));
        assert!(!text.contains("PROC"));
        assert!(!text.contains("THR"));
    }

    /// **Ranking by process puts the process that spent the most first, and keeps its threads
    /// together.** One process of three modest threads outranks a single busier thread, which is
    /// the case a per-thread ranking gets backwards.
    #[test]
    fn ranking_by_process_sums_the_threads_and_keeps_them_together() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = four_threads(&mut rows);
        s.join_process(&mut labels(FOUR));
        // Process 1 has 40 + 30 + 20 = 90; process 2 has one thread at 80.
        s.join_cpu_time(&mut times(&[(3, 40), (4, 80), (5, 30), (6, 20)]));
        s.rank_by_process_cpu_time();
        let tids: Vec<_> = s.rows().iter().map(|r| r.tid).collect();
        assert_eq!(tids, [3, 5, 6, 4]);

        // The per-thread ranking puts the single 80 ms thread first, which is the contrast.
        s.rank_by_cpu_time();
        assert_eq!(s.rows()[0].tid, 4);
    }

    /// An unlabeled row is its own group and sorts after every process; an empty table is fine.
    #[test]
    fn ranking_by_process_sorts_the_unlabeled_last_and_survives_nothing() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = four_threads(&mut rows);
        s.join_process(&mut labels(&[(4, 2), (5, 1)]));
        s.join_cpu_time(&mut times(&[(3, 999), (4, 10), (5, 5), (6, 7)]));
        s.rank_by_process_cpu_time();
        let tids: Vec<_> = s.rows().iter().map(|r| r.tid).collect();
        assert_eq!(tids, [4, 5, 3, 6]);

        let mut none = [Row::default(); MAXIMUM_ROWS];
        let mut empty = collect(&mut none, &mut domain(&[]));
        empty.rank_by_process_cpu_time();
        assert!(empty.rows().is_empty());
    }

    /// **The process with the larger total comes first even when its label is larger.** Label
    /// order and total order must disagree for the sum to be what ranks: process 2 spends 100 ms
    /// across two threads, process 1 spends 60 ms in one.
    #[test]
    fn a_larger_total_outranks_a_smaller_label() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = four_threads(&mut rows);
        s.join_process(&mut labels(&[(3, 1), (4, 2), (5, 2), (6, 2)]));
        s.join_cpu_time(&mut times(&[(3, 60), (4, 30), (5, 30), (6, 40)]));
        s.rank_by_process_cpu_time();
        let tids: Vec<_> = s.rows().iter().map(|r| r.tid).collect();
        assert_eq!(tids, [6, 4, 5, 3]);
    }

    /// The widest domain the kernel can express has `MAXIMUM_ROWS` processes, so the last label
    /// is `MAXIMUM_ROWS` itself and its total counts like any other.
    #[test]
    fn the_highest_label_still_has_a_total() {
        let mut rows = [Row::default(); MAXIMUM_ROWS];
        let mut s = four_threads(&mut rows);
        let last = MAXIMUM_ROWS as u64;
        s.join_process(&mut |cursor| match cursor {
            0 => (1, 3, 1),
            1 => (2, 4, last),
            _ => (abi::survey::DONE as i64, 0, 0),
        });
        s.join_cpu_time(&mut times(&[(3, 10), (4, 90)]));
        s.rank_by_process_cpu_time();
        assert_eq!(s.rows()[0].tid, 4);
    }
}
