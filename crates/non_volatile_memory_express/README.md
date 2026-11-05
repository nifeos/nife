# non_volatile_memory_express

The queue mechanics of an NVMe (Non-Volatile Memory Express) driver, as pure logic: register field
decode, command building, submission and completion ring arithmetic with the phase tag, doorbell
addressing, Physical Region Page (PRP) construction and IDENTIFY namespace parsing. It follows
revision 1.4 of the NVMe base specification, and every section number in this crate's
documentation is that revision's. It is `no_std`, needs no allocator and performs no I/O. You
supply the volatile MMIO (memory-mapped input/output) accessors, the DMA (direct memory access)
memory and the timing (reset, enable, timeouts); this crate computes every value you write and
checks every value the controller hands back.

A PRP entry is a physical address of one page of a transfer's data. A command carries two of them
(PRP1 and PRP2), and a longer transfer points PRP2 at a list of more entries, which this crate does
not build. An LBA (logical block address) numbers the fixed-size blocks of a namespace, which is
the unit of storage a controller exposes, from 0.

## What is proven

To run the proofs, install Kani as its
[installation guide](https://model-checking.github.io/kani/install-guide.html) says (the
`kani-verifier` crate, then `cargo kani setup` once), and run `cargo kani` from the crate's
directory. The harnesses pass on Kani 0.68.0. Eight harnesses live in the `verification` module:

- `sq_push_stays_in_bounds`: a push onto any ring returns a slot inside the ring and leaves a legal
  tail, or returns `None` and changes nothing, and it returns `None` exactly when the ring is full.
- `cq_pop_stays_in_bounds_and_flips_only_at_the_wrap`: a completion pop keeps the head inside the
  ring, and the expected phase flips exactly once per lap.
- `distinct_doorbells_never_collide`: two different (queue, role) pairs never share a doorbell
  offset, for every stride a controller may report.
- `physical_region_page_pair_is_total_and_page_disciplined`: for any base, length and page size,
  the pair either refuses or covers the transfer; the second entry is zero when one page suffices,
  and otherwise the page boundary right after the base.
- `a_transfer_command_is_only_built_for_a_block_the_namespace_has`: `Namespace::transfer_command`
  builds a command only for a block whose last byte lies inside the namespace, and never overflows
  computing its LBA, for any block number, buffer address and namespace size.
- `identify_parse_is_total_and_bounds_the_shift`: no controller response panics the IDENTIFY
  namespace parse, and every accepted LBA shift lies between 9 and 12, so a 4 KiB transfer covers
  1 to 8 logical blocks.
- `capabilities_decode_is_total_and_reads_the_specified_bits`: for any value of the capabilities
  register, every `Capabilities` field lies in the range its width allows and equals the bits at
  its place in the register.
- `completion_decode_is_total_and_rebuilds_dword_three`: for any four dwords, the command
  identifier, the phase tag and the 15-bit status field rebuild dword 3 exactly, so no bit is
  dropped, doubled or read from the wrong place.

Each harness has a falsification record at `falsifications/<module.path>.<harness>.patch`;
applying it with `patch -p3` from the crate root makes that harness fail at its own assertion,
which shows the proof checks something.

## Usage

```rust
use non_volatile_memory_express::{
    Command, Completion, CompletionQueue, Doorbell, SubmissionQueue, doorbell,
    physical_region_page_pair,
};

let mut submission_queue = SubmissionQueue::new(64);
let mut completion_queue = CompletionQueue::new(64);

// Read eight 512-byte blocks from LBA 0 of namespace 1 into a buffer that straddles a page.
let pages = physical_region_page_pair(0x4000_0800, 4096, 4096).expect("two pages suffice");
let read = Command::read(1, 1, 0, 8, pages);
// Write `read.0` into submission slot `slot`, then ring the tail:
let slot = submission_queue.push().expect("the ring has room");
assert_eq!(doorbell(1, Doorbell::SubmissionTail, 0), 0x1008); // write the tail here
assert_eq!((slot, submission_queue.tail()), (0, 1));
assert_eq!(read.0[1], 1);

// Poll completion slot `completion_queue.head()` until the controller's entry carries the
// expected phase.
let done = Completion::from_dwords([0, 1 | (1 << 16), 0, 1 | (1 << 16)]);
assert!(completion_queue.is_owned(&done) && done.cid == 1 && done.status == 0);
submission_queue.record_head(done.submission_queue_head);
// Write the new head to doorbell(1, Doorbell::CompletionHead, stride).
assert_eq!(completion_queue.pop(), 1);
```

## Limitations

- `SubmissionQueue` models one ring and keeps no count of commands in flight. `push` returns
  `None` when the ring is full, and the caller decides whether that is a bug or a reason to wait.
- `physical_region_page_pair` covers at most two pages. A transfer that needs a PRP list returns
  `None`.
- `parse_identify_namespace` refuses LBA formats outside 512 bytes to 4 KiB per block.
  `logical_block_address_format_shift` reports the shift it refused.
- Not proven: the meaning of the status code and status code type inside a completion's status
  field, and anything in time, which belongs to the caller.

The public structs that a later release may widen (`Completion`, `IdentifyNamespace` and
`Namespace`) are `#[non_exhaustive]`: build a `Namespace` with `Namespace::new`, and read the other
two from the controller's answer.

The minimum supported Rust version is 1.88.

## License

Licensed under either of MIT ([LICENSE-MIT](LICENSE-MIT)) or Apache-2.0
([LICENSE-APACHE](LICENSE-APACHE)), at your option.
