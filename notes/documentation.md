# The manual: documentation as a system service

*(Milestone 40. Markdown authored, rendered for display rather than shown raw, searchable, and
installed by the package that owns it. The pure logic is `crates/documentation`; the program is
`components/src/mdr.rs`; the store is built by `cargo xtask manual`.)*

The project's own argument is written in markdown: 328 files, three megabytes, `design/decisions/`
and a hundred notes. A nife that serves them, on itself, through a viewer that can name
nothing is a better demonstration of the component story than another synthetic test, and it costs
the documentation nothing because it already exists.

Three words carry design weight here, and each turned out to be a capability question rather than a
feature question.

## Rendered

`crates/documentation` is a streaming renderer: bytes in, styled terminal bytes out, no allocator,
no document held anywhere. That shape is not an optimization, it is what the program's capabilities
already are. `mdr` receives its input as `byte_sink_protocol` messages of sixteen bytes each and
writes its output the same way. A renderer that needed the whole document would need somewhere to
put it, and somewhere to put it is a memory grant the program can otherwise do without. The test
`framing_does_not_matter` pins it: one byte at a time and the whole document at once produce
identical output.

It handles ATX headings, wrapped paragraphs, fenced code, block quotes, nested lists, tables with
computed column widths, thematic breaks, and the inline set (`**strong**`, `*emphasis*`,
`` `code` ``, `~~strike~~`, links and images).

A link and an image both show their destination, because a terminal cannot follow one or draw the
other, so the path is the only thing a reader can act on. An image once dropped its path, and
`every_character_survives` caught it on the first page that carried one
([documentation/corpus-test.md](documentation/corpus-test.md)).

### The roadmap said take `pulldown-cmark`. Two facts overrule it.

It is not `no_std`, and a std program on this system could neither read a keypress nor take an
argument. The corpus is closed and in-tree, so the renderer needs the constructs these files use
rather than CommonMark conformance, and `every_character_survives` checks exactly that over every
note, decision and roadmap page. DECISIONS §46 (thin primitives or whole subsystems) rule 1 then
settles it: this is on the verification path, so we write it. The crate is `no_std`,
zero-dependency, allocation-free on the guest path, and reachable by Kani. The two facts and the
three defects the corpus test found are in
[documentation/pulldown-cmark.md](documentation/pulldown-cmark.md).

### Two rendering decisions worth knowing

`_` is never emphasis. CommonMark reads `__rust_alloc` as an opened strong span. This repository
writes `snake_case` identifiers in running prose constantly (`filesystem_protocol`, `line_editor`,
`c_seam`), so honoring the spec here would misrender far more than it would style. Only `*` and ``
open emphasis, and a closer must not be preceded by a space.

A code span is consumed before anything else looks at the line. There are 11,281 of them in the
corpus, many full of the exact characters the other rules hunt for. If that ordering were wrong,
`*ptr` inside backticks would open emphasis and eat the rest of the paragraph.

### The bug the corpus test could not see, and the honest filter that kept it findable

A fenced code block opened inside a block quote must close inside it, or one quoted transcript
misrenders every line after it. `every_character_survives` cannot see that failure, because verbatim
output loses no characters. A unit test is the guard. The lesson generalizes: a
subsequence check proves nothing was dropped and nothing about what was ruined, so a renderer wants
both kinds of test. The bug, the filter that was left wrong on purpose and the measurement are in
[documentation/corpus-test.md](documentation/corpus-test.md).

## Installed

A doc bundle is a package's pages plus its index shard, installed as a unit. `doc/<bundle>/` in
the filesystem image, with `doc/bundles` listing the names, built by `cargo xtask manual` from the
`DOC_BUNDLES` table in `xtask/src/manual.rs` and imported into the RedoxFS image by `mkredoxfs`.

The table names paths that already exist rather than copying notes into crate directories, and that
is deliberate: a second copy of a note is a copy that can drift, and in-tree documentation earns
its keep by there being one. A bundle that lists a page which has moved fails the build.

### What a doc-holding capability designates, which is nothing

The roadmap proposed that "the viewer holds a directory capability to the doc store". It should not,
and does not.

`mdr`'s manifest is byte-identical to `wc`'s: `InputSpec::Required`, `OutputSpec::Bytes`, and
`Forbidden` for argument, memory, file and directory. Its capability table holds two endpoints. `mdr
glob.md` is the shell resolving that name against the directory capability *it* holds and streaming
the bytes in. Nothing in the program names a file, a directory or the filesystem, and there is no
message it can send to find out what it is reading.

That matters more here than it did for `wc`, because a documentation viewer is precisely the program
a reader would expect to go and fetch things. An `mdr` that opened the page it renders would be a
`mdr` that could open any page. `mdr glob.md`, `mdr < glob.md` and `something | mdr` are one
behavior with three sources, and the program cannot tell them apart.

So there is no ambient authority to arrive by accident, because there is no authority at all. The
concentration is in the shell, where it already was.

## Searchable

There is no directory iteration in this system, and that is a feature rather than a gap. `readdir`
refuses in the std PAL and the §27 (the filesystem service) file contract has no such verb. The
capability model argues against adding one: *enumeration is authority*, and a viewer that can list a
directory can discover what it was not given. So "what pages exist" is not discoverable at runtime.
It is computed on the host at image time and shipped, which is what Unix's `mandb` does for a
different reason (scanning was slow).

### The writer and the reader must agree on what a word is, and did not

The builder once split text on `_` while the query folded `line_editor` to `lineeditor`, so a search
for any `snake_case` identifier found nothing. The fix is in the writer: `line_editor` now yields
three terms, `line`, `editor` and `lineeditor`. Only the underscore joins, because the hyphen is
ambiguous in this tree. The reasoning, and the test that claimed the property and checked a weaker
one, are in [documentation/underscore-terms.md](documentation/underscore-terms.md).

### The layout is designed for a reader that holds one page

A client of the file contract shares exactly one 4 KiB frame with the FS server, and a shell that
had to buffer a whole index would need a memory grant to search. So every section of the index
starts on a page boundary and every record divides 4096 evenly, which together mean a reader with
one page in hand never sees half a record. A lookup is then a binary search over *pages*: each
probe reads one page and compares the term it starts with, and the last page is searched in memory
for free.

```text
  page 0            header
  page_off          page records, 128 bytes each, 32 per page
  term_off          term records, 32 bytes each, 128 per page, sorted by term
  post_off          postings, 4 bytes each, 1024 per page
```

### The guest searches with `apropos`, and it is a builtin

Search is a builtin, not a program, for the reason `ls` is: a searching program would have to be
handed the whole documentation store to read every shard in it, a new principal holding more than
the answer needs. What comes back is names, never capabilities, and a capability moves only on the
next line, where a person types one of those names. So search cannot widen what its caller could
already reach. The reading is `documentation::index::search`, and `cargo xtask manual capability` on
the host and `apropos capability` at the prompt call that same function over the same bytes. The
worked example and the split between crate and shell are in
[documentation/apropos-builtin.md](documentation/apropos-builtin.md).

### And the same index, pointed at the repository

`script/apropos <word>` is the guest's builtin with a checkout underneath it instead of a filesystem
image. It is milestone 117 (the stranger test)'s finding: the pages a newcomer could not reach were
ones nothing a person would type led to. It runs the same `documentation::index::build` and
`search`, so a layout defect shows in both places. A result names a path in this repository rather
than a store location. Crate and program module headers are pages, which is what makes
`crates/abi/src/lib.rs` findable. There is no cache: a run is about a second over five megabytes,
and a stale cache is worse. An example run and the three deliberate choices are in
[documentation/repository-search.md](documentation/repository-search.md).

### The store's own layout is a thing two programs agree on

`doc/bundles` lists what is installed, one name per line; `doc/<bundle>/index` is a shard;
`doc/<bundle>/<page>.md` are the pages. Those three names are `documentation::index::STORE_DIR`,
`MANIFEST` and `SHARD`, in the crate both sides depend on, because the host writes them and the
guest opens them (AGENTS.md rule 7). The manifest is a file rather than a directory listing,
which is this whole milestone in one constant.

### What it costs, measured

Per `cargo xtask manual`, the index is about 1.16 times the markdown it indexes. Most of the bulk is
a term stored inline in 24 bytes, so a probe is one page read, and page alignment puts a 16 KiB
floor under every bundle. The number that justifies the layout is that a lookup is at most five or
six page reads, with no allocation and a 4 KiB working set. The per-bundle table and how the ratio
moved are in [documentation/index-cost.md](documentation/index-cost.md); rerun `cargo xtask manual`
for current numbers, since editing this page moves them.

## EXAMPLES

Build the store and search it from the host, with the same reader the guest runs:

```text
$ cargo xtask manual capability
documentation store: target/redoxfs-tree/doc

  bundle     pages   terms postings  markdown    index probes
  manual         1    1222     1222     34011    57344      5
  swish          2    1820     2143     91736    81920      5
  kernel         3    2461     3369    101022   106496      6
  glob           1     882      882     20019    40960      4

  246788 bytes of markdown, 286720 bytes of index

search: capability
    46  doc/kernel/capabilities.md    Capabilities, and why the kernel has no `open()`  notes/capabilities.md
    11  doc/kernel/ipc-naming.md      Who does IPC name?                              notes/ipc-naming.md
    18  doc/manual/manual.md          The manual: documentation as a system service   notes/documentation.md
     3  doc/swish/line-discipline.md  The line discipline as a userspace component    notes/line-discipline.md
     8  doc/glob/glob.md              The glob matcher                                notes/glob.md
    32  doc/swish/pipes.md            Pipes and redirection: `>`, `<` and `|` are one   notes/pipes.md
```

The host prints a fourth column the prompt does not: the page's path in the source tree, which is
provenance rather than something to open. The store location beside it is computed by the searcher
from the shard it opened, so no byte of the index carries it and the two cannot disagree.

The order is by density, not by the raw count in the first column
([documentation/resolved-bugs.md](documentation/resolved-bugs.md) has the ranking change).

Search the same store from the prompt, where the answer is what a person acts on:

```text
$ apropos capability
    46  doc/kernel/capabilities.md    Capabilities, and why the kernel has no `open()`
    11  doc/kernel/ipc-naming.md      Who does IPC name?
    18  doc/manual/manual.md          The manual: documentation as a system service
     3  doc/swish/line-discipline.md  The line discipline as a userspace component
     8  doc/glob/glob.md              The glob matcher
    32  doc/swish/pipes.md            Pipes and redirection: `>`, `<` and `|` are one
$ caps wc doc/kernel/ipc-naming.md
  wc would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    output   this shell's result endpoint (it reads the bytes and prints them)
    input    ipc-naming.md  (this shell reads it and streams it in; the program
             holds an endpoint, not a file)
    arg    (none)
  reading the command is reading its whole authority.
```

Those two lines together are the milestone: the first names pages and grants nothing, and the second
grants exactly the one page a person named out of what the first said. `apropos` itself has no
`caps` preview to print, because there is nothing to preview.

Render a page at the prompt, with no pipe or redirection needed to make it run (DECISIONS §106 (take
the `terminal_sink_caretaker` narrowing), 2026-08-22). `gate.txt` is `script/swish-check`'s own
fixture, two lines of `hello world` written earlier in that script; the renderer re-flows them to
one paragraph:

```text
$ mdr gate.txt
  hello world hello world
```

And prove the viewer is an ordinary pipeline stage rather than something that reached for a file:

```text
$ echo # The terminal contract | mdr
THE TERMINAL CONTRACT
$ mdr gate.md | wc
1 3 22
$ mdr
mdr: reads an input stream: name a file, redirect with '<', or pipe into it
```

## BUGS

`mdr <page>` renders at the prompt (DECISIONS §106 (take the `terminal_sink_caretaker`
narrowing)), a pipeline head delivers its file, and ranking divides by page length. Why each took
what it did, including the kernel's one wait point that made the first one hard, is in
[documentation/resolved-bugs.md](documentation/resolved-bugs.md).

- A page's last line and the shell's next `$ ` prompt can interleave under contention: an
  accepted interim display race, not a confinement or correctness failure, tracked at milestone 151
  (notification objects). See notes/tail-output-narrowing.md.
- A screen-narrowed child is invisible to a concurrent `ps`/`pgrep` for its short life, though
  nothing leaks. Both entries are in full in
  [documentation/narrowing-limitations.md](documentation/narrowing-limitations.md).
- **No pager, and the reason is authority rather than effort.** Paging needs a keypress; a keypress
  needs `line_editor::proto::OPERATION_READLINE`; and that opcode rides on the terminal endpoint
  whose read side *is* the keyboard. The spawn protocol has no way to hand a child the right to read
  one line without handing it the terminal, which is the exact thing `terminal_sink_caretaker`
  exists to prevent. So a long page scrolls off. The fix is a decision about the spawn protocol, and
  it is the most interesting thing this milestone found.
- `mdr` emits plain text even at a terminal. The renderer can color, and the shell has no way
  to tell a stage "you end at the terminal", for the same reason the sink contract is a good
  contract: a writer cannot tell what is underneath it. Unix answers this with `isatty`, which is a
  sniff; the honest answer here is a wiring bit the spawn protocol does not carry yet.
- A search keeps sixteen results and counts the rest ("16 of 43 pages, strongest first"), because
  one of this system's two terminals is sixteen rows tall.
- A hyphenated name is not one term, and a path is not searchable. A search is for words, and the
  location a result prints is what you hand to `mdr`.
- A search answer is up to 86 columns wide, so it wraps on both consoles. The location is never
  truncated, because it is the name the reader is meant to type.
- **A negative example cannot be written into this page.** This note is in the `manual` bundle, so
  every word here is a word the store then says, and writing `apropos <nonsense>` with its answer
  would make that answer wrong on the next build. The boot gate holds the negative control instead,
  with a word chosen to appear in no bundled page. That is funny and it is also the honest shape of
  a system that documents itself: the documentation is data.
- A shard whose version is not the reader's is refused, not migrated.
- The shell reads at most 256 bytes of the bundle manifest, and says so when that fills.
- `apropos` searches from the root of what the shell holds, not from the cwd, so a shell granted a
  subtree without `doc/` cannot search at all.
- The index is 1.16x the markdown it indexes, and the 16 KiB per-bundle floor is most of what moves
  that ratio. Why each search limit above is drawn where it is, entry by entry, is in
  [documentation/search-limitations.md](documentation/search-limitations.md).
- **A source line longer than `documentation::LINE_MAX` (2048) loses its tail.** The longest line in
  this repository is 1928 bytes <!--count:longest-markdown-line-->, so the corpus fits; a document
  from elsewhere may not, and `Renderer::is_truncated` reports it while `mdr` does not print it. The
  number carries a marker because it once drifted
  ([documentation/resolved-bugs.md](documentation/resolved-bugs.md)).
- Table cells are truncated to their column width, so a wide table on an 80-column terminal
  loses text. This is a formatting choice, not a parsing failure, and the corpus test runs at 4000
  columns to keep the two apart. A table too large for the renderer's buffers spills into a second
  aligned chunk rather than losing rows; this repository's largest table is 117 rows.
- Setext headings and reference links are not recognized, and no HTML is interpreted. There is
  one reference link in the corpus and no setext heading; `---` on its own line is a thematic break
  here 64 times, so reading it as a heading would misrender all of them to catch none.

## Where this goes next

Two things are left, in the order they pay off. Color and the pager are one spawn-protocol decision
(the wiring bit §106 did not extend, so not this lane's call). The store as something a package
installs is blocked on milestone 39 (repository structure for a loosely-coupled OS). A graphical
viewer waits on milestone 33 (a compositor). Each, with
its reasoning, and the prior art (`man`, `apropos` and `mandb`, docsets, `cargo doc`) are in
[documentation/where-next.md](documentation/where-next.md).
