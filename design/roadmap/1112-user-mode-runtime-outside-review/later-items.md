# P2: later

An appendix to [the milestone](../1112-user-mode-runtime-outside-review.md).

## P2: for later

12. `counter_frequency_contract` is an unused dependency on aarch64.
    Finding: cargo warns `unused dependency` on the aarch64 build; the manifest comment says the
    dependency is unconditional on purpose, but the warning is per build and the template runs
    clippy per target.
    Evidence: `Cargo.toml` line 26; the aarch64 `cargo check` output.
    Change: a `[target.'cfg(not(target_arch = "aarch64"))'.dependencies]` table, or a use on
    aarch64. Effort: XS. Door: two-way.

13. Parameter and item names the template's abbreviation gate and the naming rules flag.
    Finding: 53 components in 452 names are not words. The `[spelled_out]` list the gate reads
    names `va` (`virtual_address`), `val` (`value`), `num` (`number`), `src` (`source`) and `dst`
    (`destination`) directly; `aspace`, `tcb`, `sp`, `nr` and `ip` are abbreviations with no
    entry. Beyond the gate, six more names need attention.     First, `mapped_window::PAGE` holds
    4096 bytes and the units rule wants `_BYTES`. Second,     `cntfrq` and `cntfrq_checked` hold a
    rate in hertz under an ARM register mnemonic that two of     the three architectures do not have
    (28 callers). Third, `thread_control_block_start`'s     parameters are `x0`, `x1`, `x2`, aarch64
    register names on three architectures. Fourth,     `MappedWindow::sub` shares a spelling with
    `core::ops::Sub::sub` and means something else,     which the standard-library-names rule
    refuses. Fifth, `list` is one of the generic words the     naming rules name as a failure mode.
    Sixth, `virtio` is a product name the gate should     accept. The `r8` to
    `w64` family kept the migrated callers' vocabulary, as the note records.
    Evidence: `src/objects.rs` lines 22, 162, 194, 210 to 215, 241 to 246, 270. Also
    `src/syscall.rs` lines 23, 55, 88, and `src/virtio.rs` lines 31, 46. Also `src/devices.rs`
    lines 58, 72, and `src/mapped_window.rs` lines 37, 66, 145. Also `src/socket_frame.rs` lines
    69 to 90, `src/clock.rs` lines 45, 57, 72, 82, 126, 137, and
    `src/inter_process_communication.rs`     line 69.
    Change: the names review the move requires (every one of the 114 unratified names), with
    the names Decision 1 decides, provisionally. Door: two-way until the first tag; ratifying the
    names stays the architect's. Effort: M for the rename lane, after the ruling.

14. Two spellings of the empty-slot probe.
    Finding: `is_granted` invokes method `0xffff` and `retype_sleeper` invokes method `u64::MAX`,
    both to read `NoSuchSlot` against `BadMethod`. Both rely on every object type refusing an
    unknown method with `BadMethod`. Four dispatch arms in the kernel do. Nobody has verified
    whether every type does.
    Evidence: `src/objects.rs` lines 49 to 55; `src/inter_process_communication.rs` lines 301 to
    307.
    Change: `retype_sleeper` calls `is_granted`. One sentence in `is_granted`'s doc naming the
    kernel property it depends on. Effort: XS. Door: two-way.

15. Ten receive functions, three shapes for "a message or the bound notification's word".
    Finding: `receive`, `receive_badged`, `receive_badged_bound`, `receive_bound`,
    `receive_fault`, `receive_capability`, `receive_capability_badged`,
    `receive_capability_bound`, `receive_request` and `receive_request_bound`. The bound variants
    return `Received` (an enum), `Result<_, u64>` and `Result<Request, u64>` for the same
    distinction. `receive_capability_bound` has no caller in the tree. Each is one `invoke5` with
    a different projection of the same five words. The prior art returns one message record from
    one call.
    Evidence: `src/inter_process_communication.rs` lines 101 to 233 and 480 to 511.
    Change: the smaller surface, as Decision 3 decides, provisionally; a wrapper added later is
    compatible where one removed is not. Effort: M to L across callers.
    Door: two-way before the first tag.

16. The reason `socket_frame` gives for living here is stale.
    Finding: the module doc says `MappedWindow` "cannot compile for the host"; it does (item 4).
    The reason that holds is the dependency direction: a contract crate never links the runtime.
    Evidence: `src/socket_frame.rs` lines 5 to 7; `Cargo.toml` lines 31 to 35.
    Change: state the direction rule instead. Effort: XS. Door: two-way.

17. No wrapper for `SYSCALL_EXIT_THREAD`.
    Finding: `abi` defines it; this crate wraps `SYSCALL_EXIT`, `SYSCALL_YIELD`,
    `SYSCALL_CAPABILITY_DELETE` and `SYSCALL_INVOKE` and not the fifth number. Nobody has verified
    whether any     `no_std` program needs it. The std overlay may be its only caller.
    Evidence: `crates/abi/src/lib.rs` line 85; `src/process.rs`.
    Change: `exit_thread() -> !` beside `exit`, or a sentence saying why not. Effort: XS. Door:
    two-way.
