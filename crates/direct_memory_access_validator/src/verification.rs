use core::cell::RefCell;

use super::*;

/// The queue size the proofs fix. Concrete so the bounded loops unroll to a known depth; it is
/// the kernel's real `QSIZE`, so the proof is over the configuration that ships.
const QS: u16 = 8;
const Q: usize = 8;

/// **The confinement predicate is sound: an accepted range is genuinely inside the region.**
/// For every base, size, addr, and len, if [`is_in_region`] says yes then the addition did not
/// overflow and `base <= addr` and `addr + len <= base + size`. This is the arithmetic the
/// whole boundary rests on, quantified over all 2^256 input combinations at once, and it also
/// proves totality (the hardened checked adds never panic on any input).
/// Falsification: replayable `crates/direct_memory_access_validator/falsifications/verification.in_region_is_sound.patch`
#[kani::proof]
fn in_region_is_sound() {
    let base: u64 = kani::any();
    let size: u64 = kani::any();
    let addr: u64 = kani::any();
    let len: u64 = kani::any();

    if is_in_region(base, size, addr, len) {
        let end = addr
            .checked_add(len)
            .expect("acceptance implies no addr+len overflow");
        let limit = base
            .checked_add(size)
            .expect("acceptance implies no base+size overflow");
        // **The containment sentence is on the two assertions rather than below them**
        // (milestone 307). There used to be a third line here, `assert!(addr >= base && end <=
        // limit)`, carrying the readable comment "no byte the device would touch lies outside
        // the granted region". It was the conjunction of the two above it, so it could not fail
        // under any defect: the readable statement of the claim was the unreachable one, which
        // is the shape milestone 305 found in DECISIONS §31 and in row 24 of
        // notes/confinement-claims.md. Deleting it loses nothing and moving its sentence up
        // means a reader who quotes this harness quotes something that fires.
        assert!(
            addr >= base,
            "an accepted range starts below the granted region",
        );
        assert!(
            end <= limit,
            "an accepted range ends past the granted region: a byte the device would touch \
             lies outside it",
        );
    }
}

/// **An accepted descriptor is confined and not indirect, in either direction.** For every
/// descriptor bit pattern (the `flags` field is fully symbolic, so the device-writable RX bit and
/// the device-readable TX shape are both covered) and every region, if [`is_descriptor_allowed`]
/// accepts it then it carries no indirect flag and its whole buffer is in-region. This is the
/// per-descriptor core of the walk, isolated so it is proved without any loop.
/// Falsification: replayable `crates/direct_memory_access_validator/falsifications/verification.an_accepted_descriptor_is_confined.patch`
#[kani::proof]
fn an_accepted_descriptor_is_confined() {
    let base: u64 = kani::any();
    let size: u64 = kani::any();
    let d = Desc {
        addr: kani::any(),
        word: kani::any(),
    };
    if is_descriptor_allowed(base, size, QS, d) {
        // **The indirect bit and the containment stated over the word** (milestone 211).
        // `is_descriptor_allowed` refuses on `d.is_indirect()` and on `!is_in_region(..)`, so a
        // harness phrased only through those two calls proves that the guard and the
        // assertion agree. `is_in_region` is pinned independently by `in_region_is_sound`
        // just above; `is_indirect` was pinned by nothing, and an `is_indirect` stuck at
        // false would let an indirect descriptor reach the device with this harness green.
        assert_eq!(
            d.word & ((F_INDIRECT as u64) << 32),
            0,
            "an indirect descriptor was accepted",
        );
        let end = d
            .addr
            .checked_add(d.word & 0xffff_ffff)
            .expect("an accepted descriptor's buffer cannot overflow");
        let limit = base
            .checked_add(size)
            .expect("acceptance implies the region has an end");
        assert!(
            d.addr >= base && end <= limit,
            "an accepted descriptor escapes the region"
        );
        // **The two assertions that used to sit here are gone** (milestone 307).
        // `assert!(!d.is_indirect())` and `assert!(is_in_region(base, size, d.addr,
        // d.buf_len()))` were the same two calls, with the same arguments, that
        // `is_descriptor_allowed` returns false on: pure restatements of the `if` guard just
        // passed, so neither could fail under any defect in this crate. They carried the two
        // readable messages while the milestone 211 assertions above did the work, which is
        // the citation failure this milestone swept for. The comment above records why the
        // blind phrasing existed; keeping it below the fix left a reader quoting an assertion
        // that could not fire.
    }
}

// Concrete, widely-separated descriptor-table bases so that decoding an address the walk
// generates is concrete arithmetic over symbolic *contents*. A descriptor's own `addr` field is
// symbolic and checked against the region; it is never decoded as a table address, so the
// separation is clean.
const DRIVER_DESC: u64 = 0x1_0000;
const DRIVER_AVAIL: u64 = 0x2_0000;
const SHADOW_DESC: u64 = 0x3_0000;
const SHADOW_AVAIL: u64 = 0x4_0000;

/// The symbolic memory the chain walk runs against: the driver descriptor table it reads (all
/// symbolic input) and the shadow table it writes (the device's view). The write closure checks
/// the confinement invariant *as each descriptor lands*, so a harness that runs the walk and trips
/// no assertion has proved: nothing out-of-region or indirect can ever reach the shadow, for any
/// driver input. The avail rings are not modeled here because [`shadow_one_head`] does not touch
/// them (the outer loop does; its bound is proved by `an_oversized_batch_is_refused`).
struct ChainMem {
    base: u64,
    size: u64,
    d_desc: RefCell<[Desc; Q]>, // driver descriptors (symbolic; RefCell so a proof can mutate them post-hoc)
    s_desc: RefCell<[Desc; Q]>, // shadow descriptors (written by the walk = device's view)
    s_written: RefCell<[bool; Q]>,
    /// A driver racing the walk: every read of its table returns a fresh symbolic value, as a
    /// driver on another core rewriting its descriptors between any two loads would. A fixed
    /// table is one behavior of such a driver, so this is strictly more general, and it is the
    /// only model under which a second fetch of the same descriptor can differ from the first.
    racing: bool,
}

impl ChainMem {
    fn symbolic() -> ChainMem {
        let mut d_desc = [Desc { addr: 0, word: 0 }; Q];
        let mut i = 0;
        while i < Q {
            d_desc[i] = Desc {
                addr: kani::any(),
                word: kani::any(),
            };
            i += 1;
        }
        ChainMem {
            base: kani::any(),
            size: kani::any(),
            d_desc: RefCell::new(d_desc),
            s_desc: RefCell::new([Desc { addr: 0, word: 0 }; Q]),
            s_written: RefCell::new([false; Q]),
            racing: false,
        }
    }
    fn racing() -> ChainMem {
        ChainMem {
            racing: true,
            ..ChainMem::symbolic()
        }
    }
    fn read64(&self, p: u64) -> u64 {
        if self.racing {
            return kani::any();
        }
        // The two words of driver descriptor `d`: desc + d*16 (+8 for the word half).
        let off = p - DRIVER_DESC;
        let d = (off / 16) as usize;
        let entry = self.d_desc.borrow()[d];
        if off.is_multiple_of(16) {
            entry.addr
        } else {
            entry.word
        }
    }
    fn write64(&self, p: u64, v: u64) {
        // A write to any address the subtraction cannot resolve (below the shadow table) would
        // trip Kani's overflow check here, which itself proves the walk only ever writes the
        // shadow, never back into the driver's region.
        let off = p - SHADOW_DESC;
        // And it never writes *past* its own queue's descriptor table. Stated as an assertion
        // rather than left implicit in the array bound below, because this is the multi-queue
        // half of the confinement: a write bounded by `16 * qsize` stays inside `DESC_OFF ..
        // AVAIL_OFF` of one queue's ring block, and `distinct_queues_occupy_disjoint_blocks`
        // proves the blocks do not overlap. Together: validating a NIC's receive queue cannot
        // touch its transmit queue's rings.
        assert!(
            off < 16 * QS as u64,
            "a shadow write ran past this queue's descriptor table",
        );
        let d = (off / 16) as usize;
        if off.is_multiple_of(16) {
            self.s_desc.borrow_mut()[d].addr = v; // addr half, written first
        } else {
            self.s_desc.borrow_mut()[d].word = v; // word half: the pair is now complete
            self.s_written.borrow_mut()[d] = true;
            // THE INVARIANT, checked the instant a descriptor becomes readable by the device:
            // every descriptor in the shadow is in-region and not indirect.
            let landed = self.s_desc.borrow()[d];
            assert!(
                !landed.is_indirect(),
                "an indirect descriptor reached the shadow the device reads",
            );
            assert!(
                is_in_region(self.base, self.size, landed.addr, landed.buf_len()),
                "a descriptor that escapes the DMA region reached the shadow the device reads",
            );
        }
    }
}

fn walk(mem: &ChainMem, head: u16) -> bool {
    shadow_one_head(
        mem.base,
        mem.size,
        DRIVER_DESC,
        SHADOW_DESC,
        head,
        QS,
        &|p| mem.read64(p),
        &|p, v| mem.write64(p, v),
    )
}

/// The main theorem: no descriptor chain can put out-of-region or indirect memory in front of the
/// device. Over a fully symbolic driver descriptor table and a symbolic region, walk a chain from
/// any valid head with the real [`shadow_one_head`], the confinement core of
/// [`validate_and_shadow`]; the shadow write closure asserts the invariant on every descriptor as
/// it lands. That covers both directions (the symbolic flags include the device-writable RX bit),
/// indirect descriptors, any chain the queue can hold, and no access past the table. One head loses
/// no coverage, because the outer loop only repeats it per head, bounded by
/// [`an_oversized_batch_is_refused`]. See notes/dma/validator.md.
/// Falsification: replayable `crates/direct_memory_access_validator/falsifications/verification.validate_and_shadow_confines_every_chain.patch`
#[kani::proof]
#[kani::unwind(10)]
fn validate_and_shadow_confines_every_chain() {
    let mem = ChainMem::symbolic();
    let head: u16 = kani::any();
    // The outer loop refuses `head >= qsize` before ever calling `shadow_one_head`, so the walk
    // runs only under this precondition; a head naming a real descriptor slot.
    kani::assume(head < QS);
    let _ = walk(&mem, head);
}

/// **An oversized batch is refused before a single descriptor is touched.** The available ring
/// holds only `qsize` slots, so a claim of more than `qsize` newly-available entries is malformed
/// or hostile, and walking it would spin the loop up to 65535 times under the caller's lock with
/// interrupts masked. For every `from, to` whose gap exceeds `qsize`, the full
/// [`validate_and_shadow`] returns false having read and written nothing: the memory closures
/// panic if called, so reaching one is a proof failure. This is the outer-loop bound the
/// single-head main theorem factors out.
/// Falsification: replayable `crates/direct_memory_access_validator/falsifications/verification.an_oversized_batch_is_refused.patch`
#[kani::proof]
fn an_oversized_batch_is_refused() {
    let from: u16 = kani::any();
    let to: u16 = kani::any();
    kani::assume(to.wrapping_sub(from) > QS);
    let ok = validate_and_shadow(
        kani::any(),
        kani::any(),
        DRIVER_DESC,
        DRIVER_AVAIL,
        SHADOW_DESC,
        SHADOW_AVAIL,
        from,
        to,
        QS,
        &|_| panic!("a descriptor was read before the batch-size guard refused the batch"),
        &|_| panic!("a descriptor was read before the batch-size guard refused the batch"),
        &|_, _| panic!("the shadow was written before the batch-size guard refused the batch"),
        &|_, _| panic!("the shadow was written before the batch-size guard refused the batch"),
    );
    assert!(!ok, "an oversized batch was accepted instead of refused");
}

/// A descriptor mutated after validation cannot reach the device. During the walk the driver's
/// table is read through [`ChainMem::racing`], so every load is a fresh symbolic value, as from a
/// driver rewriting descriptors on another core; a double fetch (validating one read, copying a
/// second) trips the invariant in [`ChainMem::write64`]. After a successful walk the driver's own
/// copy is aimed at an arbitrary address, and what the device reads from the shadow is unchanged.
/// It cannot see the crate's `BUGS`, a device reading the shadow between the two `write64`s of one
/// descriptor: the model has no concurrent reader. See notes/dma/validator.md.
/// Falsification: replayable `crates/direct_memory_access_validator/falsifications/verification.a_descriptor_mutated_after_validation_cannot_reach_the_device.patch`
#[kani::proof]
#[kani::unwind(10)]
fn a_descriptor_mutated_after_validation_cannot_reach_the_device() {
    let mem = ChainMem::racing();
    let head: u16 = kani::any();
    kani::assume(head < QS);

    if walk(&mem, head) {
        // A successful walk validated and copied the head descriptor, so the device will read it
        // from the shadow. Capture it; the write invariant already proved it is confined.
        let slot = head as usize;
        assert!(
            mem.s_written.borrow()[slot],
            "a successful walk did not shadow the head descriptor"
        );
        let device_sees = mem.s_desc.borrow()[slot];
        assert!(
            is_in_region(mem.base, mem.size, device_sees.addr, device_sees.buf_len()),
            "the device would read an out-of-region descriptor from the shadow",
        );

        // The driver now aims its OWN copy of that descriptor at an arbitrary address, after the
        // check. On async-DMA hardware this is the time-of-check/time-of-use race.
        mem.d_desc.borrow_mut()[slot] = Desc {
            addr: kani::any(),
            word: kani::any(),
        };

        // The device reads the shadow, which is untouched by that write: the mutation landed only
        // in the driver's own copy, which nothing reads.
        assert_eq!(
            mem.s_desc.borrow()[slot],
            device_sees,
            "a post-validation driver write reached the shadow the device reads",
        );
    }
}

// A region and a descriptor that always validate, for the harness below. Concrete on purpose:
// that harness isolates the *outer* loop's index arithmetic, and descriptor content is
// `validate_and_shadow_confines_every_chain`'s job. Making both symbolic at once would put
// `qsize` heads times `qsize` descriptors of symbolic reads in one formula, which is the batching
// blowup the main theorem factors out, for no coverage the two harnesses do not already give.
const OUTER_BASE: u64 = 0x8000_0000;
const OUTER_SIZE: u64 = 0x1000;

/// The outer walk stays inside both rings and terminates, for every index pair, wraparound
/// included. `from` and `to` are unconstrained, so a wrapped batch (`from = 0xffff, to = 0x0002`)
/// is covered, and every closure asserts its address: a 16-bit read is a `ring[slot]` load with
/// `slot < qsize` (a read below the ring underflows, which Kani flags), a 16-bit write is the
/// `avail.idx` publish or a `ring[slot]` mirror with `slot < qsize`, and a 64-bit access stays in
/// a `qsize`-entry table. `#[kani::unwind(11)]` is one more than the walk can need, so Kani proves
/// termination: a hostile `avail.idx` cannot hold the `DEVICES` lock with interrupts masked. See
/// notes/dma/validator.md.
/// Falsification: replayable `crates/direct_memory_access_validator/falsifications/verification.the_outer_walk_stays_inside_the_rings_and_terminates.patch`
#[kani::proof]
#[kani::unwind(11)]
fn the_outer_walk_stays_inside_the_rings_and_terminates() {
    let from: u16 = kani::any();
    let to: u16 = kani::any();

    // A symbolic available ring: each slot names an arbitrary head, in range or not.
    let mut ring = [0u16; Q];
    let mut i = 0;
    while i < Q {
        ring[i] = kani::any();
        i += 1;
    }

    let read16 = |p: u64| -> u16 {
        let off = p - DRIVER_AVAIL; // underflow here = a read below the ring: Kani catches it
        assert!(
            off >= 4,
            "a 16-bit read below the available ring's ring[] array"
        );
        assert!(
            (off - 4).is_multiple_of(2),
            "an unaligned available-ring read"
        );
        let slot = (off - 4) / 2;
        assert!(
            slot < QS as u64,
            "a read past the end of the available ring"
        );
        ring[slot as usize]
    };
    let read64 = |p: u64| -> u64 {
        let off = p - DRIVER_DESC;
        assert!(
            off < 16 * QS as u64,
            "a descriptor read past the end of the table"
        );
        // An in-region 8-byte buffer, no flags, so every chain is exactly one descriptor.
        if off.is_multiple_of(16) {
            OUTER_BASE
        } else {
            8
        }
    };
    let write16 = |p: u64, _v: u16| {
        let off = p - SHADOW_AVAIL;
        if off == 2 {
            return; // publishing the shadow's avail.idx, the last write
        }
        assert!(
            off >= 4,
            "a 16-bit write below the shadow ring's ring[] array"
        );
        assert!(
            (off - 4).is_multiple_of(2),
            "an unaligned shadow available-ring write"
        );
        assert!(
            (off - 4) / 2 < QS as u64,
            "a write past the end of the shadow available ring"
        );
    };
    let write64 = |p: u64, _v: u64| {
        let off = p - SHADOW_DESC;
        assert!(
            off < 16 * QS as u64,
            "a shadow descriptor write past the end of the table"
        );
    };

    let ok = validate_and_shadow(
        OUTER_BASE,
        OUTER_SIZE,
        DRIVER_DESC,
        DRIVER_AVAIL,
        SHADOW_DESC,
        SHADOW_AVAIL,
        from,
        to,
        QS,
        &read16,
        &read64,
        &write16,
        &write64,
    );

    // **Non-vacuity, machine-checked rather than argued.** An assertion-only harness passes
    // trivially if its inputs cannot reach the interesting states, and notes/verification.md names
    // that ("it proves what you asserted, not what you meant") as the main failure mode, not solver
    // bugs. `kani::cover!` inverts the question: it fails if the state is *un*reachable. So these
    // are the claim "this harness really does exercise wraparound and really does walk descriptors",
    // checked. Without them, "from and to are unconstrained, so the wrapped pairs are in there
    // somewhere" would be a reading of the code rather than a result.
    kani::cover!(to < from, "a wrapped batch (to < from) is reachable");
    kani::cover!(
        to < from && ok,
        "a wrapped batch is walked to completion, not merely refused"
    );
    kani::cover!(
        to != from && ok,
        "some batch is accepted, so the walk is not vacuously refusing everything"
    );
    kani::cover!(
        !ok,
        "some batch is refused, so the guards are reachable too"
    );
}

/// **Distinct queues occupy disjoint ring blocks** (milestone 30 multi-queue isolation). A NIC
/// drives receive on queue 0 and transmit on queue 1; each queue's rings live at `queue_block(q)`
/// in both the driver region and the shadow, so validating one queue never reads or writes
/// another's rings. For any two distinct in-range queues, the lower queue's whole ring area ends
/// before the higher queue's block begins, and a queue's ring area fits inside its own block.
/// Falsification: replayable `crates/direct_memory_access_validator/falsifications/verification.distinct_queues_occupy_disjoint_blocks.patch`
#[kani::proof]
fn distinct_queues_occupy_disjoint_blocks() {
    let q1: u16 = kani::any();
    let q2: u16 = kani::any();
    kani::assume((q1 as u64) < MAX_QUEUES);
    kani::assume((q2 as u64) < MAX_QUEUES);
    kani::assume(q1 < q2);
    // The lower queue's ring area ends at or before the higher queue's block begins: disjoint.
    assert!(queue_block(q1) + RING_END <= queue_block(q2));
    // And each queue's ring area fits within its own block, so the blocks fully contain them.
    // A `const` assertion, because both operands are constants: this is a fact about the layout
    // and not about `q1`/`q2`, so it is worth failing the build rather than a proof run.
    const { assert!(RING_END <= RING_BLOCK) };
    // The descriptor table a walk writes (`16 * qsize` bytes from `DESC_OFF`) ends at or before
    // the available ring begins. This is what makes the walk's own `off < 16 * qsize` write bound
    // (asserted in `ChainMem::write64`) add up to block isolation: a shadow write stays inside
    // `DESC_OFF .. AVAIL_OFF` of one block, and the blocks do not overlap, so no shadow write can
    // land in another queue's rings or in this queue's own available ring.
    assert!(DESC_OFF + 16 * LAYOUT_QSIZE as u64 <= AVAIL_OFF);
}
