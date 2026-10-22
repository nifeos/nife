//! **Finding the board's serial port and putting it in the right mode**, with no dependency.
//!
//! # Why `stty` and not a serial crate
//!
//! Configuring a UART is one `tcsetattr` call, and the two ordinary ways to make it from Rust are
//! the `serialport` crate or `libc` plus the `termios` struct by hand. Both are new dependencies in
//! the shipping graph, and §46 (thin primitives or whole subsystems; we write everything in
//! between) makes taking one a decision rather than a convenience; this is a lane, and a lane does
//! not take that decision. `stty(1)` makes the same
//! call, is in every base system, and has been since the 1970s. The price is a process spawn per
//! session and a diagnostic that is a program's stderr rather than an `errno`, which is a fair
//! trade for a tool that runs once a boot.
//!
//! If this ever needs to *write* to the board with flow control, or set a non-standard baud, the
//! trade changes and the dependency is worth proposing. It does not need either today.
//!
//! # The ordering that is not arbitrary, and the thing it cost somebody
//!
//! **Open the device first, then run `stty` on it, then check that the speed took.** Opening a
//! macOS `cu` device resets its termios towards the driver's default, so a configuration made
//! before the read descriptor exists is undone by the open that follows it. Holding the descriptor
//! is also what keeps the setting alive: it reverts when the last user closes.
//!
//! **This is not a theory.** calef's first capture of the VisionFive 2 on 2026-09-01 was pure
//! garbage, because it configured the port with `stty` and then ran `cat`, and `cat`'s open put
//! the device back to the default rate. It looks exactly like a wiring fault or a bad adapter,
//! which is the runbook's second triage row, and it cost a power cycle to diagnose.
//!
//! So the read-back below is a gate rather than a nicety: it asks the device what speed it is
//! actually at and says so loudly when the answer is not 115200. An invisible wrong baud produces
//! plausible-looking bytes and sends the next person chasing hardware; a stated one takes a
//! second to fix.
//!
//! **The residual hazard, stated rather than hidden.** Because the configuration is a separate
//! process rather than a `tcsetattr` on our own descriptor, any *other* process that opens this
//! device mid-session can put it back to the default, and nothing here would notice. That is the
//! honest cost of having no dependency, and it is the case that would justify taking one.
//!

//! # `O_NONBLOCK`
//!
//! Not used, and the reason is that it would be redundant here. Its job would be to stop a silent
//! board hanging the tool in `read`, and `watch` already guarantees that structurally by doing the
//! read on a worker thread while the deadline runs on a channel timeout. That guarantee holds even
//! when the `stty` above fails outright, which `O_NONBLOCK` on its own would not: a non-blocking
//! read returns immediately, but only a deadline decides when to stop asking.
//!
//! # And why `cu.` rather than `tty.`
//!
//! On macOS `/dev/tty.*` is the dial-in side and blocks in `open` until carrier detect asserts,
//! which a three-wire console cable never does. That is the classic way a serial tool hangs before
//! printing anything at all, and it hangs in `open`, where no deadline of ours has started yet.
//! `/dev/cu.*` is the call-out side and opens immediately.

use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The bench's baud rate: 115200 8N1, no flow control, from the VisionFive 2 Quick Start Guide by
/// way of `notes/visionfive2.md`'s serial wiring table.
pub const BAUD: u32 = 115_200;

/// The device-name prefixes worth offering, most likely first.
///
/// The rig measured on 2026-09-01 is a WCH CH343 presenting as CDC-ACM, which is why
/// `cu.usbmodem` leads. The path on that Mac is `/dev/cu.usbmodem5C7B0104661` and the digits are
/// the adapter's own serial number, so it is **not** written down here: hard-coding it would work
/// on exactly one machine with exactly one dongle.
#[cfg(target_os = "macos")]
const PREFIXES: &[&str] = &[
    "cu.usbmodem",
    "cu.usbserial",
    "cu.SLAB_USBtoUART",
    "cu.wchusbserial",
];

/// Linux names the same two classes `ttyACM*` (CDC-ACM, which is the CH343) and `ttyUSB*`.
#[cfg(not(target_os = "macos"))]
const PREFIXES: &[&str] = &["ttyACM", "ttyUSB"];

/// Every device in `/dev` that looks like a USB serial adapter, sorted.
///
/// Sorted so that two runs on an unchanged bench pick the same port, which matters when there are
/// two adapters and the caller is a script.
#[must_use]
pub fn candidates() -> Vec<PathBuf> {
    scan(Path::new("/dev"))
}

/// [`candidates`]'s directory walk, over any directory rather than always `/dev`.
///
/// Separate for [`choose`]/[`pick`]'s own reason: whether an adapter happens to be plugged into
/// the machine running the tests is not something a host test can control, so `candidates()`
/// itself cannot be asserted on portably. This can, against a temporary directory built for the
/// purpose, which is what makes the prefix filter and the sort a tested property instead of one
/// only ever exercised by whatever `/dev` happens to hold.
fn scan(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| PREFIXES.iter().any(|prefix| name.starts_with(prefix)))
        })
        .map(|entry| entry.path())
        .collect();
    found.sort();
    found
}

/// Pick the port to use: the caller's choice if they made one, otherwise the only candidate.
///
/// Ambiguity is an error rather than a guess. Two adapters on one desk is the normal state of a
/// bench with more than one board, and a tool that silently picked the wrong one would produce a
/// log of somebody else's boot.
///
/// # Errors
///
/// If no candidate exists, or if more than one does and the caller named none. Both messages list
/// what was found and what to pass.
pub fn choose(asked: Option<&Path>) -> io::Result<PathBuf> {
    pick(&candidates(), asked)
}

/// [`choose`]'s decision, over a candidate list rather than over `/dev`.
///
/// Separate so the three cases can be tested on any machine. They cannot be tested through
/// `choose`, because which of them fires depends on what happens to be plugged into the machine
/// running the tests: this lane's own Mac has the CH343 attached and takes the one-candidate
/// branch, and CI has nothing and takes the zero branch, so neither ever sees the other's message
/// or the ambiguous one at all.
fn pick(found: &[PathBuf], asked: Option<&Path>) -> io::Result<PathBuf> {
    if let Some(path) = asked {
        return Ok(path.to_path_buf());
    }
    match found.len() {
        1 => Ok(found[0].clone()),
        0 => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "no USB serial adapter in /dev (looked for {}). \
                 Is the adapter plugged in? Pass --port to name one.",
                PREFIXES.join("*, ")
            ),
        )),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "more than one USB serial adapter; pass --port to say which:\n  {}",
                found
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n  ")
            ),
        )),
    }
}

/// `-f` on BSD and macOS, `-F` on GNU. Nothing else about `stty` differs between them, which is
/// why this is one constant and not two code paths.
const DEVICE_FLAG: &str = if cfg!(target_os = "linux") {
    "-F"
} else {
    "-f"
};

/// Open the port and put it in raw mode at [`BAUD`], in that order.
///
/// Returns the open descriptor and whatever the configuration had to say. A configuration failure
/// is **reported, not fatal**: an adapter whose driver refuses one of these flags still delivers
/// bytes, and a tool that refused to log them would be worse than one that logs them with a
/// warning attached. The read timeout is only ever belt to `watch`'s braces, so losing it costs
/// nothing but a spinning worker thread.
///
/// # Errors
///
/// If the device cannot be opened. That is the one genuinely fatal case, and its usual causes are
/// an adapter that was unplugged, a `/dev/tty.*` path (see this module's header), and another
/// program already holding the port.
///
/// # Examples
///
/// Watch a board boot, with the default policy's cap:
///
/// ```no_run
/// use std::fs::File;
/// use serial_console::{port, watch::{self, Policy}};
///
/// let path = port::choose(None)?;
/// let (device, complaint) = port::open(&path)?;
/// if let Some(complaint) = complaint {
///     eprintln!("board-console: {complaint}");
/// }
/// let mut log = File::create("target/board-console.log")?;
/// let session = watch::watch(device, &mut log, &Policy::default(), true)?;
/// println!("{}", session.summary());
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn open(path: &Path) -> io::Result<(File, Option<String>)> {
    if let Some(warning) = dial_in_warning(path) {
        eprintln!("board-console: {warning}");
    }
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    let complaint = configure(path).or_else(|| confirm_speed(path));
    Ok((file, complaint))
}

/// The `stty` arguments that put a port in the mode this tool wants.
///
/// Pure, and separate from running it, because the argument list is the part worth asserting on:
/// every flag here is a decision with a reason, and a silent drop of `-icrnl` would corrupt the
/// bare `\r` the recogniser splits on without failing anything.
fn set_args(path: &str) -> Vec<String> {
    let speed = BAUD.to_string();
    let mut args = vec![DEVICE_FLAG.to_string(), path.to_string()];
    args.extend(
        [
            // 115200 8N1, no flow control: the wiring table's line, spelled out.
            speed.as_str(),
            "cs8",
            "-cstopb",
            "-parenb",
            "-crtscts",
            // Raw. Line discipline processing on a boot log turns a firmware progress bar into
            // nonsense and, worse, `icrnl` would rewrite the bare `\r` the recogniser splits on.
            "-icanon",
            "-echo",
            "-icrnl",
            "-ixon",
            "-opost",
            // A read returns after 0.1s with whatever has arrived, including nothing. `watch` does
            // not depend on this (its worker is on its own thread precisely so it need not), but
            // with it the worker parks instead of spinning.
            "min",
            "0",
            "time",
            "1",
        ]
        .iter()
        .map(|s| (*s).to_string()),
    );
    args
}

/// What one `stty` run has to say for itself, or `None` if it had nothing to complain about.
///
/// Pure, taking the outcome rather than producing it, so the wording a person reads at a bench is
/// asserted on by a host test rather than only by whoever last ran it against hardware.
fn run_complaint(ok: bool, status: &str, stderr: &str) -> Option<String> {
    if ok {
        return None;
    }
    Some(format!("stty exited {status}: {}", stderr.trim()))
}

/// Read `stty`'s report of a port and say whether the speed is the one we asked for.
///
/// Pure for the same reason, and this is the one whose exact behaviour cost somebody a power
/// cycle. Three cases, all of them tested: the speed is right and nothing is said; the speed is
/// wrong and the message quotes what the device actually reported; and `stty` said nothing about
/// speed at all, which is reported rather than silently passed, because a report with no speed in
/// it is not evidence that the speed is right.
fn speed_complaint(said: &str) -> Option<String> {
    if said.contains(&format!("{BAUD} baud")) {
        return None;
    }
    let reported = said
        .lines()
        .find(|line| line.contains("baud"))
        .unwrap_or("(no speed reported)")
        .trim();
    Some(format!(
        "the port is not at {BAUD} after configuring it; stty says: {reported}. \
         Bytes at the wrong rate look exactly like a wiring fault, so fix this before \
         suspecting the cable"
    ))
}

/// The one line a `/dev/tty.*` path deserves, or `None` if the path is fine.
///
/// Returned rather than printed so a test can read it. See this module's header for why the
/// dial-in device is the wrong one: it blocks in `open`, before any deadline of ours exists.
fn dial_in_warning(path: &Path) -> Option<String> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let name = path.file_name().and_then(OsStr::to_str)?;
    if !name.starts_with("tty.") {
        return None;
    }
    Some(format!(
        "{} is the dial-in device; open() blocks on carrier detect and a three-wire console \
         cable never asserts it. Use the cu.* name instead.",
        path.display()
    ))
}

/// Run `stty` and hand back its outcome as plain data.
///
/// The whole IO residue of this module is these five lines, on purpose: everything a host test
/// could reach has been lifted above it.
fn stty(args: &[String]) -> Option<(bool, String, String, String)> {
    let out = Command::new("stty").args(args).output().ok()?;
    Some((
        out.status.success(),
        out.status.to_string(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    ))
}

/// Configure an already-open device. Returns its complaint, if it had one.
fn configure(path: &Path) -> Option<String> {
    let Some(path) = path.to_str() else {
        return Some("the device path is not valid UTF-8, so stty cannot be given it".to_string());
    };
    match stty(&set_args(path)) {
        Some((ok, status, _, stderr)) => run_complaint(ok, &status, &stderr),
        None => Some("could not run stty; is it on PATH?".to_string()),
    }
}

/// Ask the device what speed it is actually at, and complain if it is not the one we asked for.
///
/// Returning `None` when `stty` cannot be read is deliberate: an unreadable report is not evidence
/// that the port is wrong, and a false alarm here would train the reader to ignore a real one.
fn confirm_speed(path: &Path) -> Option<String> {
    let path = path.to_str()?;
    let (ok, _, stdout, _) = stty(&[DEVICE_FLAG.to_string(), path.to_string()])?;
    if !ok {
        return None;
    }
    speed_complaint(&stdout)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn an_explicit_port_is_taken_as_given() {
        let asked = PathBuf::from("/dev/cu.usbmodemDEADBEEF");
        assert_eq!(choose(Some(&asked)).unwrap(), asked);
    }

    /// All three candidate cases, on any machine. The messages are what a person meets when the
    /// tool will not start, so they are asserted on rather than left to whoever hits them.
    #[test]
    fn the_candidate_list_decides_and_says_why_when_it_cannot() {
        let one = PathBuf::from("/dev/cu.usbmodemONE");
        let two = PathBuf::from("/dev/cu.usbmodemTWO");
        assert_eq!(pick(std::slice::from_ref(&one), None).unwrap(), one);

        let none = pick(&[], None).unwrap_err();
        assert_eq!(none.kind(), io::ErrorKind::NotFound);
        assert!(none.to_string().contains("--port"));
        assert!(
            none.to_string().contains(PREFIXES[0]),
            "it must say what it looked for"
        );

        let both = pick(&[one, two], None).unwrap_err();
        assert_eq!(both.kind(), io::ErrorKind::InvalidInput);
        assert!(both.to_string().contains("cu.usbmodemONE"));
        assert!(both.to_string().contains("cu.usbmodemTWO"));

        // An explicit choice wins over an ambiguity, which is the whole point of having one.
        let asked = PathBuf::from("/dev/cu.usbmodemTHREE");
        assert_eq!(
            pick(&[PathBuf::from("a"), PathBuf::from("b")], Some(&asked)).unwrap(),
            asked
        );
    }

    #[test]
    fn candidates_are_sorted_so_two_runs_agree() {
        let found = candidates();
        let mut sorted = found.clone();
        sorted.sort();
        assert_eq!(found, sorted);
    }

    /// **The walk itself, against a directory this test controls rather than whatever `/dev`
    /// happens to hold.** `candidates()` cannot be pinned down portably (see [`scan`]'s own
    /// doc), so this is what actually proves the prefix filter and the sort: a real adapter
    /// name, a look-alike with the wrong prefix, and a name that happens to sort last.
    #[test]
    fn scan_keeps_only_the_matching_prefixes_and_sorts_them() {
        let dir = std::env::temp_dir().join(format!(
            "serial_console-scan-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["zz.not-a-serial-port", PREFIXES[0], "aa.also-not-one"] {
            std::fs::File::create(dir.join(name)).unwrap();
        }
        let real_name = format!("{}TESTDEVICE", PREFIXES[0]);
        std::fs::File::create(dir.join(&real_name)).unwrap();

        let found = scan(&dir);
        assert_eq!(
            found,
            vec![dir.join(PREFIXES[0]), dir.join(&real_name)],
            "only the two matching names, sorted, and neither look-alike"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A directory that does not exist is an empty list, not a panic: this is what makes
    /// `choose`'s "no candidate" message reachable on a machine with no `/dev` entries at all
    /// worth naming, rather than a path this function itself falls over on.
    #[test]
    fn scan_of_a_missing_directory_is_empty() {
        assert_eq!(
            scan(Path::new("/no/such/directory/board-console-test")),
            Vec::<PathBuf>::new()
        );
    }

    /// **`stty` itself, the one line of IO residue nothing above it reaches.** `stty` is on
    /// every base system this crate's header claims for it (that is the whole argument for
    /// calling it rather than taking a dependency), so an argument it cannot possibly accept is
    /// a real, portable way to make it fail without a device: this is not asserting on the
    /// exact wording (that varies by OS, per this module's header), only that the returned
    /// status and stderr are `stty`'s own and not a placeholder standing in for them.
    #[test]
    fn stty_runs_the_real_program_and_hands_back_what_it_actually_said() {
        let (ok, status, _stdout, stderr) =
            stty(&["--board-console-test-no-such-flag".to_string()])
                .expect("stty is on PATH in every base system, per this module's header");
        assert!(!ok, "an unrecognised flag must not report success");
        assert_ne!(status, "", "a real exit status is never the empty string");
        assert_ne!(
            status, "xyzzy",
            "a real exit status is never this placeholder"
        );
        assert!(
            !stderr.is_empty(),
            "stty must have said something about the bad flag"
        );
    }

    /// Every flag in here is a decision with a reason, and the one that would hurt most if it went
    /// missing is `-icrnl`: without it the line discipline rewrites the bare `\r` the recogniser
    /// splits on, and nothing would fail, it would just quietly stop seeing U-Boot's countdown.
    #[test]
    fn the_configuration_asks_for_the_mode_the_bench_needs() {
        let args = set_args("/dev/cu.usbmodemDEADBEEF");
        assert_eq!(args[0], DEVICE_FLAG);
        assert_eq!(args[1], "/dev/cu.usbmodemDEADBEEF");
        for wanted in [
            "115200", "cs8", "-cstopb", "-parenb", "-crtscts", "-icanon", "-echo", "-icrnl",
            "-ixon", "-opost", "min", "0", "time", "1",
        ] {
            assert!(args.contains(&wanted.to_string()), "missing {wanted}");
        }
    }

    #[test]
    fn a_successful_run_says_nothing_and_a_failed_one_quotes_the_tool() {
        assert_eq!(run_complaint(true, "exit status: 0", ""), None);
        let complaint = run_complaint(false, "exit status: 1", "stty: not a terminal\n").unwrap();
        assert!(complaint.contains("exit status: 1"));
        assert!(complaint.contains("not a terminal"));
    }

    /// The case that cost calef a power cycle on 2026-09-01: a port sitting at its default rate
    /// delivers bytes that look exactly like a wiring fault. Real `stty` output, all three shapes.
    #[test]
    fn the_wrong_speed_is_named_rather_than_left_to_look_like_a_bad_cable() {
        let right = "speed 115200 baud;\nlflags: -icanon -echo\n";
        assert_eq!(speed_complaint(right), None);

        let wrong = speed_complaint("speed 9600 baud;\nlflags: -icanon -echo\n").unwrap();
        assert!(wrong.contains("115200"), "it must say what was wanted");
        assert!(wrong.contains("9600"), "and what the device actually said");
        assert!(wrong.contains("wiring fault"), "and why it matters");

        // A report with no speed line in it is not evidence that the speed is right.
        let silent = speed_complaint("lflags: -icanon -echo\n").unwrap();
        assert!(silent.contains("(no speed reported)"));
    }

    #[test]
    fn the_dial_in_device_is_warned_about_and_the_call_out_one_is_not() {
        assert_eq!(dial_in_warning(Path::new("/dev/cu.usbmodemDEADBEEF")), None);
        if cfg!(target_os = "macos") {
            let warning = dial_in_warning(Path::new("/dev/tty.usbmodemDEADBEEF")).unwrap();
            assert!(warning.contains("carrier detect"));
            assert!(warning.contains("cu.*"));
        } else {
            // Off macOS there is no dial-in twin, so even a `tty.` name gets no warning. The weekly
            // census runs on Linux, where only this branch runs, and a deleted `!` on the `cfg!`
            // guard survived it (2026-10-07 census triage).
            assert_eq!(
                dial_in_warning(Path::new("/dev/tty.usbmodemDEADBEEF")),
                None
            );
        }
    }

    /// **The open path, against something a host actually has.**
    ///
    /// A regular file is not a serial port, and this test does not pretend otherwise: what it
    /// exercises is that `open` opens what it was given, that a device `stty` cannot configure
    /// produces a *complaint* rather than a failure, and that the descriptor it hands back reads
    /// the bytes that are there. That last one is checked against the real 2026-09-01 capture, so
    /// the whole path from `port::open` to a recognised boot runs in a host test.
    ///
    /// What no host test can reach is whether `tcsetattr` actually took, since nothing here is a
    /// tty. That was checked by hand against the CH343 dongle (9600 before, 115200 while held,
    /// reverting on exit) and is what `confirm_speed` gates at runtime.
    #[test]
    fn open_hands_back_a_readable_descriptor_and_a_complaint_it_can_survive() {
        let captured = include_bytes!("../tests/fixtures/captured/vf2-2026-09-01-manual-boot.log");
        let path = std::env::temp_dir().join(format!(
            "serial_console-open-{}-{:?}.log",
            std::process::id(),
            std::thread::current().id()
        ));
        File::create(&path).unwrap().write_all(captured).unwrap();

        let (device, complaint) = open(&path).unwrap();
        assert!(
            complaint.is_some(),
            "stty cannot configure a regular file, and that must be survivable"
        );

        let mut sink = Vec::new();
        let session =
            crate::watch::watch(device, &mut sink, &crate::watch::Policy::default(), false)
                .unwrap();
        assert_eq!(
            session.progress.reached(),
            crate::progress::Stage::Tour,
            "the descriptor `open` returned is the one the capture is in"
        );
        std::fs::remove_file(&path).ok();
    }

    /// The one genuinely fatal case, and the one a bench hits by unplugging the adapter. The
    /// second path also drives the dial-in warning, which is printed on the way past.
    #[test]
    fn a_device_that_is_not_there_is_an_error_and_not_a_complaint() {
        let e = open(Path::new("/dev/cu.usbmodem-nothing-is-here")).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::NotFound);
        let e = open(Path::new("/dev/tty.usbmodem-nothing-is-here")).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::NotFound);
    }

    /// A path `stty` cannot be handed at all. Rare, and the branch exists because the alternative
    /// is `unwrap` on a conversion that can fail: a device name is whatever the kernel put in
    /// `/dev`, not necessarily UTF-8.
    #[test]
    fn a_path_that_is_not_utf8_is_a_complaint_rather_than_a_panic() {
        use std::os::unix::ffi::OsStrExt;
        let bad = Path::new(OsStr::from_bytes(b"/dev/cu.\xff\xfe"));
        assert!(
            configure(bad).unwrap().contains("not valid UTF-8"),
            "it has to say which of the several things could be wrong"
        );
        assert_eq!(
            confirm_speed(bad),
            None,
            "and it cannot check a speed either"
        );
    }

    /// Reading a speed back from something that is not a tty says nothing, deliberately: an
    /// unreadable report is not evidence that the port is wrong, and a false alarm here would
    /// train the reader to ignore the real one.
    #[test]
    fn a_speed_that_cannot_be_read_is_not_reported_as_wrong() {
        let path =
            std::env::temp_dir().join(format!("serial_console-speed-{}", std::process::id()));
        File::create(&path).unwrap();
        assert_eq!(confirm_speed(&path), None);
        std::fs::remove_file(&path).ok();
    }
}
