//! **The script's pipes and files**: the pipeline, the redirections, the `wc` trio, and
//! milestone 40 (documentation as a system service), the viewer (`mdr`) and doc store. Naming a resource in a command is granting
//! it, and the paired `wc` lines are what make that a claim rather than an assertion.
//!
//! Cut from the table in `script.rs` verbatim by milestone 900 (the swish-check script
//! splits by topic before it crosses the ceiling).
//!
//! Name: provisional (milestone 900's lane, 2026-10-15 UTC).

use super::*;

/// The pipes-and-files rows of [`SWISH_CHECK_SCRIPT`], in table order.
pub(super) const LINES: &[Line] = &[
    line(1, "echo hello world | wc", &["1 2 12"]),
    line(0, "echo hello world > gate.txt", &[]),
    line(1, "wc < gate.txt", &["1 2 12"]),
    line(0, "echo hello world >> gate.txt", &[]),
    line(1, "wc < gate.txt", &["2 4 24"]),
    // **Milestone 31's headline, at the one interface a human touches**: naming a resource in a
    // command IS granting it. The answer has to be the same as the `<` above it, because it is the
    // same designation with the operator left out, and the pair is what makes that a claim rather
    // than an assertion: one line reaches the file through an operator and one through a name, so
    // if they disagree, one of them opened something else.
    line(1, "wc gate.txt", &["2 4 24"]),
    // **And the same name at the head of a pipeline**, which is the line that answered nothing at
    // all until milestone 50's draining lane. An input operand is resolved by the planner, and the
    // shell used to wire a pipeline's head off the `Line` (which has no `<` on it), so the planned
    // source was dropped and the stage counted an empty stream (a `receive` on an empty slot answers
    // `NoSuchSlot`, which reads as end of document). Two spawned processes, and this shell feeds the
    // first.
    //
    // `2 4 24` plus a newline is seven bytes and three words on one line, so the answer is the
    // answer above it counted. Spelled out rather than derived for this file's reason: it is a boot
    // gate, and deriving one number from another would hide the case where both are wrong.
    line(2, "wc gate.txt | wc", &["1 3 7"]),
    // The negative control the pair would be weaker without. `wc` alone is refused **at the
    // prompt**, before anything is spawned, because its manifest declares that it reads a stream;
    // on Unix the same command is a shell that appears to hang. So the line above granted
    // something, rather than falling back on a default.
    line(0, "wc", &["name a file"]),
    // Milestone 632 (graphics on demand)'s refusal, on every ISA: these legs attach no gpu, so the
    // shell holds no display and `graphical_terminal` is refused at the prompt with a sentence rather than
    // spawned into nothing. It is the half of the milestone x86_64 can prove (no virtio-gpu is
    // wired there); the launch itself is `launch_graphical_terminal`'s, on a later boot of the same leg.
    line(0, "graphical_terminal", &["no display on this boot"]),
    // And `caps` says which file and how, which is the honest half: the shell reads it and streams
    // it in, so what the child holds is an endpoint and not a capability naming the disk.
    line(0, "caps wc gate.txt", &["input    gate.txt"]),
    // **Milestone 40 at the same interface.** `doc` is in the image, is spawnable, and declares that
    // it reads a stream, so bare `doc` is refused at the prompt before anything is spawned, exactly
    // as `wc` is and for the same reason: a viewer that could open the page it renders could open
    // any page.
    line(0, "mdr", &["name a file"]),
    // **The named file reaches the viewer and comes back rendered.** The input operand comes off
    // the plan (`components/src/swish.rs`), and `MAX_OUTPUT_CHUNKS` (4096) holds a whole page.
    // `gate.txt` is 2 lines, 4 words, 24 bytes; rendered it is **1 line, 4 words, 26 bytes**, one
    // reflowed paragraph plus a two-byte body indent. An empty stream would answer `0 0 0`, so the
    // count is what separates rendering from silence.
    line(2, "mdr gate.txt | wc", &["1 4 26"]),
    // **And the line a person actually wants now renders**, which is milestone 40's whole
    // remaining phase (DECISIONS §106, 2026-08-22). `mdr gate.txt` alone used to make this shell
    // both the writer and the reader of one line, refused rather than hung, because it has one
    // wait point; see `grant_plan::check_chain` and notes/documentation.md for the refusal this replaced.
    // Now the render defaults to `terminal_sink_caretaker` instead of this shell's own result
    // endpoint, so there is no second reader for the shell to wait behind and the page appears at
    // the prompt with no `| wc` in front of it. The text is the same paragraph `mdr gate.txt | wc`
    // counted three lines up, reflowed and indented by the renderer: `gate.txt`'s two source lines
    // become the one line, four words, twenty-six bytes that count asserted, and this line checks
    // the words themselves arrived rather than merely being countable.
    line(1, "mdr gate.txt", &["hello world hello world"]),
    // **A pipe's read end reaches a `std` program's stdin** (milestone 784 (reuse what the tree
    // wrote), item (f2)): `mdr` is a `std` program, and here its input is neither a file the shell
    // streams nor the console but the far end of a pipe, written by another program that sends its
    // own end of stream. `wc`'s three numbers come back as a rendered paragraph. Then a builtin
    // upstream, which the shell feeds itself: a `#` line renders as a level-one heading, in capitals,
    // which nothing but the renderer would produce.
    line(2, "wc gate.txt | mdr", &["2 4 24"]),
    line(1, "echo # piped title | mdr", &["PIPED TITLE"]),
    // **A whole page fits**: the store's largest, about eighteen kilobytes, read into `mdr`'s heap
    // before a byte is drawn. A heap too small is an allocation failure, which faults the job, and
    // a faulted job fails this line by its job count. Counted rather than shown, because eighteen
    // kilobytes on `x86_64`'s console outruns the per-line bound; the render itself is host-tested.
    line(2, "mdr doc/swish/pipes.md | wc", &[]),
    // **The negative control on the viewer itself**, and it is the whole milestone in one screen: a
    // documentation viewer is exactly the program a reader expects to go and fetch things, and this
    // one is handed a stream. `caps` prints what would be granted before anything is spawned, and
    // there is no file capability, no directory and no filesystem endpoint in it. The manifest
    // declares the input and output `wc`'s does, which is why the assertion is the same string.
    line(0, "caps mdr gate.txt", &["input    gate.txt"]),
    // **Milestone 40 phase 2, at the same interface**: the documentation store is installed, and a
    // search of it answers with pages a person can then open.
    //
    // `doc/bundles` is the manifest the search reads, and it is checked as a *file* first, with an
    // ordinary designation, because that is the claim underneath everything below it: the store is
    // real, it is where the reader thinks it is, and nothing special is needed to read it. The
    // numbers are the four bundle names and their newlines, so a bundle added to `DOC_BUNDLES`
    // fails here, which is right: the manifest is what the guest enumerates by.
    line(1, "wc doc/bundles", &["4 4 25"]),
    // **The query a person would type, against shards built from this repository's own markdown.**
    // Two bundles, so the answer is the merge across shards rather than one shard's list, and both
    // named pages are ones a reader wanting to know what a capability is here would want. The
    // counts are deliberately not asserted: they move whenever the notes are edited, and the claim
    // is which pages were found, not how often the word appears in them.
    line(
        0,
        "apropos capability",
        &["doc/swish/pipes.md", "doc/kernel/ipc-naming.md"],
    ),
    // The negative control, and the word is chosen to appear in **no bundled page**. See
    // notes/documentation.md's BUGS for why this one cannot be written into the note that documents it:
    // that note is itself in the store, so a word written there is a word the store then says.
    line(
        0,
        "apropos photosynthesis",
        &["no page in the store says photosynthesis"],
    ),
    // And a search with nothing to search for is refused, in the same sentence every other verb
    // that needs an operand uses.
    line(0, "apropos", &["name what you mean"]),
    // **The payoff, and the reason a search may be a builtin at all.** The name the search printed
    // is an ordinary designation: this line grants `wc` exactly that one page out of the store and
    // nothing else, resolved by the shell against the directory it holds. So search produced a
    // *name*, and the authority moved on the line where a person typed it. A search that had
    // handed a program the store's directory would have moved it three lines earlier and silently.
    line(
        0,
        "caps wc doc/kernel/ipc-naming.md",
        &["input    ipc-naming.md"],
    ),
];
