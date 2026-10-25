# non_volatile_memory_express

The queue mechanics of an NVMe driver, as pure logic: register field decode, command building,
submission and completion ring arithmetic with the phase tag, doorbell addressing, PRP
construction and IDENTIFY namespace parsing, following NVMe 1.4. It is `no_std`, needs no
allocator and performs no I/O. You supply the volatile MMIO accessors, the DMA memory and the
timing (reset, enable, timeouts); this crate computes every value you write and checks every
value the controller hands back.

## What is proven

Eight Kani harnesses, in the `verification` module:

- `sq_push_stays_in_bounds`: a push onto any ring that is not full returns a slot inside the ring
  and leaves a legal tail.
- `cq_pop_stays_in_bounds_and_flips_only_at_the_wrap`: a completion pop keeps the head inside the
  ring, and the expected phase flips exactly once per lap.
- `distinct_doorbells_never_collide`: two different (queue, role) pairs never share a doorbell
  offset, for every stride a controller may report.
- `prp_pair_is_total_and_page_disciplined`: for any base, length and page size, the PRP pair
  either refuses or covers the transfer; the second pointer is zero when one page suffices, and
  otherwise the page boundary right after the base.
- `identify_parse_is_total_and_bounds_the_shift`: no controller response panics the IDENTIFY
  namespace parse, and every accepted LBA shift lies between 9 and 12, so a 4 KiB transfer covers
  1 to 8 logical blocks.
- `the_spawn_handoff_round_trips`: unpacking a packed `Handoff` with legal fields returns it
  unchanged.
- `an_accepted_handoff_keeps_its_doorbells_inside_the_mapped_page`: for any three words that
  unpack, the doorbells of queues 0 and 1 fall inside one 4 KiB page of BAR0.
- `a_transfer_command_is_only_built_for_a_block_the_namespace_has`: `Handoff::transfer_command`
  refuses any block whose last byte lies past the namespace's end, and never overflows.

Rerun them with `cargo kani`. Each harness has a falsification record at
`falsifications/<module.path>.<harness>.patch`; applying it with `patch -p3` from the crate root
makes that harness fail at its own assertion, which shows the proof checks something.

## Usage

```rust
use non_volatile_memory_express::{
    doorbell, prp_pair, Command, Completion, CqState, Doorbell, SqState,
};

let mut sq = SqState::new(64);
let mut cq = CqState::new(64);

// Read eight 512-byte blocks from LBA 0 of namespace 1 into a buffer that straddles a page.
let (prp1, prp2) = prp_pair(0x4000_0800, 4096, 4096).unwrap();
let read = Command::read(1, 1, 0, 8, prp1, prp2);
let slot = sq.push(); // write `read.0` into submission slot `slot`, then ring the tail:
assert_eq!(doorbell(1, Doorbell::SubmissionTail, 0), 0x1008); // write sq.tail() here
assert_eq!((slot, sq.tail()), (0, 1));

// Poll completion slot cq.head() until the controller's entry carries the expected phase.
let done = Completion::from_dwords([0, 1 | (1 << 16), 0, 1 | (1 << 16)]);
assert!(cq.is_owned(&done) && done.cid == 1 && done.status == 0);
sq.note_head(done.sq_head);
let head = cq.pop(); // write `head` to doorbell(1, Doorbell::CompletionHead, dstrd)
assert_eq!(head, 1);
```

## Limitations

- `SqState` assumes one command in flight per queue and panics when the ring would fill; it keeps
  no free-slot count.
- `prp_pair` covers at most two pages. A transfer that needs a PRP list returns `None`.
- `parse_identify_namespace` refuses LBA formats outside 512 bytes to 4 KiB per block.
  `lba_format_shift` reports the shift it refused.
- `Handoff`, the client windows and `ESCAPE_MAGIC` (behind the `confinement_attackers` feature)
  encode one kernel's split between an in-kernel admin plane and an unprivileged data plane, not
  NVMe itself; another kernel can ignore them.
- Not proven: the `Cap` decode beyond the doorbell stride, the status fields of
  `Completion::from_dwords`, and anything in time, which belongs to the caller.

The minimum supported Rust version is 1.88.

## License

Licensed under either of MIT ([LICENSE-MIT](LICENSE-MIT)) or Apache-2.0
([LICENSE-APACHE](LICENSE-APACHE)), at your option.
