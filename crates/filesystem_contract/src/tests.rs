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
