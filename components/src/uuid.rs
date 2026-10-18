//! **`uuid`**: print a version-4 UUID drawn from the entropy service (notes/entropy.md). Ask the
//! service for sixteen bytes, stamp the six bits RFC 9562 reserves
//! (`universally_unique_identifier::Uuid::v4_from_random`), print the canonical form in RFC order.
//! It is `disk_partitioner`'s draw with the disk taken away, and `date`'s shape one manifest field
//! over ([`grant_plan::Manifest::entropy`] rather than `Manifest::clock`): the part of randomness a
//! person at the prompt can reach.
//!
//! | slot | what | why it is that and not wider |
//! |---|---|---|
//! | 0 | the output sink, `WRITE` | where the identifier goes |
//! | 8 | the diagnostics sink, `WRITE` | where the refusal goes, so `>` cannot swallow it |
//! | 9 | the entropy service, `WRITE` | the right to `CALL` it, and nothing else |
//!
//! `READ` on slot 9 would let it take another client's request off the service's endpoint, and
//! `GRANT` would let it hand a random source on; neither is given.
//!
//! **Without the entropy grant it prints nothing** and says why on its second stream:
//! `entropy_contract::delivered` reports the empty slot as `None`, and `uuid > id.txt` must leave
//! the file empty, because a predictable identifier is wrong and looks right. No counter fallback,
//! as in `disk_partitioner` and the partition table. It takes no arguments; there is no argv on
//! this ABI (notes/abi.md).
//!
//! ```text
//! uuid                     A4E1B0C7-2F3D-4A81-9C6E-5B7D0F2A1E44
//! uuid > id.txt            (the identifier is in the file, nothing on the terminal)
//! uuid  (no entropy)       uuid: no entropy capability was granted; nothing was generated
//! caps uuid                cap 9  endpoint  entropy  WRITE. it may ask the entropy service ...
//! ```
//!
//! # BUGS
//!
//! - One identifier per invocation, until `ProgramArgumentDeclaration` has positional arguments.
//! - Version 4 only; v1, v3, v5 and v7 need a hash or a clock this program does not hold.
//! - "Unpredictable" is a claim about the boot: on QEMU the bytes come from a host-backed
//!   virtio-rng (§120 (a QEMU-only virtio-rng stopgap)), and a real TRNG's soundness is open.
//! - Two calls of eight bytes; a short draw is a failure, not retried.
//!
//! Name: ratified 2026-09-13 (calef): RFC 9562's term for the object. Refused `uuidgen` (it
//! promises an argument surface this ABI cannot deliver) and `guid` (a GUID is mixed-endian on
//! disk, a byte order this program does not produce).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and documenting an
// OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use universally_unique_identifier::Uuid;
use user_mode_runtime::{exit, is_granted, send};

/// Slot 0: where the identifier goes. An endpoint with `WRITE`, under the sink contract.
const REPORT: u64 = 0;

/// Slot 8: the declared second stream (DECISIONS §67). The refusal goes here so a `>` on the line
/// cannot swallow it into a file that then looks like a successful run.
const DIAG_SLOT: u64 = grant_plan::DIAGNOSTICS_SLOT;

/// Slot 9: the entropy service, `WRITE`. Its *presence* is what the draw finds out about, and it
/// finds out by asking rather than by probing: a `CALL` on an empty slot answers
/// `abi::Error::NoSuchSlot`, which `entropy_contract::delivered` separates from every real count.
const ENTROPY_SLOT: u64 = grant_plan::ENTROPY_SLOT;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    let has_diag = is_granted(DIAG_SLOT);

    // **Draw first, print second**, which is the ordering the refusal forces rather than a style:
    // this program must write nothing at all when it holds no entropy, so nothing may go out
    // before the draw has answered. §67's reader also drains the second stream to end-of-stream
    // before it reads a byte of output, so the complaint has to be finished before the identifier
    // starts.
    let mut random = [0u8; 16];
    let drawn = user_mode_runtime::entropy::fill(ENTROPY_SLOT, &mut random).map(|()| random);

    if drawn.is_none() {
        write_on(
            if has_diag { DIAG_SLOT } else { REPORT },
            b"uuid: no entropy capability was granted; nothing was generated\n",
        );
    }
    if has_diag {
        send(DIAG_SLOT, byte_sink_contract::eof(), 0, 0);
    }

    if let Some(bytes) = drawn {
        let mut line = [b'\n'; 37];
        line[..36].copy_from_slice(&Uuid::v4_from_random(bytes).to_ascii());
        write_on(REPORT, &line);
    }

    send(REPORT, byte_sink_contract::eof(), 0, 0);
    exit();
}

/// Write bytes to an endpoint under the sink contract, sixteen at a time. No newline is added:
/// where a line ends is the caller's business and not the transport's.
fn write_on(slot: u64, bytes: &[u8]) {
    let mut rest = bytes;
    while !rest.is_empty() {
        let (w0, w1, w2, n) = byte_sink_contract::pack(rest);
        send(slot, w0, w1, w2);
        rest = &rest[n..];
    }
}

user_mode_runtime::panic_handler!();
