---
status: BUILT
raised: 2026-07-30
built: 2026-08-26
---
# 40. Documentation as a system service: searchable, rendered, and installed by packages

Built. Phases 1 and 2 are built, with search ranked by document length, and phase 3's
caretaker-narrowing increment is built too (DECISIONS §106 (take the `terminal_sink_caretaker`
narrowing)): `doc <page>` renders at the prompt with no `| wc` in front of it, which is the
milestone's own headline demonstration. There is no graphical viewer, by calef's word: *"I don't
need a graphical viewer."* His quoted direction below never asked for one.

What exists: `crates/documentation` (a streaming markdown renderer, the byte layout of a search
index, and the query that reads one, all pure and host-tested in `tests/render.rs`),
`components/src/mdr.rs`, the viewer program, which the shell can spawn as `Prog::Mdr`, the store the
image installs, `apropos` at the nife prompt, and `script/apropos` over the whole repository. The
renderer was written against `line_editor`'s contract rather than `pulldown-cmark`, which the
block's own text anticipated as the alternative and which the crate's header records. In the phase
records below, `manual` is the crate now called `documentation` and `doc` the program now called
`mdr`.

In phase 2 the store is built on the host from the `DOC_BUNDLES` table
(`cargo xtask manual`), installed into the filesystem image as `doc/<bundle>/` with `doc/bundles`
naming what is there, and searched from the prompt by `apropos <word>`, a shell builtin. The
whole of it is gated by `script/swish-check` on both architectures: `apropos capability` names the
pages, and the line after it grants `wc` exactly one of them. Search produces names, never
capabilities, which is why it may be a builtin: a searching *program* would have to be handed the
whole store to read every shard in it, which is more authority than the answer needs. See
notes/documentation.md.

The same index is pointed at the repository as `script/apropos`, for the finding of milestone 117
(the stranger test). Where a tail stage's output goes when the shell cannot read it is decided as
DECISIONS §106, from notes/tail-output-narrowing.md.

- [build-record.md](0040-documentation-service/build-record.md): `script/apropos`, the fork §106
  decided, its build with the verifying transcript, and the ranking by page length.

calef's direction, 2026-07-30. Markdown as the authored format, rendered for display rather than
shown raw, searchable on the local machine, and installed *by the package that owns it*, so a
component brings its documentation with it.

Why this belongs on a demonstrator's roadmap rather than being a nicety. The project's own
argument is written in markdown: `design/decisions/`, thirty-plus notes, this roadmap. A nife that
serves its own design notes, on itself, through a capability-confined viewer, is a better
demonstration of milestone 23's component story than another synthetic test, and it costs the
documentation nothing because it already exists.

## Two constraints found while scoping, both real

1. There is no directory iteration. `readdir` refuses in the std PAL and the §27 file contract has
   no such verb, so nothing can walk a tree looking for documents. Adding one is a decision, not a
   detail, and the capability model argues against it anyway: enumeration is authority. A viewer
   that can list a directory can discover what it was not given. So the design below indexes at
   *package build time* and ships the index, which sidesteps the missing verb and is the more honest
   shape. Unix reached the same answer for a different reason: `apropos` reads a prebuilt `mandb`
   because scanning was slow.
2. ~~There is no font rendering yet.~~ There is now (milestone 29, 2026-07-30): a bitmap
   font, a VT engine, and a display terminal that is a compositor client. ~~A *graphical*
   documentation browser is therefore unblocked in principle~~ struck 2026-08-26: nobody asked
   for one, and this milestone's own headline demonstration renders at the prompt instead. See the
   Status block.

## Reuse: take the parser, write the renderer

CommonMark is a fiddly specification with a large conformance suite, and parsing it is exactly the
kind of work worth taking from someone else. Rendering to *our* terminal contract is ours and small.
That split is the reuse judgment, and it is the same one milestone 32 made about RedoxFS.

| Piece | Option | Judgment |
|---|---|---|
| Parse | `pulldown-cmark` (pure Rust, CommonMark, event-stream API, few dependencies) | Take it. The event stream is the right shape for a renderer that emits ANSI. Milestone 27's `std` is what makes this buildable at all. |
| Parse | `comrak` (GFM: tables, strikethrough, footnotes) | Consider later if GFM tables matter; more dependencies. |
| Render | `termimad`, `mdcat` | Do not take. Both sit on `crossterm`, which assumes a POSIX terminal (termios, ioctl). Porting that is more work than emitting ANSI against `line_editor`'s contract, which we own and already speak (§21). |
| Search | `tantivy` | Too heavy. It assumes a filesystem and mmap. |
| Search | A host-built inverted index shipped in the package | Take this shape. Built by `xtask` where there are no constraints, merged by the viewer across installed packages. |
| UI | `ratatui` | Possible for a pager later; needs a backend against our terminal contract first. |

## Shape

- A doc bundle is part of a package: rendered-source markdown plus a small index shard, installed
  into a documentation store when the component is installed. This is where milestone 39's packaging
  observation pays: manifest, hash, version, and now a doc bundle.
- ~~The viewer holds a directory capability to the doc store and nothing else.~~ It should not, and
  does not (phase 1's finding, now in notes/documentation.md): `doc`'s manifest is byte-identical to
  `wc`'s, its cspace holds two endpoints, and the shell is what resolves a page name. A viewer that
  could open the page it renders could open any page. Phase 2 kept that true by making search a
  builtin rather than a program, for the same reason `ls` is one.
- The index is a merge of shards, one per installed package, so installing a component makes its
  documentation searchable without a reindex pass and without any component being able to see
  another's files.
- `apropos <word>` and `doc <page>`, shell verbs. (Written as `doc search` / `doc view` when
  this was scoped; phase 2 split them because `doc` is a program and a builtin sharing its first
  word would make the parse ambiguous.) Milestone 31's grant expression makes this a demonstration
  rather than a convenience: `doc notes/ipc-naming.md` passes exactly one readable file capability,
  a viewer invoked with no argument can read nothing, and `apropos` in front of it passes nothing at
  all.

## Phasing

- Phase 1, the terminal viewer. `pulldown-cmark` to an ANSI renderer over `line_editor`'s contract:
  headings, emphasis, lists, block quotes, code blocks, and a pager. Works on the serial console
  today and inherits the display terminal for free when 29's glyph work lands. Host-tested in
  milliseconds like every other pure-logic piece: markdown in, styled bytes out.
- Phase 2, search. BUILT. The host-built index, the shard merge, and the search
  verb, which is `apropos <word>` rather than the `doc search` proposed above: `doc` is a
  program and a builtin sharing its first word would make the parse ambiguous, and `man`/`apropos`
  is the split this design already borrowed everywhere else. The merge is
  `manual::index::Ranked`, a fixed sixteen-result table with no allocator, so a shell with one
  stack page can hold it. Ranking is by document length, not just raw occurrence count:
  `manual::index::Page::tokens` and the `score` function above `Ranked::offer`.
- Phase 3. BUILT (DECISIONS §106): `doc <page>` renders straight at the prompt, with no `| wc`
  needed to give it a reader that is not the shell. There is no graphical viewer half; see the top
  of this block.

Prior art worth reading: `man` plus `apropos` plus `mandb` for the split between format, index and
pager, which is the architecture this proposes minus the troff. Dash/Zeal *docsets* (a bundle with its
own index) for the packaging shape. `cargo doc`'s HTML output as the road not taken, since HTML would
need a browser engine, which is a mountain with no thesis behind it.

Sequencing. Phase 1 wants milestone 31 phase 2 finished (per-file grants make `doc <file>` the
demonstration it should be) and nothing else; it can precede the packaging work and be wired into it
later. Effort: 1 lane estimated per phase, three phases, landed separately.

## BUGS

- The caretaker-hop display race is a known, accepted interim, not fixed by §106. With the
  `terminal_sink_caretaker` narrowing built, a page's last line and the shell's next `$ ` prompt can
  interleave under contention: kernel exit-delivery (DECISIONS §26) tells the shell a child is dead,
  but the caretaker's own trailing `CALL` to `line_editor` is a second, independent call with no
  ordering primitive against the shell's next prompt. A display glitch, not a confinement or
  correctness failure (no capability changes hands, no byte reaches the wrong reader). Not observed
  in either `script/swish-check` leg's transcript (`doc gate.txt`'s render is one short message, so
  the window is narrow), but the argument for it is structural rather than about this one case; see
  notes/tail-output-narrowing.md's own BUGS, which is honest that the race is named and not measured.
  Tracked at milestone 151 (notification objects), DECISIONS §101's kernel build, which lets the
  shell `WAIT` on "the caretaker's queue for this client has drained" instead of racing it.
- A screen-narrowed child does not appear in a concurrent `ps`/`pgrep`. Its DECISIONS §26 fault
  target is a fresh endpoint the shell mints for this purpose, not init's `deaths` domain channel
  (the shell needs to `RECV` its own child's exit directly, not race `job_undertaker` for the same
  message), and domain membership is exactly having `deaths` as that target. Its memory still
  returns to init's job pool when the shell reaps it. Recorded rather than fixed: nothing asked for
  a rendering `doc` invocation to be surveyable, and the alternative (giving the shell a second,
  narrower view into `deaths` itself) is a bigger change than this increment needed.

## Follow-on

- **Milestone 151.** The caretaker-hop display race: with the `terminal_sink_caretaker` narrowing
  built, a page's last line and the shell's next `$ ` prompt can interleave, because the caretaker's
  trailing `CALL` to `line_editor` has no ordering primitive against the shell's next prompt. The
  block names 151 (notification objects) as what lets the shell `WAIT` on the caretaker's queue
  draining instead of racing it.
- **Recorded.** `design/roadmap/0040-documentation-service.md`'s own BUGS: a screen-narrowed child
  does not appear in a concurrent `ps` or `pgrep`, because its fault target is a fresh endpoint the
  shell mints rather than init's `deaths` domain channel, and domain membership is exactly having
  `deaths` as that target. Its memory still returns to init's job pool when the shell reaps it.
- **Refused.** A graphical viewer. Traced to its origin 2026-08-26 and found to be the scoping
  lane's own unattributed addition rather than calef's direction; asked directly, he said "I don't
  need a graphical viewer." Struck wherever it was named, and the sentence that quietly became a
  requirement is kept in the block so a reader can see where it entered.
- **Refused.** Adding a `readdir` verb so a viewer could walk a tree for documents. Enumeration is
  authority, so a viewer that can list a directory can discover what it was not given. The design
  indexes at package build time and ships the index instead, which sidesteps the missing verb and is
  the shape Unix's prebuilt `mandb` reached for a different reason.
- **Refused.** `comrak` for GFM tables, strikethrough and footnotes. It carries more dependencies
  than the job needs and nothing in the corpus has wanted a GFM table, so the row stays "consider
  later if tables matter" rather than work anyone owes.
- **Refused.** `ratatui` for the pager. It needs a backend written against this tree's terminal
  contract before it can render anything at all, so taking it would buy a widget library and leave
  the actual work undone.
- **Milestone 334.** The other two thirds of
  §106's spawn-protocol narrowing: a bit telling a tail stage
  it ends at a real screen (color, the honest `isatty` replacement), and a way to grant one line of
  input without granting the keyboard (the pager). Both widen a protocol two programs agree on, so
  both are an architect's call; `notes/documentation.md`'s "where this goes next" is the only
  record either has.

## Index row

the OS explains itself, on itself. Phases 1 and 2 built: `crates/manual` (now `crates/documentation`; a streaming renderer and
an index layout a reader with one 4 KiB page can binary-search), `doc` (now `mdr`) at the prompt, and `apropos` as a builtin because search is enumeration and a searching *program* would need the
whole store. 2026-08-18 fixed a renderer bug the corpus test structurally could not see (a fence
inside a block quote never closed, so one quoted transcript misrendered every line after it),
fixed an index defect where the builder and the reader disagreed about what a word is (`apropos
fs_proto` could not match a term the builder never wrote), corrected three `BUGS` entries that
described a system which no longer existed, and pointed the same index at the repository as `script/apropos`, which is milestone 117's finding rather than a nicety: three strangers reached
neither `notes/capabilities.md` nor any `design/decisions/` file. The `terminal_sink_caretaker`
narrowing is decided and built (§106, 2026-08-22): `doc <page>` renders at the prompt. An
unredirected tail stage's output now defaults to the same adapter `2>` already uses; `grant_plan::check_chain` no longer refuses that shape, only the redirected one; and the shell's
completion signal is DECISIONS §26's kernel exit-delivery on a fresh endpoint. Verified at the
real prompt, both architectures: `doc gate.txt` prints `hello world hello world` with no `| wc` in
front of it. Milestone 151 tracks the one known cost (the caretaker-hop display race) as a
carried, accepted interim rather than a blocker. A graphical viewer, named as phase 3's other
half through every revision, is struck 2026-08-26: traced to its origin, an unattributed
sentence, and confirmed unrequested.
