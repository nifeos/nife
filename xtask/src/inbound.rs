//! The inbound-connection probe (milestone 140): a host-side client that connects to the
//! guest's listener through QEMU's port forward and says what came back.
//!
//! It lives on the host because the claim is about the outside world reaching in, which
//! nothing inside the guest can make on its own.

/// The bytes the inbound prober sends into the guest and the answer it requires back. They must
/// match `socket_contract::fixture` (`IN_MSG`/`OUT_MSG`), which is what the two guest programs read
/// them from, and they are deliberately different strings: an echo would pass even if the guest
/// were only reflecting our own bytes, and the point of this gate is that the guest **composed** an
/// answer to a connection it did not make.
///
/// **Spelled again here rather than imported**, which is the same call the pinned [MS-NLMP] vectors
/// beside the credential tests make: the claim is that two independently-written sides agree, and a
/// shared constant would let one edit move both. A drift is loud, because the guest's answer would
/// not match and the prober says exactly what came back instead.
const INBOUND_IN: &[u8] = b"nife-in!";
const INBOUND_OUT: &[u8] = b"nife-out!";
/// How many connections the guest **offers** over a whole boot: two each from the two programs that
/// listen on the forwarded port, one after the other (`socket_test_client`'s hand-written accept
/// role from milestone 107 (the socket contract learns to accept), and `std_exerciser`'s
/// `std::net::TcpListener` half from milestone 64 (enough `std` to run somebody else's crate)). The
/// prober keeps connecting until it has this many or the run ends, and it must keep going even once
/// it has enough to pass, because a guest sitting in `ACCEPT` needs a peer: stopping early would
/// hang the guest's own test rather than the host's.
const INBOUND_OFFERED: usize = 4;

/// How many of those the prober must actually **collect** for the leg to pass.
///
/// Neither program can supply more than two, so three collected means at least one from *each*
/// listener reached a process outside the machine, the half no in-guest assertion can make. The
/// fourth would only re-confirm the re-arm, which each guest test asserts itself
/// (`serve_one_inbound` fails the run if a second `accept` does not answer). Run 32195227733's
/// riscv64 leg collected three of four with all 279 guest tests passing, and the mechanism of that
/// lost round is not identified. Read notes/net/the-inbound-check.md before touching this number:
/// at two it proves nothing one listener could not fake.
const INBOUND_REQUIRED: usize = 3;

/// Ask the OS for a free TCP port on the loopback and let it go again.
///
/// There is a race between letting go and QEMU binding it, and it is the right trade: the
/// alternative is a *fixed* port, which two lanes running the suite on one machine collide on every
/// time rather than rarely. A lost race fails loudly (QEMU refuses to start), which is the failure
/// mode this project prefers to a quiet one.
fn free_loopback_port() -> Option<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
    drop(listener);
    Some(port)
}

/// **The host side of the inbound gate** (milestone 107): a host process that connects TO the
/// guest.
///
/// Nothing inside the guest can open a connection to it from outside, so QEMU's `hostfwd` forwards
/// a loopback port into the guest's listening port, and this thread connects, sends a payload, and
/// requires the guest's own answer back. It **retries for the whole run**, because nothing here
/// knows when the accept test starts: a connection that arrives while another net test holds the
/// NIC is reset by smoltcp at no cost. It stops once both rounds have completed. See
/// notes/net/the-inbound-half.md.
pub(crate) struct InboundProber {
    arch: String,
    port: Option<u16>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<(Result<(), String>, InboundTrace)>>,
    /// The one clock (milestone 514 (the HVF std listener hangs in some full runs)): the prober's
    /// events and the guest lines [`Self::mark`] stamps are both milliseconds since this instant,
    /// so a timeline can put a connection's open beside the guest's listen and accept without a
    /// second clock to reconcile.
    ///
    /// Name: provisional, milestone 514's lane, 2026-10-12 (UTC). The name `InboundTrace` already
    /// used for the same instant.
    started: std::time::Instant,
    /// Guest lines about the two listeners, stamped as the relay read them. See [`Self::mark`].
    ///
    /// Name: provisional, milestone 514's lane, 2026-10-12 (UTC). The plural of [`Self::mark`].
    marks: Vec<(u128, String)>,
    /// The previous line marked a listener test starting, so this one carries its verdict.
    ///
    /// Name: provisional, milestone 514's lane, 2026-10-12 (UTC). Says what the next line will be
    /// read for.
    marking_result: bool,
}

impl InboundProber {
    /// Pick the port, tell the runner about it, and start poking. Call this **before** the child is
    /// spawned: the runner reads `NIFE_HOSTFWD_PORT` from the environment it inherits.
    pub(crate) fn new(arch: &str) -> Self {
        let started = std::time::Instant::now();
        let Some(port) = free_loopback_port() else {
            eprintln!("inbound prober ({arch}): could not get a free loopback port");
            return Self {
                arch: arch.to_string(),
                port: None,
                stop: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
                thread: None,
                started,
                marks: Vec::new(),
                marking_result: false,
            };
        };
        // SAFETY: `set_var` became unsafe in edition 2024 because it races other threads. This runs
        // on the main thread before both the child that reads it and the prober thread below, and
        // that thread only touches sockets; no thread xtask starts ever writes the environment.
        unsafe { std::env::set_var("NIFE_HOSTFWD_PORT", port.to_string()) };

        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_thread = stop.clone();
        let thread = std::thread::spawn(move || probe_inbound(port, stop_thread, started));
        Self {
            arch: arch.to_string(),
            port: Some(port),
            stop,
            thread: Some(thread),
            started,
            marks: Vec::new(),
            marking_result: false,
        }
    }

    /// **Stamp a guest line on the prober's clock, if it is about a listener** (milestone 514).
    ///
    /// A leg that relays the guest's transcript line by line (the HVF leg) hands each `line` here,
    /// so a lost round can be read against when the guest listened and accepted. Kept: each
    /// listener test's start (the hand-written test's thread-census line, printed once `net_stack`
    /// is up and before it listens, and the std test's start line), each `std listen| ` echo
    /// (`listen ok` is the bind; `in use refused` is the last thing before the first `accept`), and
    /// their verdicts. The stamp trails the guest by the serial path's few milliseconds, against
    /// rounds seconds apart.
    ///
    /// Name: provisional, milestone 514's lane, 2026-10-12 (UTC). A verb for stamping a line;
    /// `stamp` was considered.
    pub(crate) fn mark(&mut self, line: &str) {
        let at = self.started.elapsed().as_millis();
        let listener_test = line.starts_with("test ")
            && (line.contains("::a_host_process_connects_to_the_guest_and_is_answered ")
                || line.contains("::a_std_program_serves_a_granted_listening_port "));
        let echoed = line.starts_with("    std listen| ");
        if (self.marking_result || listener_test || echoed) && self.marks.len() < 64 {
            self.marks
                .push((at, line.trim().chars().take(120).collect()));
        }
        // A test line that ends in its verdict (`... ok`) needs nothing more. One that does not
        // (the census, or an echo that started a line) gets its verdict from the next line that
        // is not an echo.
        if listener_test {
            self.marking_result = !line.trim_end().ends_with(" ok");
        } else if !echoed {
            self.marking_result = false;
        }
    }

    /// Stop poking, and say whether the guest answered. Fails the leg when it did not: the guest's
    /// own assertion covers "somebody connected", and this covers the other half, that what came
    /// back was the answer the guest meant to send.
    pub(crate) fn report(mut self) -> bool {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let arch = &self.arch;
        // Which emulator produced a red line here is part of reading it (milestone 414 (a red
        // post-run check does not say which emulator produced it)): the port forward this probe
        // drives is QEMU's, and versions fail disjoint sets of the host-side checks. Computed
        // once and carried by the FAILED lines.
        let qemu = crate::host::qemu_version(arch);
        let Some(thread) = self.thread.take() else {
            eprintln!("inbound check ({arch}) FAILED (QEMU {qemu}): the prober never started");
            return false;
        };
        let port = self.port.unwrap_or(0);
        let joined = thread.join();
        if let Ok((_, trace)) = &joined {
            // Printed on a GREEN run too, and that is the point: a red run is only diagnosable
            // against a known-good shape, and the first two failures of this check had none to
            // compare with.
            eprintln!(
                "inbound prober (port {port}): {}",
                trace.summary(&self.marks)
            );
        }
        match joined.map(|(verdict, _)| verdict) {
            Ok(Ok(())) => {
                eprintln!(
                    "inbound check ({arch}): host connections to 127.0.0.1:{port} were forwarded \
                     into the guest, accepted, and answered with the guest's own bytes, by both \
                     listeners: the hand-written one and `std::net::TcpListener`. At least \
                     {INBOUND_REQUIRED} of the {INBOUND_OFFERED} offered, which is the floor that \
                     proves each of the two answered, since neither can supply more than two."
                );
                true
            }
            Ok(Err(reason)) => {
                eprintln!();
                eprintln!("inbound check ({arch}) FAILED (QEMU {qemu}): {reason}");
                eprintln!(
                    "  A host process connecting to the guest is the one thing no in-guest test can \
                     stage. See notes/net/the-inbound-half.md."
                );
                false
            }
            Err(_) => {
                eprintln!(
                    "inbound check ({arch}) FAILED (QEMU {qemu}): the prober thread panicked"
                );
                false
            }
        }
    }
}

/// One prober thread on `port`: connect, speak, and require the guest's answer, up to
/// `INBOUND_OFFERED` times, passing at `INBOUND_REQUIRED`, until `stop`; events are timed from
/// `started`. Returns the verdict and the trace.
///
/// **Never abandon a connection because it is slow.** `connect` succeeds when QEMU accepts the host
/// side; slirp then holds the payload and completes the guest side whenever the guest next polls,
/// so a dropped connection becomes a round the guest serves to nobody. A timeout keeps reading the
/// same connection until it answers, dies or the run ends; a hard error (the RST of a guest with no
/// listener, most of a boot) gives up cheaply, since nothing was consumed. See
/// notes/net/the-inbound-half.md.
fn probe_inbound(
    port: u16,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    started: std::time::Instant,
) -> (Result<(), String>, InboundTrace) {
    use std::io::Write;
    use std::sync::atomic::Ordering;

    let addr: std::net::SocketAddr = ([127, 0, 0, 1], port).into();
    let mut done = 0usize;
    let mut last = String::from("nothing ever answered on the forwarded port");
    let mut trace = InboundTrace::new(started);

    while done < INBOUND_OFFERED && !stop.load(Ordering::Relaxed) {
        let opened = std::time::Instant::now();
        let mut s = match std::net::TcpStream::connect_timeout(
            &addr,
            std::time::Duration::from_millis(500),
        ) {
            Ok(s) => s,
            Err(e) => {
                last = format!("could not connect to 127.0.0.1:{port}: {e}");
                trace.note("connect-failed", opened, 0);
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
        };
        // Short, so the loop below can notice `stop`; not a deadline for the exchange.
        let _ = s.set_read_timeout(Some(std::time::Duration::from_millis(250)));
        let _ = s.set_nodelay(true);

        if let Err(e) = s.write_all(INBOUND_IN) {
            last = format!(
                "the guest closed before reading our {} bytes: {e}",
                INBOUND_IN.len()
            );
            trace.note("write-failed", opened, 0);
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }

        let (got, outcome, error) = read_the_answer(&mut s, &stop, &mut last);
        drop(s);

        if got == INBOUND_OUT {
            done += 1;
            trace.note("answered", opened, got.len());
            continue;
        }
        // The loop filled its quota without matching: bytes that are not the guest's answer.
        let outcome = if outcome == "answered" {
            "wrong-bytes"
        } else {
            outcome
        };
        trace.note_error(outcome, opened, got.len(), error);
        // Not an answer: almost always "no listener yet", which is the normal state for most of the
        // run. Keep the last one only so a genuine failure has something to say.
        if !got.is_empty() {
            last = format!(
                "the guest answered {:?}, wanted {:?}",
                String::from_utf8_lossy(&got),
                String::from_utf8_lossy(INBOUND_OUT),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    let verdict = if done >= INBOUND_REQUIRED {
        Ok(())
    } else {
        Err(format!(
            "the guest served {done} of the {INBOUND_OFFERED} inbound connections it offers on the \
             port forwarded to 127.0.0.1:{port}, and {INBOUND_REQUIRED} is the floor that proves \
             both listeners answered; last attempt: {last}\n  what the {} attempts did is the \
             `inbound prober` timeline printed just above.\n  \
             READ THE TIMESTAMPS FIRST. The four rounds come from two listeners in two separate \
             windows about eight seconds apart, two rounds in each, so the answers cluster. Two \
             clusters and a missing round means the host lost one that the guest served, and the \
             outcome beside it names how. ONE cluster means a whole listener never ran: look for \
             `(no virtio-net device attached; skipping)` in the transcript, which is the only way \
             either guest test passes without offering its two. See notes/net/the-inbound-check.md.",
            trace.attempts,
        ))
    };
    (verdict, trace)
}

/// **Read one connection until the guest's answer is whole, it ends, or the run does.** Returns
/// the bytes read, what ended the connection (for the timeline), and the read error's detail when
/// an error ended it; a failure worth reporting is also left in `last`.
fn read_the_answer(
    s: &mut std::net::TcpStream,
    stop: &std::sync::atomic::AtomicBool,
    last: &mut String,
) -> (Vec<u8>, &'static str, Option<String>) {
    use std::io::{ErrorKind, Read};
    use std::sync::atomic::Ordering;

    let mut got = Vec::new();
    let mut buf = [0u8; 64];
    // What ended this connection, for the tally the failure text prints. The read loop has
    // five exits and they mean different things: only two of them are "nothing was consumed",
    // and telling them apart is the whole diagnosis when a round goes missing.
    let mut outcome = "answered";
    let mut error: Option<String> = None;
    while got.len() < INBOUND_OUT.len() {
        match s.read(&mut buf) {
            Ok(0) => {
                // The guest had no listener and closed; nothing was consumed. Unless bytes had
                // already arrived, in which case a round WAS served and we lost the tail of it.
                outcome = if got.is_empty() {
                    "closed-empty"
                } else {
                    "closed-partial"
                };
                break;
            }
            Ok(n) => got.extend_from_slice(&buf[..n]),
            Err(e) if read_error_is_not_yet(e.kind()) => {
                // Still waiting on a guest that has not polled yet. Hold the connection: see
                // [`probe_inbound`]'s note on why dropping it would feed the guest a round we
                // cannot collect. The run ending is the only thing that ends this wait.
                if stop.load(Ordering::Relaxed) {
                    *last = String::from(
                        "the run ended while a connection was still waiting for the guest to \
                         accept it",
                    );
                    outcome = "stopped-while-waiting";
                    break;
                }
            }
            Err(e) => {
                *last = format!("reading the guest's answer failed: {e}");
                // Kept on the event, not only in `last`, which is printed on a red run alone.
                // notes/net/the-inbound-check.md: a green run's `read-failed` once threw away the one fact that
                // would have named it.
                error = Some(format!("{:?}, os error {:?}", e.kind(), e.raw_os_error()));
                outcome = match e.kind() {
                    ErrorKind::ConnectionReset => "reset",
                    ErrorKind::ConnectionAborted => "aborted",
                    ErrorKind::BrokenPipe => "broken-pipe",
                    _ => "read-failed",
                };
                break;
            }
        }
    }
    (got, outcome, error)
}

/// **Whether a read error means "not yet" rather than "this connection is over".**
///
/// `WouldBlock` and `TimedOut` are the 250 ms read timeout expiring. `Interrupted` is a signal
/// landing on the blocked `receive` (`EINTR`), which says nothing about the connection at all, and
/// treating it as fatal was the inbound check's lost round. On CI, every `read-failed` in two weeks
/// of traces landed on the five-second grid `HostLoad` samples on, and the first trace that kept its
/// errno said `Interrupted, os error 4`. Dropping the connection there does not take back the
/// payload already written: slirp still delivers it when the guest next polls, the guest serves a
/// round into a socket nobody holds, and two of those in one boot is "2 of 4" and a red leg. See
/// notes/net/the-inbound-half.md.
fn read_error_is_not_yet(kind: std::io::ErrorKind) -> bool {
    use std::io::ErrorKind;
    matches!(
        kind,
        ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
    )
}

/// **What every prober connection did, kept so a failure can name a mechanism instead of a count.**
///
/// "The guest served 3 of 4" cannot tell an abandoned connection from a lost answer from a teardown
/// race, and each wants a different fix. This records which of `probe_inbound`'s five read-loop
/// exits each connection took, how long it was held, and how many bytes it had collected. It is
/// cheap (a few hundred attempts a boot, counters, lines only for connections worth one), and its
/// summary prints on a passing run too, so a red run can be read against a known-good shape.
#[derive(Default)]
struct InboundTrace {
    attempts: usize,
    counts: std::collections::BTreeMap<&'static str, usize>,
    /// `(ms since the prober started, outcome, held ms, bytes)` for connections worth a line: the
    /// ones that collected bytes, the ones the guest closed empty, and the ones held over a second. A connection that was reset in
    /// under a millisecond with nothing on it is the boring majority and is only counted.
    events: Vec<(u128, &'static str, u128, usize, Option<String>)>,
    started: Option<std::time::Instant>,
}

impl InboundTrace {
    /// `started` is the prober's clock, shared with [`InboundProber::mark`].
    pub(crate) fn new(started: std::time::Instant) -> Self {
        Self {
            started: Some(started),
            ..Default::default()
        }
    }

    pub(crate) fn note(&mut self, outcome: &'static str, opened: std::time::Instant, bytes: usize) {
        self.note_error(outcome, opened, bytes, None);
    }

    /// [`note`](Self::note), with the error that ended the connection when one did. Any event that
    /// carries an error gets a line, however briefly it was held.
    pub(crate) fn note_error(
        &mut self,
        outcome: &'static str,
        opened: std::time::Instant,
        bytes: usize,
        error: Option<String>,
    ) {
        self.attempts += 1;
        *self.counts.entry(outcome).or_insert(0) += 1;
        let held = opened.elapsed().as_millis();
        // A guest closing a connection with nothing on it is what `net_stack`'s listener reset
        // looks like from here (milestone 514), so it gets a line however brief: it shows a race
        // with the re-armed listener happening and being recovered, on green runs too.
        if bytes > 0
            || held >= 1000
            || matches!(outcome, "answered" | "closed-empty")
            || error.is_some()
        {
            let at = self
                .started
                .map(|s| s.elapsed().as_millis())
                .unwrap_or_default();
            // Bounded, so a pathological run cannot grow this without limit.
            if self.events.len() < 64 {
                self.events.push((at, outcome, held, bytes, error));
            }
        }
    }

    /// The counts, then one line per kept connection and per guest mark, in time order. A
    /// connection's line names when it was opened as well as when it ended, because the open is
    /// what has to be read against the guest's listen and accept.
    pub(crate) fn summary(&self, marks: &[(u128, String)]) -> String {
        let mut out = String::new();
        for (k, v) in &self.counts {
            if !out.is_empty() {
                out.push_str(", ");
            }
            out.push_str(&format!("{k} x{v}"));
        }
        if out.is_empty() {
            out.push_str("no attempts");
        }
        let mut lines: Vec<(u128, String)> = marks
            .iter()
            .map(|(at, line)| (*at, format!("\n    +{at} ms: guest: {line}")))
            .collect();
        for (at, outcome, held, bytes, error) in &self.events {
            let opened = at.saturating_sub(*held);
            let mut line = format!(
                "\n    +{at} ms: {outcome} after {held} ms (opened +{opened} ms), {bytes} bytes"
            );
            if let Some(error) = error {
                line.push_str(&format!(" ({error})"));
            }
            lines.push((*at, line));
        }
        // Stable, so a mark and an event in the same millisecond keep the order they were made in.
        lines.sort_by_key(|(at, _)| *at);
        for (_, line) in lines {
            out.push_str(&line);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;

    /// A signal on a blocked read is a reason to read again, never a verdict on the connection.
    /// Before this, `Interrupted` fell to the catch-all arm, the connection was dropped as
    /// `read-failed`, and the round its payload bought was served to nobody.
    #[test]
    fn an_interrupted_read_keeps_the_connection() {
        assert!(super::read_error_is_not_yet(ErrorKind::Interrupted));
        assert!(super::read_error_is_not_yet(ErrorKind::WouldBlock));
        assert!(super::read_error_is_not_yet(ErrorKind::TimedOut));
    }

    /// And a connection the peer really ended is still over, so the fix cannot turn a reset into
    /// an endless wait.
    #[test]
    fn a_reset_still_ends_it() {
        assert!(!super::read_error_is_not_yet(ErrorKind::ConnectionReset));
        assert!(!super::read_error_is_not_yet(ErrorKind::ConnectionAborted));
        assert!(!super::read_error_is_not_yet(ErrorKind::BrokenPipe));
    }
}
