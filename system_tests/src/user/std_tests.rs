use super::*;

/// Reassemble a std program's stdout off its endpoint until the writer says the stream is over,
/// and compare byte for byte.
///
/// The framing is the sink contract's (`crates/byte_sink_contract`, milestone 50 (pipes and
/// redirection: one sink protocol, and `|` turns out to be an endpoint)): `w0` is the count, `w1`
/// and `w2` the bytes, little-endian. `SEND` blocks until a receiver takes it, so the program is
/// between its last `println!` and `SYS_EXIT` when the bytes land. Reading to `OPERATION_EOF`
/// rather than to `want.len()` proves the program printed exactly this and then finished. Every
/// std test on every ISA asserts through here, so a drift is a diff in one place.
pub(super) fn assert_std_transcript(report: crate::sched::RendezvousId, want: &[u8], what: &str) {
    let mut got = [0u8; 768];
    let len = drain_sink(report, &mut got, what);
    assert_eq!(
        &got[..len],
        want,
        "{what}: stdout did not match the transcript"
    );
}

/// [`assert_std_transcript`] through [`drain_sink_from`]: the same comparison, and a program that
/// dies before ending its stream fails the test at once with what it wrote, instead of hanging it.
///
/// Name: provisional, milestone 514's lane, 2026-10-12 (UTC). `_from` for the writer it watches,
/// beside `drain_sink_from`; `_watching` was considered and is longer for the same claim.
pub(super) fn assert_std_transcript_from(
    report: crate::sched::RendezvousId,
    writer: crate::thread::ThreadId,
    want: &[u8],
    what: &str,
    echo: bool,
) {
    let mut got = [0u8; 768];
    let len = drain_sink_from(report, writer, &mut got, what, echo);
    assert_eq!(
        &got[..len],
        want,
        "{what}: stdout did not match the transcript"
    );
}

/// Reassemble a sink-contract stream into `out` until end of stream; returns its length.
///
/// The one decoder for every sink in the suite, so the indifference test's arms share it.
///
/// # BUGS
///
/// A program that aborts is never heard: this ends only on the end-of-stream marker `cleanup`
/// sends, so a `panic = "abort"` trap leaves the caller blocked until the watchdog, its panic
/// message unread. [`drain_sink_from`] covers callers that know the writer's thread; see
/// notes/std/the-proof.md, "How the tests read a std program's stream".
pub(super) fn drain_sink(ep: crate::sched::RendezvousId, out: &mut [u8], what: &str) -> usize {
    let mut len = 0usize;
    loop {
        let words = crate::sched::ipc_receive(ep);
        let mut chunk = [0u8; byte_sink_contract::INLINE_MAX];
        match byte_sink_contract::unpack(words[0], words[1], words[2], &mut chunk) {
            byte_sink_contract::Msg::Bytes(n) => {
                for &b in &chunk[..n] {
                    assert!(len < out.len(), "{what}: wrote more than the buffer holds");
                    out[len] = b;
                    len += 1;
                }
            }
            byte_sink_contract::Msg::Eof => return len,
            byte_sink_contract::Msg::Malformed => {
                panic!(
                    "{what}: a sink message the contract does not define: {:#x}",
                    words[0]
                )
            }
        }
    }
}

/// How often [`drain_sink_from`] looks to see whether its writer is still alive. A tenth of a
/// second is "at once" next to the 60 s watchdog it replaces, and costs one timer expiry per tick
/// while a stream is open.
///
/// Name: provisional, milestone 514's lane, 2026-10-12 (UTC). Named for what is checked and how
/// often; `LIVENESS_PERIOD_MS` was considered.
const WRITER_CHECK_MS: u64 = 100;

/// [`drain_sink`] for a caller that knows the writing thread: if it dies before ending the stream,
/// fail at once and print everything it wrote (milestone 514).
///
/// A timer on a bound notification ends the `RECEIVE` every [`WRITER_CHECK_MS`] (§101
/// (notification objects)) to ask whether the writer still resolves. `echo` prints lines as they
/// arrive, for the host relay (`xtask/src/inbound.rs`). See notes/std/the-proof.md.
///
/// # BUGS
/// - The tick is progress to the lost-wakeup watchdog, so a stuck writer reports as a livelock.
///
/// Name: provisional, milestone 514's lane, 2026-10-12 (UTC). `drain_sink` plus the thread it
/// drains from; `drain_sink_watching` and `drain_sink_or_death` were considered.
pub(super) fn drain_sink_from(
    ep: crate::sched::RendezvousId,
    writer: crate::thread::ThreadId,
    out: &mut [u8],
    what: &str,
    echo: bool,
) -> usize {
    use crate::arch::timer::{frequency, now};
    let region = crate::memory_region::create(2).expect("no region for a writer watch");
    let bell =
        crate::sched::create_notification_from(region).expect("no notification for a writer watch");
    let tick = crate::sched::create_timer_from(region).expect("no timer for a writer watch");
    crate::sched::notification_bind(bell, crate::sched::current())
        .expect("this test thread already has a notification bound to it");
    let period = frequency() * WRITER_CHECK_MS / 1000;
    let arm = || {
        crate::sched::timer_arm(tick, now() + period, bell, 1)
            .expect("could not arm the writer watch");
    };
    arm();

    let mut len = 0usize;
    let mut line = 0usize;
    let ended = loop {
        let words = crate::sched::ipc_receive(ep);
        if words[4] == abi::notification::BOUND {
            if !crate::sched::is_thread_present(writer) {
                break false;
            }
            arm();
            continue;
        }
        let mut chunk = [0u8; byte_sink_contract::INLINE_MAX];
        match byte_sink_contract::unpack(words[0], words[1], words[2], &mut chunk) {
            byte_sink_contract::Msg::Bytes(n) => {
                for &b in &chunk[..n] {
                    assert!(len < out.len(), "{what}: wrote more than the buffer holds");
                    out[len] = b;
                    len += 1;
                    if echo && b == b'\n' {
                        crate::println!("    {what}| {}", printable(&out[line..len - 1]));
                        line = len;
                    }
                }
            }
            byte_sink_contract::Msg::Eof => break true,
            byte_sink_contract::Msg::Malformed => {
                panic!(
                    "{what}: a sink message the contract does not define: {:#x}",
                    words[0]
                )
            }
        }
    };

    // Unbinding is the region going: a bound name that no longer resolves counts as unbound, so the
    // next test may bind this thread again.
    let _ = crate::sched::timer_cancel(tick);
    assert!(
        super::wait_for(|| crate::sched::reclaim_region(region).is_ok()),
        "{what}: the writer watch's region could not be reclaimed"
    );
    assert!(
        ended,
        "{what}: the program writing this stream (thread {writer}) died before ending it. \
         Everything it wrote follows, and if it panicked, its panic message is the end of it:\n{}",
        printable(&out[..len])
    );
    len
}

/// Bytes as text for a failure message: the valid UTF-8 prefix, which for a std program's output
/// is all of it.
///
/// Name: provisional, milestone 514's lane, 2026-10-12 (UTC). `lossy` was considered and refused,
/// since nothing is replaced.
fn printable(bytes: &[u8]) -> &str {
    match core::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => core::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or(""),
    }
}

/// Kill the filesystem mid-transaction on a real device, then mount what is left of it (milestone
/// 37, DECISIONS §34 condition 1), asserted identically on each ISA (rule 5, §19).
///
/// A block server brings up the crash test's own disk and an FS server mounts it. The driver writes
/// payload A, reads it back and reports it acknowledged; its second write walks into the injector,
/// which tears a block at 2048 bytes and traps with the commit unwritten (the `CUT` word says the
/// kill was the injector's). A different FS-server process mounts the disk, its readiness sentinel
/// being the consistency result, and a fresh client reads which payload is there. Either payload
/// passes; a mixture, a length nobody wrote or the pre-boot contents fail. See
/// notes/fs-server/crash-consistency.md.
pub(super) fn assert_a_kill_mid_transaction_recovers(
    blk_image: &'static [u8],
    redoxfs_server_image: &'static [u8],
    client_image: &'static [u8],
) {
    use filesystem_contract::READY;
    use filesystem_fixture::{SUCCESS, crash};
    // The caller has already established the disk is there (`fs_service::is_crash_disk_present`)
    // and skipped its own `#[test_case]` if it is not. This `expect` is that guard restated where
    // it can fail loudly: a `None` here means the disk went away between the two calls, which is a
    // machine fault rather than an absent fixture, and it must not be reported as a skip.
    let run = fs_service::start_crash(blk_image, redoxfs_server_image, client_image)
        .expect("the crash disk was present a moment ago and the block server still refused it");
    assert_eq!(
        crate::sched::ipc_receive(run.blk_ready)[0],
        READY,
        "the block server did not bring the crash-test disk up",
    );
    assert_eq!(
        crate::sched::ipc_receive(run.fs_ready)[0],
        READY,
        "the FS server did not open the crash-test image, so there was nothing to crash",
    );

    let [w0, w1, ..] = crate::sched::ipc_receive(run.driver_report);
    assert_eq!(
        (w0, w1),
        (SUCCESS, crash::SAW_A),
        "the driver's first write was not acknowledged and read back, so nothing that follows \
         is a statement about an acknowledged write",
    );

    assert_eq!(
        crate::sched::ipc_receive(run.fs_ready)[0],
        crash::CUT,
        "the FS server did not die inside the injector: whatever killed it, it was not this \
         test, and the recovery below would be measuring the wrong thing",
    );

    let (ready, report) = fs_service::recover_crash(redoxfs_server_image, client_image);
    assert_eq!(
        crate::sched::ipc_receive(ready)[0],
        READY,
        "the recovery mount FAILED: a fresh FS server could not open the image the killed one \
         left behind, so a torn write mid-transaction cost the whole filesystem",
    );

    let [len, saw, ..] = crate::sched::ipc_receive(report);
    assert!(
        saw == crash::SAW_A || saw == crash::SAW_B,
        "after a kill mid-transaction the file held {len} bytes that were neither payload \
         whole: a write is either wholly present or wholly absent, and this was neither",
    );
    crate::println!(
        "    (crash recovery: the file holds payload {}, {len} bytes, whole)",
        if saw == crash::SAW_A { "A" } else { "B" },
    );
}

/// **The extended-attribute witness's verdict** (milestone 57), asserted as an exact set so that
/// a client which could do nothing and one which could do everything both fail.
///
/// One copy, here beside the other shared FS assertions, because the two ISA legs assert the
/// same thing and the useful part is naming *which* claim broke rather than printing a number
/// the reader has to decode. Each missing bit is one sentence about the layer.
pub(super) fn assert_attrs(attrs: u64) {
    use filesystem_fixture::attrs as a;
    if attrs == a::EXPECTED {
        return;
    }
    for (bit, what) in [
        (
            a::SET_AND_READ_BACK,
            "an attribute set and read back with its type code",
        ),
        (a::LISTED, "the listing named it and nothing else"),
        (a::SURVIVED_RENAME, "it followed its file across a rename"),
        (a::GONE_AFTER_REMOVE, "removing it made it ENODATA"),
        (
            a::GONE_AFTER_UNLINK,
            "a file remade at the same name inherited nothing",
        ),
        (
            a::OVERSIZE_REFUSED,
            "an over-long value was refused with E2BIG",
        ),
        (
            a::STORE_UNNAMEABLE,
            "the attribute store could not be named",
        ),
        (a::STORE_UNLISTED, "and did not appear in an enumeration"),
    ] {
        if attrs & bit == 0 {
            crate::println!("    MISSING: {what}");
        }
    }
    if attrs & a::ATTRS_FAILED != 0 {
        crate::println!("    the witness reported that something it should do failed");
    }
    panic!(
        "the extended-attribute witness reported {attrs:#x}, expected {:#x}",
        a::EXPECTED,
    );
}

/// Consume the FS service's two readiness sentinels, if this caller is the one that wired it.
///
/// One boot has one FS service (the block server owns the device), so the hand-written client's
/// test and the `std::fs` test share it, and only the first of them to run gets the sentinels.
/// Asserting on them where they exist is what separates a hang in the mount from one in the
/// serve path.
pub(super) fn assert_fs_service_ready(readiness: fs_service::Readiness) {
    // One copy, in `fs_service`, because draining these is **sequencing** and not only an
    // assertion: each server is parked inside its own blocking announcement until somebody
    // receives it, so nothing it serves can be answered first. The caretakers depend on that.
    fs_service::wait_for_service(readiness);
}

/// Build the exact bytes `std_exerciser` prints when it is granted a directory capability, into
/// `buf`; returns the length. Not a `const` because the motd's contents are spliced in from the
/// shared fixture, and that is the load-bearing part: those bytes came off the RedoxFS image,
/// through the FS server, through `std::fs`, and out the stdout endpoint.
pub(super) fn std_fs_expected(buf: &mut [u8; 768]) -> usize {
    // The lengths spelled out below are the motd's; if the fixture changes, fail here rather
    // than in a byte comparison nobody can read.
    assert_eq!(
        filesystem_fixture::MOTD.len(),
        70,
        "the motd fixture changed; the expected transcript's lengths must change with it",
    );
    let mut n = 0;
    for part in [
        b"std fs on nife\n".as_slice(),
        filesystem_fixture::MOTD,
        b"read_to_string 70\nmetadata len 70\n".as_slice(),
        // `/motd` and `motd` open the same file and `current_dir` names their root. The refusals
        // are `/../motd`, the level above the only root, and `sub/../other/secret`, a file this
        // process reaches as `other/secret` but not through an ascent.
        b"absolute is my root\n".as_slice(),
        b"absolute dotdot refused\ndotdot refused\ninner dotdot refused\n".as_slice(),
        b"missing not found\n".as_slice(),
        // Milestone 31 phase 2: `create unsupported` became `write create ok`, plus the two
        // refusals that prove CREATE did not widen what a client can reach.
        b"write create ok\ncreate_new refused\n".as_slice(),
        b"create refused absolute\ncreate refused dotdot\n".as_slice(),
        b"write readback ok\n".as_slice(),
        // Milestone 64: the namespace verbs. Every one of these was `Unsupported` in the PAL
        // while the FS server had been dispatching the verb behind it since milestones 47 and 48,
        // so these nine lines are a binding proven rather than a contract widened.
        b"mkdir ok\nread_dir ok\nread_dir descend ok\n".as_slice(),
        b"unlink refused a directory\nrmdir refused a file\n".as_slice(),
        b"rename ok\nunlink ok\nis_dir ok\nrmdir ok\n".as_slice(),
        // Milestone 64's second pass: `File::set_len` (`TRUNCATE` with a size, both directions) and
        // `fs::copy` (no verb, and none needed). Same shape of finding as the nine lines above, one
        // milestone later: the refusal outlived the reason for it.
        b"set_len ok\ncopy ok\n".as_slice(),
        // Milestone 64's last pass: file times, on the verbs milestone 47's `touch` added
        // (DECISIONS §112). The first line is the motd's host-stamped mtime landing in the
        // wall-clock window; the next three are a write moving a made file's mtime forward, a
        // set-by-name round trip in whole seconds, and a directory's mtime read both by name and
        // through a listing entry. The
        // last two are refusals: through an open handle (the contract asks by name), and an access
        // time, the granted directory itself and an empty set on a missing name.
        b"mtime from the image ok\nwrite moves mtime ok\nset_times ok\ndir mtime ok\n".as_slice(),
        b"handle mtime refused\nset_times refusals ok\n".as_slice(),
        // **Milestone 122: descent.** The first three lines are a nested path resolving, a second
        // descent below it, and the pair that was actually broken: list a subdirectory, then open
        // every file the listing named through the `path()` the listing handed back. That pair is
        // what every filesystem walker is built out of, and one level down it used to fail.
        b"descend read ok\ndescend twice ok\nwalk entry ok\n".as_slice(),
        // A path may only walk *through* directories, and a held directory is bound exactly as the
        // granted one is. `dir handle ok` is `std::fs::Dir`, std's own `openat`-shaped API, running
        // on the capability it already is: nife used to get std's generic fallback, which stores a
        // `canonicalize`d path and therefore failed at its first call.
        b"through a file refused\ndir handle ok\n".as_slice(),
        // `rename` across two directories is one atomic message, because `RENAME` carries both
        // handles and both are now walked. `remove_dir_all` needed no nife code at all: std's
        // generic recursion is written in terms of `read_dir` and `remove_file` on paths it
        // composes itself, so it started working the moment those paths resolved.
        b"rename across ok\nremove_dir_all ok\n".as_slice(),
        b"fs ok\n".as_slice(),
    ] {
        buf[n..n + part.len()].copy_from_slice(part);
        n += part.len();
    }
    n
}

/// The exact bytes `std_exerciser` prints, in order. `println!` is line-buffered and every line
/// ends in `\n`, so the whole transcript is flushed by the time the program exits. Pinned here
/// so a drift in std's behaviour, the PAL, or the demo is a loud diff rather than a mystery.
/// `os nife` proves `std::env::consts::OS` resolves through the patched `env_consts`; the
/// two `unsupported` lines prove `fs`/`net` refuse honestly rather than pretend.
pub(super) const EXPECTED: &[u8] = b"hello from std on nife\n\
    os nife\n\
    args []\n\
    vec sum 149985000\n\
    string len 500\n\
    map lookup 1369\n\
    fs honestly unsupported\n\
    net honestly unsupported\n\
    instant monotonic ok\n\
    wall clock ok\n\
    entropy ok\n\
    config seeded\n\
    env ok\n\
    paths ok\n\
    exiting through process::exit\n";

/// A whole Rust `std` program runs on the native ABI and its output is exactly right.
///
/// Granted a heap, a stdout endpoint, a clock and a configuration page, so `fs` and `net` refuse:
/// the two `unsupported` lines are no ambient filesystem and no ambient network, felt from inside
/// std. Several lines (`wall clock ok`, `config seeded`, `env ok`, `paths ok`, `entropy ok`) guard
/// a call that once compiled and then killed the program, so a no-crash check would have passed.
/// `std::process::exit` must end the program as an exit, not a trap. See notes/std/the-proof.md,
/// "What the offline transcript's lines guard".
#[test_case]
fn a_whole_std_program_runs_on_the_native_abi() {
    if std_service::std_exerciser_image().is_none() {
        crate::testing::skip!(std_service::NO_STD_EXERCISER);
    }
    use core::sync::atomic::Ordering;

    use crate::arch::exceptions::USER_FAULTS;

    let image = program("std_exerciser").expect("no std_exerciser program in the initial ramdisk");
    let clock = program("clock").expect("no clock program in the initial ramdisk");
    let entropy = program("entropy").expect("no entropy program in the initial ramdisk");
    let faults_before = USER_FAULTS.load(Ordering::Relaxed);
    let run = std_service::start_reclaimable(image, clock, entropy);
    let (report, tid) = (run.report, run.thread);
    assert_std_transcript(report, EXPECTED, "std_exerciser");

    // The exit is part of the claim: a program can print a perfect transcript and then trap. The
    // transcript ends inside `rt::cleanup`, before the process leaves, and both a clean exit and a
    // fault reap an unsupervised thread, so the departure is the synchronization and the fault
    // counter is the verdict.
    assert!(
        super::wait_for(|| !crate::sched::is_thread_present(tid)),
        "the std program never left: it is neither exited nor faulted",
    );
    assert_eq!(
        USER_FAULTS.load(Ordering::Relaxed),
        faults_before,
        "std::process::exit trapped instead of exiting: a clean exit reported as a crash",
    );
    run.give_back("std_exerciser");
}

/// Start `std_exerciser`, read its transcript to the end, wait for it to leave, and hand back
/// everything it was given.
fn run_and_give_back(image: &'static [u8], clock: &'static [u8], entropy: &'static [u8]) {
    let run = std_service::start_reclaimable(image, clock, entropy);
    let mut out = [0u8; 4096];
    drain_sink(run.report, &mut out, "std_exerciser");
    assert!(
        super::wait_for(|| !crate::sched::is_thread_present(run.thread)),
        "std_exerciser never left",
    );
    run.give_back("std_exerciser");
}

/// **A std program the harness starts gives back every frame it was given once it has gone**
/// (milestone 801 (packages over the internet), on calef's ruling of 2026-10-10 (UTC): "Lets fix
/// the leaks.").
///
/// Before this, each std spawn kept about 56 frames for the boot: a clock service nothing could
/// end, the configuration page and the stack, all kernel frames. The first run here pays what a
/// boot builds once (the entropy service, the scheduler's lazily grown tables); the second must
/// leave the free count exactly where it found it, so a single kept frame fails.
///
/// Falsification: replayable `system_tests/falsifications/user.std_tests.a_std_program_gives_back_every_frame_it_was_given.patch`
#[test_case]
fn a_std_program_gives_back_every_frame_it_was_given() {
    let Some(image) = std_service::std_exerciser_image() else {
        crate::testing::skip!(std_service::NO_STD_EXERCISER);
    };
    let clock = program("clock").expect("no clock program in the initial ramdisk");
    let entropy = program("entropy").expect("no entropy program in the initial ramdisk");
    run_and_give_back(image, clock, entropy);
    let before = crate::memory::free_page_frames();
    run_and_give_back(image, clock, entropy);
    let after = crate::memory::free_page_frames();
    assert_eq!(
        after,
        before,
        "a std spawn kept {} frames after it was handed back",
        before as i64 - after as i64,
    );
}
