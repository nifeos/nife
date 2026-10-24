use crate::proto;

/// `FLAG_RETRY` is distinguishable from every other read reply, in both shapes: a line reply
/// of length zero with no flags is an empty line, and a raw reply always carries 1..=8 bytes.
#[test]
fn a_retry_is_never_mistaken_for_data() {
    assert!(proto::is_retry(0, proto::FLAG_RETRY));
    assert!(!proto::is_retry(0, 0), "an empty line is not a retry");
    assert!(!proto::is_retry(0, proto::FLAG_INTERRUPTED));
    assert!(
        !proto::is_retry(1, proto::FLAG_RETRY),
        "a one-byte raw read of 0x04 is data"
    );
    assert_eq!(
        proto::FLAG_RETRY & (proto::FLAG_EOF | proto::FLAG_INTERRUPTED),
        0
    );
    assert_eq!(proto::OPERATION_QUIESCE, 8);
}

/// The proto word packing round-trips, and the reserved bits stay zero.
#[test]
fn proto_words_round_trip() {
    let w = proto::req(proto::OPERATION_READLINE, 42);
    assert_eq!(proto::operation(w), proto::OPERATION_READLINE);
    assert_eq!(proto::len(w), 42);
    assert_eq!(w & 0x00ff_ffff_0000_0000, 0);
    // The reply flags are wire ABI shared with every client: pin the values, because a
    // drifted FLAG_INTERRUPTED makes an interrupted read look like a normal empty line.
    assert_eq!(proto::FLAG_EOF, 1);
    assert_eq!(proto::FLAG_INTERRUPTED, 2);
    // Pinned for the same reason: OPERATION_RAWMODE and OPERATION_READRAW are wire ABI too, and a drifted
    // opcode number would silently reassign OPERATION_PRINT's or a future opcode's traffic.
    assert_eq!(proto::OPERATION_RAWMODE, 6);
    assert_eq!(proto::OPERATION_READRAW, 7);
}

// ---- the survivors of the 2026-10-03 mutation census ----

/// The two start bits are the two lowest bits, one each.
#[test]
fn the_two_start_bits_are_distinct_low_bits() {
    assert_eq!(proto::START_HANDOFF, 1);
    assert_eq!(proto::START_ABSORB, 2);
}

/// **Only the supervisor's badge admits a quiesce, and it admits nothing else** (DECISIONS §232
/// (the `line_editor` swap contract): "only a supervisor sends it"). An unbadged copy is what the
/// shell, `login` and the input driver hold; a quiesce honored from one of them stops the terminal
/// until a supervisor that never asked says `CTL_RESUME`, which on a boot is never.
#[test]
fn only_the_supervisor_badge_admits_a_quiesce() {
    for badge in [0, proto::RAW_ONLY_BADGE, proto::STDIN_BADGE, 4, u64::MAX] {
        assert!(
            !proto::admits(badge, proto::OPERATION_QUIESCE),
            "badge {badge} admitted OPERATION_QUIESCE"
        );
    }
    assert!(proto::admits(
        proto::SUPERVISOR_BADGE,
        proto::OPERATION_QUIESCE
    ));
    for operation in 0..=u8::MAX as u64 {
        if operation != proto::OPERATION_QUIESCE {
            assert!(
                !proto::admits(proto::SUPERVISOR_BADGE, operation),
                "the supervisor's badge admitted operation {operation}"
            );
        }
    }
    // Distinct from the two client badges, so no client copy is the supervisor's.
    assert_ne!(proto::SUPERVISOR_BADGE, proto::RAW_ONLY_BADGE);
    assert_ne!(proto::SUPERVISOR_BADGE, proto::STDIN_BADGE);
    assert_ne!(proto::SUPERVISOR_BADGE, 0);
}

/// The unbadged surface is exactly the client operations, an allowlist like every other badge's,
/// and the two badged client surfaces are exactly what their constants say; any other badge is
/// served nothing.
#[test]
fn each_badge_admits_exactly_its_surface() {
    for operation in 0..=u8::MAX as u64 {
        assert_eq!(
            proto::admits(0, operation),
            matches!(
                operation,
                proto::OPERATION_WRITE
                    | proto::OPERATION_READLINE
                    | proto::OPERATION_BYTES
                    | proto::OPERATION_INTRCOUNT
                    | proto::OPERATION_SUSPEND_COUNT
                    | proto::OPERATION_PRINT
                    | proto::OPERATION_RAWMODE
                    | proto::OPERATION_READRAW
            ),
            "the unbadged surface on operation {operation}"
        );
    }
    for operation in 0..=u8::MAX as u64 {
        assert_eq!(
            proto::admits(proto::RAW_ONLY_BADGE, operation),
            matches!(
                operation,
                proto::OPERATION_RAWMODE | proto::OPERATION_READRAW
            ),
            "the raw-only badge on operation {operation}"
        );
        assert_eq!(
            proto::admits(proto::STDIN_BADGE, operation),
            operation == proto::OPERATION_READ_LINE_BYTES,
            "the stdin badge on operation {operation}"
        );
        assert!(
            !proto::admits(4, operation),
            "badge 4 on operation {operation}"
        );
    }
}
