//! The real board: its serial console (milestone 216), and the U-Boot script that boots it
//! without a person at its prompt (milestone 218).
//!
//! The recogniser that says how far a boot got lives in `serial_console`; this keeps the
//! argument parsing, the log, and the report. See notes/visionfive2.md.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

// ---------------------------------------------------------------------------
// board-console (milestone 216)
// ---------------------------------------------------------------------------

/// **Read a finished capture of many boots and print what the lottery drew** (milestone 249).
///
/// `serial_console::lottery` is the logic and has the tests; this reads a file and prints. `0`
/// even when a series is short or a draw is unjudged, because the report says so in words; `4`
/// only when the file cannot be read, the code the watcher gives an argument it cannot use. There
/// is no board to have gone quiet, so the watcher's other codes do not arise.
fn board_console_tally(path: &std::path::Path) -> ExitCode {
    let log = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("board-console: cannot read {}: {e}", path.display());
            return ExitCode::from(4);
        }
    };
    let series = serial_console::lottery::tally(&log);
    print!("{}", series.report());
    if series.draws.len() < 50 {
        eprintln!(
            "board-console: {} draw(s). Milestone 249 asks for at least fifty before this is a \
             distribution rather than a handful of boots.",
            series.draws.len()
        );
    }
    ExitCode::SUCCESS
}

/// **Read a finished soak capture into the defect curve's exposure row** (milestone 225 (run the
/// soak on radon, argon and xenon), for milestone 201 (is multicore reliability converging)).
/// `serial_console::exposure` is the logic and has the tests. This parses `<log>` and the three
/// columns no console line carries, and prints a row per boot that soaked.
///
/// Exit statuses keep the watcher's meanings where they overlap: `0` every soaked boot is clean,
/// `1` one is not (the board announced a failure or a counter is nonzero), `3` no boot in the log
/// reached a heartbeat, `4` the file or the arguments.
fn board_console_exposure(args: &[String]) -> ExitCode {
    use serial_console::exposure::{self, Meta};

    let usage = || {
        eprintln!(
            "usage: cargo xtask board-console --exposure <log> [--machine <name>] \
             [--build <sha>] [--start '<YYYY-MM-DD HH:MM>']"
        );
        ExitCode::from(4)
    };
    let mut path: Option<&str> = None;
    let mut meta = Meta::default();
    let mut i = 0;
    while i < args.len() {
        let slot = match args[i].as_str() {
            "--machine" => &mut meta.machine,
            "--build" => &mut meta.build,
            "--start" => &mut meta.start,
            other if path.is_none() && !other.starts_with("--") => {
                path = Some(other);
                i += 1;
                continue;
            }
            _ => return usage(),
        };
        let Some(v) = args.get(i + 1) else {
            return usage();
        };
        *slot = Some(v.clone());
        i += 2;
    }
    let Some(path) = path else {
        return usage();
    };
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("board-console: cannot read {path}: {e}");
            return ExitCode::from(4);
        }
    };
    meta.source = Some(path.to_string());
    // A machine with a profile is judged against it (radon may lack `deviceirq`, xenon may
    // not); argon, `hvf` and an unnamed machine have none, so no absence fails their boots.
    let profile = meta
        .machine
        .as_deref()
        .and_then(serial_console::board::profile);
    // Lossy, not `read_to_string`: a serial capture carries whatever the wire did, and one bad
    // byte in eight hours must not cost the row.
    let text = String::from_utf8_lossy(&bytes);
    let boots = match profile {
        Some(p) => exposure::read_for(&text, p),
        None => exposure::read(&text),
    };
    let soaked: Vec<_> = boots.iter().filter(|b| b.soaked()).collect();
    println!(
        "exposure: {} boot(s) in the capture, {} reached a soak heartbeat",
        boots.len(),
        soaked.len()
    );
    for (n, boot) in soaked.iter().enumerate() {
        println!();
        println!("boot {}: {}", n + 1, boot.figures());
        for check in boot.checks() {
            println!("  {check}");
        }
        println!("{}", boot.row(&meta));
    }
    if soaked.is_empty() {
        return ExitCode::from(3);
    }
    println!();
    println!(
        "Paste each row into the machine's table under notes/multicore-defect-curve/ (radon's is \
         radon-rows.md) with the next free E id; `?` is a column the log cannot supply."
    );
    if soaked.iter().all(|b| b.is_clean()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// Open a real board's serial console, log every byte, and stop on a deadline (milestone 216).
///
/// The engine is `crates/serial_console`, a recogniser over text with host tests; this is argument
/// parsing and a report. Returns one of four exit codes, not a `bool`, because a scripted bench run
/// needs to tell apart reached, announced a failure, went quiet and ran out
/// (`serial_console::watch`'s `exit_code`).
pub(crate) fn board_console() -> ExitCode {
    use std::io::Write;

    let mut args = match ConsoleArgs::parse() {
        Ok(ConsoleCommand::Watch(args)) => args,
        Ok(ConsoleCommand::Done(code)) | Err(code) => return code,
    };
    if let Err(code) = args.resolve() {
        return code;
    }

    // The log path is chosen before anything can fail, and it is never optional. A console session
    // whose evidence exists only in a terminal that has since scrolled is the rung-four failure
    // this tree keeps writing down; the file is the artifact.
    let Some((log_path, mut sink)) =
        open_capture("board-console", "board-console", args.log.take())
    else {
        return ExitCode::from(4);
    };

    // A replayed log ends; a serial port does not. That single bit is the difference between
    // "the file is over" and "the board has not said anything yet", and getting it wrong turns
    // one of them into the other.
    let session = if let Some(path) = &args.replay {
        let captured = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("board-console: cannot read {}: {e}", path.display());
                return ExitCode::from(4);
            }
        };
        eprintln!("--- replaying {} ---", path.display());
        serial_console::watch::watch(captured, &mut sink, &args.policy, false)
    } else {
        match watch_the_port(&args, &mut sink, &log_path) {
            Ok(session) => session,
            Err(code) => return code,
        }
    };

    let session = match session {
        Ok(s) => s,
        Err(e) => {
            eprintln!("board-console: {e}");
            eprintln!("board-console: log at {}", log_path.display());
            return ExitCode::from(4);
        }
    };

    let _ = sink.flush();
    report_console_session(&session, args.replay.is_none(), &log_path);
    ExitCode::from(u8::try_from(session.exit_code()).unwrap_or(4))
}

/// What `board-console`'s command line asked for: a session to watch, or a reader that has
/// already run and has its exit status.
enum ConsoleCommand {
    Watch(ConsoleArgs),
    Done(ExitCode),
}

/// **A watching session's command line**, as [`ConsoleArgs::parse`] reads it and
/// [`ConsoleArgs::resolve`] finishes it.
struct ConsoleArgs {
    port: Option<PathBuf>,
    replay: Option<PathBuf>,
    log: Option<PathBuf>,
    policy: serial_console::watch::Policy,
    /// **The writing mode** (milestone 324 (the bench console cannot speak to any of the three
    /// boards)). `None` is every reading session, which is still the default and still the common
    /// case. `Some(n)` is `--stop-after n`, and `--stop` is `Some(1)`:
    /// one mechanism with two spellings, so the two flags cannot disagree about anything.
    stop_after: Option<usize>,
    /// Whether `--for` and `--until` were given, as opposed to left at their defaults. A stop mode
    /// has to know, because it changes both, and silently overriding something a person typed is
    /// worse than refusing it.
    until_given: bool,
    cap_given: bool,
    /// **`--until` is resolved after the loop** (milestone 324 part 3), because four of its words
    /// name rungs of a board's firmware prologue and `--board` may come after it on the line.
    /// Parsing it in place would make argument order load-bearing, which is the kind of thing
    /// nobody remembers.
    until_word: Option<String>,
}

impl ConsoleArgs {
    /// Read the command line after the subcommand. `--tally` and `--exposure` run their readers
    /// here and come back as [`ConsoleCommand::Done`]. `Err` is exit status 4, after saying why.
    fn parse() -> Result<ConsoleCommand, ExitCode> {
        let args: Vec<String> = std::env::args().skip(2).collect();
        let mut c = ConsoleArgs {
            port: std::env::var_os("NIFE_BOARD_PORT").map(PathBuf::from),
            replay: None,
            log: None,
            policy: serial_console::watch::Policy::default(),
            stop_after: None,
            until_given: false,
            cap_given: false,
            until_word: None,
        };
        let mut i = 0;
        while i < args.len() {
            let value = |i: usize| -> Result<&str, ExitCode> {
                args.get(i + 1).map(String::as_str).ok_or_else(|| {
                    eprintln!("board-console: {} wants a value", args[i]);
                    ExitCode::from(4)
                })
            };
            match args[i].as_str() {
                "--port" => c.port = Some(PathBuf::from(value(i)?)),
                "--replay" => c.replay = Some(PathBuf::from(value(i)?)),
                // **Milestone 249's reader**, and it is the only mode that does not watch anything: it
                // reads a finished capture of *many* boots and reports what the placement lottery drew
                // each time. It returns before the log file below is created, because a tally writes no
                // capture of its own and a fresh empty `board-console-<stamp>.log` beside a fifty-boot
                // one is litter a reader has to tell apart.
                "--tally" => {
                    let v = value(i)?;
                    return Ok(ConsoleCommand::Done(board_console_tally(
                        std::path::Path::new(v),
                    )));
                }
                // **Milestone 225's reader**: a finished soak capture into the curve's exposure row.
                // It takes the rest of the line, for the three columns no console line carries, and
                // like `--tally` it returns before a capture file is created.
                "--exposure" => {
                    return Ok(ConsoleCommand::Done(board_console_exposure(&args[i + 1..])));
                }
                "--log" => c.log = Some(PathBuf::from(value(i)?)),
                // **A writing mode, and the only two the ruling permits** (milestone 324). Each is a
                // command with a purpose rather than a keyboard, which is the decision rather than
                // caution about it: milestone 249's lane sent this escape by detaching the console,
                // hit U-Boot's autoboot countdown with it, and paid a power cycle.
                "--stop" => {
                    c.stop_after = Some(1);
                    // Not `i += 2` at the bottom of the loop: this flag takes no value, and the
                    // increment below assumes every argument does.
                    i += 1;
                    continue;
                }
                "--stop-after" => match value(i)?.parse::<usize>() {
                    Ok(n) if n >= 1 => c.stop_after = Some(n),
                    _ => {
                        eprintln!(
                            "board-console: --stop-after wants a count of draws, 1 or more. \
                             `--stop` is `--stop-after 1`."
                        );
                        return Err(ExitCode::from(4));
                    }
                },
                "--for" | "--timeout" => match parse_duration(value(i)?) {
                    Some(d) => {
                        c.policy.total = d;
                        c.cap_given = true;
                    }
                    None => {
                        eprintln!("board-console: --for wants a duration like 90, 90s, 30m or 2h");
                        return Err(ExitCode::from(4));
                    }
                },
                "--quiet-after" => match parse_duration(value(i)?) {
                    // Zero disables it: on a long sustained watch, a board that is legitimately quiet
                    // for a while is not a hang, and the operator is the one who knows which.
                    Some(d) => c.policy.quiet_after = if d.is_zero() { None } else { Some(d) },
                    None => {
                        eprintln!("board-console: --quiet-after wants a duration, or 0 to disable");
                        return Err(ExitCode::from(4));
                    }
                },
                "--until" => {
                    c.until_word = Some(value(i)?.to_string());
                    c.until_given = true;
                }
                // **Which board is on the other end** (milestone 324 part 3). It chooses the firmware
                // prologue and nothing else: everything from the kernel banner up is shared, which is
                // what calef's ruling settled and what `crates/serial_console::board` implements.
                "--board" => c.policy.board = board_profile(value(i)?)?,
                other => {
                    eprintln!("board-console: unknown argument {other}");
                    eprintln!(
                        "usage: cargo xtask board-console [--port <dev>] [--replay <log>] \
                         [--log <file>] [--for <duration>] [--until <stage>] [--quiet-after <duration>] \
                         [--board <name>]\n\
                         \x20      cargo xtask board-console [--stop | --stop-after <n>]\n\
                         \x20      cargo xtask board-console --tally <log>\n\
                         \x20      cargo xtask board-console --exposure <log> [--machine <name>] \
                         [--build <sha>] [--start <utc>]"
                    );
                    return Err(ExitCode::from(4));
                }
            }
            i += 2;
        }
        Ok(ConsoleCommand::Watch(c))
    }

    /// **Finish what the loop could not**: `--until` against the board now named, and what a
    /// writing mode changes about the rest of the session. `Err` is exit status 4, after saying
    /// why.
    fn resolve(&mut self) -> Result<(), ExitCode> {
        // `--until`, now that `--board` is known. A word this board has no rung for is refused rather
        // than watched for: `--until spl` against a machine with no SPL can only ever end in the time
        // running out, and a refusal that names the words it does know costs the operator nothing.
        if let Some(word) = &self.until_word {
            match parse_stage(self.policy.board, word) {
                Some(stage) => self.policy.until = stage,
                None => {
                    eprintln!(
                        "board-console: --until {word} is not a stage {} reaches. Known: {}.",
                        self.policy.board.name,
                        stage_words(self.policy.board)
                    );
                    return Err(ExitCode::from(4));
                }
            }
        }

        // **What a writing mode changes about the rest of the session** (milestone 324), decided here
        // rather than inside the crate because these are argument-shaped decisions and the crate's job
        // is the decision to send.
        let Some(n) = self.stop_after else {
            return Ok(());
        };
        // A replay has no board on the other end of it. Refused rather than ignored: a person who
        // typed `--stop --replay` believes a byte is going somewhere, and silently reading a file
        // instead would be the tool agreeing with them.
        if self.replay.is_some() {
            eprintln!(
                "board-console: --stop writes to a board and --replay reads a file, so the \
                 two cannot be combined. Drop --replay to watch a real port."
            );
            return Err(ExitCode::from(4));
        }
        // Both answer "when does this session end", and a stage would usually answer it first: a
        // soak is reached long before its reboot loop arms, so `--until soak --stop` would return
        // before sending anything. Refused rather than overridden, for the reason above.
        if self.until_given && self.policy.until.is_some() {
            eprintln!(
                "board-console: --stop and --until both say when the session ends, and \
                 --until would win before the escape was ever sent. Drop --until; the stop is \
                 the ending."
            );
            return Err(ExitCode::from(4));
        }
        self.policy.until = None;
        if !self.cap_given {
            // **Derived from the kernel's own draw length rather than picked.** One draw is
            // `kernel/src/soak.rs`'s `REBOOT_AFTER_SECONDS` (120s) plus the twenty-odd seconds a
            // boot takes, so 150s per draw, and one draw's slack on top for the session that
            // attaches mid-draw and has to wait the current one out. `--stop-after 50` then gets
            // the two unattended hours milestone 249's block prices it at.
            //
            // This is a default and not an agreement: getting it wrong costs a re-run with
            // `--for`, never a wrong answer, which is why the number is duplicated here instead of
            // being hoisted into a shared crate.
            self.policy.total = std::time::Duration::from_secs(
                150 * u64::try_from(n).unwrap_or(u64::MAX / 300) + 120,
            );
        }
        Ok(())
    }
}

/// `--board`'s profile by name, or exit status 4 after naming the ones there are.
fn board_profile(name: &str) -> Result<&'static serial_console::board::Profile, ExitCode> {
    serial_console::board::profile(name).ok_or_else(|| {
        eprintln!(
            "board-console: unknown board {name}. Known: {}.",
            serial_console::board::PROFILES
                .iter()
                .map(|p| p.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
        eprintln!(
            "board-console: argon has no profile on purpose; it has never booted \
             nife and a prologue read out of vendor documentation would be a guess."
        );
        ExitCode::from(4)
    })
}

/// **Watch a real serial port**, the one `--port` or `NIFE_BOARD_PORT` names or the one there is,
/// and in a writing mode open its write half for the escape. `Err` is exit status 4, after saying
/// why.
fn watch_the_port(
    args: &ConsoleArgs,
    sink: &mut Tee,
    log_path: &std::path::Path,
) -> Result<std::io::Result<serial_console::watch::Session>, ExitCode> {
    let path = serial_console::port::choose(args.port.as_deref()).map_err(|e| {
        eprintln!("board-console: {e}");
        ExitCode::from(4)
    })?;
    let (device, complaint) = serial_console::port::open(&path).map_err(|e| {
        eprintln!("board-console: cannot open {}: {e}", path.display());
        ExitCode::from(4)
    })?;
    if let Some(complaint) = complaint {
        // Not fatal: an adapter whose driver refused one of the flags still delivers bytes,
        // and the deadline does not depend on the read timeout.
        eprintln!("board-console: {complaint} (continuing; the deadline does not need it)");
    }
    eprintln!(
        "--- {} at {} baud, logging to {}, up to {:?} ---",
        path.display(),
        serial_console::port::BAUD,
        log_path.display(),
        args.policy.total
    );
    // The write half of the same descriptor. A second handle rather than a shared one because
    // `watch` moves its source onto the reader thread, and a tty is happy to be read and
    // written through two descriptors at once.
    let mut writer = match args.stop_after {
        Some(_) => match device.try_clone() {
            Ok(w) => Some(w),
            Err(e) => {
                eprintln!(
                    "board-console: cannot open a write handle on {}: {e}",
                    path.display()
                );
                return Err(ExitCode::from(4));
            }
        },
        None => None,
    };
    let escape = match (args.stop_after, writer.as_mut()) {
        (Some(n), Some(w)) => {
            eprintln!(
                "--- stopping after {n} armed draw(s); one byte will be sent to the board \
                 and printed into the log ---"
            );
            Some(serial_console::stop::Escape::new(n, w))
        }
        _ => None,
    };
    Ok(serial_console::watch::watch_with(
        device,
        sink,
        &args.policy,
        true,
        escape,
    ))
}

/// **The session's report under its transcript**: the summary, the writing mode's verdict, and
/// the lines a person reads instead of the log. `from_port` is false for a replay.
fn report_console_session(
    session: &serial_console::watch::Session,
    from_port: bool,
    log_path: &std::path::Path,
) {
    eprintln!();
    eprintln!("board-console: {}", session.summary());
    // The writing mode's own verdict, on its own line, whenever the session did not already end on
    // it. `--stop` is run to find out one thing and a reader should not have to infer it from an
    // exit status.
    if session.outcome != serial_console::watch::Outcome::Stopped
        && let Some(report) = &session.stop
    {
        eprintln!("board-console: {}", report.describe());
    }
    if let Some(line) = session.progress.banner_line() {
        eprintln!("board-console: banner: {line}");
    }
    // Milestone 268's two summary lines, echoed here so the answer to "what did I boot and did it
    // work" is in the report rather than a scroll back through a kilobyte of hex.
    if let Some(line) = session.progress.machine_line() {
        eprintln!("board-console: machine: {line}");
    }
    if let Some(line) = session.progress.self_test_line() {
        eprintln!("board-console: self-test: {line}");
    }
    // The difference between the two captured successes, and the one a reader would otherwise have
    // to go back to the log for: whether there was an archive on the card at all.
    if session.progress.userspace_ran() {
        eprintln!("board-console: the userspace progenitor built its child");
    }
    if session.bytes == 0 && from_port {
        // The runbook's first triage row, said here so nobody starts by suspecting the kernel.
        eprintln!(
            "board-console: not one byte arrived. Check TX/RX are crossed, that the board has \
             power, that the DIP switches are on QSPI, and that this is the cu.* device."
        );
    }
    eprintln!("board-console: log at {}", log_path.display());
}

/// **The capture a console session writes beside the terminal**: `log` if one was named, or
/// `target/<stem>-<seconds>.log`. `None` after saying, as `command`, why it could not be written.
/// Shared by `board-console`, `soak-test` and `job-mix`.
pub(crate) fn open_capture(
    command: &str,
    stem: &str,
    log: Option<PathBuf>,
) -> Option<(PathBuf, Tee)> {
    let log_path = log.unwrap_or_else(|| {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        PathBuf::from(format!("target/{stem}-{stamp}.log"))
    });
    if let Some(parent) = log_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("{command}: cannot create {}: {e}", parent.display());
        return None;
    }
    let file = match std::fs::File::create(&log_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{command}: cannot write {}: {e}", log_path.display());
            return None;
        }
    };
    let sink = Tee {
        file,
        terminal: std::io::stdout(),
    };
    Some((log_path, sink))
}

/// Write every byte to the log and to this terminal at once.
///
/// Both, not either. The file is the artifact a later reader needs and the terminal is what makes
/// a person at the bench willing to use the tool at all.
pub(crate) struct Tee {
    pub(crate) file: std::fs::File,
    pub(crate) terminal: std::io::Stdout,
}

impl std::io::Write for Tee {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // The file first, and its error is the one that propagates: losing the terminal copy is a
        // cosmetic loss, losing the log is the whole evidence.
        self.file.write_all(buf)?;
        let _ = self.terminal.write_all(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = self.terminal.flush();
        self.file.flush()
    }
}

/// `90`, `90s`, `30m`, `2h`. Bare digits are seconds.
pub(crate) fn parse_duration(text: &str) -> Option<std::time::Duration> {
    let (digits, scale) = match text.strip_suffix(['s', 'm', 'h']) {
        Some(rest) => (
            rest,
            match text.as_bytes()[text.len() - 1] {
                b's' => 1,
                b'm' => 60,
                _ => 3600,
            },
        ),
        None => (text, 1),
    };
    digits
        .parse::<u64>()
        .ok()
        .map(|n| std::time::Duration::from_secs(n * scale))
}

/// The stage to wait for, or `none` to watch for the whole duration.
///
/// `none` is not a formality: sustained watching with nothing to wait for is what
/// `design/fatal-risks/README.md`'s multicore entry (risk 5) needs, and it is the case a boot check
/// cannot cover.
///
/// **It takes a board** (milestone 324 part 3), because four of these words name rungs of a
/// firmware prologue and a prologue belongs to a board. `spl` against xenon is not a stage this
/// tool is missing, it is a line that machine will never print, and answering it with `None` here
/// is what turns a two-minute wait into a refusal that says so.
fn parse_stage(
    board: &'static serial_console::board::Profile,
    text: &str,
) -> Option<Option<serial_console::progress::Stage>> {
    use serial_console::progress::Stage;
    // The firmware prologue first, because a board may name a rung whatever it likes and the
    // portable words below are not a board's to redefine.
    if let Some(rung) = board.rung(text) {
        return Some(Some(Stage::Firmware(rung)));
    }
    match text {
        "none" => Some(None),
        "banner" => Some(Some(Stage::Banner)),
        // Milestone 268's three rungs. `selftest` is the one to reach for: unlike `tour` it is
        // printed by every architecture, and unlike `banner` it means the kernel proved something
        // about the machine rather than only that the console works.
        "machine" => Some(Some(Stage::Machine)),
        "selftest" => Some(Some(Stage::SelfTest)),
        "tour" => Some(Some(Stage::Tour)),
        "prompt" => Some(Some(Stage::Prompt)),
        // Milestone 219. Useful at a bench as `--until soak`: stop as soon as the workload has
        // announced itself, which answers "did this build actually start soaking" in seconds
        // rather than making the operator watch a beat go by.
        "soak" => Some(Some(Stage::Soak)),
        // Milestone 324 part 2, and `sweep-done` is the one a bench script wants: it is the only
        // answer that separates a finished sweep from a wedged one. `sweep` alone answers the
        // quicker question, whether this build started sweeping at all.
        "sweep" => Some(Some(Stage::Sweep)),
        "sweep-done" => Some(Some(Stage::SweepDone)),
        _ => None,
    }
}

/// The `--until` words this board understands, for the message printed when one is refused.
fn stage_words(board: &'static serial_console::board::Profile) -> String {
    let mut words: Vec<&str> = board.keys().collect();
    words.extend([
        "banner",
        "machine",
        "selftest",
        "tour",
        "prompt",
        "soak",
        "sweep",
        "sweep-done",
        "none",
    ]);
    words.join(", ")
}

// ===========================================================================================
// The board's boot script (milestone 218). The VisionFive 2 cannot boot this kernel from an
// `extlinux.conf`: with no `fdt` line, U-Boot's pxe path hands `bootm` no device tree, and RISC-V's
// `boot_prep_linux` refuses with a `hang()` only the reset button clears
// (crates/serial_console/tests/fixtures/captured/vf2-2026-09-01-extlinux-refused.log). So the card
// carries a script `scan_dev_for_scripts` sources, issuing the commands a person types at the
// `StarFive #` prompt (vf2-2026-09-01-manual-boot.log). See notes/visionfive2.md.
// ===========================================================================================

/// The script the board runs, verbatim: the manual sequence of notes/visionfive2.md, every line
/// proven on silicon, with two edits that each remove a dependency. The load device comes from the
/// variables `boot_a_script` sets before sourcing it (`devtype`, `devnum`, `distro_bootpart`), and
/// the archive's length is stashed under our own name as soon as `load` reports it, so a later
/// command that sets `filesize` cannot change what `booti` is told.
///
/// The device tree's `0x8600_0000` and the archive's `0x9000_0000` are choices; the note has the
/// arithmetic. No `#` comment lines: U-Boot's parser is a cut-down hush, so the script uses only
/// verbs the bench transcript shows working.
const BOARD_BOOT_SCRIPT: &str = "\
echo nife: boot.scr is driving this boot, milestone 218
load ${devtype} ${devnum}:${distro_bootpart} ${kernel_addr_r} /nife-vf2.img
load ${devtype} ${devnum}:${distro_bootpart} 0x90000000 /nife-initial-ramdisk.img
setenv nife_archive_size ${filesize}
fdt addr ${fdtcontroladdr}
fdt move ${fdtcontroladdr} 0x86000000
booti ${kernel_addr_r} 0x90000000:${nife_archive_size} 0x86000000
";

/// The name in the image header. `iminfo` prints it and nothing else reads it.
const BOARD_BOOT_SCRIPT_NAME: &str = "nife board boot";

/// The address the archive is loaded to, and the one `booti` is then handed. Written once here
/// because the two have to agree and there are now four places that name it: two `load` lines and
/// two `tftpboot` lines, in two scripts.
const BOARD_ARCHIVE_ADDR: &str = "0x90000000";

/// This machine's own private IPv4 addresses, paired with the interface each sits on, in the order
/// the host's tool lists them.
///
/// No address constant exists in this tree: a card carries the address of the machine that wrote
/// it, read at that moment, and the script echoes it at boot. Interfaces are enumerated rather than
/// asking the routing table, because a VPN's default route answers an address the board cannot
/// reach; anything outside RFC 1918 is dropped. Several are normal: the first is taken and all are
/// printed, so `--server` can pick another. See notes/visionfive2/network-boot.md.
fn host_private_addresses() -> Vec<(String, std::net::Ipv4Addr)> {
    // BSD/macOS first, then iproute2, because the bench machine is the Mac and CI is Ubuntu.
    if let Some(out) = host_tool_output("ifconfig", &[]) {
        let found = parse_ifconfig_addresses(&out);
        if !found.is_empty() {
            return found;
        }
    }
    if let Some(out) = host_tool_output("ip", &["-4", "-o", "addr", "show"]) {
        return parse_ip_addr_addresses(&out);
    }
    Vec::new()
}

/// Run a host tool and hand back its stdout, or `None` if it is not there or refused.
fn host_tool_output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Keep only the addresses a board on a private LAN could route to.
fn keep_private(found: Vec<(String, std::net::Ipv4Addr)>) -> Vec<(String, std::net::Ipv4Addr)> {
    found
        .into_iter()
        .filter(|(_, addr)| addr.is_private())
        .collect()
}

/// Parse BSD `ifconfig` output: an interface name in column zero ending in `:`, then indented
/// `inet <addr> netmask ...` lines belonging to it.
fn parse_ifconfig_addresses(text: &str) -> Vec<(String, std::net::Ipv4Addr)> {
    let mut interface = String::new();
    let mut found = Vec::new();
    for line in text.lines() {
        if !line.starts_with(char::is_whitespace) {
            if let Some(name) = line.split(':').next() {
                interface = name.to_string();
            }
            continue;
        }
        let mut words = line.split_whitespace();
        if words.next() != Some("inet") {
            continue;
        }
        if let Some(Ok(addr)) = words.next().map(str::parse) {
            found.push((interface.clone(), addr));
        }
    }
    keep_private(found)
}

/// Parse `ip -4 -o addr show`: `2: en0    inet 192.168.8.216/24 brd ... scope global en0`.
fn parse_ip_addr_addresses(text: &str) -> Vec<(String, std::net::Ipv4Addr)> {
    let mut found = Vec::new();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let (_index, interface) = (words.next(), words.next());
        let Some(interface) = interface else { continue };
        if words.next() != Some("inet") {
            continue;
        }
        let Some(cidr) = words.next() else { continue };
        if let Ok(addr) = cidr.split('/').next().unwrap_or(cidr).parse() {
            found.push((interface.to_string(), addr));
        }
    }
    keep_private(found)
}

/// **The network-first script, with the card underneath it** (milestone 257). The two `load` lines
/// of [`BOARD_BOOT_SCRIPT`] become `dhcp` plus two `tftpboot` lines, with the card's copy as the
/// fallback; `fdt addr`, `fdt move` and `booti` are byte for byte milestone 218's.
///
/// The kernel and archive are one measured pair, so a transfer that loses either takes both from
/// the card. `autoload no` stops bare `dhcp` from fetching the bootfile DHCP names, and `netretry
/// no` makes a missing network fail in seconds rather than hang. The shape is deliberately dull
/// (one state variable, `if cmd; then` and `fi`, nesting at most two, no `else`) because U-Boot's
/// hush has not been watched running it. See notes/visionfive2/network-boot.md.
fn board_network_boot_script(server: &str) -> String {
    let archive = BOARD_ARCHIVE_ADDR;
    format!(
        "\
echo nife: boot.scr is driving this boot, milestones 218 and 257
setenv nife_source none
setenv netretry no
setenv autoload no
if test x${{nife_boot_server}} = x; then
setenv nife_boot_server {server}
fi
echo nife: tftp server is ${{nife_boot_server}}, setenv nife_boot_server to point somewhere else
setenv nife_next none
if dhcp; then
setenv serverip ${{nife_boot_server}}
setenv nife_next kernel
fi
if test x${{nife_next}} = xkernel; then
setenv nife_next none
if tftpboot ${{kernel_addr_r}} nife-vf2.img; then
setenv nife_next archive
fi
fi
if test x${{nife_next}} = xarchive; then
setenv nife_next none
if tftpboot {archive} nife-initial-ramdisk.img; then
setenv nife_archive_size ${{filesize}}
setenv nife_source net
fi
fi
if test x${{nife_source}} = xnone; then
echo nife: nothing came over the network, falling back to the card
setenv nife_next kernel
fi
if test x${{nife_next}} = xkernel; then
setenv nife_next none
if load ${{devtype}} ${{devnum}}:${{distro_bootpart}} ${{kernel_addr_r}} /nife-vf2.img; then
setenv nife_next archive
fi
fi
if test x${{nife_next}} = xarchive; then
setenv nife_next none
if load ${{devtype}} ${{devnum}}:${{distro_bootpart}} {archive} /nife-initial-ramdisk.img; then
setenv nife_archive_size ${{filesize}}
setenv nife_source card
fi
fi
echo nife: payload came from ${{nife_source}}
if test x${{nife_source}} != xnone; then
fdt addr ${{fdtcontroladdr}}
fdt move ${{fdtcontroladdr}} 0x86000000
booti ${{kernel_addr_r}} {archive}:${{nife_archive_size}} 0x86000000
fi
echo nife: still at the prompt, so nothing loaded or booti refused what did
"
    )
}

/// `0x27051956`, the legacy U-Boot image magic, big-endian at offset 0 (u-boot `include/image.h`).
const UIMAGE_MAGIC: u32 = 0x2705_1956;
/// The 64-byte header in front of every legacy image.
const UIMAGE_HEADER_LEN: usize = 64;
/// The header's fixed-width name field, NUL-padded.
const UIMAGE_NAME_LEN: usize = 32;
/// `IH_OS_LINUX`. `source` does not check it; mkimage writes it for scripts and so do we.
const IH_OS_LINUX: u8 = 5;
/// `IH_ARCH_RISCV`.
const IH_ARCH_RISCV: u8 = 26;
/// `IH_TYPE_SCRIPT`. This one **is** checked: `source` refuses any other type.
const IH_TYPE_SCRIPT: u8 = 6;
/// `IH_COMP_NONE`.
const IH_COMP_NONE: u8 = 0;

/// Wrap `script` in a legacy U-Boot script image, the thing `mkimage -T script` produces.
///
/// Written here rather than shelled out because `mkimage` is a host package this project does not
/// otherwise need, and the format is 64 bytes and two CRCs. The CRC is
/// `globally_unique_identifier_partition_table`'s, the tree's one definition of IEEE CRC-32,
/// Kani-proved equal to its bitwise form: a second copy of a checksum is a second place to be wrong.
fn uboot_script_image(name: &str, script: &str) -> Vec<u8> {
    // A script image's payload is a size table, then the text. `source` reads the first u32 as the
    // script's length and skips two u32s to reach the bytes (u-boot `cmd/source.c`), so one script
    // is `[len, 0]` followed by it.
    let mut data = Vec::new();
    data.extend_from_slice(
        &u32::try_from(script.len())
            .expect("script fits in u32")
            .to_be_bytes(),
    );
    data.extend_from_slice(&0u32.to_be_bytes());
    data.extend_from_slice(script.as_bytes());

    let mut header: Vec<u8> = Vec::with_capacity(UIMAGE_HEADER_LEN);
    header.extend_from_slice(&UIMAGE_MAGIC.to_be_bytes());
    // The header CRC covers the header with this field read as zero, so it is written zero here
    // and patched below, the same shape `globally_unique_identifier_partition_table`'s header CRC
    // has.
    header.extend_from_slice(&0u32.to_be_bytes());
    // Timestamp. Zero rather than the wall clock, so rebuilding the same payload produces the same
    // bytes and a card can be diffed against the tree. Nothing on the boot path reads it.
    header.extend_from_slice(&0u32.to_be_bytes());
    header.extend_from_slice(
        &u32::try_from(data.len())
            .expect("payload fits in u32")
            .to_be_bytes(),
    );
    // Load address and entry point: meaningless for a script, and zero is what mkimage writes.
    header.extend_from_slice(&0u32.to_be_bytes());
    header.extend_from_slice(&0u32.to_be_bytes());
    header.extend_from_slice(
        &globally_unique_identifier_partition_table::crc::crc32(&data).to_be_bytes(),
    );
    header.push(IH_OS_LINUX);
    header.push(IH_ARCH_RISCV);
    header.push(IH_TYPE_SCRIPT);
    header.push(IH_COMP_NONE);
    let mut name_field = [0u8; UIMAGE_NAME_LEN];
    let truncated = name.len().min(UIMAGE_NAME_LEN - 1);
    name_field[..truncated].copy_from_slice(&name.as_bytes()[..truncated]);
    header.extend_from_slice(&name_field);
    assert_eq!(
        header.len(),
        UIMAGE_HEADER_LEN,
        "the legacy header is 64 bytes"
    );

    let header_crc = globally_unique_identifier_partition_table::crc::crc32(&header);
    header[4..8].copy_from_slice(&header_crc.to_be_bytes());

    header.extend_from_slice(&data);
    header
}

/// Write `target/board/boot.scr.uimg`, and the script's text beside it as `target/board/boot.cmd`
/// so that what the board will run can be read without a hex dump.
///
///     cargo xtask board-script                        the card, and only the card (milestone 218)
///     cargo xtask board-script --tftp                 the network first, the card underneath (257)
///     cargo xtask board-script --tftp --server 10.0.0.5   ...and say the address rather than ask
///
/// The default is the card and stays the card. A network-booting script is a promise about a
/// machine that has to be running, so it is asked for rather than arrived at.
pub(crate) fn board_script() -> bool {
    let mut tftp = false;
    let mut server: Option<String> = None;
    let mut args = std::env::args().skip(2);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--tftp" => tftp = true,
            "--server" => match args.next() {
                Some(value) => server = Some(value),
                None => {
                    eprintln!("board-script: --server needs an address");
                    return false;
                }
            },
            other => {
                eprintln!("board-script: unknown argument {other} (--tftp, --server <ip>)");
                return false;
            }
        }
    }
    if server.is_some() && !tftp {
        eprintln!("board-script: --server only means something with --tftp");
        return false;
    }

    // The address, and the one thing that must never happen: a card carrying a stale one that
    // nothing announces. Either the operator names it, or this machine reads it off its own
    // interfaces here and now, or the script is not written at all. There is no built-in default,
    // on purpose; see `host_private_addresses`.
    let script = if tftp {
        let address = match server {
            Some(given) => {
                println!("  tftp server: {given} (as given)");
                given
            }
            None => {
                let candidates = host_private_addresses();
                let Some((interface, first)) = candidates.first() else {
                    eprintln!(
                        "board-script: no private IPv4 address on this machine, so there is no\n\
                         \x20              address a board could fetch from. Pass --server <ip>."
                    );
                    return false;
                };
                println!("  tftp server: {first} (this machine, on {interface})");
                for (other, addr) in candidates.iter().skip(1) {
                    println!(
                        "               also here: {addr} on {other}, pass --server to use it"
                    );
                }
                first.to_string()
            }
        };
        board_network_boot_script(&address)
    } else {
        BOARD_BOOT_SCRIPT.to_string()
    };
    let script = script.as_str();

    let out = Path::new("target/board");
    if let Err(e) = std::fs::create_dir_all(out) {
        eprintln!("board-script: cannot create {}: {e}", out.display());
        return false;
    }
    let image = uboot_script_image(BOARD_BOOT_SCRIPT_NAME, script);
    let image_path = out.join("boot.scr.uimg");
    let text_path = out.join("boot.cmd");
    if let Err(e) = std::fs::write(&image_path, &image) {
        eprintln!("board-script: cannot write {}: {e}", image_path.display());
        return false;
    }
    if let Err(e) = std::fs::write(&text_path, script) {
        eprintln!("board-script: cannot write {}: {e}", text_path.display());
        return false;
    }
    println!(
        "  {}  ({} bytes, legacy U-Boot script image)",
        image_path.display(),
        image.len()
    );
    println!("  {}  (the same script as text)", text_path.display());
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The card's boot script is a legacy U-Boot image or it is nothing**, and this reads the
    /// bytes back the way `source` does rather than trusting the writer that just produced them:
    /// magic, the type field `source` actually checks, both CRCs recomputed, and the size table
    /// that tells it where the text starts.
    #[test]
    fn the_board_script_is_a_legacy_uboot_script_image() {
        let image = uboot_script_image("nife test", "echo hi\n");
        assert!(image.len() > UIMAGE_HEADER_LEN);

        let be = |at: usize| u32::from_be_bytes(image[at..at + 4].try_into().unwrap());
        assert_eq!(be(0), UIMAGE_MAGIC);
        assert_eq!(image[UIMAGE_HEADER_LEN - 32 - 4], IH_OS_LINUX);
        assert_eq!(image[UIMAGE_HEADER_LEN - 32 - 3], IH_ARCH_RISCV);
        assert_eq!(image[UIMAGE_HEADER_LEN - 32 - 2], IH_TYPE_SCRIPT);
        assert_eq!(image[UIMAGE_HEADER_LEN - 32 - 1], IH_COMP_NONE);

        let data = &image[UIMAGE_HEADER_LEN..];
        assert_eq!(
            be(12) as usize,
            data.len(),
            "the header states the payload length"
        );
        assert_eq!(
            be(24),
            globally_unique_identifier_partition_table::crc::crc32(data),
            "data CRC"
        );

        // The header CRC is over the header with its own field zeroed, so the check has to zero it
        // again; a test that compared the stored value with a CRC of the stored value would pass on
        // any number at all.
        let mut header = image[..UIMAGE_HEADER_LEN].to_vec();
        let stored = u32::from_be_bytes(header[4..8].try_into().unwrap());
        header[4..8].copy_from_slice(&0u32.to_be_bytes());
        assert_eq!(
            stored,
            globally_unique_identifier_partition_table::crc::crc32(&header),
            "header CRC"
        );

        // `source` reads the first u32 as the script length and starts the text after the pair.
        assert_eq!(
            u32::from_be_bytes(data[0..4].try_into().unwrap()) as usize,
            "echo hi\n".len()
        );
        assert_eq!(u32::from_be_bytes(data[4..8].try_into().unwrap()), 0);
        assert_eq!(&data[8..], b"echo hi\n");
    }

    /// **The script must not reach for anything the bench transcript has not already shown
    /// working.** Nobody is watching when this runs, and U-Boot's parser is a cut-down hush whose
    /// failures are silent, so the guard is on the vocabulary rather than on the outcome: no
    /// comments, no parentheses, no shell forms this project has never seen the board execute. It
    /// also pins the two addresses whose arithmetic notes/visionfive2.md carries.
    #[test]
    fn the_board_script_stays_inside_the_proven_vocabulary() {
        let script = BOARD_BOOT_SCRIPT;
        assert!(script.ends_with('\n'), "every line is terminated");
        assert!(
            !script.contains('#'),
            "no comments: hush's handling of them is untested here"
        );
        for forbidden in ['(', ')', '`', '\'', '&', '|', '<', '>', ';'] {
            assert!(
                !script.contains(forbidden),
                "{forbidden} is not in the proven vocabulary"
            );
        }
        for line in script.lines() {
            let verb = line.split_whitespace().next().expect("no blank lines");
            assert!(
                matches!(verb, "echo" | "load" | "setenv" | "fdt" | "booti"),
                "{verb} is a verb the bench transcript does not show"
            );
        }
        assert!(script.contains("fdt move ${fdtcontroladdr} 0x86000000"));
        assert!(
            script.contains("booti ${kernel_addr_r} 0x90000000:${nife_archive_size} 0x86000000")
        );
        // The archive's length is captured under our own name the moment `load` reports it, so no
        // later command that happens to set `filesize` can change what `booti` is handed.
        let stash = script
            .find("setenv nife_archive_size")
            .expect("the archive length is stashed");
        let boot = script.find("booti").expect("the script boots something");
        assert!(stash < boot);
    }

    /// **The address in a card is read off the machine that writes it**, so the reading has to be
    /// right on both host tools. Both samples below are real output, taken from patagonia on
    /// 2026-09-05 and from an `ip -4 -o addr show` line, rather than written to suit the parser.
    ///
    /// The `utun6` row is the whole reason this function exists. patagonia's default route belongs
    /// to Tailscale, so the first version of this discovery asked the routing table and got
    /// `100.75.22.70`, an address radon has no path to. The private-space filter is what drops it,
    /// and the test asserts the drop rather than only asserting the keeps.
    #[test]
    fn the_hosts_own_lan_addresses_are_read_off_both_tools() {
        let ifconfig = "\
lo0: flags=8049<UP,LOOPBACK,RUNNING,MULTICAST> mtu 16384
\tinet 127.0.0.1 netmask 0xff000000
en0: flags=8863<UP,BROADCAST,SMART,RUNNING,SIMPLEX,MULTICAST> mtu 1500
\tinet 192.168.8.216 netmask 0xffffff00 broadcast 192.168.8.255
en9: flags=8863<UP,BROADCAST,SMART,RUNNING,SIMPLEX,MULTICAST> mtu 1500
\tinet 192.168.8.206 netmask 0xffffff00 broadcast 192.168.8.255
utun6: flags=8051<UP,POINTOPOINT,RUNNING,MULTICAST> mtu 1280
\tinet 100.75.22.70 --> 100.75.22.70 netmask 0xffffffff
";
        let found = parse_ifconfig_addresses(ifconfig);
        assert_eq!(
            found,
            vec![
                ("en0".to_string(), "192.168.8.216".parse().unwrap()),
                ("en9".to_string(), "192.168.8.206".parse().unwrap()),
            ],
            "loopback and the CGNAT tunnel are both dropped, and en0 comes first"
        );

        let ip_addr = "\
1: lo    inet 127.0.0.1/8 scope host lo\\       valid_lft forever preferred_lft forever
2: eth0    inet 10.1.2.3/24 brd 10.1.2.255 scope global eth0\\       valid_lft forever
3: wg0    inet 100.64.0.5/32 scope global wg0\\       valid_lft forever
";
        assert_eq!(
            parse_ip_addr_addresses(ip_addr),
            vec![("eth0".to_string(), "10.1.2.3".parse().unwrap())]
        );
    }

    /// The network-first script obeys the same vocabulary rule, plus the two it needs of its own.
    ///
    /// `;` separates commands in hush, so a stray one in an `echo` would run the rest of the line.
    /// The card script forbids it outright; this one cannot, because `if cmd; then` is how a branch
    /// is written, so the only `;` on any line is the one just before `then`. Nesting never goes
    /// deeper than two and there is no `else`: the guard is on the shape, not the outcome, as the
    /// card script's test is.
    #[test]
    fn the_network_script_stays_inside_a_deliberately_dull_shape() {
        let script = board_network_boot_script("10.1.2.3");
        assert!(script.ends_with('\n'), "every line is terminated");
        assert!(!script.contains('#'), "no comments");
        for forbidden in ['(', ')', '`', '\'', '&', '|', '<', '>'] {
            assert!(
                !script.contains(forbidden),
                "{forbidden} is not in the proven vocabulary"
            );
        }

        let mut depth = 0usize;
        let mut deepest = 0usize;
        for line in script.lines() {
            let mut words = line.split_whitespace();
            let first = words.next().expect("no blank lines");

            // Positional `;`, and only there.
            let semicolons = line.matches(';').count();
            if semicolons > 0 {
                assert_eq!(semicolons, 1, "one `;` per line at most: {line}");
                assert!(
                    line.ends_with("; then"),
                    "the only `;` precedes `then`: {line}"
                );
            }

            match first {
                "if" => {
                    depth += 1;
                    deepest = deepest.max(depth);
                    // The command being branched on has to be a verb too, which a naive first-word
                    // check would skip entirely.
                    let guarded = words
                        .next()
                        .expect("`if` branches on something")
                        .trim_end_matches(';');
                    assert!(
                        matches!(guarded, "test" | "dhcp" | "tftpboot" | "load"),
                        "{guarded} is not a status this script knows how to read"
                    );
                }
                "fi" => depth = depth.checked_sub(1).expect("no `fi` without an `if`"),
                // `else` is deliberately not in this list: a chain of guards costs a few more
                // lines and needs one fewer parser feature than a branch does.
                other => assert!(
                    matches!(other, "echo" | "setenv" | "load" | "fdt" | "booti"),
                    "{other} is a verb the bench transcript does not show"
                ),
            }
        }
        assert_eq!(depth, 0, "every `if` is closed");
        assert!(deepest <= 2, "nesting stays shallow, was {deepest}");

        // The tail milestone 218 proved is unchanged, which is why this is a small change.
        assert!(script.contains("fdt move ${fdtcontroladdr} 0x86000000"));
        assert!(
            script.contains("booti ${kernel_addr_r} 0x90000000:${nife_archive_size} 0x86000000")
        );
        // The address is baked, and it is also announced before anything depends on it.
        assert!(script.contains("setenv nife_boot_server 10.1.2.3"));
        assert!(script.contains("echo nife: tftp server is ${nife_boot_server}"));
        // And the card script is untouched by any of this: the default is still the default, and
        // it still has no network verb in it at all.
        assert!(!BOARD_BOOT_SCRIPT.contains("tftpboot"));
        assert!(!BOARD_BOOT_SCRIPT.contains("dhcp"));
        assert!(!BOARD_BOOT_SCRIPT.contains("if "));
    }

    /// **The fallback, actually taken**: the generated script under `/bin/sh`, with the six U-Boot
    /// verbs stubbed as shell functions whose exit status the test chooses. The constructs it uses
    /// are common to hush and POSIX `sh`.
    ///
    /// It proves the control flow (which loads run in which order, that a network failure reaches
    /// the card, that a half-transfer takes both halves from the card, that `booti` gets the length
    /// of the load that happened) and nothing about whether hush accepts the file; the block's BUGS
    /// says so. `tftp_fails_at` is a call number because the interesting failure is the second
    /// transfer.
    fn run_under_sh(script: &str, dhcp: i32, tftp_fails_at: i32, load: i32) -> String {
        let preamble = format!(
            "\
kernel_addr_r=0x40200000
fdtcontroladdr=0x4fe00000
devtype=mmc
devnum=1
distro_bootpart=1
tftp_calls=0
setenv() {{ name=$1; shift; eval \"$name=\\\"\\$*\\\"\"; }}
dhcp() {{ echo CALL dhcp; return {dhcp}; }}
tftpboot() {{
  tftp_calls=$((tftp_calls + 1))
  echo CALL tftpboot $1 $2
  filesize=9044480
  [ \"$tftp_calls\" != \"{tftp_fails_at}\" ]
}}
load() {{ echo CALL load $3 $4; filesize=1234; return {load}; }}
fdt() {{ echo CALL fdt $*; }}
booti() {{ echo CALL booti $*; }}
"
        );
        let dir = std::env::temp_dir().join(format!(
            "nife-boot-script-{}-{dhcp}{tftp_fails_at}{load}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("boot.cmd");
        std::fs::write(&path, format!("{preamble}{script}")).expect("write the script");
        let out = Command::new("/bin/sh")
            .arg(&path)
            .output()
            .expect("/bin/sh runs the generated script");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.status.success(), "the script itself must not fail");
        String::from_utf8(out.stdout).expect("utf-8 output")
    }

    #[test]
    fn the_network_script_falls_back_to_the_card_when_the_network_is_not_there() {
        let script = board_network_boot_script("10.1.2.3");

        // Everything works: two TFTP transfers, no card read at all, and the archive length is the
        // one TFTP reported.
        let net = run_under_sh(&script, 0, 0, 0);
        assert!(
            net.contains("CALL tftpboot 0x40200000 nife-vf2.img"),
            "{net}"
        );
        assert!(
            net.contains("CALL tftpboot 0x90000000 nife-initial-ramdisk.img"),
            "{net}"
        );
        assert!(!net.contains("CALL load"), "the card is not touched: {net}");
        assert!(net.contains("payload came from net"), "{net}");
        assert!(
            net.contains("CALL booti 0x40200000 0x90000000:9044480 0x86000000"),
            "{net}"
        );

        // No lease: the card path runs, and it is the whole card path rather than a kernel from one
        // place and an archive from another.
        let card = run_under_sh(&script, 1, 0, 0);
        assert!(
            !card.contains("CALL tftpboot"),
            "no lease, no transfer: {card}"
        );
        assert!(
            card.contains("CALL load 0x40200000 /nife-vf2.img"),
            "{card}"
        );
        assert!(
            card.contains("CALL load 0x90000000 /nife-initial-ramdisk.img"),
            "{card}"
        );
        assert!(card.contains("payload came from card"), "{card}");
        assert!(
            card.contains("CALL booti 0x40200000 0x90000000:1234 0x86000000"),
            "the length is the card load's, not a stale one: {card}"
        );

        // A lease and a kernel, and then the archive transfer dies. This is the case worth having:
        // a kernel over TFTP with an archive off the card is a mismatched pair, which halts at
        // MEASURED BOOT REFUSED, so the fallback has to take BOTH halves from the card.
        let half = run_under_sh(&script, 0, 2, 0);
        assert_eq!(
            half.matches("CALL tftpboot").count(),
            2,
            "the kernel arrived and the archive was attempted: {half}"
        );
        assert!(
            half.contains("CALL load 0x40200000 /nife-vf2.img"),
            "{half}"
        );
        assert!(
            half.contains("CALL load 0x90000000 /nife-initial-ramdisk.img"),
            "{half}"
        );
        assert!(half.contains("payload came from card"), "{half}");
        assert!(
            half.contains("CALL booti 0x40200000 0x90000000:1234 0x86000000"),
            "and the length is the card's, not the abandoned transfer's: {half}"
        );

        // The kernel transfer itself failing stops there rather than chasing the archive.
        let first = run_under_sh(&script, 0, 1, 0);
        assert_eq!(
            first.matches("CALL tftpboot").count(),
            1,
            "no archive after a failed kernel: {first}"
        );
        assert!(first.contains("payload came from card"), "{first}");

        // Neither: nothing is booted, and the board says so rather than jumping into whatever is
        // at 0x40200000 from a previous boot.
        let neither = run_under_sh(&script, 1, 0, 1);
        assert!(!neither.contains("CALL booti"), "{neither}");
        assert!(!neither.contains("CALL fdt"), "{neither}");
        assert!(neither.contains("payload came from none"), "{neither}");
        assert!(neither.contains("still at the prompt"), "{neither}");
    }
}
