# Performing a rename: traps

*An appendix to [`design/naming.md`](../naming.md), which is the rule. This file holds the smaller
failures past renames hit: foreign identifiers, parser artefacts, stale pointers, broken word
boundaries, rewraps and generated files. It exists to verify or challenge the main page. A reader
who only needs to name, ratify or rename something should not have to open it. The directory
`design/naming/` and this file's stem are provisional names; naming is an architect's.*

## A neighbouring crate's identifiers are the hardware's, not yours

`crates/pci` holds `CLASS_NVME`, `PciNvmeDevice` and `find_nvme_device`, and none of them moved.
They name the PCI class code the specification defines (`01:08:02`), the same way `satp.ASID` and
`flush_asid` named a hardware field through the first rename under §154 (the acronym test). This is
the
[ownership test](rename-what-moves.md#the-ownership-test-does-it-keep-its-name-when-our-crate-is-deleted)
again: if the thing keeps its name when this tree's crate is deleted, the name is not this tree's to
change.

The same test disposes of the rest of that family in one pass. It is worth listing, because a
sweep's pattern matches every one of them: QEMU's `-device nvme`, the `NIFE_NVME` environment
variable, `target/nife-nvme.img`, and `clippy.toml`'s `doc-valid-idents` entry. The clippy entry is
the interesting one, because the instinct on a rename is to delete it. It exists so `doc_markdown`
tolerates the proper noun, and the proper noun is what survives the rename. Deleting it would turn
30 spec citations red for a word nobody renamed.

## A refusal the parser invented is not a refusal, and the count is why you leave it

`script/names --check` reports `non_volatile_memory_express` as "refused but live". It is neither
refused nor a bug in the rename. A refusal clause runs to the end of its sentence. That crate's
sentence refusing `nvm_express` names the winning spelling while arguing against the loser, so the
parser records the winner too. It is the `video_terminal` NOTE's shape without `video_terminal`'s
real reason behind it.

It was left alone deliberately, and the reasoning generalises. Rewording the sentence would have
moved the tree-wide refusal count, which is the one gate a performed rename is measured by.
Spending that signal to silence a NOTE that never fails a build is a bad trade. Record the artifact
beside the block instead, which is what that crate now does.

## A sweep can turn a stale pointer into a fabricated one

Contributed by the second performed rename (`address_space_builder` to `address_space_witness`,
2026-09-18), which found one site the sweep would have made worse rather than wrong.
`kernel/src/user.rs` carried ``See fixtures/src/hello.rs `address_space_builder()` ``. But
`hello.rs` has held no such function since milestone 291 (thirty-one programs wearing one name)
split the role out into its own fixture. The pointer was already stale. Nothing notices that,
because a prose reference resolves in a reader's head rather than in a compiler.

A sweep does not fix that and does not leave it alone; it upgrades it. Swept, the line would have
read `` hello.rs `address_space_witness()` ``: a symbol that has never existed anywhere, cited by
its current name, and so indistinguishable from a true reference. The stale version at least names
something that used to exist and can be traced. This is the internal cousin of the
foreign-identifier row in [the program-string
table](rename-where-names-hide.md#renaming-a-crate-is-compiler-checked-renaming-a-program-is-not).
It hides better, because there is no upstream tree to check it against.

So when a match is a *pointer* rather than a declaration, resolve it before rewriting it. The cost
is one grep per site, and it is not optional. The two neighbouring doc comments on the same kernel
module point into `hello.rs` for `ep_maker()`, `ep_user()`, `call_server()` and `call_client()`.
None of those are there either. They were left alone only because this rename did not touch them.

The seventh rename found the same shape at scale (`ipc` to `inter_process_communication`,
2026-09-19). §113 (eleven kernel object names move) renamed `Endpoint` to `Rendezvous` on 2026-08-23
and nothing moved the prose. So nine sites still said `ipc::Endpoint` a month later. A `ipc::` sweep
would have turned every one into `inter_process_communication::Endpoint`, a type that has never
existed. They were classified instead. The ones in decided sections are accounts, and kept their
words with the current name beside them. The ones in notes describing today's code were repointed to
`Rendezvous`. Reading those lines found two more pointers of the same age (`Rendezvous<Tid>`, and
`crates/intrusive` for a crate now called `intrusive_fifo`). A type rename leaves a trail of stale
prose, and the next crate rename walks straight into it.

## A tool that does not understand `\b` does not say so

Contributed by the `dtb` and `ipc` renames (2026-09-19), which each lost a count to it. On macOS,
`git grep` does not support `\b` in its default pattern syntax and matches nothing. So
`git grep -c '\bdtb\b'` reports zero across a tree with eighty-six hits in it. The macOS `sed` drops
`\b` the same way. A substitution meant to be word-bounded silently rewrites nothing, or with a
different pattern rewrites too much. Neither prints a warning.

Treat a zero as a claim to re-check, never as a result. Re-run it with `grep -rE` and an explicit
class (`(^|[^a-z_])ipc::`), or with `git grep -w` where a word match is what you want.

## A `Name:` block can move its own census

The `dtb` block described the crate's files as `.dtb` files, so the rename that wrote the block
counted it: 88 hits against a census of 86, and two phantom survivors to classify. The block now
says "the blob files' extension". When the after-census is off by a small number, check the
provenance block you just wrote before the tree.

## A hand rewrap needs a width check afterwards

Expanding a name lengthens lines, and every rename in this series rewrapped paragraphs by hand or by
script. Two failures were both invisible to the gates: lines left past the file's hundred columns,
and a list marker given a second space by a wrap script. After a rewrap, list the added lines longer
than the file's width (`git diff -U0 | grep '^+[^+]' | awk 'length > 101'`). Then read a
`--word-diff` of the result; it should show only the names you meant to change.

## The generated roadmap index was not a sweep target, and running the generator proved it

*The index this section is about was retired on 2026-09-21 and `--write` went with it, so there is
no generated file left in a rename's path. The lesson is kept because the next generated file will
raise the same question.*

The same lane was briefed to run `script/roadmap --write` after editing. The reasoning was that
`design/roadmap/README.md` is generated and would pick the rename up. It reported "index already
current" and wrote nothing, which is the right answer. The README's one occurrence sat inside the
summary of milestone 295 (retire `components/src/builder.rs`), mirrored from a `BUILT` block that
keeps the name it was written under.

A generated file inherits its sources' status rather than having one of its own, so it needs no
classification at all. Running the generator is still worth the ten seconds, because it is the one
command that decides the question. A generator that writes nothing has confirmed the sources were
classified correctly, and one that writes something has found a source you missed.

## What is checked, and what is not

`script/names --check` catches one member of this family: a name recorded as refused that is also
live. Nothing catches a rewritten quotation, a stale `PROPOSED` proposal, or a lint that shared a
substring. A check that tried would be guessing at intent.

So this is rung three, a written record at the thing a person is about to do, and it says so. The
higher rung is not available: no gate can tell an account from an intention in prose. That is the
reason `AGENTS.md` gives for not gating identified work either.
