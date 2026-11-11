# P2: later

An appendix to [the milestone](../1075-abi-outside-review.md).

## P2: after the move, before publication

14. `Error` implements neither `core::fmt::Display` nor `core::error::Error`.
    Finding: `src/lib.rs:292` derives `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq` and nothing else.
    `core::error::Error` is stable and `no_std`. Without it the enum cannot sit behind `?` into a boxed error or an `anyhow`-style type, which is the first thing a host-side test harness tries.
    `Hash` is missing too.
    Fix: derive `Hash`; `impl Display` with the variant's one-line meaning; `impl core::error::Error`.
    Effort: S. Door: two-way (additive).

15. `CapabilitySlot` has no user.
    Finding: `src/lib.rs:92` defines the alias; nothing in the tree names it, and the crate's own
    method docs write `u64`.
    Fix: use it in the root's `invoke` signature and the wrappers' docs, or delete it.
    Effort: S. Door: two-way.

16. `uart::RegisterShape::new(reg_shift, reg_io_width)` keeps a foreign spelling without its form.
    Finding: the parameters (`src/uart.rs:38`) carry the device-tree binding's names; `reg`
    abbreviates "register", and the naming rules apply to parameters. The rule for a kept foreign name wants the doc to open with the full name and source and the item to carry `#[doc(alias)]`. The module doc names the bindings, but the function does not.
    Fix: `register_shift` and `access_width_bytes`, with `#[doc(alias = "reg-shift")]` and
    `#[doc(alias = "reg-io-width")]` on the constructor; or the foreign-name form as written.
    Effort: S. Door: two-way.

17. `cycle_counter::CycleMeaning` is not a syscall contract.
    Finding: `src/lib.rs:222` to `229` say no syscall carries it; it is a message word a program
    sends a test harness, keyed to what a runtime wrapper returns. In a crate whose claim is "the
    numbers a program and a kernel agree on", a stranger reads it as a method and looks for one.
    Fix: decide whether it moves to the runtime crate or the witness contract that reads it, or stays with one sentence saying it is a message-word convention and not a method.
    Effort: S. Door: two-way.

18. Doc nits.
    Finding: `src/page_frame.rs:27` to `28` repeat "so a slice escapes its source's revoke" twice in one sentence. `src/memory_region.rs:52` has a space before its period. `src/survey.rs:64` and `:80` have "provisional. an architect" with a lowercase sentence start. `src/lib.rs:66` to `67` run "freeing the slot" into "Deleting an empty slot" with no period. `Cargo.toml:14` keeps a
    `cfg(kani)` allowance for a crate the README says has no harness.
    Fix: as stated.
    Effort: S. Door: two-way.

19. `Name:` blocks quote the decider.
    Finding: `src/interrupt.rs:3`, `src/virtio.rs:5` and `:13`, `src/thread_control_block.rs:55`
    and `:66`, and `src/rendezvous.rs:111` record a ruling as a quotation. A record states the
    decision and its date and never quotes. The move strips every block, so this is nife's to fix
    and not the new repository's.
    Fix: restate each as the decision.
    Effort: S. Door: two-way.

20. The six mutation survivors have no per-mutant record.
    Finding: the baseline row reads 67 caught, 6 missed, 8 unviable; the dry run carries the row
    into `.github/mutants-exempts` as a comment because the census did not record which six. The triage notes name three: `|` as `^` in `futex::admitted` (disjoint bits), `1 << 0` as `1 >> 0` in `rights::READ`, and an `is_core_cycles` arm. This review found the other three named nowhere, so they stay unverified.
    Fix: after the first weekly run in the new repository, one `mutants-exempts` line per survivor
    with its reason; a survivor without an equivalence argument gets a test instead. Item 11's
    counting test kills the `rights` class outright.
    Effort: S. Door: two-way.

21. No machine checks it, and the proofs job's behaviour on zero harnesses stays unverified.
    Finding: the README says so plainly. `timer::counter_ticks_for` is the one function with
    arithmetic; the recorded Kani attempt with a symbolic rate did not finish. A bounded harness
    over concrete rates and `seconds < 2^20` may finish where the symbolic divide did not; the tests
    pin the boundaries today. Whether `cargo kani` on a crate with no harness exits green is not
    verified here; `falsify.py --check` passes.
    Fix: try the bounded harness once and record the result either way. If the proofs job needs a harness to be green, that is the template's to say, and the README's "What is proven" stays honest.
    Effort: M. Door: two-way.
