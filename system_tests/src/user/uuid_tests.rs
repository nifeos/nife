use super::*;
use crate::cap::{Rights, rendezvous_cap};
use crate::sched::RendezvousId;

/// Spawn `uuid` with **nothing but somewhere to print**, which is what a program that did not
/// declare [`grant_plan::Manifest::entropy`] is born holding: its whole cspace is slot 0, so
/// [`grant_plan::ENTROPY_SLOT`] is empty and a `CALL` on it can only be refused by the kernel
/// (`printenv_tests::spawn_printenv`'s `None` arm, one authority over).
///
/// There is no `Some` arm, as a limitation: `Spawn::grants` fills a table from slot 0 upward and
/// cannot place a capability at the slot a manifest names (the gap `xtask`'s `swish-check` list
/// records for `date`'s second stream). The endowed half is proven at the real prompt by
/// `script/swish-check`; see `design/roadmap/0111-entropy-for-a-child.md`.
fn spawn_uuid_holding_no_entropy(endpoints: u64) -> RendezvousId {
    let image = program("uuid").expect("no uuid program in the initial ramdisk");
    let out = crate::sched::create_rendezvous_from(endpoints).expect("no stdout rendezvous");
    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[rendezvous_cap(out, Rights::WRITE)],
                maps: &[],
            },
        )
    })
    .expect("could not spawn uuid");
    out
}

/// One message off the sink, as bytes, or `None` at end of stream.
/// `printenv_tests::line`'s reader with the framing left visible, because this test needs to see
/// **where the stream ends** and not only what a line said.
fn chunk(out: RendezvousId, buf: &mut [u8; 16]) -> Option<usize> {
    let words = crate::sched::ipc_receive(out);
    if words[0] == byte_sink_contract::eof() {
        return None;
    }
    let count = words[0] as usize;
    assert!(
        (1..=16).contains(&count),
        "uuid: a stdout message with a bad byte count: {count}"
    );
    buf[..8].copy_from_slice(&words[1].to_le_bytes());
    buf[8..].copy_from_slice(&words[2].to_le_bytes());
    Some(count)
}

/// **A program that was not endowed entropy draws none, prints no identifier, and says so.**
///
/// The half of milestone 111 (a shell that can endow a child with entropy) that carries the claim:
/// authority a program did not declare is not quietly available anyway. A process that draws a key
/// and one that hardcodes it make the same syscalls, so only taking the capability away tells them
/// apart, and the assertions are about silence: no byte of the 36-character identifier reaches the
/// sink, the stream is the one sentence, and the process ends normally. A `uuid` that fell back to
/// a counter, a boot-time seed or a tick fails, the fallback
/// `crates/globally_unique_identifier_partition_table` and `disk_partitioner` refuse. Arch-neutral
/// (DECISIONS §19 (architectural parity is a tenet)): every ISA runs this very test.
#[test_case]
fn a_process_granted_no_entropy_prints_no_identifier() {
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let out = spawn_uuid_holding_no_entropy(endpoints);

    let mut said = [0u8; 128];
    let mut len = 0usize;
    let mut buf = [0u8; 16];
    while let Some(n) = chunk(out, &mut buf) {
        assert!(
            len + n <= said.len(),
            "uuid with no entropy said more than a sentence"
        );
        said[len..len + n].copy_from_slice(&buf[..n]);
        len += n;
    }
    let said = &said[..len];

    assert_eq!(
        said, b"uuid: no entropy capability was granted; nothing was generated\n",
        "uuid with an empty entropy slot said something other than its refusal"
    );

    // And the shape of what it did not say. A version-4 identifier is 36 characters with four
    // hyphens in fixed places, so a single hyphen anywhere in this stream would mean bytes derived
    // from *something* got out. `entropy_contract::delivered` reading a kernel error as `None` rather
    // than as a short count is what makes that impossible, and this is the assertion that it holds
    // through the whole program rather than only at the accessor.
    assert!(
        !said.contains(&b'-'),
        "uuid with no entropy emitted identifier-shaped bytes"
    );
    crate::sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}
