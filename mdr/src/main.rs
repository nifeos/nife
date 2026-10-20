//! **`mdr`: the markdown renderer** (notes/documentation.md). It reads markdown on its standard
//! input and writes it rendered on its standard output; the rendering is `crates/documentation`,
//! which parses with `pulldown-cmark` and is host-tested.
//!
//! **It cannot name a page.** A Unix `man` opens the file it prints, so it can open any file its
//! caller can. This is handed bytes: `mdr notes/glob.md` is the shell resolving the name against
//! its own directory capability and streaming the file, and `mdr < notes/glob.md`, `mdr
//! notes/glob.md` and `something | mdr` are one behavior with three sources. Finding a page is
//! `apropos`'s job, a shell builtin, because a search must read the store's index.
//!
//! A `std` program (`std_runtime_contract`): its heap's budget at slot 0, its output at slot 1,
//! and its input as a byte stream at slot 30, which `std::io::stdin` reads. No file, directory,
//! clock or shared page. It cannot tell whether its output is a terminal, so it emits plain text.
//!
//! ```text
//! $ mdr gate.txt | wc
//!   1 4 26
//! $ mdr
//!   mdr: reads an input stream: name a file, redirect with '<', or pipe into it
//! ```
//!
//! `gate.txt` is `2 4 24`; the rendering is one re-flowed line plus two bytes of indent.
//!
//! # BUGS
//!
//! - **It reads the whole input before it writes a byte**, because CommonMark needs the whole
//!   document (a link can be defined at the foot of the page). A page is bounded by the heap, which
//!   is the job's region (`grant_plan::STD_REGION_PAGES`); past it, allocation fails and ends it.
//! - `mdr <page> > out.txt` is refused, naming the fix (notes/pipes.md).
//! - No pager: a keypress needs the terminal endpoint, whose read side is the keyboard.
//! - Eighty columns always, and plain text only: the terminal contract cannot say either.
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
