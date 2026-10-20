//! **`mdr`: the markdown renderer** (notes/documentation.md). It reads markdown on its standard input
//! and writes it rendered on its standard output; the rendering is `crates/documentation`, which
//! parses with `pulldown-cmark` and is host-tested.
//!
//! **It cannot name a page.** A Unix `man` opens the file it prints, so it can open any file its
//! caller can. This is handed bytes: `mdr notes/glob.md` is the shell resolving the name against
//! its own directory capability and streaming the file, and `mdr < notes/glob.md`, `mdr
//! notes/glob.md` and `something | mdr` are one behaviour with three sources. Finding a page is
//! `apropos`'s job, a shell builtin, because a search must read the store's index.
//!
//! It is a `std` program (`std_runtime_contract`): its heap's budget at slot 0, its output at slot
//! 1, and its input as a byte stream at slot 30 (`STDIN_STREAM_SLOT`), which `std::io::stdin`
//! reads. No file, directory, clock or shared page. It cannot tell whether its output is a
//! terminal, and the sink contract keeps it so, which is what makes redirection one grant; so it
//! emits plain text.
//!
//! ```text
//! $ mdr gate.txt | wc
//!   1 4 26
//! $ mdr gate.txt
//!   hello world hello world
//! $ mdr
//!   mdr: reads an input stream: name a file, redirect with '<', or pipe into it
//! ```
//!
//! `gate.txt` is `2 4 24`; the rendering is one re-flowed line plus two bytes of indent
//! (`script/swish-check`).
//!
//! # BUGS
//!
//! - **It reads the whole input before it writes a byte**, because CommonMark's grammar needs the
//!   whole document (a link can be defined at the foot of the page). So a page can be as long as
//!   the heap allows, and the heap is the job's region (`grant_plan::STD_REGION_PAGES`); a longer
//!   one is an allocation failure, which ends the program.
//! - `mdr <page> > out.txt` is refused, naming the fix (notes/pipes.md).
//! - No pager: a keypress needs the terminal endpoint, whose read side is the keyboard, and the
//!   spawn protocol cannot hand over one line of input without it.
//! - Eighty columns always: the terminal contract has no "how wide are you" verb.
//! - Plain text only, until the shell can say a stage ends at the terminal.
//! - Input that is not UTF-8 is rendered with each bad sequence replaced by `U+FFFD`.
//!
//! Name: ratified 2026-09-13 (calef). Refused `doc` (promises a lookup, and builtins shadow it),
//! `man` (a format and a lookup it does neither of), `manual` (the crate's), `markdown` and
//! `markdown_renderer` (longer than a typed command needs) and `md` (names the format).

use std::io::{self, BufWriter, Read, Write};

use documentation::{Sink, Style, render};

/// The width the renderer wraps to. See `BUGS`: there is nothing to ask.
const WIDTH: u16 = 80;

fn main() {
    let mut markdown = Vec::new();
    // A read error ends the input where it stopped, the way a malformed frame does in std's own
    // stream reader: whatever arrived is rendered.
    let _ = io::stdin().read_to_end(&mut markdown);
    let markdown = String::from_utf8_lossy(&markdown);

    let mut out = Out {
        writer: BufWriter::new(io::stdout().lock()),
        gone: false,
    };
    render(
        &markdown,
        Style {
            width: WIDTH,
            color: false,
        },
        &mut out,
    );
    if !out.gone {
        let _ = out.writer.flush();
    }
}

/// Standard output as a [`Sink`] the renderer can write into.
struct Out<W: Write> {
    writer: W,
    /// The sink has been destroyed and there is nowhere left to write. Unix's `SIGPIPE`, arriving
    /// as an error (notes/pipes.md): everything after it is dropped rather than retried.
    gone: bool,
}

impl<W: Write> Sink for Out<W> {
    fn put(&mut self, bytes: &[u8]) {
        if !self.gone && self.writer.write_all(bytes).is_err() {
            self.gone = true;
        }
    }
}
