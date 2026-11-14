//! **nife's boot-line heads**, the one place this crate names the system.
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane `milestone/1078-console-heads-name-the-system`.
//! A child named for what it holds (§282 (when a file is split, the parent becomes an index)).

/// **The heads of nife's boot lines**, built from the name every one of them begins with. Every
/// reader here, and `xtask`'s gates, match through it; the kernel, the self-test program and the
/// shell build the same heads from the same name to print them.
///
/// Name: provisional.
pub const HEADS: boot_ladder::Heads<'static> = boot_ladder::Heads::new("nife");

/// nife's heads print exactly the bytes every stored log, board profile and gate holds.
#[cfg(test)]
#[test]
fn nife_heads_are_the_bytes_the_logs_hold() {
    let printed = [
        HEADS.banner(),
        HEADS.machine(),
        HEADS.self_test(),
        HEADS.self_test_subject(),
        HEADS.tour(),
        HEADS.prompt(),
        HEADS.screen_held(),
    ]
    .map(|head| head.to_string());
    assert_eq!(
        printed,
        [
            "nife on ",
            "nife machine: ",
            "nife self-test: ",
            "nife self-test subject: thread ",
            "nife: the capability core runs on ",
            "nife capability shell",
            "nife screen: held for the host; ",
        ]
    );
    assert_eq!(boot_ladder::SELF_TEST_FAILED, "FAILED: ");
    assert_eq!(boot_ladder::SELF_TEST_ABSENT, "absent: ");
}
