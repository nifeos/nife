//! **The loop that cannot hang**, which is the whole difficulty of this milestone.
//!
//! Reading a serial port is one `open` and one `read`. Knowing when to stop reading is the part
//! that has kept every board milestone waiting on a person with eyes, and it has three answers,
//! all of which have to be here at once: the boot got where it was going, the board announced a
//! failure, or nothing more is coming.
//!
//! # Why the read happens on another thread
//!
//! Because the deadline must hold **whatever the reader does**. `port::configure` asks the tty
//! layer for a read timeout (`min 0 time 10`), and when that works a read returns on its own after
//! a tenth of a second. When it does not work, and there are several ordinary ways for it not to
//! (an `stty` that failed and was only warned about, a device that ignores the setting, a file or
//! a pipe standing in for a port), a read blocks forever and a single-threaded loop blocks with it.
//!
//! So the read runs on a worker and this loop waits on a channel with a timeout. The worker may
//! still be parked in `read` when we decide to stop; that is fine, because deciding to stop means
//! returning, and the process exits, and the descriptor goes with it. What must never happen is
//! this loop waiting on the board's goodwill, and it never does. This is `CLAUDE.md`'s
//! `Never leave QEMU running` rule wearing different clothes: an emulator that never exits and a
//! board that never speaks are the same bug from the tool's side.

use std::io::{self, Read, Write};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use crate::board;
use crate::progress::{BootProgress, Failure, LineFeeder, Stage};
use crate::stop::{Escape, Report};

/// When to stop.
#[derive(Debug, Clone)]
pub struct Policy {
    /// The hard cap. The session always ends by here, and this is the promise that the tool does
    /// not hang.
    pub total: Duration,
    /// Stop early once the boot reaches this stage, if a stage was asked for.
    ///
    /// `None` means "watch for the whole duration", which is what sustained stress wants:
    /// `design/fatal-risks/README.md`'s multicore entry needs a long look, not a boot check.
    pub until: Option<Stage>,
    /// Stop if the board falls silent for this long **after it has said something**.
    ///
    /// The "after" is load-bearing. Before the first byte, silence means nobody has pressed power
    /// yet, and a tool that gave up on that would be unusable at a bench where the operator is
    /// three feet from the switch. Once bytes have flowed, silence is the runbook's
    /// `Starting kernel ...`-then-nothing row, which is what a multicore hang looks like.
    ///
    /// `None` disables it.
    ///
    /// **Suppressed once the boot tour completes, and re-armed if a soak starts** (milestone 219).
    /// The kernel halts in `wfi` after its last line, so quiet at exactly [`Stage::Tour`] is normal
    /// termination and reporting it as a hang would fail every good boot. A `--features soak_test`
    /// kernel does not halt: it prints a heartbeat on the wall clock every five seconds whatever
    /// the workload is doing, and reaching [`Stage::Soak`] says so, so from there silence means the
    /// thing that prints is wedged.
    ///
    /// **That asymmetry is the whole of how this tool tells a hang from a slow run**, and it works
    /// because the heartbeat is not on the work: a machine crawling at one round trip a second
    /// still speaks on time, with a rate that says it is crawling. `kernel/src/soak.rs` is the
    /// other half of the agreement and its beat interval (five seconds) is chosen against this
    /// field's default (fifteen), so a run is called a hang after three missed beats.
    pub quiet_after: Option<Duration>,
    /// After [`Self::until`] is reached, keep reading this long before calling it a success.
    ///
    /// **This exists because the two captured successes and the captured refusal all contain the
    /// nife banner.** A boot that halts at the measured-boot gate prints `Starting kernel ...`,
    /// the banner, and most of a tour before refusing, so a watcher that returned the instant its
    /// stage arrived would report a refusal as a success, and would do it in the case a bench
    /// script most needs to be right about. Two seconds of extra reading buys the difference.
    ///
    /// Zero disables it, at that cost.
    pub settle: Duration,
    /// **Which board's firmware prologue to expect** (milestone 324 part 3).
    ///
    /// It lives here rather than being a parameter because a `Policy` already describes the session
    /// rather than only its clocks: [`Self::until`] is a [`Stage`], and a `Stage::Firmware` is a
    /// rung *of a profile*, so the board was already implicit in this struct and is now written
    /// down. A session has exactly one, chosen before any byte is read.
    ///
    /// The default is [`board::RADON`], which is what every caller watched before profiles existed;
    /// see [`board`]'s `BUGS` for why that is cheap rather than correct over an emulator.
    pub board: &'static board::Profile,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            total: Duration::from_secs(120),
            until: Some(Stage::Banner),
            quiet_after: Some(Duration::from_secs(15)),
            settle: Duration::from_secs(2),
            board: &board::RADON,
        }
    }
}

/// Why the session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The stage the caller asked for arrived.
    Reached(Stage),
    /// The board said outright that something was wrong.
    Announced(Failure),
    /// It spoke, then stopped, for longer than [`Policy::quiet_after`].
    WentQuiet,
    /// **A writing mode finished its business** (milestone 324): the escape was acknowledged, or
    /// the write failed, or the board said there was nothing to stop. What happened is in
    /// [`Session::stop`], and the exit status is computed from that rather than from this.
    ///
    /// It is a fifth way a session can end rather than a kind of [`Outcome::Reached`], because
    /// `reached soak running` would be true of a session that sent nothing and says nothing about
    /// the only thing `--stop` was run to find out.
    Stopped,
    /// The source ended: a replayed log ran out, or the device reported end of file.
    Ended,
    /// [`Policy::total`] elapsed.
    RanOut,
}

/// Everything one session learned, whether it succeeded or not.
#[derive(Debug)]
pub struct Session {
    /// Why it ended.
    pub outcome: Outcome,
    /// How far the boot got, and what it announced.
    pub progress: BootProgress,
    /// Bytes read from the port, which is the first thing to look at when a session is confusing:
    /// zero bytes is a wiring or a power question, not a software one.
    pub bytes: u64,
    /// Wall time from the first read to the decision.
    pub elapsed: Duration,
    /// The stage that was asked for, kept so the exit status can be computed here rather than
    /// re-derived by every caller.
    pub wanted: Option<Stage>,
    /// What the writing mode did, if one was asked for (milestone 324). `None` for every reading
    /// session, which is still every session that does not name a stop mode.
    pub stop: Option<Report>,
}

impl Session {
    /// The process exit status this session deserves.
    ///
    /// `0` reached what was asked for (or, with no stage asked for, watched the whole duration
    /// without the board announcing a failure). `1` the board announced a failure. `2` it went
    /// quiet. `3` the time ran out with the requested stage unreached. Callers add `4` for a port
    /// that could not be opened, which is not a session at all.
    ///
    /// **A writing mode adds no status and reuses `3`** (milestone 324). A stop that was asked for
    /// and not confirmed is the time running out with the requested thing unreached, which is what
    /// `3` already means; giving it a sixth code would make every bench script that already reads
    /// these five wrong. A failure the board announced and a hang still win, because those are
    /// facts about the board and an unconfirmed stop is a fact about this tool.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        let base = match &self.outcome {
            Outcome::Reached(_) | Outcome::Stopped => 0,
            Outcome::Announced(_) => 1,
            Outcome::WentQuiet => 2,
            // With no stage requested, running out of time IS the plan, and so is a replayed log
            // reaching its end. Neither is a failure unless something was being waited for.
            Outcome::RanOut | Outcome::Ended => {
                if self.wanted.is_some() {
                    3
                } else {
                    0
                }
            }
        };
        match &self.stop {
            Some(report) if base == 0 && !report.reached_the_goal() => 3,
            _ => base,
        }
    }

    /// One line a person can read, and the line the tool prints last.
    #[must_use]
    pub fn summary(&self) -> String {
        let reached = self.progress.reached();
        let head = match &self.outcome {
            Outcome::Reached(stage) => format!("reached {stage}"),
            Outcome::Announced(failure) => format!("failed: {}", failure.describe()),
            Outcome::WentQuiet => format!("went quiet after {reached}"),
            Outcome::Stopped => match &self.stop {
                Some(report) => report.describe(),
                // Unreachable through `watch`, which only sets this outcome from an escape that
                // has a report. Spelled out rather than unwrapped so a future caller cannot make
                // the summary panic at a bench.
                None => "the writing mode ended the session".to_string(),
            },
            Outcome::Ended => format!("input ended after {reached}"),
            Outcome::RanOut => format!("time ran out after {reached}"),
        };
        format!(
            "{head} ({} bytes in {:.1}s)",
            self.bytes,
            self.elapsed.as_secs_f64()
        )
    }
}

/// What the reader worker hands back.
enum Read1 {
    Bytes(Vec<u8>),
    /// The source is finished. A file or a pipe reports this; a serial port configured with
    /// `min 0 time 10` does not, which is why `stream_never_ends` exists.
    Ended,
    Failed(io::Error),
}

/// Watch a source until the policy says to stop, copying every byte to `sink` as it arrives.
///
/// `stream_never_ends` says how to read a zero-byte result. For a serial port it is `true`: a
/// timed-out `read` returns nothing and the board is simply not talking yet. For a replayed log it
/// is `false`, where nothing means the file is over.
///
/// `sink` is written and flushed per chunk, deliberately, and the caller is expected to have it
/// pointed at a file. A console session whose evidence exists only in a terminal that has since
/// scrolled is the failure this tree keeps writing down; flushing per chunk is what makes the log
/// survive the tool being killed.
///
/// # Errors
///
/// Returns the sink's error if the log cannot be written, and the source's error if the read
/// failed for a reason other than reaching the end. A read error does **not** discard the session:
/// callers get the error, and the bytes are already in the log.
///
/// # Examples
///
/// Replay a captured log, which is how a session is re-read and how the recogniser is exercised
/// with no board present:
///
/// ```no_run
/// use std::fs::File;
/// use serial_console::watch::{self, Policy};
///
/// let captured = File::open("target/board-console.log")?;
/// let mut nowhere = std::io::sink();
/// // `false`: end of file means the log is over, not that the board went quiet.
/// let session = watch::watch(captured, &mut nowhere, &Policy::default(), false)?;
/// println!("{}", session.summary());
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn watch<R>(
    source: R,
    sink: &mut dyn Write,
    policy: &Policy,
    stream_never_ends: bool,
) -> io::Result<Session>
where
    R: Read + Send + 'static,
{
    watch_with(source, sink, policy, stream_never_ends, None)
}

/// [`watch`], with a writing mode attached (milestone 324).
///
/// The reading half is identical and is deliberately not duplicated: a second copy of this loop
/// would be a second place for the deadline to be got wrong, and the deadline is the whole
/// milestone. What `escape` adds is one call per complete line and one check per iteration.
///
/// **Complete lines only reach the escape.** The partial tail is offered to
/// [`BootProgress::observe_partial`] as before, because a stage ratchet is monotone and more bytes
/// cannot unmake it. Sending a byte is not monotone: it cannot be unsent, so it may only be decided
/// on evidence that has finished arriving. That asymmetry is the reason the two are fed
/// differently, and it is the one difference between this function and [`watch`].
///
/// # Errors
///
/// As [`watch`]. A failure to write to the *board* is not an error here: it is recorded in
/// [`Session::stop`] and logged, because the board is still talking and the session is still
/// worth finishing.
pub fn watch_with<R>(
    source: R,
    sink: &mut dyn Write,
    policy: &Policy,
    stream_never_ends: bool,
    mut escape: Option<Escape<'_>>,
) -> io::Result<Session>
where
    R: Read + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    // Detached on purpose: it is never joined. It may be parked in `read` when this function
    // returns, and the only thing that unparks it is the process exiting, which closes the
    // descriptor. Joining it would reintroduce exactly the hang this design removes.
    thread::spawn(move || reader(source, &tx, stream_never_ends));

    let started = Instant::now();
    let mut feeder = LineFeeder::new();
    let mut progress = BootProgress::new(policy.board);
    let mut bytes = 0u64;
    let mut spoke_at: Option<Instant> = None;
    // Set when the wanted stage arrives; the session then ends when the settle window closes, or
    // sooner if a failure turns up inside it, which is the whole reason for waiting.
    let mut settling: Option<Instant> = None;
    let mut outcome = Outcome::RanOut;
    let mut error: Option<io::Error> = None;

    loop {
        let left = policy.total.saturating_sub(started.elapsed());
        if left.is_zero() {
            break;
        }
        // Never wait longer than the shortest thing we owe an answer about. The 250 ms floor keeps
        // the loop responsive without busy-waiting; nothing here depends on it being that number.
        let wait = left.min(Duration::from_millis(250));

        match rx.recv_timeout(wait) {
            Ok(Read1::Bytes(chunk)) => {
                bytes += chunk.len() as u64;
                spoke_at = Some(Instant::now());
                sink.write_all(&chunk)?;
                sink.flush()?;

                feed_lines(
                    &chunk,
                    &mut feeder,
                    &mut progress,
                    escape.as_mut(),
                    started,
                    sink,
                )?;

                // Failure first, so a chunk that both reaches the banner and announces a refusal
                // (which is exactly what boot 12 looked like) reports the refusal.
                if let Some(failure) = progress.failure() {
                    outcome = Outcome::Announced(failure.clone());
                    break;
                }
                if settling.is_none()
                    && let Some(wanted) = policy.until
                    && progress.reached() >= wanted
                {
                    settling = Some(Instant::now());
                }
            }
            Ok(Read1::Ended) => {
                // A source that ends after the wanted stage arrived has answered the question;
                // only one that ends before it has run out. This is the replay case, where the
                // settle window would otherwise turn every capture into two seconds of waiting.
                outcome = settled_or(&settling, &progress, Outcome::Ended);
                break;
            }
            Ok(Read1::Failed(e)) => {
                error = Some(e);
                outcome = Outcome::Ended;
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                outcome = settled_or(&settling, &progress, Outcome::Ended);
                break;
            }
        }

        // The writing mode's own ending, checked before the settle window and before the quiet
        // timer because it outranks both: a board that has acknowledged the escape has answered
        // the question this session was run to ask.
        if escape.as_ref().is_some_and(Escape::is_finished) {
            outcome = Outcome::Stopped;
            break;
        }

        if let Some(since) = settling
            && since.elapsed() >= policy.settle
        {
            outcome = Outcome::Reached(progress.reached());
            break;
        }

        if went_quiet(policy, spoke_at, settling, &progress) {
            outcome = Outcome::WentQuiet;
            break;
        }
    }

    if settling.is_some() && matches!(outcome, Outcome::RanOut | Outcome::WentQuiet) {
        outcome = Outcome::Reached(progress.reached());
    }

    let outcome = judge_the_tail(&feeder, &mut progress, outcome);

    let session = Session {
        outcome,
        progress,
        bytes,
        elapsed: started.elapsed(),
        wanted: policy.until,
        stop: escape.as_ref().map(|e| e.report().clone()),
    };
    match error {
        Some(e) => Err(io::Error::new(
            e.kind(),
            format!("{}: {e}", session.summary()),
        )),
        None => Ok(session),
    }
}

/// `Reached` if the wanted stage had already arrived, otherwise whatever the caller says.
fn settled_or(settling: &Option<Instant>, progress: &BootProgress, otherwise: Outcome) -> Outcome {
    if settling.is_some() {
        Outcome::Reached(progress.reached())
    } else {
        otherwise
    }
}

/// Feed one chunk's complete lines to the stage ratchet and, when a writing mode is attached, to
/// its escape; then offer the incomplete tail as a partial.
fn feed_lines(
    chunk: &[u8],
    feeder: &mut LineFeeder,
    progress: &mut BootProgress,
    mut escape: Option<&mut Escape<'_>>,
    started: Instant,
    sink: &mut dyn Write,
) -> io::Result<()> {
    let feeding = feeder.feed(chunk);
    for line in &feeding.lines {
        progress.observe_line(line);
        // The writing mode, and the only place in this crate a byte goes to the board.
        // After `observe_line` so the escape's own log annotation lands below the line
        // that provoked it, which is the order a reader of the capture needs.
        if let Some(escape) = escape.as_mut() {
            escape.observe_line(line, started.elapsed(), &mut *sink)?;
        }
    }
    // The incomplete tail too, or U-Boot's newline-less `StarFive #` prompt is never
    // seen. Offered as a *partial*, which is a weaker kind of evidence for the reasons
    // `observe_partial` records; safe to re-offer as it grows, because the ratchet
    // only moves forward.
    progress.observe_partial(&feeding.tail);
    Ok(())
}

/// Whether the board has been silent past `quiet_after` at a stage where silence is a hang.
///
/// Silence, and the two things that are not it. A board still inside its settle window is quiet
/// because we asked it to be. A board that finished its tour is quiet because the kernel halted in
/// `wfi`, which is how a good boot ends.
///
/// Named stages rather than `< Tour`, since milestone 219 (the boot tour ends and the kernel halts,
/// so there is nothing to soak): the exemption belongs to the stages where quiet is the correct end
/// state and not to everything past them. A soak has started and is under contract to keep
/// speaking, so its silence is the hang this tool exists to name.
///
/// **[`Stage::Prompt`] joined the exemption at milestone 268** (every architecture boots the same
/// way: describe the machine, test yourself, hand over), and for a different reason from `Tour`'s:
/// a kernel that finished its tour is quiet because it halted, and a system sitting at a `swish`
/// prompt is quiet because it is *waiting for somebody to type*. Both are correct terminal states
/// of a boot, and since that milestone the prompt is the one a default boot is supposed to reach.
///
/// **[`Stage::SweepDone`] joined it in part 2 of milestone 324** (the bench console cannot speak
/// to any of the three boards), on `Tour`'s reason exactly: `kernel/src/job_mix.rs` halts in `wfi`
/// after `job-mix: done`. [`Stage::Sweep`] is deliberately NOT here, and that asymmetry is the
/// whole of what part 2 bought: a sweep that started and stopped is the wedge this tool exists to
/// name, where before it and a finished sweep both ended as the clock running out. What the
/// exemption cannot do is shorten a subrun, so see `Stage::Sweep`'s own documentation on sizing
/// `quiet_after` against the slowest one.
fn went_quiet(
    policy: &Policy,
    spoke_at: Option<Instant>,
    settling: Option<Instant>,
    progress: &BootProgress,
) -> bool {
    let (Some(limit), Some(last)) = (policy.quiet_after, spoke_at) else {
        return false;
    };
    settling.is_none()
        && !matches!(
            progress.reached(),
            Stage::Tour | Stage::Prompt | Stage::SweepDone
        )
        && last.elapsed() >= limit
}

/// Nothing more is coming, so the tail is as complete as it will ever be. This is what catches
/// a last line printed without a newline, which a panic on a machine that then halts can be.
/// It can only ever turn an inconclusive ending into a named failure, never the other way.
fn judge_the_tail(feeder: &LineFeeder, progress: &mut BootProgress, outcome: Outcome) -> Outcome {
    if !feeder.tail().is_empty() {
        progress.observe_line(feeder.tail());
        if let Some(failure) = progress.failure()
            && matches!(
                outcome,
                Outcome::RanOut | Outcome::WentQuiet | Outcome::Ended
            )
        {
            return Outcome::Announced(failure.clone());
        }
    }
    outcome
}

fn reader<R: Read>(mut source: R, tx: &mpsc::Sender<Read1>, stream_never_ends: bool) {
    let mut buffer = [0u8; 4096];
    loop {
        match source.read(&mut buffer) {
            Ok(0) => {
                if stream_never_ends {
                    // A configured port with `min 0 time 10` already blocked for a tenth of a
                    // second before returning nothing. If the configuration did not take, this
                    // sleep is what stops the loop spinning a core while the board is quiet.
                    thread::sleep(Duration::from_millis(20));
                    continue;
                }
                let _ = tx.send(Read1::Ended);
                return;
            }
            Ok(n) => {
                if tx.send(Read1::Bytes(buffer[..n].to_vec())).is_err() {
                    return;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => {
                let _ = tx.send(Read1::Failed(e));
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A source that blocks forever without ever yielding a byte, which is what a board that is
    /// powered off looks like through a port whose read timeout did not take. This is the one the
    /// whole threaded design exists for.
    struct NeverSpeaks;

    impl Read for NeverSpeaks {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            thread::sleep(Duration::from_secs(3600));
            Ok(0)
        }
    }

    /// Says everything it has, in whatever chunks the caller's buffer allows, then blocks forever.
    ///
    /// The chunking is not incidental. A real UART hands over whatever has arrived, so a source
    /// that assumed the reader's buffer could take a whole transcript would be testing a stream
    /// nothing produces (and would panic the moment a capture outgrew 4 KiB, which is how this was
    /// found).
    struct SpeaksThenStops(Vec<u8>);

    impl SpeaksThenStops {
        fn new(text: impl Into<Vec<u8>>) -> Self {
            Self(text.into())
        }
    }

    impl Read for SpeaksThenStops {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if self.0.is_empty() {
                thread::sleep(Duration::from_secs(3600));
                return Ok(0);
            }
            let n = self.0.len().min(out.len());
            out[..n].copy_from_slice(&self.0[..n]);
            self.0.drain(..n);
            Ok(n)
        }
    }

    /// **The writing mode through the real loop** (milestone 324), which is the half the pure
    /// tests in [`stop`](crate::stop) cannot reach: line assembly from a chunked stream, the
    /// escape's annotations landing in the same sink as the board's own bytes, and the session
    /// ending on the acknowledgement rather than on the clock.
    ///
    /// The board here is a transcript rather than silicon. No byte this test sends has ever
    /// reached one; see `stop`'s `BUGS`.
    #[test]
    fn a_stop_mode_sends_one_byte_and_ends_on_the_boards_acknowledgement() {
        let transcript = concat!(
            "U-Boot SPL 2021.10 (Feb 12 2023 - 20:24:34 +0800)\r\n",
            "Starting kernel ...\r\n",
            "nife on RISC-V (rv64, S-mode, Sv39)\r\n",
            "soak-test: started 4 groups of one responder, 3 callers (20 user threads)\r\n",
            "soak-test-reboot: THIS BUILD REBOOTS THE BOARD. It soaks for 120s, then asks the \
             firmware for a cold reboot (SBI SRST reset type 1).\r\n",
            "soak-test: t=5s beat=1 rounds=100 rate=20/s workers=20 refused=0 mismatch=0 stalled=0\r\n",
            "soak-test-reboot: DISARMED at t=5s: a byte arrived on this console.\r\n",
        );

        let mut port: Vec<u8> = Vec::new();
        let mut log: Vec<u8> = Vec::new();
        let session = {
            let escape = Escape::new(1, &mut port);
            watch_with(
                SpeaksThenStops::new(transcript),
                &mut log,
                // No stage to wait for: with a stop mode the escape is what ends the session, and
                // this is what `cargo xtask board-console` sets when no `--until` is given.
                &quick(None, None),
                true,
                Some(escape),
            )
            .expect("the log is a Vec and the source does not fail")
        };

        assert_eq!(port, vec![crate::stop::ESCAPE_BYTE], "one byte, once");
        assert_eq!(session.outcome, Outcome::Stopped);
        assert_eq!(session.stop, Some(Report::Confirmed));
        assert_eq!(session.exit_code(), 0);

        let log = String::from_utf8(log).expect("the transcript is ASCII");
        assert!(
            log.contains("sending the soak escape"),
            "the log must show the byte that went out"
        );
        assert!(log.contains("0x0d"));
        assert!(
            log.contains("soak-test-reboot: DISARMED"),
            "the board's own bytes are still in the log beside the annotations"
        );
    }

    /// A stop that was asked for and never sent is the time running out with the requested thing
    /// unreached, which is exit 3 and not a quiet success.
    #[test]
    fn a_stop_that_never_had_an_armed_loop_to_stop_exits_three() {
        let mut port: Vec<u8> = Vec::new();
        let mut log: Vec<u8> = Vec::new();
        let session = {
            let escape = Escape::new(1, &mut port);
            watch_with(
                SpeaksThenStops::new("nife on RISC-V (rv64, S-mode, Sv39)\r\n"),
                &mut log,
                &quick(None, None),
                true,
                Some(escape),
            )
            .expect("the log is a Vec")
        };

        assert!(port.is_empty(), "nothing announced an armed reboot loop");
        assert_eq!(session.stop, Some(Report::NotSent));
        assert_eq!(session.exit_code(), 3);
    }

    fn quick(until: Option<Stage>, quiet_after: Option<Duration>) -> Policy {
        Policy {
            total: Duration::from_millis(1500),
            until,
            quiet_after,
            settle: Duration::from_millis(50),
            // radon, the same default `Policy::default` takes, so these tests exercise the
            // profile-driven prologue rather than an empty one.
            board: &board::RADON,
        }
    }

    #[test]
    fn a_good_boot_is_recognised_and_stops_early() {
        let log = include_bytes!("../tests/fixtures/captured/vf2-2026-09-01-manual-boot.log");
        let mut sink = Vec::new();
        let policy = Policy {
            total: Duration::from_secs(10),
            ..Policy::default()
        };
        let session = watch(&log[..], &mut sink, &policy, false).unwrap();
        assert_eq!(session.outcome, Outcome::Reached(Stage::Tour));
        assert_eq!(session.exit_code(), 0);
        assert!(!sink.is_empty());
        // The tally is the byte count, not merely "some bytes arrived": every byte that reaches
        // the sink is counted, once, on the way past.
        assert_eq!(
            session.bytes,
            sink.len() as u64,
            "the byte tally must match what actually reached the log"
        );
    }

    /// The evidence has to be on disk whatever happened, and this is the case where it matters
    /// most: the session failed and the only account of it is the log.
    #[test]
    fn the_log_is_written_even_when_the_boot_fails() {
        let log = include_bytes!("../tests/fixtures/synthetic/vf2-bad-magic.log");
        let mut sink = Vec::new();
        let session = watch(&log[..], &mut sink, &Policy::default(), false).unwrap();
        assert_eq!(
            session.outcome,
            Outcome::Announced(Failure::FirmwareRefused {
                diagnosis: "U-Boot rejected the image header (Bad Linux RISCV Image magic!)",
                reason: String::new(),
            })
        );
        assert_eq!(session.exit_code(), 1);
        assert!(String::from_utf8_lossy(&sink).contains("Bad Linux RISCV Image magic!"));
    }

    /// The property the milestone is actually about: whatever the board does, the tool returns.
    #[test]
    fn a_source_that_never_speaks_does_not_hang_the_tool() {
        let mut sink = Vec::new();
        let started = Instant::now();
        let session = watch(
            NeverSpeaks,
            &mut sink,
            &quick(Some(Stage::Banner), None),
            true,
        )
        .unwrap();
        assert_eq!(session.outcome, Outcome::RanOut);
        assert_eq!(session.exit_code(), 3);
        assert_eq!(session.bytes, 0);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    /// Silence only counts once the board has spoken. A tool that started the clock at `open`
    /// would give up while the operator was still walking to the power switch.
    #[test]
    fn silence_before_the_first_byte_is_not_a_hang() {
        let mut sink = Vec::new();
        let policy = quick(Some(Stage::Banner), Some(Duration::from_millis(200)));
        let session = watch(NeverSpeaks, &mut sink, &policy, true).unwrap();
        assert_eq!(session.outcome, Outcome::RanOut);
    }

    #[test]
    fn silence_after_the_handoff_is_a_hang() {
        let mut sink = Vec::new();
        let policy = Policy {
            total: Duration::from_secs(10),
            until: Some(Stage::Banner),
            quiet_after: Some(Duration::from_millis(300)),
            settle: Duration::from_millis(50),
            board: &board::RADON,
        };
        let source =
            SpeaksThenStops::new(&b"Moving Image from 0x40200000\nStarting kernel ...\n"[..]);
        let session = watch(source, &mut sink, &policy, true).unwrap();
        assert_eq!(session.outcome, Outcome::WentQuiet);
        assert_eq!(session.exit_code(), 2);
        assert_eq!(
            session.progress.reached(),
            Stage::Firmware(board::RADON.rung("handoff").expect("radon hands off"))
        );
        assert!(session.progress.is_relocated());
    }

    /// Sustained watching, which is what `design/fatal-risks/README.md`'s multicore entry needs: no stage
    /// to wait for, so running the clock out is the plan rather than a disappointment.
    #[test]
    fn watching_for_a_duration_ends_successfully() {
        let mut sink = Vec::new();
        let session = watch(NeverSpeaks, &mut sink, &quick(None, None), true).unwrap();
        assert_eq!(session.outcome, Outcome::RanOut);
        assert_eq!(session.exit_code(), 0);
    }

    /// A replayed log that ends before the stage asked for is not a hang, and saying so keeps the
    /// two apart in the exit status.
    #[test]
    fn a_replay_that_runs_out_ends_rather_than_going_quiet() {
        let log = include_bytes!("../tests/fixtures/captured/vf2-2026-09-01-manual-boot.log");
        let mut sink = Vec::new();
        let policy = Policy {
            until: Some(Stage::Tour),
            ..Policy::default()
        };
        // Truncated just after the handoff, which is the shape of a capture that was stopped
        // early: everything before `Starting kernel ...` and nothing after it.
        let at = log
            .windows(19)
            .position(|w| w == b"Starting kernel ...")
            .unwrap()
            + 19;
        let session = watch(&log[..at], &mut sink, &policy, false).unwrap();
        assert_eq!(session.outcome, Outcome::Ended);
        assert_eq!(session.exit_code(), 3);
        assert_eq!(
            session.progress.reached(),
            Stage::Firmware(board::RADON.rung("handoff").expect("radon hands off"))
        );
    }

    /// **The case the third capture forced.** A boot that halts at the measured-boot gate prints
    /// `Starting kernel ...`, the whole banner, and most of a tour before refusing, so a watcher
    /// that returned the instant `--until banner` was satisfied would call it a success. Two
    /// seconds of settle is what buys the right answer, and this is the case a bench script most
    /// needs to be right about.
    #[test]
    fn a_refusal_after_the_banner_is_not_a_success() {
        let log =
            include_bytes!("../tests/fixtures/captured/vf2-2026-09-01-measured-boot-refused.log");
        let mut sink = Vec::new();
        let session = watch(&log[..], &mut sink, &Policy::default(), false).unwrap();
        assert_eq!(
            session.outcome,
            Outcome::Announced(crate::progress::Failure::MeasuredBootRefused)
        );
        assert_eq!(session.exit_code(), 1);
        assert!(
            session.progress.reached() >= Stage::Banner,
            "it really did get past the banner, which is what makes this hard"
        );
    }

    /// **Silence after the tour is how a good boot ends**, because the kernel halts in `wfi`. A
    /// watcher that called that a hang would fail every successful boot, and this one runs with a
    /// quiet timer far shorter than the watch itself to prove the suppression is real.
    #[test]
    fn quiet_after_the_tour_is_normal_termination_and_not_a_hang() {
        let full =
            include_bytes!("../tests/fixtures/captured/vf2-2026-09-01-userspace.log").to_vec();
        let mut sink = Vec::new();
        let policy = Policy {
            total: Duration::from_millis(900),
            until: None,
            quiet_after: Some(Duration::from_millis(100)),
            settle: Duration::from_millis(10),
            board: &board::RADON,
        };
        // Speaks the whole successful boot, then stops, exactly as the board does.
        let session = watch(SpeaksThenStops::new(full), &mut sink, &policy, true).unwrap();
        assert_eq!(session.outcome, Outcome::RanOut);
        assert_eq!(session.exit_code(), 0, "a good boot must not exit 2");
        assert_eq!(session.progress.reached(), Stage::Tour);
        assert!(session.progress.userspace_ran());
    }

    /// **A soak that stops speaking is a hang**, which is the whole of milestone 219's agreement
    /// between the workload and this tool. The fixture is a real QEMU riscv64 soak cut off after
    /// its second heartbeat, so what is being asserted is that reaching [`Stage::Soak`] re-arms
    /// the quiet check the completed boot tour in the same log had just suppressed.
    ///
    /// This is the test the exemption's shape would otherwise get wrong: the log reaches `Tour`
    /// (its last tour line is in there) and then reaches `Soak`, and a `< Stage::Tour` guard would
    /// have called the silence a normal halt.
    #[test]
    fn a_soak_that_goes_quiet_is_a_hang_even_though_the_tour_completed() {
        let cut = include_bytes!("../tests/fixtures/synthetic/qemu-soak-then-silence.log").to_vec();
        let mut sink = Vec::new();
        let policy = Policy {
            total: Duration::from_secs(5),
            until: None,
            quiet_after: Some(Duration::from_millis(200)),
            settle: Duration::from_millis(10),
            board: &board::RADON,
        };
        let session = watch(SpeaksThenStops::new(cut), &mut sink, &policy, true).unwrap();
        assert_eq!(session.outcome, Outcome::WentQuiet);
        assert_eq!(session.exit_code(), 2, "a stalled soak must exit 2");
        assert_eq!(session.progress.reached(), Stage::Soak);
        assert!(
            session.progress.reached() > Stage::Tour,
            "the tour completing must not exempt a soak from the quiet check"
        );
    }

    /// **A soak that keeps beating for the whole watch is a success**, and it hands back the number
    /// the run exists to produce. Same fixture, uncut, which is what `script/soak-test` sees.
    #[test]
    fn a_soak_that_keeps_beating_succeeds_and_reports_its_round_trip_total() {
        // A pre-297 capture, respelled at read time and not on disk; see
        // `crate::respell_pre_297_markers`.
        let full = crate::respell_pre_297_markers(include_str!(
            "../tests/fixtures/captured/qemu-2026-09-01-riscv64-soak.log"
        ))
        .into_bytes();
        let mut sink = Vec::new();
        // The whole capture arrives in one burst here, which a real board's 115200-baud trickle
        // does not, so the quiet window is set beyond the watch rather than inside it: what this
        // test is about is the parse and the success verdict, and the sibling above is the one
        // about silence.
        let policy = Policy {
            total: Duration::from_millis(400),
            until: None,
            quiet_after: Some(Duration::from_secs(30)),
            settle: Duration::from_millis(10),
            board: &board::RADON,
        };
        let session = watch(SpeaksThenStops::new(full), &mut sink, &policy, true).unwrap();
        assert_eq!(session.outcome, Outcome::RanOut);
        assert_eq!(session.exit_code(), 0);
        let beat = session.progress.soak().expect("the beats must be parsed");
        assert_eq!(beat.beat, 5, "the last heartbeat is the one that counts");
        assert_eq!(beat.rounds, 595_432);
        assert_eq!(beat.refused, 0);
        assert_eq!(beat.mismatches, 0);
        assert_eq!(beat.stalled, 0);
        // The finding milestone 219 measured rather than assumed: a saturated workload does not
        // migrate under this scheduler. Asserted so that a future run which DOES cross cores fails
        // this test and makes somebody read notes/soak.md.
        //
        // Milestone 221 did not change this number and could not: this is a captured log from
        // before the tick route existed, frozen on purpose, and its `wakes` parses as zero. A
        // fixture is a record of what a machine said, so it is the one thing here a later
        // mechanism must not update.
        assert_eq!(
            beat.crossings, 21,
            "this capture crossed cores 21 times in 25 seconds and then stopped; see notes/soak.md"
        );
    }

    /// The genuine hang, which is the one outcome with no real sample: the kernel starts, says a
    /// few lines, and stops before the tour. Nothing suppresses the quiet timer here, because the
    /// tour never completed.
    #[test]
    fn a_kernel_that_stops_before_the_tour_is_a_hang() {
        let full = include_bytes!("../tests/fixtures/synthetic/vf2-handoff-hang.log").to_vec();
        let mut sink = Vec::new();
        let policy = Policy {
            total: Duration::from_secs(10),
            until: Some(Stage::Tour),
            quiet_after: Some(Duration::from_millis(200)),
            settle: Duration::from_millis(10),
            board: &board::RADON,
        };
        let session = watch(SpeaksThenStops::new(full), &mut sink, &policy, true).unwrap();
        assert_eq!(session.outcome, Outcome::WentQuiet);
        assert_eq!(session.exit_code(), 2);
        assert_eq!(session.progress.reached(), Stage::Banner);
    }

    /// The captured failure, end to end: exit 1, not exit 2. U-Boot refusing is a failure the
    /// board announced, and the whole point of telling it from a hang is that a person reading
    /// exit 2 goes looking for a kernel bug while a person reading exit 1 resets the board.
    #[test]
    fn the_captured_extlinux_failure_exits_as_a_failure_not_a_hang() {
        let log = include_bytes!("../tests/fixtures/captured/vf2-2026-09-01-extlinux-refused.log");
        let mut sink = Vec::new();
        let session = watch(&log[..], &mut sink, &Policy::default(), false).unwrap();
        assert_eq!(session.exit_code(), 1);
        assert!(matches!(
            session.outcome,
            Outcome::Announced(crate::progress::Failure::FirmwareRefused { .. })
        ));
    }

    /// **`Session::summary`, read directly.** Every other test reaches it only through
    /// `exit_code` or `outcome`, never through the one line a person at a bench actually reads
    /// last; constructed rather than watched through, since every field here is `pub` for
    /// exactly this reason.
    #[test]
    fn the_summary_names_the_outcome_and_the_tally() {
        let session = Session {
            outcome: Outcome::Reached(Stage::Banner),
            progress: BootProgress::default(),
            bytes: 42,
            elapsed: Duration::from_millis(3_500),
            wanted: Some(Stage::Banner),
            stop: None,
        };
        let summary = session.summary();
        assert!(summary.contains("reached kernel banner"), "{summary}");
        assert!(summary.contains("42 bytes"), "{summary}");
        assert!(summary.contains("3.5s"), "{summary}");
    }

    /// **The last line, printed without a trailing newline, is still read.** This is the case
    /// the module header names outright: a panic followed by a halted machine. The tail-flush
    /// after the loop is the only thing that catches it, and nothing before this test called it.
    #[test]
    fn a_last_line_with_no_trailing_newline_is_still_read() {
        let log = "Starting kernel ...\nnife on RISC-V (rv64, S-mode, Sv39)\n\
                   [PANIC] hart 3 took a load fault";
        let mut sink = Vec::new();
        let session = watch(
            SpeaksThenStops::new(log),
            &mut sink,
            &quick(None, Some(Duration::from_millis(50))),
            true,
        )
        .unwrap();
        assert_eq!(
            session.outcome,
            Outcome::Announced(Failure::KernelPanic("hart 3 took a load fault".to_string())),
            "the unterminated last line must be read once the session ends"
        );
    }

    /// **`reader`'s one distinction: `Interrupted` is retried, and any other error is not.**
    /// Nothing else in the suite hands the source a real `io::Error`, so this is the only test
    /// that exercises the `Err` arm of the reader loop at all.
    #[test]
    fn an_interrupted_read_is_retried_and_a_real_error_is_not() {
        struct FlakyThenFails {
            interrupted_once: bool,
        }
        impl Read for FlakyThenFails {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                if !self.interrupted_once {
                    self.interrupted_once = true;
                    return Err(io::Error::from(io::ErrorKind::Interrupted));
                }
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }
        }

        let mut sink = Vec::new();
        let err = watch(
            FlakyThenFails {
                interrupted_once: false,
            },
            &mut sink,
            &quick(None, None),
            true,
        )
        .unwrap_err();
        assert_eq!(
            err.kind(),
            io::ErrorKind::PermissionDenied,
            "the interrupted read must have been retried rather than reported, and the real \
             error after it must have been reported rather than retried"
        );
    }
}
