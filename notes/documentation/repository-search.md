# And the same index, pointed at the repository

*Name: provisional.*

An appendix to [`notes/documentation.md`](../documentation.md), which states the conclusion.

`script/apropos <word>` is the guest's builtin with a checkout underneath it instead of a filesystem
image. It is milestone 117 (the stranger test)'s finding rather than a convenience. Three stranger
runs have measured what a newcomer cannot reach by following this tree while doing ordinary work. It
is a list rather than an impression: `notes/net.md`, `notes/capabilities.md`, any
`design/decisions/` file, and `crates/abi/src/lib.rs`, which is four syscall numbers and the whole
design on one screen. None of them is hidden. Nothing a person would type led to them.

```text
$ script/apropos capability
512 pages, 5049711 bytes of documentation in this repository

searching for: capability

     46  notes/capabilities.md                             Capabilities, and why the kernel has no `open()`
     43  notes/README.md                                   Concept notes
     37  design/roadmap/0047-navigation-and-naming.md        47. Navigation and naming: `cd`, `pwd`, `ls`, `m
     33  notes/std.md                                      Rust `std` on the native ABI
     32  notes/pipes.md                                    Pipes and redirection: `>`, `<` and `|` are one

  16 of 302 pages, strongest first
```

Every number in that block moves whenever anything in the tree is edited, this page included, and
that is the same property the store's own table has: the documentation is the data. What does
not move is which page came first.

Three things about it are deliberate.

It is the same code, `documentation::index::build` and `documentation::index::search`, one shard per
part of the tree and the same merge across shards the shell does with one 4 KiB page. Not a second
implementation, so a defect in the layout shows up in both places and a fix lands in both. What
differs is what a result *names*: a guest result names `doc/<bundle>/<page>`, because that is what a
shell there can designate, and this one names a path in this repository, because that is what a
person with a checkout opens. Both come out of the same `Found`; the store location and the origin
are two fields it already carried.

Crate and program module headers are pages. That is the half that makes `crates/abi/src/lib.rs`
findable at all, and no markdown page was ever going to do it: the document a reader wants about the
ABI *is* that file's header. A `//!` block is markdown already, so it indexes with no conversion and
no copy, and the result names the source file, which is the thing to open. A header shorter than a
paragraph is skipped, because indexing it would put noise in front of the pages that answer.

There is no cache, and a run is about a second over five megabytes. A cache that can be stale is
worse here than a second of work, for the same reason `script/catch-up` and `script/names` are
derived views: a maintained one rots and nothing says so.
