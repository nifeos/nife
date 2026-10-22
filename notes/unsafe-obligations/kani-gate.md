# The gate over cfg(kani), and the measurement that chose it (milestone 113)

An appendix to [notes/unsafe-obligations.md](../unsafe-obligations.md), which summarizes it. Its
BUGS item 2 is the finding this page closes. Milestone 113 (the proofs' own unsafe code is ungated)
closed it.

Two candidates, and the brief said to measure before arguing. The measurement is one-sided enough
that there was nothing left to argue about.

| | clippy with `--cfg kani` | `-D warnings` on `script/verify` |
|---|---|---|
| Undocumented `unsafe` it finds | **13** | **0** |
| Other warnings it finds | **13** | 0 |
| Needs Kani installed | no | yes |
| Runs | every pull request, ~1 s | when someone runs the proofs, ~20 min |
| Compiles the harnesses truthfully | no, against a shim | yes, by definition |

Why the second column is zero, which is the whole decision. `cargo kani` drives a *rustc*, not a
clippy-driver. `undocumented_unsafe_blocks` is a `clippy::` lint and simply does not exist in that
compiler, so no amount of `-D warnings` can make it fire. This was measured rather than reasoned
about: `RUSTFLAGS="-D warnings" cargo kani -p ipc --only-codegen` compiles clean while thirteen
undocumented unsafe sites sit in the file. (That is the command as it was run, when the crate was
`ipc`; it is `-p inter_process_communication` since the 2026-09-19 rename.) The same command
*does* fail on a deliberately added unused variable, so `RUSTFLAGS` reaches Kani and the gate would
be real for rustc lints (`unsafe_op_in_unsafe_fn` among them). It is only the clippy half,
which is the half this milestone is about, that it cannot reach.

So `script/lint` grew a fourteenth clippy configuration. The tree's `#[cfg(kani)]` modules are all in
`crates/`, so it is the host pass's package selection with three flags added:

```sh
cargo clippy --workspace --exclude kernel --exclude user --exclude user_mode_runtime --all-targets -- \
    --cfg kani --extern kani=target/kani-lint-shim/libkani.rlib -L target/kani-lint-shim -D warnings
```

## The shim, and what it does not promise

`--cfg kani` alone does not compile: the harnesses are written against Kani's intrinsics, and
without the crate that provides them rustc stops at `use of unresolved module or unlinked crate
kani`. `helpers/kani-lint-shim/` is that crate, built by `script/lint` with two plain `rustc`
invocations before the pass runs. The surface is small, which makes this cheap: across 36 packages <!--count:harness-crates--> the tree uses exactly five Kani items, `any`, `proof`,
`assume`, `unwind` and `cover!`, and no `Arbitrary` derive, no contracts, no
`any_where`. A sixth, `stub`, appears only in `kernel`, which this pass excludes, so the shim
lacks it. Those five items are what the shim has to cover, and they do not move when a harness is
added.

It is two crates because an attribute macro can only come from a proc-macro crate. The
one-crate route was tried and does not work: registering `kani` as a tool namespace with
`-Zcrate-attr=register_tool(kani)` loses to the extern crate the same code needs for `kani::any`, and
rustc reports `cannot find proof in kani`.

It is deliberately looser than Kani in one place. The real `any` requires `T: Arbitrary`; the
shim's takes any `T`. A lint gate must never reject code the model checker accepts, and the error
that remains possible (code only the *shim* accepts) fails under `cargo kani`, loudly, where anybody
would look.

A clean pass here is not a proof, and the shim is not a second implementation of Kani. It has no
semantics at all: `any` returns nothing, `assume` constrains nothing. `script/verify` remains the
thing that proves.

When a harness reaches for Kani API the shim lacks, the lint pass breaks and the proof does not.
The failure is a compile error naming the missing item, and the fix is to add the item, not to drop
the pass.

## What it found, and the correction to the parent's count

26 warnings in 9 crates, none of which any gate had ever printed.

Thirteen are the unsafe half, and the number in the parent's BUGS item 2 was 11, which was an undercount. The
survey enumerated `unsafe {}` blocks; `undocumented_unsafe_blocks` also fires on an `unsafe impl`,
and there are two of those under `#[cfg(kani)]`, one in each crate, both undocumented. Counting by
hand found the population the lint's own rule would have found for free, which is the argument for
gates in one line.

| Crate | Sites | Shape |
|---|---|---|
| `inter_process_communication` | 11 blocks + 1 `unsafe impl` | the harness's `seed`, and every call into `send`/`receive` |
| `intrusive_fifo` | 1 `unsafe impl` | `Node for N` in the proof module (its two blocks were already commented) |

The other thirteen are ordinary clippy, in crates nobody suspected: `doc_markdown` (4),
`manual_range_contains` (4), `manual_let_else` (2), `len_zero` (2), `needless_range_loop` (1),
`assertions_on_constants` (1), across `asid`, `calendar`, `credential_contract`, `nifefs`, `dma_validator`,
`paging`, `pci` and `generational_table`. That half is the answer to "does this find anything besides unsafe",
and it is yes: half of what the pass finds has nothing to do with unsafe at all. One of them,
`dma_validator`'s `assert!(RING_END <= RING_BLOCK)` over two constants, became a `const {}` assertion
and so moved from a proof-time check to a compile-time one.

All 26 are fixed. Every proof in the eight crates whose harness code changed was re-run and still
passes.

## Writing the eleven comments, which was the point of doing the gate first

DECISIONS §61 records why a generated pass is the wrong instrument here: the lint checks that a
comment exists, never that it is true, so a false comment passes the gate and misleads a reader who
now believes somebody checked. The eleven are worth reading as an example of the alternative.

Every `unsafe` call in `inter_process_communication`'s proof module discharges the same two
obligations, and they are stated once in the module's own doc rather than eleven times. The first is
that the nodes outlive the endpoint: they are declared in one `let` before `e`, and locals drop in
reverse declaration order. The second is that no node is on a queue when it is passed: each
`N::new()` starts with a null link, and no harness hands the same node to two calls. The `#[cfg(test)]` module beside it had already chosen exactly this shape, which is why
its twenty-odd sites read as one argument and not twenty.

Each site's own comment then adds only what is particular to it, and the particulars are where the
real content is. `a_collected_sender_is_forgotten` carries a fourth node, `me2`, purely so its second
receive does not reuse `me`. `me` is provably not queued at that point. But a separate node makes
the site's obligation independent of that reasoning, and the comment says so rather than asserting
the conclusion. `send_rendezvous_iff_a_receiver_waited` takes `&mut r` once into a `receiver_ptr` it
keeps, so its comment records that no second pointer to `r` exists. `seed`'s two match arms are
exclusive, which is what makes "pushed at most once" true.

One warning fired only because the module doc grew: `mixed_attributes_style`, when the paragraph was
first written as `//!` inside a module that already had a `///` block above it. It belongs in the
outer doc.
