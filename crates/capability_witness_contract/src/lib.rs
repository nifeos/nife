//! **The words the capability demonstrations agree on**, and nothing else. Each demonstration is a
//! pair of user programs and an assertion in `kernel/src/user`, and what two programs share is a
//! crate (AGENTS.md rule 7). Everything but [`process_composition`]'s verdict bits is a
//! **sentinel**, a value chosen only to be recognised: none is a wire format, and changing one
//! costs a rebuild, not a migration.
//!
//! # Bugs
//!
//! Nothing checks that a reader of a constant is one of the two parties that agreed on it; a third
//! program reading `USED_WORD` for another purpose would break silently on the next change.
//!
//! Name: ratified 2026-09-18 (calef): named for what it proves, as `process_composition_witness`
//! is, since a sentinel recognised by a second program is a witness value. Refused
//! `capability_demo_protocol` (`demo` says why the code exists, not what it is), `demo_words` (the
//! representation, not the agreement), `capability_demo` (reads as the demo itself), and folding
//! these into `crates/abi` (fixture constants have no business widening the syscall surface).

#![no_std]

/// The word the frame producer writes into a page it owns, and the consumer, a separate process,
/// reads back through its own mapping: the whole evidence that one physical page is under both.
///
/// # Examples
///
/// ```
/// use capability_witness_contract::PAGE_FRAME_SENTINEL;
///
/// // The producer, in its own address space.
/// let mut page = [0u64; 512];
/// page[0] = PAGE_FRAME_SENTINEL;
///
/// // The consumer, through its own read-only mapping of the same physical frame.
/// let shared: &[u64] = &page;
/// assert_eq!(shared[0], PAGE_FRAME_SENTINEL, "the page was not shared");
///
/// // And a frame that was merely retyped, never written through, does not look like one.
/// let fresh = [0u64; 512];
/// assert_ne!(fresh[0], PAGE_FRAME_SENTINEL);
/// ```
pub const PAGE_FRAME_SENTINEL: u64 = 0xF00D_CAFE_D00D_1234;

/// The word the delegation receiver sends back through the capability it was *delegated*, so that
/// whoever holds the other end can confirm a capability minted by one process carries real
/// authority when a different process invokes it.
///
/// `system_tests/src/user/delegation_service.rs` re-exports this.
pub const USED_WORD: u64 = 0x5A;

/// The word the cycle-counter reader reports when it read the counter without being killed for it.
///
/// **The word is the grant's result.** An ungranted read of `PMCCNTR_EL0` or the `cycle` CSR
/// traps, and this kernel turns that into a fault that ends the thread, so a program that gets as
/// far as sending anything is a program the grant reached. Word 1 is the difference of two reads
/// taken across a yield, and word 2 is what they count, an `abi::cycle_counter::CycleMeaning` as
/// its word (milestone 353 (the aarch64 half of 74)). The kernel's test checks the meaning on every architecture, and that
/// the difference moved forward only where the kernel itself says the counter runs (milestone 74 (cycle counters);
/// the reasoning is on the test).
pub const CYCLE_COUNTER_WORD: u64 = 0xC1C1E;

/// The word `report_child` sends on the one capability `report_child_builder` granted it.
///
/// The builder parsed the child's ELF and built its process in userspace, so this word reaching
/// the kernel's test is the evidence that the build produced a running thread.
///
/// Name: provisional (milestone 405 (the nine init roles and the entry the kernel picks)), after
/// the program that sends it.
pub const REPORT_CHILD_WORD: u64 = 0xC0FFEE;

/// The word `interrupt_child` sends once the interrupt `interrupt_child_builder` delegated to it
/// has arrived as a message. Any fixed value would do; the child sends nothing before the
/// interrupt.
///
/// Name: provisional (milestone 405), after the program that sends it.
pub const INTERRUPT_CHILD_WORD: u64 = 0x1590;

/// The line `console_server_builder` writes into the page it shares with the console server it
/// built. The server prints it and acknowledges its length, and the kernel's test asserts that
/// length.
///
/// Name: provisional (milestone 405), after the program that writes it.
pub const CONSOLE_SERVER_BUILDER_LINE: &[u8] =
    b"nife: the console server was built and started by a userspace builder.\n";

/// The input `least_authority_demo_builder` starts its child with, in `x1`. The child reports its
/// square, so the kernel's test can tell the argument crossed `START` from a child that echoed it.
///
/// Name: provisional (milestone 405), after the program that passes it.
pub const LEAST_AUTHORITY_DEMO_INPUT: u64 = 7;

/// **The verdict `process_composition_witness` reports**, one bit per step, so a failure says which
/// step stopped (milestone 404 (composing a process from two capabilities is proved for two verbs
/// and no more), building §185 (what carries the claim that userspace composes a process from an
/// authority you can count on one hand)).
///
/// Not a sentinel like the words above: a bit layout the fixture writes and the kernel's test reads,
/// which is the same rule 7 reason to live here.
///
/// Name: provisional (the §185 build lane, 2026-10-05), with every constant in it.
pub mod process_composition {
    /// The child's ELF was found in the archive by name and parsed.
    pub const CHILD_FOUND: u64 = 1 << 0;
    /// An address space was retyped from the witness's budget and the child laid into it: segments,
    /// stack, a thread control block and its one capability.
    pub const SPACE_BUILT: u64 = 1 << 1;
    /// A frame of the witness's own was mapped into that space at a fresh address.
    pub const FRAME_MAPPED: u64 = 1 << 2;
    /// Mapping the same address again was refused: break-before-make holds in a space a process built.
    pub const DOUBLE_MAP_REFUSED: u64 = 1 << 3;
    /// The thread was configured at the child's entry, in that space, and started.
    pub const STARTED: u64 = 1 << 4;
    /// The child's word arrived on the rendezvous the witness minted for it, and it was the input
    /// squared: the thread ran, in the space it was built in, with the capability it was given.
    pub const CHILD_ANSWERED: u64 = 1 << 5;
    /// Every step.
    pub const WHOLE: u64 =
        CHILD_FOUND | SPACE_BUILT | FRAME_MAPPED | DOUBLE_MAP_REFUSED | STARTED | CHILD_ANSWERED;
}

#[cfg(test)]
mod tests {
    use super::process_composition::*;

    /// **Each step owns one bit of its own**, and [`WHOLE`] is all of them, so a verdict short of
    /// `WHOLE` names exactly the steps that did not happen. A step at zero, or two steps on one bit,
    /// would make a stopped composition read as a finished one.
    #[test]
    fn every_step_is_its_own_bit_and_whole_is_all_of_them() {
        let steps = [
            CHILD_FOUND,
            SPACE_BUILT,
            FRAME_MAPPED,
            DOUBLE_MAP_REFUSED,
            STARTED,
            CHILD_ANSWERED,
        ];
        let mut seen = 0;
        for step in steps {
            assert_eq!(step.count_ones(), 1, "{step:#x} is not one bit");
            assert_eq!(seen & step, 0, "{step:#x} is shared with an earlier step");
            seen |= step;
        }
        assert_eq!(WHOLE, seen);
    }
}
