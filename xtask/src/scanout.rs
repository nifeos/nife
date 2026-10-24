//! The scanout check (milestone 29): prove the pixels reached the DEVICE, not only our buffer.
//!
//! The in-guest test proves the framebuffer byte for byte, but the suite runs `-display none` and
//! nothing inside the guest can read QEMU's host-side surface, so a wrong pixel format or scanout
//! rectangle would pass it. QEMU's monitor `screendump FILE` writes a PPM of the scanout even
//! headless, so the runners take a monitor socket (`NIFE_GPU_MON`) and this polls, dumps and
//! compares **while the ordinary test run happens**, with no second boot and no synchronization
//! with the guest. Fail-safe: a pattern that never reaches the scanout, a display test that stops
//! running, or a device reset that wipes the surface leaves no dump matching, and that is reported.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::host::workspace_root;
use crate::inbound::InboundProber;

/// The unix socket the QEMU monitor listens on for `arch`. **In /tmp on purpose**: a unix socket path
/// must fit in 104 bytes, and a worktree checkout plus `target/` gets close enough to that limit to
/// break on someone else's machine. The PPM it dumps goes under `target/`, where path length is free.
pub(crate) fn gpu_mon_socket(arch: &str) -> String {
    format!("/tmp/nife-gpu-{arch}-{}.sock", std::process::id())
}

fn gpu_shot_path(arch: &str) -> PathBuf {
    workspace_root().join(format!("target/gpu-scanout-{arch}.ppm"))
}

/// Where the composed screen's matching dump is kept (milestone 33). A separate file because the
/// composed screen is transient: the poll loop overwrites [`gpu_shot_path`] on every dump, so the one
/// that matched has to be copied aside or there is nothing left to look at after a run.
fn gpu_compose_path(arch: &str) -> PathBuf {
    workspace_root().join(format!("target/gpu-compose-{arch}.ppm"))
}

/// Where the display terminal's matching dump is kept (milestone 29's text increment). Transient for
/// the same reason the composed screen is: rung one's pattern replaces it on the same scanout.
fn gpu_text_path(arch: &str) -> PathBuf {
    workspace_root().join(format!("target/gpu-text-{arch}.ppm"))
}

/// Does this PPM hold the pattern rung one's client painted (milestone 29)?
///
/// Compares against `graphics_contract::pixel`, the same definition the client painted from and the kernel
/// test digested against, so the host cannot disagree with the guest about what the pattern is.
fn scanout_holds_the_pattern(ppm: &[u8]) -> Result<(), String> {
    scanout_matches(ppm, graphics_contract::pixel)
}

/// Does this PPM hold the screen rung two's compositor composed (milestone 33)?
///
/// The same check against a different definition: `compositor::expected_screen_pixel` with every window
/// of the scene committed, which is the picture the kernel test predicted and the capture client
/// digested. **This is the check a guest-side digest cannot replace.** Three witnesses inside the
/// guest agree about the framebuffer; only the host can see what the device is actually scanning out,
/// so a wrong pixel format, a wrong scanout rectangle, or a compositor that wrote its picture
/// somewhere other than the scanout would pass all three and fail here.
fn scanout_holds_the_composed_screen(ppm: &[u8]) -> Result<(), String> {
    scanout_matches(ppm, |x, y| {
        compositor::expected_screen_pixel(compositor::SCENE.len(), x, y)
    })
}

/// Does `ppm` hold the **text** the display terminal drew (milestone 29's remaining increment)?
/// `Ok` if every pixel matches, else why not.
///
/// The definition is the VT engine itself, run on the host over `video_terminal::script`, the
/// script and engine the kernel's terminal drew from: every pixel of every glyph, in the right cell
/// and colour, with the cursor where the engine says. **A guest-side digest cannot replace this**:
/// a wrong pixel format turns text into text nobody can read. Its negative control is
/// `tests::the_scanout_check_rejects_text_that_is_one_letter_wrong`.
fn scanout_holds_the_terminals_text(ppm: &[u8]) -> Result<(), String> {
    // `Vt::new` then the script fed into it: a `Vt` is hundreds of KiB, and this is the one shape
    // that works for the crate's kernel-side callers too (see `Vt`'s and `script::full_screen`'s
    // doc comments). The scrolled script, because the kernel-side test scrolls the terminal before
    // holding the picture up, so the host grades the scroll path (pixels moved, not re-rendered) as
    // well.
    let mut expect =
        video_terminal::Vt::new(video_terminal::script::COLS, video_terminal::script::ROWS);
    video_terminal::script::scrolled_full_screen(&mut expect, None);
    scanout_matches(ppm, |x, y| expect.pixel(x, y))
}

/// Compare a `screendump` PPM against a per-pixel definition of what should be on the screen.
///
/// The geometry must match too: a scanout of the wrong size is a `SET_SCANOUT` bug, not a near miss.
///
/// Returns `Err(reason)` rather than a bool so a mismatch says which pixel and what it should have
/// been, since "the screen is wrong" is otherwise the least actionable failure in graphics.
fn scanout_matches(ppm: &[u8], want_pixel: impl Fn(u32, u32) -> u32) -> Result<(), String> {
    // P6 header: "P6\n<w> <h>\n<maxval>\n", then w*h*3 bytes, RGB per pixel.
    let text = String::from_utf8_lossy(&ppm[..ppm.len().min(64)]).to_string();
    let mut fields = text.split_ascii_whitespace();
    if fields.next() != Some("P6") {
        return Err("not a P6 PPM".into());
    }
    let w: u32 = fields
        .next()
        .and_then(|f| f.parse().ok())
        .ok_or("no width")?;
    let h: u32 = fields
        .next()
        .and_then(|f| f.parse().ok())
        .ok_or("no height")?;
    let maxval = fields.next().ok_or("no maxval")?;
    if maxval != "255" {
        return Err(format!("maxval {maxval}, expected 255"));
    }
    if (w, h) != (graphics_contract::WIDTH, graphics_contract::HEIGHT) {
        return Err(format!(
            "scanout is {w}x{h}, the surface is {}x{}",
            graphics_contract::WIDTH,
            graphics_contract::HEIGHT
        ));
    }
    // The pixel data starts after the fourth whitespace-terminated field. Find it by walking the
    // header rather than assuming a byte offset, because QEMU is free to format the header its way.
    let mut seen = 0;
    let mut i = 0;
    while i < ppm.len() && seen < 4 {
        if ppm[i].is_ascii_whitespace() {
            seen += 1;
            while seen < 4 && i + 1 < ppm.len() && ppm[i + 1].is_ascii_whitespace() {
                i += 1;
            }
        }
        i += 1;
    }
    let pixels = &ppm[i..];
    let want_len = (w * h * 3) as usize;
    if pixels.len() < want_len {
        // A dump caught mid-write. Not a failure, just not usable yet.
        return Err(format!(
            "short by {} bytes (QEMU may still be writing)",
            want_len - pixels.len()
        ));
    }
    for y in 0..h {
        for x in 0..w {
            let o = ((y * w + x) * 3) as usize;
            let want = want_pixel(x, y);
            let (r, g, b) = (
                ((want >> 16) & 0xff) as u8,
                ((want >> 8) & 0xff) as u8,
                (want & 0xff) as u8,
            );
            if (pixels[o], pixels[o + 1], pixels[o + 2]) != (r, g, b) {
                return Err(format!(
                    "pixel ({x},{y}) is rgb({},{},{}), it should be rgb({r},{g},{b})",
                    pixels[o],
                    pixels[o + 1],
                    pixels[o + 2],
                ));
            }
        }
    }
    Ok(())
}

/// **Press `key` on the guest's keyboard** over the monitor socket `sock`, the one the scanout
/// check uses; a connection that fails is ignored.
///
/// Nothing inside the guest can press a key, so here the host is an *actor*. `sendkey` sends a
/// press and a release, which makes the driver's handling of the value field checkable: counting
/// the release would double every character. It is sent every poll from the start, because QEMU
/// **drops key events until a driver sets `DRIVER_OK`**. `video_terminal::script::HOST_KEY` is the
/// one definition of which key, shared with the kernel test that asserts the byte.
pub(crate) fn sendkey(sock: &str, key: &str) {
    if let Ok(mut monitor) = Monitor::connect(sock) {
        let _ = monitor.sendkey(key);
    }
}

/// Ask the QEMU monitor on `sock` for a screendump into `out`, and return once QEMU has written it.
/// Returns false when the monitor did not answer (QEMU still starting, or already gone) or QEMU
/// refused the dump, which the caller treats as "try again".
pub(crate) fn screendump(sock: &str, out: &Path) -> bool {
    Monitor::connect(sock).is_ok_and(|mut monitor| monitor.screendump(out).is_ok())
}

/// **One connection to QEMU's human monitor, which waits for each command's answer.**
///
/// QEMU's monitor socket serves one client with a listen backlog of one, so a second connection
/// made before the first is accepted can be refused (9 of 10 times against an idle QEMU, and more
/// under load). QEMU prints its `(qemu) ` prompt only after a command finishes, `screendump`
/// included (50 of 50 dumps whole when the prompt came back), so waiting for it makes each command
/// synchronous and leaves nothing waiting in the backlog.
/// notes/framebuffer-contract/scanout-referee.md has the runs.
///
/// Name: provisional, minted 2026-10-12 by lane/scanout-referee-flake.
pub(crate) struct Monitor {
    stream: std::os::unix::net::UnixStream,
}

/// What QEMU's human monitor prints when it is ready for the next command.
const MONITOR_PROMPT: &[u8] = b"(qemu) ";

/// How long one command may take before the connection is treated as dead. A screendump on an
/// idle host answers in milliseconds; this is the bound for a host so loaded that the poll has
/// stopped meaning anything, not a margin anything is expected to approach.
const MONITOR_ANSWER_WITHIN: std::time::Duration = std::time::Duration::from_secs(10);

impl Monitor {
    /// Connect and wait for the greeting's prompt. Fails while the socket is not there yet.
    pub(crate) fn connect(sock: &str) -> std::io::Result<Self> {
        let stream = std::os::unix::net::UnixStream::connect(sock)?;
        stream.set_read_timeout(Some(MONITOR_ANSWER_WITHIN))?;
        stream.set_write_timeout(Some(MONITOR_ANSWER_WITHIN))?;
        let mut monitor = Self { stream };
        monitor.until_prompt()?;
        Ok(monitor)
    }

    /// Run one command and return what it printed, once QEMU has finished it.
    fn command(&mut self, line: &str) -> std::io::Result<String> {
        use std::io::Write;
        self.stream.write_all(format!("{line}\n").as_bytes())?;
        let reply = self.until_prompt()?;
        Ok(monitor_output(&reply))
    }

    /// Press and release `key` on the guest's keyboard.
    pub(crate) fn sendkey(&mut self, key: &str) -> std::io::Result<()> {
        self.command(&format!("sendkey {key}")).map(drop)
    }

    /// Dump the scanout into `out`. On `Ok` the file is whole. The old file is removed first, so a
    /// dump that failed can never leave the previous picture behind to be graded as this one.
    pub(crate) fn screendump(&mut self, out: &Path) -> std::io::Result<()> {
        let _ = std::fs::remove_file(out);
        let said = self.command(&format!("screendump {}", out.display()))?;
        if said.is_empty() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "QEMU refused the screendump: {said}"
            )))
        }
    }

    /// Read until the monitor's prompt. One command is in flight at a time, so nothing follows it.
    fn until_prompt(&mut self) -> std::io::Result<Vec<u8>> {
        use std::io::Read;
        let mut reply = Vec::new();
        let mut chunk = [0u8; 4096];
        while !reply.ends_with(MONITOR_PROMPT) {
            let n = self.stream.read(&mut chunk)?;
            if n == 0 {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
            reply.extend_from_slice(&chunk[..n]);
        }
        Ok(reply)
    }
}

/// What a command printed, from the bytes the monitor sent between our line and the next prompt.
///
/// The monitor's line editor echoes what we typed, redrawn once per character with terminal escapes,
/// and ends the echo with the first `\r\n`; everything after that, up to the prompt, is the
/// command's own output. `sendkey` and a successful `screendump` print nothing.
fn monitor_output(reply: &[u8]) -> String {
    let text = String::from_utf8_lossy(reply);
    let body = text
        .strip_suffix(std::str::from_utf8(MONITOR_PROMPT).unwrap_or_default())
        .unwrap_or(&text);
    body.split_once("\r\n")
        .map(|(_echo, output)| output.trim_end().to_string())
        .unwrap_or_default()
}

/// **The host's load average across one emulated leg**, so a red timing assertion says whether the
/// machine was busy (milestone 117 (the stranger test)). The guest knows how late it was, not who
/// shared its cores; this process does. It **suggests**, and says so, with min/mean/peak, since the
/// one-minute average decays over a leg. See notes/load-sensitive-assertions/host-load-line.md.
///
/// # BUGS
/// - Sampled only while a leg runs, so contention during the build before it is not seen.
/// - It is one `uptime` every five seconds; where that is missing or prints an unfamiliar shape,
///   the report says "unavailable" rather than guessing.
pub(crate) struct HostLoad {
    min: f64,
    max: f64,
    total: f64,
    samples: u32,
    last: std::time::Instant,
}

impl HostLoad {
    /// Every five seconds. The callers poll on a 100 ms cadence for the scanout referee, and one
    /// `fork`/`exec` per poll would be 3,000 of them over a five-minute leg to resolve a number
    /// that moves on a sixty-second decay.
    const EVERY: std::time::Duration = std::time::Duration::from_secs(5);

    /// Start sampling, taking the first reading now so a leg that fails in its first second still
    /// reports something.
    pub(crate) fn new() -> Self {
        let mut load = Self {
            min: f64::INFINITY,
            max: 0.0,
            total: 0.0,
            samples: 0,
            // Back-dated so the first `sample()` call fires rather than waiting out the interval.
            // `checked_sub` rather than `-`: `Instant` counts from boot on both our platforms, and
            // subtracting past zero panics on a machine that booted seconds ago, a real CI shape.
            last: std::time::Instant::now()
                .checked_sub(Self::EVERY)
                .unwrap_or_else(std::time::Instant::now),
        };
        load.sample();
        load
    }

    /// Take a reading if the interval has elapsed. Cheap enough to call from a 100 ms poll loop or
    /// from a line-at-a-time transcript reader, which is what the two legs do.
    pub(crate) fn sample(&mut self) {
        if self.last.elapsed() < Self::EVERY {
            return;
        }
        self.last = std::time::Instant::now();
        let Some(now) = one_minute_load_average() else {
            return;
        };
        self.min = self.min.min(now);
        self.max = self.max.max(now);
        self.total += now;
        self.samples += 1;
    }

    /// Say what the host was doing, but only when the leg went red. On a green leg this is noise,
    /// and a diagnostic that prints on every run is a diagnostic readers learn to skip.
    pub(crate) fn report_if_failed(&self, ok: bool, arch: &str) {
        if ok {
            return;
        }
        eprintln!();
        if self.samples == 0 {
            eprintln!(
                "host load ({arch}): unavailable (`uptime` did not answer in a shape this parses)"
            );
            return;
        }
        let mean = self.total / f64::from(self.samples);
        let cpus = std::thread::available_parallelism().map_or(0, |n| n.get());
        eprintln!(
            "host load ({arch}): 1-minute average {:.2} / {:.2} / {:.2} (min/mean/peak over {} \
             samples), on {} cores",
            self.min, mean, self.max, self.samples, cpus,
        );
        if cpus > 0 && self.max > cpus as f64 {
            eprintln!(
                "  {:.1}x oversubscribed at the peak. A timing assertion that failed above may be \
                 measuring this machine rather than this kernel; `script/icount` asserts the timer \
                 claims in instructions, which nothing the host does can move. See \
                 notes/load-sensitive-assertions.md.",
                self.max / cpus as f64,
            );
        } else {
            eprintln!(
                "  Not oversubscribed, so contention is the less likely explanation for a failure \
                 above. See notes/load-sensitive-assertions.md."
            );
        }
    }
}

/// The host's one-minute load average, from `uptime`.
///
/// `uptime` rather than `getloadavg(3)` because reaching the libc call means taking the `libc`
/// crate, and §46 makes a dependency a decision rather than a convenience: this is one number, read
/// once every five seconds, on a machine that is already running an emulator. `script/repeat-under-load`
/// parses the same command with the same trick, and this is deliberately the same parse in Rust:
/// macOS prints `load averages: 4.14 4.86 4.29` and Linux prints `load average: 0.50, 0.40, 0.30`,
/// so stripping commas first lets one scan serve both.
fn one_minute_load_average() -> Option<f64> {
    let out = Command::new("uptime").output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_load_average(std::str::from_utf8(&out.stdout).ok()?)
}

/// The pure half of [`one_minute_load_average`], split out so it can be tested on the host without
/// an `uptime` to run.
///
/// **The two formats are not both reachable from one machine**, which is what makes the test worth
/// having rather than filler: development is macOS and CI is `ubuntu-24.04-arm`, so a parse that
/// only understood the shape in front of its author would keep working here and quietly report
/// "unavailable" on every CI run, which is exactly the silence this whole feature exists to end.
fn parse_load_average(uptime_output: &str) -> Option<f64> {
    // Commas out first: Linux separates the three figures with them and macOS does not, so one
    // scan serves both once they are gone. This is `script/repeat-under-load`'s `load_now` in Rust.
    let text = uptime_output.replace(',', " ");
    let mut fields = text.split_whitespace();
    while let Some(f) = fields.next() {
        if f == "average:" || f == "averages:" {
            return fields.next()?.parse().ok();
        }
    }
    None
}

/// **Run the kernel test suite for `arch` and prove every scanout picture while it runs.**
/// `test_args` is the cargo invocation the caller would otherwise have handed to
/// [`crate::host::run`]; returns whether the suite and every host-side check passed.
///
/// Three pictures, in the order the suite runs them (tests sort by name, so `compositor_tests`
/// precede `display_tests`, and there `a_backing...` < `a_bitmap...` < `a_confined...`): rung two's
/// composed screen (milestone 33) and the display terminal's text (milestone 29's remaining
/// increment), each held a few seconds so the poll cannot miss it, then rung one's test pattern
/// (milestone 29), which stays until QEMU exits. Each is looked for only once the one before is
/// found, so a reordered suite or a missing picture fails loudly. Stdio passes through.
pub(crate) fn cargo_test_with_scanout_check(arch: &str, test_args: &[&str]) -> bool {
    scanout_check(arch, test_args, true)
}

/// **The same run without the inbound prober**, for a machine with no host-forwarded listener.
///
/// x86_64's `q35` has no virtio-mmio NIC, and the two programs that listen on the forwarded port
/// (milestone 107 (the socket contract learns to accept), milestone 64 (enough `std` to run
/// somebody else's crate)) reach it through that NIC, so `helpers/qemu-runner-x86_64.sh`
/// carries no `hostfwd`. A prober there would fail every run for want of a guest to connect to,
/// which would make its verdict noise. The scanout referee is wanted all the same: it grades the
/// three pictures and presses the key the keyboard test waits for.
pub(crate) fn cargo_test_with_scanout_only(arch: &str, test_args: &[&str]) -> bool {
    scanout_check(arch, test_args, false)
}

fn scanout_check(arch: &str, test_args: &[&str], inbound_wanted: bool) -> bool {
    let mut referee = ScanoutReferee::new(arch);
    // The other host-side actor (milestone 107): a process that connects INTO the guest, which is
    // the one thing no in-guest test can stage. Constructed before the child for the same reason
    // the referee is: it is what sets `NIFE_HOSTFWD_PORT`, and the runner reads it from the
    // environment the child inherits.
    let prober = inbound_wanted.then(|| InboundProber::new(arch));
    // Timed and recorded (milestone 807 (the kernel suite reports what each test cost)): stdout
    // passes through as it did when the child inherited it, and the transcript is kept for
    // `time_record::account` below.
    let (mut child, watch) =
        match crate::time_record::Watch::spawn(Command::new("cargo").args(test_args)) {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("failed to run cargo: {e}");
                return false;
            }
        };

    // **The suite's verdict is collected, not returned early**: the probers' reports matter most
    // on exactly the run where a guest-side assertion failed, and an early return would skip them.
    let mut child_ok = false;
    // Sampled here, in the loop that already exists, because the number worth having is the one
    // from *while the leg ran*: a suite takes minutes and the one-minute average has decayed by the
    // time the verdict is in. Reported only if something below goes red.
    let mut load = HostLoad::new();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                child_ok = status.success();
                break;
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("waiting for the test child failed: {e}");
                break;
            }
        }
        // The host watchdog (time_record's module note): a test past its bound ends the boot here,
        // and `child_ok` stays false.
        if watch.enforce(
            &mut child,
            &format!("{} system_tests", crate::time_record::leg(arch)),
        ) {
            break;
        }
        referee.poll();
        load.sample();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // All of them, and not short-circuited: a run that lost the scanout AND a network answer should
    // say so once rather than making the reader run it again to find the next failure.
    // **Under `--test` the host-side referees are advisory** (milestone 210). Each asserts
    // something a particular guest test does (pixels on the scanout, an inbound connection
    // accepted), so a filter that did not select that test fails them for a
    // reason that has nothing to do with what was run. They still RUN, because the referee is also
    // what presses keys over the monitor and the keyboard test needs that; only their verdict is
    // dropped. The guest's own verdict (`child_ok`) is never advisory.
    let filtered = std::env::var_os("NIFE_TEST_FILTER").is_some_and(|v| !v.is_empty());
    if filtered {
        eprintln!();
        eprintln!(
            "--- the host-side checks below are ADVISORY under --test: they assert what particular \
             guest tests write, and a filter may not have selected those ---"
        );
    }
    let scanout = referee.report();
    let inbound = prober.is_none_or(|prober| prober.report());
    let (lines, exited) = watch.finish();
    let image = test_args
        .iter()
        .skip_while(|a| **a != "-p")
        .nth(1)
        .copied()
        .unwrap_or("unknown");
    let accounted = crate::time_record::account(
        &crate::time_record::leg(arch),
        image,
        &lines,
        exited,
        child_ok,
    );
    let ok = child_ok && accounted && (filtered || (scanout && inbound));
    load.report_if_failed(ok, arch);
    ok
}

/// **The host-side referee for one booted suite**: presses a key through QEMU's monitor and watches
/// the device's own scanout for the three pictures the suite puts there.
///
/// It exists as a struct rather than a loop body because **two legs need the same referee driven
/// two different ways** (milestone 81). Under TCG the harness exits by itself, so the loop can be
/// "poll until the child is gone". Under HVF nothing exits (QEMU does not answer the semihosting
/// trap), so the verdict comes from reading the transcript, which blocks, and the referee has to be
/// driven from a second thread beside it. Same state machine, same messages, two drivers.
pub(crate) struct ScanoutReferee {
    /// The leg's name, which is the architecture except on the OVMF leg.
    arch: String,
    qemu_arch: String,
    sock: String,
    shot: PathBuf,
    composed_shot: PathBuf,
    text_shot: PathBuf,
    composed: Option<String>,
    text: Option<String>,
    matched: Option<String>,
    last_composed: Option<String>,
    last_text: Option<String>,
    last_reason: Option<String>,
    /// Held across polls rather than reopened for each command; see [`Monitor`] for why a
    /// connection per command lost dumps. `None` until QEMU is listening, and again after any
    /// error, so the next poll reconnects.
    monitor: Option<Monitor>,
    /// Dumps taken and read, in all, and graded against each picture in turn. A red report prints
    /// them, so it says how hard the referee looked rather than leaving that to a rerun.
    dumps: u32,
    graded: [u32; 3],
    /// The last thing that went wrong talking to the monitor, for a report with no dumps at all.
    monitor_error: Option<String>,
}

impl ScanoutReferee {
    /// Clear last run's evidence and tell the runner where to put the monitor socket.
    pub(crate) fn new(arch: &str) -> Self {
        Self::for_leg(arch, arch)
    }

    /// [`new`](Self::new) for a leg that is not the architecture's PVH or `virt` boot: `leg` names
    /// the socket, the evidence files and the report's lines, and `arch` still picks the QEMU whose
    /// version a red line quotes. The OVMF leg (milestone 420 (x86_64 has one fixture of five and
    /// no NIC, GPU, keyboard or RNG)) is `x86_64-ovmf`, so its dumps do not overwrite the PVH
    /// leg's.
    pub(crate) fn for_leg(arch: &str, leg: &str) -> Self {
        let sock = gpu_mon_socket(leg);
        let shot = gpu_shot_path(leg);
        let composed_shot = gpu_compose_path(leg);
        let text_shot = gpu_text_path(leg);
        let _ = std::fs::remove_file(&sock);
        let _ = std::fs::remove_file(&shot);
        let _ = std::fs::remove_file(&composed_shot);
        let _ = std::fs::remove_file(&text_shot);

        // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
        // threads. xtask is single-threaded here: this runs on the main thread before the child
        // that reads it is spawned, and the threads xtask ever starts (the transcript reader in
        // swish_check_leg, and this referee's driver in hvf_kernel_leg) copy pipe bytes and poll a
        // socket, and neither touches the environment.
        unsafe { std::env::set_var("NIFE_GPU_MON", &sock) };

        Self {
            arch: leg.to_string(),
            qemu_arch: arch.to_string(),
            sock,
            shot,
            composed_shot,
            text_shot,
            composed: None,
            text: None,
            matched: None,
            last_composed: None,
            last_text: None,
            last_reason: None,
            monitor: None,
            dumps: 0,
            graded: [0; 3],
            monitor_error: None,
        }
    }

    /// One pass: press a key, take a screendump, and see whether it is the picture we are waiting
    /// for. Call it on a cadence for as long as the suite is running.
    ///
    /// **Costs more per poll than it used to, and that is a considered, recorded choice rather than
    /// an oversight** (milestone 142 (a text display good enough that people use it instead of a
    /// GUI)). The scanout is 1120x624 (698,880 pixels), 85 times the 128x64 it grew from, moved
    /// through `screendump` on the same 100 ms cadence. calef chose to keep the cadence rather than
    /// widen it preemptively, since nothing was slow enough to measure a real problem against;
    /// revisit if a resolution increase (or a slower CI runner) makes this cadence cost something
    /// observable.
    pub(crate) fn poll(&mut self) {
        // Press a key every poll. Harmless before the keyboard driver exists (QEMU drops the event)
        // and harmless after its test has passed (the driver ends up parked in a `CALL` nobody
        // answers), so there is nothing to time.
        //
        // Both commands go over one connection and each waits for its answer, so the dump is never
        // refused for arriving while the key's connection was still in QEMU's backlog ([`Monitor`]).
        if self.monitor.is_none() {
            self.monitor = Monitor::connect(&self.sock).ok();
        }
        let Some(monitor) = self.monitor.as_mut() else {
            return;
        };
        let dumped = monitor
            .sendkey(video_terminal::script::HOST_KEY)
            .and_then(|()| match self.matched {
                None => monitor.screendump(&self.shot).map(|()| true),
                Some(_) => Ok(false),
            });
        match dumped {
            Ok(true) => {}
            Ok(false) => return,
            Err(e) => {
                self.monitor_error = Some(e.to_string());
                self.monitor = None;
                return;
            }
        }
        if let Ok(bytes) = std::fs::read(&self.shot) {
            self.dumps += 1;
            // Each picture is transient except the last (the next test on the same device replaces
            // it), so the dump that matched is copied aside: `shot` is overwritten on every poll.
            if self.composed.is_none() {
                self.graded[0] += 1;
                match scanout_holds_the_composed_screen(&bytes) {
                    Ok(()) => {
                        let _ = std::fs::write(&self.composed_shot, &bytes);
                        self.composed = Some(format!("{}", self.composed_shot.display()));
                    }
                    Err(reason) => self.last_composed = Some(reason),
                }
            } else if self.text.is_none() {
                self.graded[1] += 1;
                match scanout_holds_the_terminals_text(&bytes) {
                    Ok(()) => {
                        let _ = std::fs::write(&self.text_shot, &bytes);
                        self.text = Some(format!("{}", self.text_shot.display()));
                    }
                    Err(reason) => self.last_text = Some(reason),
                }
            } else {
                self.graded[2] += 1;
                match scanout_holds_the_pattern(&bytes) {
                    Ok(()) => self.matched = Some(format!("{}", self.shot.display())),
                    Err(reason) => self.last_reason = Some(reason),
                }
            }
        }
    }

    /// Say what reached the device's scanout and what did not, and return whether all three did.
    pub(crate) fn report(self) -> bool {
        let _ = std::fs::remove_file(&self.sock);
        let arch = &self.arch;
        // Which emulator produced a verdict here is part of reading it (milestone 414 (a red
        // post-run check does not say which emulator produced it)): these checks drive QEMU's
        // monitor, and versions fail disjoint sets of them. Computed once and carried by the
        // FAILED lines, so a red line interprets itself; the green lines stay as they were.
        let qemu = crate::host::qemu_version(&self.qemu_arch);
        let (composed, text, matched) = (&self.composed, &self.text, &self.matched);
        let seen = Seen {
            dumps: self.dumps,
            monitor_error: self.monitor_error.as_deref(),
        };
        let last_composed = seen.why_unmatched(None, self.graded[0], self.last_composed.as_deref());
        let last_text = seen.why_unmatched(
            Some("composed screen"),
            self.graded[1],
            self.last_text.as_deref(),
        );
        let last_reason = seen.why_unmatched(
            Some("terminal's text"),
            self.graded[2],
            self.last_reason.as_deref(),
        );

        let mut ok = true;
        match composed {
            Some(path) => eprintln!(
                "scanout check ({arch}): the compositor's {} windows reached the DEVICE's scanout, \
             verified pixel for pixel against compositor::expected_screen_pixel ({path})",
                compositor::SCENE.len(),
            ),
            None => {
                eprintln!();
                eprintln!(
                    "scanout check ({arch}) FAILED (QEMU {qemu}): the compositor test passed, so the \
                 guest's witnesses agree about the framebuffer, but QEMU's scanout never held the \
                 composed screen. Last mismatch: {last_composed}"
                );
                eprintln!(
                    "  A compositor's output is exactly what a guest-side digest cannot confirm; this is \
                 the check that can. See notes/compositor.md."
                );
                ok = false;
            }
        }
        match text {
            Some(path) => eprintln!(
                "scanout check ({arch}): the display terminal's text reached the DEVICE's scanout, \
             verified pixel for pixel against the vt engine run over video_terminal::script ({path})",
            ),
            None => {
                eprintln!();
                eprintln!(
                    "scanout check ({arch}) FAILED (QEMU {qemu}): the display-terminal test passed, \
                 so the guest agrees about the framebuffer, but QEMU's scanout never held the \
                 terminal's text. Last mismatch: {last_text}"
                );
                eprintln!(
                    "  A wrong pixel format makes a test pattern look odd and makes text unreadable, \
                 which is why this check exists for glyphs too. See notes/glyphs.md."
                );
                ok = false;
            }
        }
        match matched {
            Some(path) => eprintln!(
                "scanout check ({arch}): the {}x{} pattern reached the DEVICE's scanout, verified pixel \
             for pixel against graphics_contract::pixel ({path})",
                graphics_contract::WIDTH,
                graphics_contract::HEIGHT,
            ),
            None => {
                eprintln!();
                eprintln!(
                    "scanout check ({arch}) FAILED (QEMU {qemu}): the display test passed, so the \
                 framebuffer holds the pattern, but QEMU's scanout never did. Last mismatch: \
                 {last_reason}"
                );
                eprintln!(
                    "  This is the check that catches a wrong pixel format or scanout rectangle, which \
                 the in-guest test cannot see. See notes/framebuffer-contract.md."
                );
                ok = false;
            }
        }
        ok
    }
}

/// What the referee saw over a whole run, for saying why one picture never matched.
struct Seen<'a> {
    dumps: u32,
    monitor_error: Option<&'a str>,
}

impl Seen<'_> {
    /// The "Last mismatch" a red report prints for one picture.
    ///
    /// **The pictures are looked for in suite order**, so a picture graded zero times was never
    /// looked at: the referee was still waiting for the one before it (`earlier`). Before
    /// 2026-10-12 that case printed "no screendump was ever taken", which was false (the earlier
    /// picture had been graded against dumps all along) and sent the reader to the wrong question.
    fn why_unmatched(&self, earlier: Option<&str>, graded: u32, last: Option<&str>) -> String {
        if self.dumps == 0 {
            let said = self.monitor_error.map_or(String::new(), |e| {
                format!("; the monitor's last error: {e}")
            });
            return format!("no screendump was ever taken (did QEMU get a monitor?){said}");
        }
        match (graded, last, earlier) {
            (0, _, Some(earlier)) => format!(
                "never looked for: the referee looks for the pictures in suite order and was still                  waiting for the {earlier} ({} dumps taken in all)",
                self.dumps
            ),
            (n, Some(last), _) => format!("{last} (the last of {n} dumps graded against it)"),
            (n, None, _) => format!("{n} dumps graded against it, and no mismatch recorded"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Both operating systems' `uptime`, because only one of them is ever in front of you.**
    /// Development happens on macOS and CI runs on `ubuntu-24.04-arm`, so the format not under the
    /// author's nose is the one that breaks silently: a failed parse reports "unavailable" rather
    /// than erroring, which is right at run time and useless as a signal.
    ///
    /// The real strings, copied from each system rather than reconstructed.
    #[test]
    fn the_load_average_parse_serves_macos_and_linux() {
        let macos = "11:07  up 5 days, 22:33, 3 users, load averages: 4.14 4.86 4.29";
        let linux = " 18:02:11 up 12 days,  3:41,  2 users,  load average: 0.50, 0.40, 0.30";
        assert_eq!(parse_load_average(macos), Some(4.14));
        assert_eq!(parse_load_average(linux), Some(0.50));
        // A shape nothing here recognises is `None`, not a wrong number: the report says so out
        // loud, and a made-up load average would be worse than no line at all.
        assert_eq!(parse_load_average("up 3 days"), None);
        assert_eq!(parse_load_average("load average:"), None);
        assert_eq!(parse_load_average("load average: n/a"), None);
    }

    /// **The monitor's answer is read past its echo.** QEMU 11.1.2's own bytes for `sendkey a`
    /// (copied from a socket, not reconstructed): the line editor redraws the line once per
    /// character typed, then `\r\n`, then the output, then the prompt. A parse that took the
    /// echo for output would call every command a refusal.
    #[test]
    fn the_monitor_reply_is_read_past_the_line_editors_echo() {
        let sendkey =
            b"s\x1b[K\x1b[Dse\x1b[K\x1b[D\x1b[Dsen\x1b[K\x1b[D\x1b[D\x1b[Dsend\x1b[K\x1b[D\
\x1b[D\x1b[D\x1b[Dsendk\x1b[K\x1b[D\x1b[D\x1b[D\x1b[D\x1b[Dsendke\x1b[K\x1b[D\x1b[D\x1b[D\x1b[D\
\x1b[D\x1b[Dsendkey\x1b[K\x1b[D\x1b[D\x1b[D\x1b[D\x1b[D\x1b[D\x1b[Dsendkey \x1b[K\x1b[D\x1b[D\
\x1b[D\x1b[D\x1b[D\x1b[D\x1b[D\x1b[Dsendkey a\x1b[K\r\n(qemu) ";
        assert_eq!(monitor_output(sendkey), "");
        // The tail of a refused dump, from the same QEMU.
        let refused = b"\x1b[Dscreendump /nonexistent/dir/x.ppm\x1b[K\r\nError: Could not create \
'/nonexistent/dir/x.ppm': No such file or directory\r\n(qemu) ";
        assert_eq!(
            monitor_output(refused),
            "Error: Could not create '/nonexistent/dir/x.ppm': No such file or directory"
        );
    }

    /// **A dump is not reported done until the monitor says so.** A stand-in monitor takes 300 ms
    /// to write the file, twice the fixed sleep the referee used to trust, and only then prints its
    /// prompt. `Monitor::screendump` must return with the file whole. Falsified: the 2026-10-12
    /// `screendump` (write the line, sleep 150 ms, return) returns before the file exists, and this
    /// fails on the existence assertion.
    #[test]
    fn a_screendump_returns_only_once_the_file_is_written() {
        use std::io::{BufRead, Write};
        let dir = std::env::temp_dir().join(format!("nife-monitor-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let sock = dir.join("mon.sock");
        let out = dir.join("shot.ppm");
        let _ = std::fs::remove_file(&sock);
        let listener = std::os::unix::net::UnixListener::bind(&sock).expect("bind");
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut w = stream.try_clone().expect("clone");
            w.write_all(b"QEMU stand-in monitor\r\n(qemu) ")
                .expect("greet");
            for line in std::io::BufReader::new(stream).lines() {
                let line = line.expect("line");
                if let Some(path) = line.strip_prefix("screendump ") {
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    std::fs::write(path, b"P6\n1 1\n255\n\0\0\0").expect("write");
                }
                w.write_all(format!("{line}\r\n(qemu) ").as_bytes())
                    .expect("answer");
            }
        });
        let mut monitor = Monitor::connect(sock.to_str().expect("utf-8")).expect("connect");
        monitor.sendkey("a").expect("sendkey");
        monitor.screendump(&out).expect("screendump");
        assert_eq!(
            std::fs::read(&out).ok().as_deref(),
            Some(&b"P6\n1 1\n255\n\0\0\0"[..]),
            "screendump returned before the monitor had written the file"
        );
        drop(monitor);
        server.join().expect("server");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A red report says how far the referee got.** The case that misled two lanes: the composed
    /// screen matched, the text never did, so the pattern was never looked for, and the report
    /// said "no screendump was ever taken".
    #[test]
    fn an_unmatched_picture_says_whether_it_was_ever_looked_for() {
        let none = Seen {
            dumps: 0,
            monitor_error: Some("Connection refused (os error 61)"),
        };
        let r = none.why_unmatched(None, 0, None);
        assert!(r.starts_with("no screendump was ever taken"), "{r}");
        assert!(r.contains("Connection refused"), "{r}");

        let some = Seen {
            dumps: 40,
            monitor_error: None,
        };
        let pattern = some.why_unmatched(Some("terminal's text"), 0, None);
        assert!(pattern.starts_with("never looked for"), "{pattern}");
        assert!(
            pattern.contains("terminal's text") && pattern.contains("40 dumps"),
            "{pattern}"
        );
        let text = some.why_unmatched(Some("composed screen"), 12, Some("pixel (0, 0) is wrong"));
        assert_eq!(
            text,
            "pixel (0, 0) is wrong (the last of 12 dumps graded against it)"
        );
    }

    /// Build a P6 PPM of the surface's geometry from a per-pixel function, the way QEMU's
    /// `screendump` writes one.
    fn ppm(pixel: impl Fn(u32, u32) -> (u8, u8, u8)) -> Vec<u8> {
        let (w, h) = (graphics_contract::WIDTH, graphics_contract::HEIGHT);
        let mut v = format!("P6\n{w} {h}\n255\n").into_bytes();
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = pixel(x, y);
                v.extend_from_slice(&[r, g, b]);
            }
        }
        v
    }

    fn pattern_rgb(x: u32, y: u32) -> (u8, u8, u8) {
        let w = graphics_contract::pixel(x, y);
        (
            ((w >> 16) & 0xff) as u8,
            ((w >> 8) & 0xff) as u8,
            (w & 0xff) as u8,
        )
    }

    /// **The scanout check accepts the pattern and rejects everything else.**
    ///
    /// This is the negative control for the milestone-29 scanout proof, and it matters: a checker that
    /// accepted anything would report "the pixels reached the device" on every run, which is exactly
    /// the kind of test that is worse than none. Each rejection below is a real failure mode of a
    /// framebuffer driver: a scanout never set (the default console size), a resource that was never
    /// transferred into (black), a channel order mixed up (the single most common framebuffer bug),
    /// and one wrong pixel.
    #[test]
    fn the_scanout_check_accepts_the_pattern_and_rejects_near_misses() {
        assert!(scanout_holds_the_pattern(&ppm(pattern_rgb)).is_ok());

        assert!(
            scanout_holds_the_pattern(&ppm(|_, _| (0, 0, 0))).is_err(),
            "a black scanout was accepted",
        );

        // Red and blue swapped: what a wrong virtio-gpu format code produces, and precisely what the
        // in-guest test cannot see (the guest's own bytes are unchanged).
        assert!(
            scanout_holds_the_pattern(&ppm(|x, y| {
                let (r, g, b) = pattern_rgb(x, y);
                (b, g, r)
            }))
            .is_err(),
            "a red/blue-swapped scanout was accepted: the format check is not doing anything",
        );

        // Shifted one row: a stride bug.
        assert!(
            scanout_holds_the_pattern(&ppm(|x, y| pattern_rgb(
                x,
                (y + 1) % graphics_contract::HEIGHT
            )))
            .is_err(),
            "a scanout shifted by one row was accepted",
        );

        // Exactly one wrong pixel, in the middle.
        assert!(
            scanout_holds_the_pattern(&ppm(|x, y| {
                if (x, y) == (64, 32) {
                    (1, 2, 3)
                } else {
                    pattern_rgb(x, y)
                }
            }))
            .is_err(),
            "a scanout with one wrong pixel was accepted",
        );

        // QEMU's default console size, i.e. a scanout that was never set.
        let mut wrong_geometry = b"P6\n640 480\n255\n".to_vec();
        wrong_geometry.extend(std::iter::repeat_n(0u8, 640 * 480 * 3));
        assert!(
            scanout_holds_the_pattern(&wrong_geometry).is_err(),
            "the default 640x480 console was accepted as our 128x64 surface",
        );

        // A dump caught mid-write is not a failure, but it must not be a pass either.
        let short = &ppm(pattern_rgb)[..1000];
        assert!(scanout_holds_the_pattern(short).is_err());
    }

    fn composed_rgb(x: u32, y: u32) -> (u8, u8, u8) {
        let w = compositor::expected_screen_pixel(compositor::SCENE.len(), x, y);
        (
            ((w >> 16) & 0xff) as u8,
            ((w >> 8) & 0xff) as u8,
            (w & 0xff) as u8,
        )
    }

    /// **The composed-screen check accepts the compositor's screen and rejects the ways a compositor
    /// goes wrong** (milestone 33).
    ///
    /// The negative control for the rung-two half of the scanout proof, and the failure modes are
    /// different from rung one's, which is why it needs its own. In particular a **z-order inversion**
    /// and a **missing window** are both pictures made entirely of correct pixels in almost the right
    /// places: exactly the sort of thing a checker written as "is it not black?" would wave through,
    /// and exactly what a compositor gets wrong.
    #[test]
    fn the_composed_check_accepts_the_screen_and_rejects_the_compositors_own_bugs() {
        assert!(scanout_holds_the_composed_screen(&ppm(composed_rgb)).is_ok());

        assert!(
            scanout_holds_the_composed_screen(&ppm(|_, _| (0, 0, 0))).is_err(),
            "a black screen was accepted",
        );

        // Rung one's pattern is not rung two's screen. Both are 128x64 and both are legitimate
        // pictures, so this pins that the two checks cannot be satisfied by the same dump: if they
        // could, the ordering the poll loop relies on would be meaningless.
        assert!(
            scanout_holds_the_composed_screen(&ppm(pattern_rgb)).is_err(),
            "rung one's test pattern was accepted as the composed screen",
        );
        assert!(
            scanout_holds_the_pattern(&ppm(composed_rgb)).is_err(),
            "the composed screen was accepted as rung one's test pattern",
        );

        // **Stacking order inverted**: the bottom-most window covering a pixel wins instead of the top.
        // Every pixel is a real window pixel; only the order is wrong.
        assert!(
            scanout_holds_the_composed_screen(&ppm(|x, y| {
                for (i, win) in compositor::SCENE.iter().enumerate() {
                    if win.rect().contains(x as i32, y as i32) {
                        let w = compositor::window_pixel(
                            i as u32,
                            (x as i32 - win.origin_x) as u32,
                            (y as i32 - win.origin_y) as u32,
                        );
                        return (
                            ((w >> 16) & 0xff) as u8,
                            ((w >> 8) & 0xff) as u8,
                            (w & 0xff) as u8,
                        );
                    }
                }
                composed_rgb(x, y)
            }))
            .is_err(),
            "a screen with the windows stacked in the wrong order was accepted",
        );

        // **One window missing**: the picture as if the last client never committed. This is what a
        // compositor that dropped a commit, or never mapped a surface, produces.
        assert!(
            scanout_holds_the_composed_screen(&ppm(|x, y| {
                let w = compositor::expected_screen_pixel(compositor::SCENE.len() - 1, x, y);
                (
                    ((w >> 16) & 0xff) as u8,
                    ((w >> 8) & 0xff) as u8,
                    (w & 0xff) as u8,
                )
            }))
            .is_err(),
            "a screen missing its top window was accepted",
        );

        // **Windows placed one pixel off**: the classic clipping error, and the reason the crate's
        // rectangle math is host-tested.
        assert!(
            scanout_holds_the_composed_screen(&ppm(|x, y| composed_rgb(
                (x + 1) % compositor::SCREEN_W,
                y
            )))
            .is_err(),
            "a screen shifted one pixel left was accepted",
        );

        // Red and blue swapped, the format bug the guest cannot see.
        assert!(
            scanout_holds_the_composed_screen(&ppm(|x, y| {
                let (r, g, b) = composed_rgb(x, y);
                (b, g, r)
            }))
            .is_err(),
            "a red/blue-swapped composed screen was accepted",
        );
    }

    fn text_rgb(x: u32, y: u32) -> (u8, u8, u8) {
        // Built once and cached rather than once per pixel: `ppm` below calls this per pixel of a
        // 1120x624 image (698,880 times), and reconstructing and re-feeding a `Vt` that many times
        // would dominate this test's runtime the moment the grid grew past a few dozen cells.
        static SCREEN: std::sync::OnceLock<video_terminal::Vt> = std::sync::OnceLock::new();
        let screen = SCREEN.get_or_init(|| {
            let mut vt =
                video_terminal::Vt::new(video_terminal::script::COLS, video_terminal::script::ROWS);
            video_terminal::script::scrolled_full_screen(&mut vt, None);
            vt
        });
        let w = screen.pixel(x, y);
        (
            ((w >> 16) & 0xff) as u8,
            ((w >> 8) & 0xff) as u8,
            (w & 0xff) as u8,
        )
    }

    /// **The text check accepts the terminal's screen and rejects text that is wrong** (milestone
    /// 29's remaining increment).
    ///
    /// The negative control that makes the glyph proof mean anything, and its failure modes are the
    /// *terminal's* rather than the compositor's or the driver's. The one that matters most is the
    /// first: **one letter changed**. Everything else on that screen is identical, every glyph is a
    /// real glyph, the layout is right, and the picture is wrong. A checker that could not tell the
    /// difference would report "readable text reached the scanout" for a terminal that drew the wrong
    /// text, which is the failure this whole increment is about not having.
    #[test]
    fn the_scanout_check_rejects_text_that_is_one_letter_wrong() {
        assert!(scanout_holds_the_terminals_text(&ppm(text_rgb)).is_ok());

        // One letter. `glyphs_ok` against `glyphs_0k`: an `o` for a zero, which is the closest pair
        // of glyphs in the font and therefore the hardest case, deliberately.
        let mut typo =
            video_terminal::Vt::new(video_terminal::script::COLS, video_terminal::script::ROWS);
        video_terminal::script::scrolled_full_screen(
            &mut typo,
            Some(video_terminal::script::SCROLL_LINES + 1),
        );
        assert!(
            scanout_holds_the_terminals_text(&ppm(|x, y| {
                let w = typo.pixel(x, y);
                (
                    ((w >> 16) & 0xff) as u8,
                    ((w >> 8) & 0xff) as u8,
                    (w & 0xff) as u8,
                )
            }))
            .is_err(),
            "a screen with one letter wrong was accepted as the terminal's text",
        );

        // **The typing never arrived.** A terminal that rendered an application's output but dropped
        // the keystrokes routed to it draws a picture that is correct as far as it goes.
        let mut no_input =
            video_terminal::Vt::new(video_terminal::script::COLS, video_terminal::script::ROWS);
        no_input.feed(video_terminal::script::GREETING);
        assert!(
            scanout_holds_the_terminals_text(&ppm(|x, y| {
                let w = no_input.pixel(x, y);
                (
                    ((w >> 16) & 0xff) as u8,
                    ((w >> 8) & 0xff) as u8,
                    (w & 0xff) as u8,
                )
            }))
            .is_err(),
            "a screen missing the typed input was accepted",
        );

        // **The rendition ignored.** Every glyph in the right cell, drawn in the default colours: a
        // terminal that parsed SGR as an unknown sequence and swallowed it. The picture is *nearly*
        // right, which is the point.
        let mut plain =
            video_terminal::Vt::new(video_terminal::script::COLS, video_terminal::script::ROWS);
        for &b in video_terminal::script::GREETING
            .iter()
            .chain(video_terminal::script::TYPED)
        {
            // Strip the escape sequences by feeding only what a colour-blind terminal would keep.
            if b != 0x1b {
                plain.feed(&[b]);
            }
        }
        assert!(
            scanout_holds_the_terminals_text(&ppm(|x, y| {
                let w = plain.pixel(x, y);
                (
                    ((w >> 16) & 0xff) as u8,
                    ((w >> 8) & 0xff) as u8,
                    (w & 0xff) as u8,
                )
            }))
            .is_err(),
            "a screen that ignored every rendition was accepted",
        );

        // A blank terminal, which is what a component that came up and drew nothing leaves.
        let blank =
            video_terminal::Vt::new(video_terminal::script::COLS, video_terminal::script::ROWS);
        assert!(
            scanout_holds_the_terminals_text(&ppm(|x, y| {
                let w = blank.pixel(x, y);
                (
                    ((w >> 16) & 0xff) as u8,
                    ((w >> 8) & 0xff) as u8,
                    (w & 0xff) as u8,
                )
            }))
            .is_err(),
            "a blank terminal was accepted as text",
        );

        // And the three pictures on this one scanout are mutually exclusive, which is what makes the
        // poll loop's ordering a real assertion rather than three chances to match once.
        assert!(scanout_holds_the_terminals_text(&ppm(pattern_rgb)).is_err());
        assert!(scanout_holds_the_terminals_text(&ppm(composed_rgb)).is_err());
        assert!(scanout_holds_the_pattern(&ppm(text_rgb)).is_err());
        assert!(scanout_holds_the_composed_screen(&ppm(text_rgb)).is_err());
    }
}
