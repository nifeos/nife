use super::*;

#[test]
fn every_opcode_is_distinct_and_fits_its_field() {
    // Two verbs sharing a number is a wire bug the packing tests cannot see: each would pack and
    // unpack perfectly and mean the wrong thing. Cheap to assert, so assert it.
    let operations = [
        ("OPEN", fs::OPEN),
        ("READ", fs::READ),
        ("WRITE", fs::WRITE),
        ("CLOSE", fs::CLOSE),
        ("FSTAT", fs::FSTAT),
        ("CREATE", fs::CREATE),
        ("TRUNCATE", fs::TRUNCATE),
    ];
    for (i, (na, a)) in operations.iter().enumerate() {
        assert!(*a <= 0xff, "{na} does not fit the 8-bit opcode field");
        assert_ne!(
            *a, 0,
            "0 is not a verb: a zeroed word must not decode as one"
        );
        for (nb, b) in &operations[i + 1..] {
            assert_ne!(a, b, "{na} and {nb} share an opcode");
        }
    }
    // The blk protocol is a separate namespace on a separate endpoint, so its numbers may and do
    // overlap. Asserted so nobody "fixes" the overlap and renumbers a live wire.
    assert_eq!(
        blk::READ,
        fs::OPEN,
        "the two protocols are deliberately independent"
    );
}

#[test]
fn errors_are_negated_errnos_and_invert() {
    // The boundary convention: -errno on the wire, invertible, and successes read as no error.
    assert_eq!(reply_err(2), -2); // ENOENT
    assert_eq!(reply_err(5), -5); // EIO
    assert_eq!(reply_errno(reply_err(9)), Some(9)); // EBADF round trips
    assert_eq!(reply_errno(0), None);
    assert_eq!(reply_errno(4096), None); // a byte count is a success, not an error
}

#[test]
fn the_error_range_is_bounded_at_both_ends() {
    // The bounds: -1 and -MAXIMUM_ERRNO are the first and last errors.
    assert_eq!(MAXIMUM_ERRNO, 4095);
    assert_eq!(reply_errno(-1), Some(1));
    assert_eq!(reply_errno(-4095), Some(4095));
    // One past the bound is a protocol violation, and so is the word with no negation.
    assert_eq!(reply_errno(-4096), Some(EPROTO));
    assert_eq!(reply_errno(i64::MIN), Some(EPROTO));
    assert_eq!(reply_errno(-5_000_000_000), Some(EPROTO)); // once a truncated 705_032_704
    assert_eq!(EPROTO, 71);
    // Success never reads as an error, at its own bounds.
    assert_eq!(reply_errno(0), None);
    assert_eq!(reply_errno(i64::MAX), None);
    // The sender's end of the same bounds.
    assert_eq!(reply_err(1), -1);
    assert_eq!(reply_err(MAXIMUM_ERRNO), -4095);
}

#[test]
#[should_panic(expected = "1..=MAXIMUM_ERRNO")]
fn a_sender_cannot_send_zero_as_an_error() {
    // Zero would be a success on the wire.
    let _ = reply_err(0);
}

#[test]
#[should_panic(expected = "1..=MAXIMUM_ERRNO")]
fn a_sender_cannot_send_a_code_past_the_bound() {
    let _ = reply_err(MAXIMUM_ERRNO + 1);
}

#[test]
#[should_panic(expected = "1..=MAXIMUM_ERRNO")]
fn a_sender_cannot_send_a_negative_code() {
    let _ = reply_err(-2);
}
