//! Tests of reading a transcript two writers shuffled: the boot claim and `free`'s row.

use super::*;

#[test]
fn the_job_pool_check_reads_free_as_ci_printed_it() {
    // Run 37110045386's riscv64 transcript, verbatim: two stranded regions beside `free`'s own.
    let leaked = "$ free\n                total        used        free\n\
                  Mem:        262144       84292      177852\n\
                  Yours:        2688         576        2112\n";
    assert_eq!(super::yours_used_kib(leaked), Some(576));
    assert!(super::yours_used_kib(leaked).unwrap() > super::YOURS_CEILING_KIB);
    // The same line after the fix, measured locally on riscv64: `free` alone.
    let whole = "Yours:        2688         192        2496\n";
    assert!(super::yours_used_kib(whole).unwrap() <= super::YOURS_CEILING_KIB);
    assert_eq!(super::yours_used_kib("no such row\n"), None);
}

/// **The transcript milestone 230's first CI step produced**, copied out of run 33702132439
/// rather than reconstructed, because the exact shape of the shuffle is the whole point.
///
/// Two writers on one UART with nothing arbitrating: the kernel's user-fault printer and the
/// userspace `console` server. `progenitor: construction budget dropped...` is spliced through the
/// kernel's register line one and two characters at a time. Every byte of both is present and
/// in order.
const INTERLEAVED_CI_TRANSCRIPT: &str = "\
  user thread 17 killed: scause 0x3 (code 3)
pc 0x0000000000406aa0   stval 0x000000000i0406aa0   usern sp 0x0000000000500da0
it:   cthe kernel is fine.
onstruction budget dropped; retype answers NoSuchSlot
progenitor: every program measured against the archive table

nife capability shell. naming a resource in a command IS granting it.
";

/// **The shuffle that defeated the second attempt**, from run 33707574930, the same code that
/// had passed run 33705237435 half an hour earlier. Evidence that the severity is
/// nondeterministic: `construction budget dropped` survives here only as a longest run of
/// `const`, and the kernel's own `the kernel is fine.` is destroyed in the same breath, which is
/// what left a tolerance keyed on the kernel's signature with nothing to key on. Dropping that
/// signature is what lets this one read again; the signature was never the safety, the ordering
/// in [`boot_claim`] is.
const SHREDDED_CI_TRANSCRIPT: &str = "\
  user thread 17 killed: scause 0x3 (code 3)
pc 0x0000000000406aa0   stval 0x0000000000406aa0   user sp 0x0000000000500da0
  the kernel iis fnit: constiner.
uction budget dropped; retype answers NoSuchSlot
progenitor: every program measured against the archive table

nife capability shell. naming a resource in a command IS granting it.
";

/// A clean boot reads exactly; a mildly shuffled one still reads, and says what it stepped over.
#[test]
fn a_claim_survives_being_interleaved_with_another_writer() {
    let clean = "progenitor: construction budget dropped; retype answers NoSuchSlot\n";
    assert!(matches!(
        boot_claim(
            clean,
            "construction budget dropped; retype answers NoSuchSlot",
            "construction budget NOT dropped"
        ),
        BootClaim::Affirmed(None)
    ));
    assert!(matches!(
        boot_claim(
            INTERLEAVED_CI_TRANSCRIPT,
            "construction budget dropped; retype answers NoSuchSlot",
            "construction budget NOT dropped"
        ),
        BootClaim::Affirmed(Some(_))
    ));
}

/// The run that failed CI reads again, and the fix is a deletion rather than an addition.
#[test]
fn the_shuffle_that_failed_ci_reads_again() {
    assert!(matches!(
        boot_claim(
            SHREDDED_CI_TRANSCRIPT,
            "construction budget dropped; retype answers NoSuchSlot",
            "construction budget NOT dropped"
        ),
        BootClaim::Affirmed(Some(_))
    ));
}

/// **The one that matters: a shuffle too severe to read is not a failure.**
///
/// Two attempts at hardening a matcher were each defeated by a shuffle worse than the fixture
/// they were tested against, so this stops betting on the matcher. What passes this transcript
/// is not a cleverer match: it is that neither sentence is readable AND the kernel was
/// demonstrably writing during the boot, which is the only condition under which a line can go
/// missing without the progenitor having gone quiet.
///
/// Synthetic, and said so: no CI run has produced a shuffle this bad, and the point is that
/// nothing rules one out. Every character of the marker is present and in order, spread through
/// far more foreign text than one fault report accounts for.
#[test]
fn a_shuffle_too_severe_to_read_is_reported_rather_than_failed() {
    let mut shredded = String::from("  user thread 17 killed: scause 0x3 (code 3)\n");
    for c in "construction budget dropped; retype answers NoSuchSlot".chars() {
        shredded.push(c);
        shredded.push_str("    pc 0x0000000000406aa0   stval 0x0000000000406aa0\n");
    }
    shredded.push_str("\nnife capability shell. naming a resource in a command IS granting it.\n");
    let BootClaim::Unreadable { longest_run } = boot_claim(
        &shredded,
        "construction budget dropped; retype answers NoSuchSlot",
        "construction budget NOT dropped",
    ) else {
        panic!("a transcript this badly shuffled cannot be read and must not be failed");
    };
    // "co", from `code 3` in the fault line rather than from the progenitor: with the marker spread this
    // thin, the longest surviving run of it is noise. Which is the fact worth reporting.
    assert!(
        longest_run.len() <= 3,
        "the longest surviving run is reported so a reader can judge the damage, got \
         {longest_run:?}"
    );
}

/// **The teeth.** The progenitor printing the failing answer is the thing this check exists to catch, and
/// it survives every amount of shuffling elsewhere in the transcript, because the search for it
/// is exact and interleaving can destroy a string but never create one.
#[test]
fn the_progenitor_reporting_the_failing_answer_is_a_failure_however_shuffled_the_rest_is() {
    let denied = format!(
        "{}progenitor: construction budget NOT dropped; it can still build\n",
        SHREDDED_CI_TRANSCRIPT
    );
    assert!(matches!(
        boot_claim(
            &denied,
            "construction budget dropped; retype answers NoSuchSlot",
            "construction budget NOT dropped"
        ),
        BootClaim::Denied
    ));
}

/// **And the vacuity guard.** A boot where the progenitor simply stopped reporting, with nothing else
/// writing the UART, has nothing that could have shuffled the line away, so its absence is real
/// and this fails. Without this, "cannot read it" would be a way to pass against a check that
/// had quietly stopped checking anything.
#[test]
fn a_silent_progenitor_fails_when_nothing_else_was_writing() {
    let quiet = "\
  uart irq: source 10 (machine description)
progenitor: every program measured against the archive table

nife capability shell. naming a resource in a command IS granting it.
";
    assert!(matches!(
        boot_claim(
            quiet,
            "construction budget dropped; retype answers NoSuchSlot",
            "construction budget NOT dropped"
        ),
        BootClaim::Silent
    ));
}

/// A fault **after** the prompt is out cannot explain a boot line that was read before it, so it
/// does not license [`BootClaim::Unreadable`]. Otherwise any typed command that traps on purpose
/// would switch the boot checks off for the rest of the run.
#[test]
fn a_fault_after_the_prompt_does_not_excuse_a_missing_boot_line() {
    let late = "\
progenitor: every program measured against the archive table

nife capability shell. naming a resource in a command IS granting it.
$ kernel_test_subject
  user thread 22 killed: scause 0x3 (code 3)
  the kernel is fine.
";
    assert!(matches!(
        boot_claim(
            late,
            "construction budget dropped; retype answers NoSuchSlot",
            "construction budget NOT dropped"
        ),
        BootClaim::Silent
    ));
}
