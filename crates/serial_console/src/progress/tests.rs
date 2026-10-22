//! The progress parser's host tests, moved out of `progress.rs` unchanged on 2026-10-12 (UTC) by
//! lane maintainer/near-ceiling-splits, so the parser stays under §266 (a Rust source file stays
//! under 2,000 lines). The fixture paths gain one `../` for the extra directory.

use super::*;

/// radon's firmware rung by its `--until` word, which is what `Stage::Spl` and its three
/// siblings used to be. Assertions read the same; the rung is now the profile's rather than
/// this file's, which is milestone 324 part 3 in one line.
fn rung(key: &str) -> Stage {
    Stage::Firmware(
        board::RADON
            .rung(key)
            .unwrap_or_else(|| panic!("radon has no {key} rung")),
    )
}

/// Offer text to a fresh radon ratchet the way the watcher does, tail included.
fn run(text: &str) -> BootProgress {
    feed(&board::RADON, text)
}

/// [`run`], for a board that is not radon (milestone 324 part 3).
fn feed(board: &'static board::Profile, text: &str) -> BootProgress {
    let mut feeder = LineFeeder::new();
    let mut progress = BootProgress::new(board);
    // One byte at a time, which is the worst case a real UART delivers and the case that
    // catches a recogniser depending on chunk boundaries.
    for byte in text.as_bytes() {
        let feeding = feeder.feed(&[*byte]);
        for line in &feeding.lines {
            progress.observe_line(line);
        }
        progress.observe_partial(&feeding.tail);
    }
    progress
}

/// **The real thing**: bytes off the wire on 2026-09-01, control characters and all, fed one
/// byte at a time. Everything else in this file is a unit test; this is the only one that
/// says the markers are the text a board prints.
#[test]
fn the_captured_boot_runs_the_whole_tour() {
    let progress = run(include_str!(
        "../../tests/fixtures/captured/vf2-2026-09-01-manual-boot.log"
    ));
    assert_eq!(progress.reached(), Stage::Tour);
    assert_eq!(progress.failure(), None);
    assert!(progress.is_relocated());
    assert!(
        !progress.userspace_ran(),
        "this card carried no archive, and the tour says so"
    );
    assert_eq!(
        progress.banner_line(),
        Some("nife on RISC-V (rv64, S-mode, Sv39)")
    );
}

/// The third outcome, and the one no documentation in this tree described: U-Boot gives up
/// **after** loading and relocating the image and **before** the kernel runs. It is not a
/// hang, and reporting it as one would send somebody hunting a multicore bug in a kernel that
/// never executed an instruction.
#[test]
fn the_captured_extlinux_failure_is_u_boot_giving_up() {
    let progress = run(include_str!(
        "../../tests/fixtures/captured/vf2-2026-09-01-extlinux-refused.log"
    ));
    assert_eq!(progress.reached(), rung("uboot"));
    assert!(progress.is_relocated(), "the image did load and relocate");
    assert_eq!(
        progress.failure(),
        Some(&Failure::FirmwareRefused {
            diagnosis: "U-Boot gave up before the kernel ran and wants the board reset",
            reason: "Device tree not found or missing FDT support".to_string(),
        }),
        "the reason is the line before the ERROR, and a reader needs it"
    );
}

/// **The whole portable ladder, in order** (milestone 268), and this is the test that says the
/// three rungs a boot climbs on every architecture are recognised in the order they arrive.
///
/// The lines are the kernel's own markers plus the tails it actually prints, rather than
/// invented text: a recogniser matching text no board ever prints fails in the direction that
/// looks like success.
#[test]
fn the_ladder_climbs_banner_machine_self_test_prompt() {
    let mut progress = BootProgress::default();
    progress.observe_line("nife on aarch64 (EL1, MMU off: physical addresses until mmu::init)");
    assert_eq!(progress.reached(), Stage::Banner);
    progress.observe_line("nife machine: aarch64, 4 processor(s), 256 MiB, 100 Hz");
    assert_eq!(progress.reached(), Stage::Machine);
    progress.observe_line("nife self-test: 5 of 5 passed");
    assert_eq!(progress.reached(), Stage::SelfTest);
    assert_eq!(progress.failure(), None, "a green verdict is not a failure");
    progress.observe_line("nife capability shell. naming a resource in a command IS granting it.");
    assert_eq!(progress.reached(), Stage::Prompt);
}

/// **Every architecture reaches the verdict, and that is the point of the milestone.** The
/// same three assertions against the three banners this tree prints, because finding 3 was
/// exactly a marker that only one architecture's arm produced.
#[test]
fn every_architecture_reaches_the_self_test_verdict() {
    for (banner, machine) in [
        (
            "nife on aarch64 (EL1, MMU off: physical addresses until mmu::init)",
            "nife machine: aarch64, 4 processor(s), 256 MiB, 100 Hz",
        ),
        (
            "nife on RISC-V (rv64, S-mode, Sv39)",
            "nife machine: riscv64, 4 processor(s), 256 MiB, 100 Hz",
        ),
        (
            "nife on x86_64 (long mode, ring 0, 4-level paging)",
            "nife machine: x86_64, 1 processor(s), 254 MiB, 100 Hz",
        ),
    ] {
        let mut progress = BootProgress::default();
        progress.observe_line(banner);
        progress.observe_line(machine);
        progress.observe_line("nife self-test: 5 of 5 passed");
        assert_eq!(
            progress.reached(),
            Stage::SelfTest,
            "{banner} did not climb the ladder"
        );
        assert_eq!(progress.machine_line(), Some(machine));
    }
}

/// **The machine line is kept from its first arrival, not overwritten by a later one**, the
/// same rule `banner_line` follows and for the same reason: `&&` rather than `||` in the
/// guard that captures it.
#[test]
fn the_machine_line_is_kept_from_first_arrival_only() {
    let mut progress = BootProgress::default();
    progress.observe_line("nife machine: riscv64, 4 processor(s), 256 MiB, 100 Hz");
    progress.observe_line("nife machine: aarch64, 8 processor(s), 512 MiB, 200 Hz");
    assert_eq!(
        progress.machine_line(),
        Some("nife machine: riscv64, 4 processor(s), 256 MiB, 100 Hz"),
        "the first machine line is kept, not replaced by a second"
    );
}

/// **A red verdict is a failure and not a stage.** Milestone 268's item 5: a degraded board on
/// the bench must not read as a good one, and this is the assertion that says so.
#[test]
fn a_failed_verdict_is_announced_with_the_names() {
    let mut progress = BootProgress::default();
    progress.observe_line("nife machine: riscv64, 4 processor(s), 256 MiB, 100 Hz");
    progress.observe_line("nife self-test: 4 of 5 passed, 1 FAILED: exceptions");
    assert_eq!(
        progress.failure(),
        Some(&Failure::SelfTestFailed("exceptions".to_string())),
    );
    // And the stage still ratchets: the verdict *did* arrive, which is a different fact from
    // whether it was green, and a report that lost it would say the board never self-tested.
    assert_eq!(progress.reached(), Stage::SelfTest);
    assert_eq!(
        progress.self_test_line(),
        Some("nife self-test: 4 of 5 passed, 1 FAILED: exceptions"),
    );
}

/// **A green verdict over a set that is not `boot_ladder::SELF_TEST_CHECKS` is a failure.**
/// Milestone 268's parity hole, measured before it was closed: `x86_64` with `scheduler` cut and
/// the count set to four printed exactly this line, and `boot-check` passed it.
#[test]
fn a_green_verdict_over_the_wrong_set_is_a_failure() {
    let mut progress = BootProgress::default();
    progress.observe_line("nife self-test: 4 of 4 passed");
    assert_eq!(
        progress.failure(),
        Some(&Failure::SelfTestFailed(format!(
            "the verdict counted 4 checks and boot_ladder lists {}",
            boot_ladder::SELF_TEST_CHECKS.len()
        ))),
    );

    let mut right = BootProgress::default();
    right.observe_line(&format!(
        "nife self-test: {n} of {n} passed",
        n = boot_ladder::SELF_TEST_CHECKS.len()
    ));
    assert_eq!(right.failure(), None, "the listed total must stay green");
}

/// More than one failed check, because the verdict names them all and a reader needs all of
/// them: fixing the first and re-running to discover the second is the loop this avoids.
#[test]
fn a_failed_verdict_carries_every_name() {
    let mut progress = BootProgress::default();
    progress.observe_line("nife self-test: 3 of 5 passed, 2 FAILED: mapping scheduler");
    assert_eq!(
        progress.failure(),
        Some(&Failure::SelfTestFailed("mapping scheduler".to_string())),
    );
}

/// **`FAILED:` on its own is not a self-test failure**, and this is the regression that says
/// so. The boot tour has printed that token since milestone 267 for its preemption check, so a
/// recogniser keying on the word alone would report every such line as a broken self-test.
#[test]
fn the_tours_own_failed_token_is_not_a_self_test_failure() {
    let mut progress = BootProgress::default();
    progress.observe_line("  FAILED: a spinner did not run, or nothing was preempted.");
    assert_eq!(progress.failure(), None);
}

/// A green verdict's counts are kept, because a reader comparing two runs needs to see both ran
/// the same set. "0 of 0 passed" used to be a *passing* verdict on a broken build, visible only
/// if a person read the numbers; since milestone 268 checks the total against
/// `boot_ladder::SELF_TEST_CHECKS`, it is a failure, and its raw line is still kept.
#[test]
fn a_green_verdict_keeps_its_counts() {
    let green = format!(
        "nife self-test: {n} of {n} passed",
        n = boot_ladder::SELF_TEST_CHECKS.len()
    );
    let mut progress = BootProgress::default();
    progress.observe_line(&green);
    assert_eq!(progress.failure(), None);
    assert_eq!(progress.self_test_line(), Some(green.as_str()));

    // `0 of 0 passed` is the vacuous pass the counts exist to expose, and since the total is
    // checked against `boot_ladder::SELF_TEST_CHECKS` it is red rather than kept as green.
    let mut vacuous = BootProgress::default();
    vacuous.observe_line("nife self-test: 0 of 0 passed");
    assert!(matches!(
        vacuous.failure(),
        Some(Failure::SelfTestFailed(_))
    ));
    assert_eq!(
        vacuous.self_test_line(),
        Some("nife self-test: 0 of 0 passed"),
        "the raw line is kept on a red verdict too"
    );
}

/// A partial verdict ratchets the stage and captures nothing, which is `observe_partial`'s
/// contract: a substring match is monotone, so more bytes cannot unmake the rung, but a
/// truncated line would record a count that has not finished arriving.
#[test]
fn a_partial_verdict_ratchets_but_does_not_capture() {
    let mut progress = BootProgress::default();
    progress.observe_partial("nife self-test: 4 of 5 pas");
    assert_eq!(progress.reached(), Stage::SelfTest);
    assert_eq!(progress.self_test_line(), None);
    assert_eq!(
        progress.failure(),
        None,
        "the FAILED token has not arrived yet, and half a verdict is not a verdict"
    );
}

/// The ladder is an order, and these are the two comparisons the tools actually make.
#[test]
fn the_new_rungs_sit_where_the_milestone_put_them() {
    assert!(Stage::Machine > Stage::Banner);
    assert!(Stage::SelfTest > Stage::Machine);
    assert!(
        Stage::Tour > Stage::SelfTest,
        "the riscv tour runs after it"
    );
    assert!(Stage::Prompt > Stage::Tour, "a prompt is past any tour");
    assert!(Stage::Soak > Stage::Prompt, "a soak replaces the handoff");
}

/// The banner is not the end of the story, and the two captures differ by exactly this: both
/// reach U-Boot, one reaches the banner and then the tour, the other never reaches either.
#[test]
fn the_tour_line_outranks_the_banner() {
    let mut progress = BootProgress::default();
    progress.observe_line("nife on RISC-V (rv64, S-mode, Sv39)");
    assert_eq!(progress.reached(), Stage::Banner);
    progress.observe_line("nife: the capability core runs on RISC-V.");
    assert_eq!(progress.reached(), Stage::Tour);
}

/// The trust boundary refusing, captured rather than imagined. Note where it got to: past the
/// banner and well into the tour, which is what makes "reached the banner" a useless test for
/// whether a boot worked.
#[test]
fn the_captured_refusal_gets_past_the_banner_before_refusing() {
    let progress = run(include_str!(
        "../../tests/fixtures/captured/vf2-2026-09-01-measured-boot-refused.log"
    ));
    assert_eq!(progress.reached(), Stage::Banner);
    assert_eq!(progress.failure(), Some(&Failure::MeasuredBootRefused));
    assert!(
        !progress.userspace_ran(),
        "it halted rather than handing the archive to init"
    );
    // The wording is load-bearing: this is the gate working, and a message that read like a
    // crash would send somebody debugging the boot mechanism instead of their build.
    let said = progress.failure().unwrap().describe();
    assert!(said.contains("the gate working"));
    assert!(said.contains("script/board-image"));
}

/// The other successful capture, and the difference between the two: this one had an archive
/// on the card, so `init` built a child and a userspace driver came up.
#[test]
fn the_captured_userspace_boot_ran_a_child() {
    let progress = run(include_str!(
        "../../tests/fixtures/captured/vf2-2026-09-01-userspace.log"
    ));
    assert_eq!(progress.reached(), Stage::Tour);
    assert_eq!(progress.failure(), None);
    assert!(progress.userspace_ran());
}

/// **The soak stage and its numbers, from a real QEMU riscv64 run** (milestone 219). Fed one
/// byte at a time by `run`, which is the case that catches a field parser depending on chunk
/// boundaries.
#[test]
fn the_captured_soak_reaches_the_soak_stage_and_its_last_beat_is_read() {
    // A pre-297 capture, respelled at read time and not on disk; see
    // `crate::respell_pre_297_markers` for why the file is left as the machine printed it.
    let progress = run(&crate::respell_pre_297_markers(include_str!(
        "../../tests/fixtures/captured/qemu-2026-09-01-riscv64-soak.log"
    )));
    assert_eq!(progress.reached(), Stage::Soak);
    assert!(
        progress.reached() > Stage::Tour,
        "a soak is a stage past the tour, which is what re-arms the quiet check"
    );
    assert_eq!(progress.failure(), None);
    let beat = progress.soak().expect("the heartbeats must be parsed");
    assert_eq!(beat.beat, 5);
    assert_eq!(beat.seconds, 25);
    assert_eq!(beat.rounds, 595_432);
    assert_eq!(beat.rate, 24_160);
    assert_eq!(beat.refused, 0);
    assert_eq!(beat.crossings, 21);
    // The fixture is a pre-221 capture and has no `wakes=` field, so the parser leaves it at
    // zero. That is the case the summary keys on to tell "nothing made it cross" apart from
    // "something did and it still did not".
    assert_eq!(beat.wakes, 0);
}

/// **The same parse against the marker vocabulary the kernel prints today** (milestone 297).
///
/// `qemu-2026-09-14-riscv64-soak-test.log` is `script/soak-test --arch riscv64 --for 30s`, taken
/// unedited on the day the rename landed. It exists so that the live spelling is proved against
/// a machine rather than only against text this project wrote, which is the standard
/// `tests/fixtures/README.md` holds every other marker to. It is also post-221, so it carries
/// the `wakes=` field the capture above predates.
#[test]
fn the_renamed_markers_are_read_from_a_real_run_of_the_renamed_command() {
    let progress = run(include_str!(
        "../../tests/fixtures/captured/qemu-2026-09-14-riscv64-soak-test.log"
    ));
    assert_eq!(progress.reached(), Stage::Soak);
    assert_eq!(progress.failure(), None);
    let beat = progress.soak().expect("the heartbeats must be parsed");
    assert_eq!(beat.beat, 5);
    assert_eq!(beat.seconds, 25);
    assert_eq!(beat.rounds, 274_744);
    assert_eq!(beat.rate, 11_014);
    assert_eq!(beat.refused, 0);
    assert_eq!(beat.mismatches, 0);
    assert_eq!(beat.stalled, 0);
    // The tick route is live in this build, so unlike the pre-221 capture above this one both
    // wakes and crosses. Neither number is a result: see notes/soak.md on what a soak rate is
    // worth under emulation.
    assert_eq!(beat.wakes, 10_032);
    assert_eq!(beat.crossings, 2_717);
}

/// **A soak failure is a failure, not a stage.** The kernel prints `soak-test: FAILED` and then
/// panics, and it is the panic the recogniser names, carrying the reason; the `FAILED` line
/// must not be mistaken for a heartbeat or for the workload starting.
#[test]
fn a_soak_failure_is_reported_as_the_panic_it_becomes() {
    let progress = run(concat!(
        "soak-test: started 4 groups of one responder and 3 callers (20 user threads)\n",
        "soak-test: t=5s beat=1 rounds=100 rate=20/s workers=20 refused=1 mismatch=0 stalled=0\n",
        "soak-test: FAILED at t=5s beat=1: the wake gate refused a wake\n",
        "[PANIC] soak failed at t=5s beat=1\n",
    ));
    assert_eq!(progress.reached(), Stage::Soak);
    assert_eq!(
        progress.failure(),
        Some(&Failure::KernelPanic(
            "soak failed at t=5s beat=1".to_string()
        ))
    );
    assert_eq!(progress.soak().map(|b| b.refused), Some(1));
}

/// **The placement census must be invisible to this recogniser** (milestone 240).
///
/// `kernel/src/soak.rs` prints a block of `soak-test-census:` lines at soak start and again whenever
/// the arrangement changes. The two agree by that prefix being outside both substrings this
/// file matches on, and that is an argument until something checks it: these lines are verbatim
/// from an aarch64 QEMU run on 2026-09-03, interleaved exactly as the kernel emits them.
///
/// Three claims. The census does not advance the stage on its own, the start line still does,
/// and a beat carrying the new `drifted=` field still parses field for field, `t=` included,
/// which is the one a census token could plausibly have collided with.
#[test]
fn the_placement_census_changes_nothing_the_recogniser_reads() {
    let progress = run(concat!(
        "soak-test-census: core=0 threads=10 R0 C0 C0 C0 G0 R2 C2 C2 C2 G3\n",
        "soak-test: started 4 groups of one responder, 3 callers, 1 grinder and 1 tick waiter \
         (24 user threads) on 4 online core(s), beating every 5s\n",
        "soak-test-census: where the kernel placed each worker at spawn: R=responder, C=caller, \
         G=grinder, W=tick waiter, and the number after each letter is its group\n",
        "soak-test-census: core=1 threads=7 W0 W1 W2 R3 C3 C3 C3\n",
        "soak-test-census: core=2 threads=3 G1 G2 W3\n",
        "soak-test: t=10s beat=2 rounds=314048 rate=35576/s wakes=3747 wakerate=386/s workers=24 \
         refused=0 mismatch=0 stalled=0 drifted=0 crossings=1849 remote=2165 steals=4 \
         deferred=6\n",
    ));
    assert_eq!(progress.reached(), Stage::Soak);
    assert_eq!(progress.failure(), None);
    let beat = progress.soak().expect("the heartbeat must still parse");
    assert_eq!(beat.seconds, 10);
    assert_eq!(beat.beat, 2);
    assert_eq!(beat.rounds, 314_048);
    assert_eq!(beat.rate, 35_576);
    assert_eq!(beat.crossings, 1849);
    assert_eq!(beat.remote, 2165);
}

/// A census line that arrives BEFORE anything else must not reach the soak stage by itself.
///
/// Separate from the test above because that one proves the start line still works, and this
/// one proves the census does not stand in for it: a log truncated to census lines has not seen
/// a soak start, and reporting one would make `serial_console` claim a stage from an artefact.
#[test]
fn a_census_alone_does_not_reach_the_soak_stage() {
    let progress = run("soak-test-census: core=0 threads=6 R0 C0 C0 C0 G0 W0\n");
    assert_eq!(progress.reached(), Stage::Cold);
    assert_eq!(progress.soak(), None);
}

#[test]
fn a_stale_card_is_named_at_u_boot() {
    let progress = run(include_str!(
        "../../tests/fixtures/synthetic/vf2-bad-magic.log"
    ));
    assert_eq!(progress.reached(), rung("uboot"));
    assert_eq!(
        progress.failure(),
        Some(&Failure::FirmwareRefused {
            diagnosis: "U-Boot rejected the image header (Bad Linux RISCV Image magic!)",
            reason: String::new(),
        })
    );
}

#[test]
fn a_panic_carries_its_message() {
    let progress = run(
        "Starting kernel ...\n\nnife on RISC-V (rv64, S-mode, Sv39)\n[PANIC] hart 3 took a load fault\n",
    );
    assert_eq!(progress.reached(), Stage::Banner);
    assert_eq!(
        progress.failure(),
        Some(&Failure::KernelPanic(
            "hart 3 took a load fault".to_string()
        ))
    );
}

/// The distinction the whole recogniser turns on, and the one a `contains("U-Boot")` gets
/// wrong: SPL's banner must not be read as U-Boot proper, or a board that dies in SPL is
/// reported two stages further along than it got.
#[test]
fn spl_is_not_u_boot_proper() {
    let progress = run("U-Boot SPL 2021.10 (Feb 12 2023 - 20:24:34 +0800)\n");
    assert_eq!(progress.reached(), rung("spl"));
}

#[test]
fn u_boot_proper_is_recognised_by_its_version() {
    let progress = run("U-Boot 2021.10 (Feb 12 2023 - 20:24:34 +0800), Build: jenkins-github\n");
    assert_eq!(progress.reached(), rung("uboot"));
}

/// The prompt has no newline after it. A feeder that only reported complete lines would sit
/// there while U-Boot sat waiting for a command.
#[test]
fn the_prompt_is_seen_without_a_newline() {
    let mut feeder = LineFeeder::new();
    let mut progress = BootProgress::default();
    let feeding = feeder.feed(b"Hit any key to stop autoboot:  0 \nStarFive # ");
    for line in &feeding.lines {
        progress.observe_line(line);
    }
    progress.observe_partial(&feeding.tail);
    assert_eq!(progress.reached(), rung("uboot"));
    assert_eq!(feeding.tail, "StarFive # ");
}

/// A wrong baud rate is bytes, not text. The tool must keep going and log them.
#[test]
fn garbage_bytes_do_not_stop_the_feeder() {
    let mut feeder = LineFeeder::new();
    let mut progress = BootProgress::default();
    let feeding = feeder.feed(&[0xff, 0xfe, 0x80, b'\n', 0xc0]);
    for line in &feeding.lines {
        progress.observe_line(line);
    }
    progress.observe_partial(&feeding.tail);
    assert_eq!(progress.reached(), Stage::Cold);
    assert_eq!(feeding.lines.len(), 1);
}

/// A marker split across two reads is the normal case at 115200, not an edge one.
#[test]
fn a_marker_split_across_chunks_is_still_seen() {
    let mut feeder = LineFeeder::new();
    let mut progress = BootProgress::default();
    for chunk in [&b"Starting ke"[..], &b"rnel ...\n"[..]] {
        let feeding = feeder.feed(chunk);
        for line in &feeding.lines {
            progress.observe_line(line);
        }
        progress.observe_partial(&feeding.tail);
    }
    assert_eq!(progress.reached(), rung("handoff"));
}

/// The bug the byte-at-a-time feeding found, kept as its own test because reasoning did not
/// find it: mid-word, `U-Boot ` reads as U-Boot proper, and a payload captured from a tail is
/// captured truncated.
#[test]
fn a_partial_line_does_not_settle_a_word_or_capture_text() {
    let mut progress = BootProgress::default();
    progress.observe_partial("U-Boot ");
    assert_eq!(progress.reached(), Stage::Cold);
    progress.observe_partial("U-Boot SPL 2021.10 (Feb");
    assert_eq!(progress.reached(), rung("spl"));

    let mut progress = BootProgress::default();
    progress.observe_partial("nife on ");
    assert_eq!(progress.reached(), Stage::Banner);
    assert_eq!(progress.banner_line(), None);
    progress.observe_line("nife on RISC-V (rv64, S-mode, Sv39)");
    assert_eq!(
        progress.banner_line(),
        Some("nife on RISC-V (rv64, S-mode, Sv39)")
    );

    let mut progress = BootProgress::default();
    progress.observe_partial("[PANIC] hart 3 took");
    assert_eq!(progress.failure(), None);
    progress.observe_line("[PANIC] hart 3 took a load fault");
    assert_eq!(
        progress.failure(),
        Some(&Failure::KernelPanic(
            "hart 3 took a load fault".to_string()
        ))
    );
}

#[test]
fn the_ratchet_never_goes_backwards() {
    let mut progress = BootProgress::default();
    progress.observe_line("Starting kernel ...");
    progress.observe_line("U-Boot SPL 2021.10");
    assert_eq!(progress.reached(), rung("handoff"));
}

/// A carriage return alone ends a line: firmware progress indicators use it to overwrite.
#[test]
fn a_bare_carriage_return_ends_a_line() {
    let mut feeder = LineFeeder::new();
    let feeding =
        feeder.feed(b"Hit any key to stop autoboot:  2 \rHit any key to stop autoboot:  1 \r");
    assert_eq!(feeding.lines.len(), 2);
    assert_eq!(feeding.tail, "");
}

/// **The sweep, from a real run of `--features job_mix`** (milestone 324 part 2), fed one byte
/// at a time. This is the test that says the markers are the text a kernel prints: the fixture
/// is `helpers/qemu-runner-aarch64.sh` on 2026-09-19, unedited, CRLF and all.
///
/// **And it is the test that says the *fields* are, which it did not say before.** The capture
/// it read until 2026-09-19 was taken from a pre-milestone-168 kernel, so it agreed with a
/// parser that had stopped agreeing with the kernel; both were green and both were wrong. This
/// one was taken from the merged tree, which is the whole of why it was retaken. See the
/// module's `BUGS`.
#[test]
fn the_captured_sweep_runs_to_its_done_line() {
    let progress = run(include_str!(
        "../../tests/fixtures/captured/qemu-2026-09-19-aarch64-job-mix-medians.log"
    ));
    assert_eq!(progress.reached(), Stage::SweepDone);
    assert_eq!(progress.failure(), None);
    // The whole boot ladder is underneath it, which is what makes the sweep a stage past the
    // tour rather than a thing beside it.
    assert_eq!(
        progress.self_test_line(),
        Some("nife self-test: 5 of 5 passed")
    );
    let point = progress.sweep_point().expect("the points must be parsed");
    assert_eq!(
        *point,
        SweepPoint {
            tasks: 32,
            jobs: 4096,
            repeats: 21,
            ticks_min: 228_108_401,
            ticks_median: 233_958_411,
            ticks_max: 249_234_771,
            jpm_median: 65_652,
        },
        "the last point is the top of job_mix::TASK_SWEEP"
    );
    assert_eq!(
        u64::from(u32::try_from(job_mix::MAX_TASKS).expect("the sweep tops out in a u32")),
        point.tasks,
        "the last point is job_mix::MAX_TASKS, and the two agree by the kernel reading it"
    );
    assert_eq!(point.jobs, point.tasks * job_mix::JOBS_PER_TASK);
    // **The field the parse is most likely to be silently wrong about.** Every other number
    // here would survive a parser that read nothing, because `Default` is zero and a stale
    // expectation is also a number; this one is `job_mix::REPEATS` and the kernel printed it,
    // so the two disagreeing means one of them moved.
    assert_eq!(
        point.repeats,
        job_mix::REPEATS as u64,
        "the kernel prints job_mix::REPEATS and this build reads the same constant"
    );
    assert!(
        point.ticks_min <= point.ticks_median && point.ticks_median <= point.ticks_max,
        "the three ticks fields are a sorted spread, and reading them out of order is how a \
         near-miss parse would look"
    );
    let subrun = progress.sweep_subrun().expect("the subruns must be parsed");
    assert_eq!(
        *subrun,
        SweepSubrun {
            tasks: 32,
            repeat: 20,
            ticks: 233_743_670,
        },
        "the last subrun is the last repeat of the last point, and it is NOT the median one"
    );
    // These numbers are a draw rather than a result: TCG models no cache, and milestone 240's
    // census says the placement lottery decides throughput by up to fifteenfold on real
    // silicon. See notes/job-mix.md. What is asserted here is the parse, not the figure.
}

/// **The whole of part 2, in one pair of assertions**: a sweep that stops partway is a
/// different stage from one that finished, where before this milestone both ended as the clock
/// running out and shared an exit status.
///
/// The truncation is the real capture cut after the third point, which is the honest stand-in
/// for a wedge: nothing was invented, and the bytes before the cut are bytes a machine printed.
#[test]
fn a_sweep_cut_off_partway_has_not_reached_its_done_line() {
    let whole =
        include_str!("../../tests/fixtures/captured/qemu-2026-09-19-aarch64-job-mix-medians.log");
    let cut = whole
        .find("job-mix: tasks=8")
        .expect("the capture has a fourth point to cut before");
    let progress = run(&whole[..cut]);
    assert_eq!(progress.reached(), Stage::Sweep);
    assert!(
        progress.reached() < Stage::SweepDone,
        "a wedged sweep must not read as a finished one"
    );
    assert_eq!(progress.failure(), None, "a wedge announces nothing");
    assert_eq!(
        progress.sweep_point().map(|p| p.tasks),
        Some(4),
        "three points printed and the third was four tasks"
    );
}

/// The census must be invisible to the recogniser, the same claim
/// `the_placement_census_changes_nothing_the_recogniser_reads` makes for the soak and for the
/// same reason: `job_mix::CENSUS` is outside `job_mix::STARTED` and `job_mix::POINT`, and that
/// is an argument until something checks it. The lines are verbatim from the capture.
#[test]
fn a_job_mix_census_alone_reaches_nothing() {
    let progress = run(concat!(
        "job-mix-census: where the kernel placed each thread at spawn: S=echo server, T=task\n",
        "job-mix-census: core=0 threads=7 T3 T8 T14 T15 T22 T24 T29\n",
        "job-mix-census: core=2 threads=10 S0 T2 T4 T9 T11 T17 T18 T20 T27 T28\n",
    ));
    assert_eq!(progress.reached(), Stage::Cold);
    assert_eq!(progress.sweep_point(), None);
    assert_eq!(progress.failure(), None);
}

/// **A refused sweep is a failure and not a stage**, milestone 268's item 5 applied to the
/// workload: a kernel that would not start the sweep must not read like one that ran it.
///
/// The line is built from `job_mix::FAILED` rather than written out, so this test cannot
/// disagree with the kernel about the prefix; the tail is `kernel/src/job_mix.rs`'s own.
#[test]
fn a_refused_sweep_is_announced_with_its_reason() {
    let progress = run(&format!(
        "{}no 'job_mix_task' program in the initial ramdisk; nothing to run\n",
        job_mix::FAILED
    ));
    assert_eq!(
        progress.failure(),
        Some(&Failure::SweepFailed(
            "no 'job_mix_task' program in the initial ramdisk; nothing to run".to_string()
        ))
    );
    assert_eq!(
        progress.reached(),
        Stage::Cold,
        "it refused before the sweep started"
    );
    let said = progress.failure().expect("it failed").describe();
    assert!(
        said.contains("No point of the sweep was measured"),
        "an empty log reads like a wedge until the report says nothing ran"
    );
}

/// The two sweep rungs are an order, and these are the comparisons the tools make.
#[test]
fn the_sweep_rungs_sit_above_the_boot_ladder() {
    assert!(Stage::Sweep > Stage::Prompt, "a sweep replaces the handoff");
    assert!(
        Stage::SweepDone > Stage::Sweep,
        "finishing outranks starting, which is the whole of part 2"
    );
}

/// A partial sweep line ratchets and captures nothing, which is `observe_partial`'s contract.
/// The tail is offered again as it grows, so a counter here would double-count and a truncated
/// `ticks=` would record a figure that has not finished arriving.
#[test]
fn a_partial_sweep_line_ratchets_but_does_not_capture() {
    let mut progress = BootProgress::default();
    progress.observe_partial("job-mix: started 32 tasks and 2 ser");
    assert_eq!(progress.reached(), Stage::Sweep);
    let mut progress = BootProgress::default();
    progress.observe_partial("job-mix: tasks=32 jobs=4096 repeats=21 ticks_min=2281");
    assert_eq!(
        progress.sweep_point(),
        None,
        "half a point is not a point, and 2281 is not 228108401"
    );
}

/// **The board profile's whole claim, checked against radon's own capture** (milestone 324 part
/// 3): the firmware prologue is the profile's and the rest of the ladder is not.
///
/// The same bytes, read twice. With radon's profile the boot climbs SPL, OpenSBI, U-Boot and
/// the handoff on the way to the tour. With xenon's, whose prologue is empty, it reaches
/// exactly the same tour and never reports a firmware rung, because none of those lines are
/// xenon's to claim. A hard-coded prologue cannot tell those two readings apart, which is what
/// the ruling meant by *the profile is the firmware prologue only*.
#[test]
fn the_same_capture_climbs_radons_prologue_and_not_xenons() {
    let log = include_str!("../../tests/fixtures/captured/vf2-2026-09-01-manual-boot.log");

    let as_radon = feed(&board::RADON, log);
    assert_eq!(as_radon.reached(), Stage::Tour);
    assert!(as_radon.is_relocated(), "Moving Image from is U-Boot's");

    let as_xenon = feed(&board::XENON, log);
    assert_eq!(
        as_xenon.reached(),
        Stage::Tour,
        "the kernel's own ladder is shared and is read the same either way"
    );
    assert!(
        !as_xenon.is_relocated(),
        "a relocation note belongs to the firmware that printed it"
    );
    assert_eq!(as_xenon.board().name, "xenon");
}

/// **xenon's prologue is empty because its capture is**, which is part 3's premise rather than
/// an omission. Milestone 324's block says a `--replay` of this file reports the banner, the
/// machine line, the five-of-five verdict and the measured-boot refusal; this is that claim as
/// an assertion, read through the profile that says xenon has no firmware rungs at all.
///
/// The file is `bench/` rather than `tests/fixtures/` on purpose: it is calef's bench record of
/// first light on that machine, and a copy here would be a second one to keep in step.
#[test]
fn xenons_capture_needs_no_prologue_to_be_read_correctly() {
    let progress = feed(
        &board::XENON,
        include_str!("../../../../bench/xenon-2026-09-17/first-light-095500.log"),
    );
    assert_eq!(
        progress.banner_line(),
        Some("nife on x86_64 (long mode, ring 0, 4-level paging)")
    );
    assert_eq!(
        progress.machine_line(),
        Some("nife machine: x86_64, 4 processor(s), 17119 MiB, 100 Hz")
    );
    assert_eq!(
        progress.self_test_line(),
        Some("nife self-test: 5 of 5 passed")
    );
    assert_eq!(progress.failure(), Some(&Failure::MeasuredBootRefused));
    assert_eq!(
        progress.reached(),
        Stage::SelfTest,
        "it halted at the trust boundary, after the verdict and before any handoff"
    );
}

/// `--until spl` against a board with no SPL is a request that can never be satisfied, and
/// answering it with a refusal beats watching for two minutes and reporting that time ran out.
/// `xtask` is where the refusal is printed; this is the lookup it rests on.
#[test]
fn a_firmware_rung_is_only_offered_by_a_board_that_has_one() {
    assert!(board::RADON.rung("spl").is_some());
    assert!(board::XENON.rung("spl").is_none());
}

/// Every stage's label, which every report reads through [`fmt::Display`] rather than
/// through this function directly, so nothing else in the suite pins the exact wording down.
#[test]
fn every_stage_has_its_own_label() {
    assert_eq!(Stage::Cold.label(), "nothing recognisable");
    assert_eq!(Stage::Banner.label(), "kernel banner");
    assert_eq!(Stage::Machine.label(), "machine described");
    assert_eq!(Stage::SelfTest.label(), "self-test verdict");
    assert_eq!(Stage::Tour.label(), "boot tour complete");
    assert_eq!(Stage::Prompt.label(), "shell prompt");
    assert_eq!(Stage::Soak.label(), "soak running");
    assert_eq!(Stage::Sweep.label(), "job-mix sweep running");
    assert_eq!(Stage::SweepDone.label(), "job-mix sweep complete");
    let rung = board::RADON.rung("spl").expect("radon has an spl rung");
    assert_eq!(Stage::Firmware(rung).label(), "U-Boot SPL");
}

/// `Display` writes the label into the formatter; a mutant that turned this into a no-op
/// would still return `Ok`, and only reading what actually landed in the string catches it.
#[test]
fn displaying_a_stage_writes_its_label() {
    assert_eq!(Stage::Tour.to_string(), "boot tour complete");
}

/// `Failure::describe`'s one match guard, both ways: with a reason, the diagnosis and the
/// reason both appear; without one, only the diagnosis does.
#[test]
fn firmware_refused_describes_with_and_without_a_reason() {
    let with_reason = Failure::FirmwareRefused {
        diagnosis: "U-Boot gave up",
        reason: "the line before it".to_string(),
    };
    assert_eq!(with_reason.describe(), "U-Boot gave up: the line before it");

    let without_reason = Failure::FirmwareRefused {
        diagnosis: "U-Boot gave up",
        reason: String::new(),
    };
    assert_eq!(without_reason.describe(), "U-Boot gave up");
}

/// [`LineFeeder::tail`] reads back exactly the bytes still pending, which is the only thing
/// this accessor promises.
#[test]
fn line_feeder_tail_reads_back_the_pending_bytes() {
    let mut feeder = LineFeeder::new();
    let feeding = feeder.feed(b"a partial line, no terminator yet");
    assert_eq!(feeding.tail, "a partial line, no terminator yet");
    assert_eq!(feeder.tail(), "a partial line, no terminator yet");
}

/// **An `absent` the board does not allow is a failure** (calef's ruling of 2026-10-20 (UTC) on
/// #2447: `absent` must never hide a device that should be there). radon has a TRNG, so a radon
/// boot that reports `trng` absent lost a device, and the verdict's green count does not save it.
/// The same line from an emulator, which may lack every device, stays green.
#[test]
fn an_absence_the_board_forbids_is_a_failure() {
    let n = boot_ladder::SELF_TEST_CHECKS.len();
    let line = format!("nife self-test: {} of {n} passed, 1 absent: trng\n", n - 1);

    let on_radon = feed(&board::RADON, &line);
    assert_eq!(
        on_radon.failure(),
        Some(&Failure::SelfTestFailed(
            "trng (absent on radon, which has the device for it)".to_string()
        )),
    );

    let on_emulator = feed(&board::EMULATOR, &line);
    assert_eq!(
        on_emulator.failure(),
        None,
        "an emulator may lack any device"
    );
}

/// **The absence list ends where the failures begin**, so a check that failed is reported as a
/// failure, and one that was absent is not read as failed.
#[test]
fn absences_and_failures_are_read_apart() {
    let n = boot_ladder::SELF_TEST_CHECKS.len();
    let line = format!(
        "nife self-test: {} of {n} passed, 2 absent: blockdev deviceirq, 1 FAILED: entropy\n",
        n - 3
    );
    let progress = feed(&board::EMULATOR, &line);
    assert_eq!(
        progress.failure(),
        Some(&Failure::SelfTestFailed("entropy".to_string())),
    );
    assert_eq!(
        BootProgress::verdict_absences(&line).collect::<Vec<_>>(),
        ["blockdev", "deviceirq"],
    );
}
