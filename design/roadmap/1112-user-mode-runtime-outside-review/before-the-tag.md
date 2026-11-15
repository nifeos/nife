# P1: before the tag

An appendix to [the milestone](../1112-user-mode-runtime-outside-review.md).

## P1: before the first tag

6. `cntfrq_checked` on aarch64 cannot return `None`, and a zero register divides by zero.
   Finding: the aarch64 `cntfrq` returns `CNTFRQ_EL0` raw and `cntfrq_checked` wraps it in `Some`
   unconditionally, with the doc "aarch64: never, the machine states it". Firmware may leave    the
   register at zero (unverified that such firmware is in use here; the Linux device-tree    binding
   for the architected timer carries a `clock-frequency` property for the case where    firmware did
   not program it). Then `monotonic_nanoseconds` divides by zero, which panics, which    traps, with
   no message. The other two architectures treat an unknown rate as `None`; aarch64 should too.
   Evidence: `src/clock.rs` lines 44 to 59 and 304 to 311.
   Change: `cntfrq_checked` returns `None` when the register reads zero; aarch64's `cntfrq` gains
   the same `expect` as its twins. One sentence in the doc.
   Door: two-way. Effort: XS.

7. The aarch64 child stub hard-codes two `abi` constants the other two architectures derive.
   Finding: `SEND_THEN_EXIT` on aarch64 encodes `movz x1, #0` for `rendezvous::SEND` and
   `movz x8, #0` for `SYSCALL_EXIT` as literal words, while the riscv64 and `x86_64` arrays shift
   `abi::rendezvous::SEND` and `abi::SYSCALL_EXIT` into their immediates. A renumbering of either
   constant breaks one architecture silently. No test decodes any of the three arrays.
   Evidence: `src/child_stub.rs` lines 22, 26 and 28 against lines 38, 44 and 56 to 58.
   Change: `0xD280_0001 | ((abi::rendezvous::SEND as u32) << 5)` and
   `0xD280_0008 | ((abi::SYSCALL_EXIT as u32) << 5)`, and the decoding test of item 4.
   Door: two-way. Effort: XS.

8. The text speaks to a reader inside nife.
   Finding: the sources carry 38 `§` citations, 78 mentions of a milestone, 30 paths under
   `notes/`, 18 `DECISIONS` citations, and paths into `kernel/`, `fixtures/`, `components/`,
   `script/lint`, `helpers/` and `patches/`. The standalone check counts 35 milestone citation
   lines, 32 section citation lines, 33 lines with a path into the tree and one line naming the
   architect outside a `Name:` block (`src/virtio.rs` line 72). The `Name:` blocks, which carry
   the quotations and the dates, go at the move (the dry run strips all of them); the rest stays.
   A stranger following "see notes/user-mode-runtime.md" finds nothing. The two notes that explain
   the crate (337 lines) are the best documentation it has and are not in it.
   Evidence: every source file; `src/virtio.rs` line 72; `notes/user-mode-runtime.md` and
   `notes/user-mode-runtime/time.md`.
   Change: rewrite each doc comment to state the rule rather than cite the ruling: "the sixth word
   goes in as zero so `receive_fault` reads 0 unless the kernel stamped a label" needs no section
   number. Move the two notes into the repository's `docs/`, trimmed of tree paths, and point the
   crate root at them. Keep the kernel-side facts a reader needs (which register the kernel
   writes, what maps the timebase page) as statements, not as paths. Delete the one line naming
   the architect. The externalized-crates lint is at its ceiling of five, so this lane also lowers
   that ceiling to zero.
   Door: two-way. Effort: M.

9. Twenty-four acronyms are not expanded at first use.
   Finding: the template's acronym gate (ceiling zero, judged once per crate) reports `ABIs`,
   `EL0`, `EL1`, `EL2`, `CSR`, `MHz`, `TCG`, `HVF`, `TSC`, `TCB`, `GIC`, `TSS`, `ISA`, `COM1`,
   `PAL`, `SIGPIPE`, `UART`, `IPC`, `DMA`, `ELF`, `UDP`, `TSO`, `REX` and `SEND`. The last is a
   method name in prose and wants a code span, not an expansion.
   Evidence: the gate's output. First uses are at `src/lib.rs` line 8, `src/child_stub.rs` line 2,
   `src/clock.rs` lines 25, 39, 102, 175 and 271, and `src/devices.rs` lines 25, 52, 56 and 69.
   They are also at `src/entropy.rs` line 14, `src/inter_process_communication.rs` lines 80,    122
   and 136, `src/mapped_window.rs` line 36, and `src/process.rs` line 176. The rest are at
   `src/socket_frame.rs` line 68, `src/virtio.rs` line 68, and `src/child_stub.rs` lines 18 and 53.
   Change: expand each at its first use in the crate root or the README (the gate counts those
   for every file), or list the ones every reader knows in the repository's
   `helpers/prose_acronyms.toml`. Wrap `SEND` in backticks.
   Door: two-way. Effort: S.

10. `Frame::payload_read` trusts a reply length against the page, and `Frame` cannot read the
    length field it can write.
    Finding: `payload_read(n, out)` clamps `n` to `out.len()` and then copies from `PAYLOAD_OFFSET_BYTES`;
    an `n` above `DATA_MAXIMUM` with a buffer that large panics at the window's bounds check. The
    doc says "a peer's idea of a length is data", and the clamp is "the bounds check's first
    line", but it does not clamp to what the frame can hold. After #3517, `RECEIVE` is the only
    writer of `LENGTH_OFFSET_BYTES` and its doc says a client "clears it before a `RECEIVE` to watch for the
    reply"; `Frame` has `set_len` and no `len`, so the client it describes cannot watch.
    Evidence: `src/socket_frame.rs` lines 92 to 110; #3517's change to the `set_len` doc.
    Change: `let n = n.min(out.len()).min(DATA_MAXIMUM);` and `pub fn len(&self) -> u16` reading
    `LENGTH_OFFSET_BYTES`. `len` is the standard library's spelling for a length, which the naming rules
    prefer. The peer-word ruling (`ACCEPT`'s second reply word carries the peer's address and
    port) rides in the reply word, not the frame, so `Frame` gains no field for it. A decoder for
    that word belongs in `socket_contract`, where the host tests are. The doc of
    `dst_ip`/`dst_port` ("the sender's after a UDP `RECEIVE`") stays true.
    Door: two-way (additions). Effort: XS.

11. `INITIAL_RAMDISK_VIRTUAL_ADDRESS` is a literal here and a derived value in the kernel, and
    nothing checks they agree.
    Finding: this crate says `0x2000_0000`; the kernel says
    `address_space_map::runtime_window(0x2000_0000)`. The doc says "must match" and no test
    asserts it (none found by search; unverified beyond that). After the move the two live in
    different repositories.
    Evidence: `src/initial_ramdisk.rs` line 34; `kernel/src/user/loader.rs` line 297.
    Change: `address_space_map` leaves before this crate (milestone 1017 (the contract crates leave in this order), step 3), so depend on it
    and define the constant as the kernel does, from one source; or have the kernel import this
    crate's constant. One of the two, not both.
    Door: two-way (both sides are nife's). Effort: XS.
