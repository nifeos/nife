//! **The prompt**: `apropos`, reading a line through the terminal, the shell editing its own
//! line (§227 (how Tab reaches the shell) option D), and Tab completion. One of the modules §273
//! (the shell program becomes a directory of modules) lays out.
//!
//! Name: provisional 2026-10-11 (UTC), milestone 844's lane. calef deferred every module name
//! in §273 to a name-ratification pass.

use super::*;

// ---- `apropos`: searching the documentation store (milestone 40 phase 2) ----

/// **One index shard, as pages read one at a time through the file contract.**
///
/// The whole guest half of the search: the layout lets a reader hold one 4 KiB page, the one a file
/// contract client already shares with the FS server, so there is no buffer and no memory grant.
struct Shard {
    /// The slot the directory capability lives in, which is what a request is sent on.
    dir: u64,
    /// The open file handle for `<bundle>/index`.
    handle: u64,
}

impl documentation::index::Pages for Shard {
    fn page(&mut self, index: u64, out: &mut [u8; documentation::index::PAGE]) -> bool {
        let got = call(
            self.dir,
            fs::req(fs::READ, self.handle, documentation::index::PAGE as u64),
            index * documentation::index::PAGE as u64,
        )
        .0 as i64;
        if got <= 0 {
            return false;
        }
        let got = (got as usize).min(documentation::index::PAGE);
        get_page(got, &mut out[..]);
        // A short read is a page at the end of the file. The layout guarantees a record never
        // straddles a page, so the tail is padding and zeroing it is the truth rather than a
        // convenience: leaving the previous page's bytes there would let a stale record be read.
        out[got..].fill(0);
        true
    }
}

/// The most of the bundle manifest one search reads. It is one short name per line and the shipped
/// store has four, so this is two orders of magnitude of headroom in a buffer that costs a quarter
/// of a page.
const MANIFEST_MAX: usize = 256;

/// **The merge across shards**, in `.bss` rather than on the stack: it is about 2.4 KiB and this
/// program's stack is twelve pages that several deeper paths already spend.
pub(super) static mut RANKED: documentation::index::Ranked = documentation::index::Ranked::new();

/// **`apropos <word>`: name the installed pages that mention it.**
///
/// A builtin (`grant_plan::Command::Apropos` says why): it opens the store at the root of what this
/// shell holds, so a `cd` does not move the manual, reads the manifest, and hands each shard to
/// `documentation::index::search`, which `cargo xtask manual <word>` runs on the host. It grants
/// and spawns nothing. Returns the filesystem's errno for a bundle that would not open, after the
/// results.
pub(super) fn apropos(nav: &mut Nav, term: &[u8]) -> Say {
    use documentation::index;

    if nav.dir.is_none() {
        return Say::NoDirectory;
    }
    if term.is_empty() {
        return Say::NeedsAName;
    }
    // The first tree whichever one this shell stands in: the manual is installed at the root of
    // what the system itself was granted, and a second tree is somebody else's files.
    let t = nav.first();
    let dir = t.slot;

    let store = nav.name_call_in(
        t,
        fs::OPENDIR,
        fs::ROOT,
        index::STORE_DIR.as_bytes(),
        t.rights,
    );
    if store < 0 {
        return Say::Failed(-store as i32);
    }
    let store = store as u64;

    // The manifest, copied out of the shared page before anything else touches it: every `OPENDIR`
    // below stages a name in that same page.
    let mut names = [0u8; MANIFEST_MAX];
    let handle = nav.name_call_in(t, fs::OPEN, store, index::MANIFEST.as_bytes(), 0);
    if handle < 0 {
        nav.close_in(t, store);
        return Say::Failed(-handle as i32);
    }
    let got = call(
        dir,
        fs::req(fs::READ, handle as u64, MANIFEST_MAX as u64),
        0,
    )
    .0 as i64;
    nav.close_in(t, handle as u64);
    if got < 0 {
        nav.close_in(t, store);
        return Say::Failed(-got as i32);
    }
    let got = (got as usize).min(MANIFEST_MAX);
    get_page(got, &mut names);

    // SAFETY: this process is single-threaded, so there is exactly one live reference to this
    // static for the whole run. It is taken once, here, and passed down by reference.
    let ranked = unsafe { &mut *core::ptr::addr_of_mut!(RANKED) };
    *ranked = index::Ranked::new();
    let mut trouble = 0i32;
    let mut unreadable = false;
    index::bundles(&names[..got], |bundle| {
        let at = nav.name_call_in(t, fs::OPENDIR, store, bundle, t.rights);
        if at < 0 {
            trouble = -at as i32;
            return;
        }
        let shard = nav.name_call_in(t, fs::OPEN, at as u64, index::SHARD.as_bytes(), 0);
        if shard < 0 {
            trouble = -shard as i32;
        } else {
            let mut src = Shard {
                dir,
                handle: shard as u64,
            };
            // A shard this reader cannot read is reported rather than skipped: a search that
            // quietly left a bundle out would answer "no page says that" about pages that do.
            unreadable |= index::search(bundle, term, &mut src, ranked).is_err();
            nav.close_in(t, shard as u64);
        }
        nav.close_in(t, at as u64);
    });
    nav.close_in(t, store);

    // A manifest that filled the buffer is a search that did not cover the store; say so rather
    // than answer "no page says that" about bundles nobody looked in.
    if got == MANIFEST_MAX {
        print(b"  the bundle manifest filled this shell's buffer; bundles past it were not searched\n");
    }
    if unreadable {
        print(b"  a shard in the store is not an index this reader knows\n");
    }
    swish::write_apropos(term, ranked, &mut print);
    // A bundle that would not open is the filesystem's answer and belongs in the status, but only
    // after the results: the pages that *were* found are still the answer to the question.
    if trouble != 0 {
        return Say::Failed(trouble);
    }
    Say::Nothing
}

/// Read a command line with the terminal's own editor: stage the prompt, CALL `OPERATION_READLINE`, and
/// block until the terminal has a line. Kept for a terminal that refuses raw mode, where the shell
/// cannot edit its own line; [`edit_line`] is the path every real boot takes.
fn read_line(prompt: &[u8], out: &mut [u8]) -> (usize, u64) {
    stage(prompt, prompt.len());
    let (len, flags) = loop {
        let r = call(
            TERM,
            proto::req(proto::OPERATION_READLINE, prompt.len() as u64),
            0,
        );
        // The terminal is being replaced and handed this read back (FLAG_RETRY, milestone 23 (a capability-routed component OS with live replacement)).
        // Ask again, unchanged: whichever terminal answers resumes the line where it was.
        if !proto::is_retry(r.0, r.1) {
            break r;
        }
    };
    let len = (len as usize).min(out.len());
    for (i, b) in out[..len].iter_mut().enumerate() {
        *b = LINE_WINDOW.r8(i as u64);
    }
    (len, flags)
}

// ---- the shell edits its own line (DECISIONS §227 (how Tab reaches the shell) option D) ----

/// **The line editor, in this process** (milestone 47 (navigation and naming), §227 option D).
/// The same sans-IO engine the terminal runs, fed from `OPERATION_READRAW`, so a Tab reaches the process
/// that holds the authority completion needs. A `static` for `line_editor`'s own reason: the
/// engine is a few KiB and this shell's stack has run out before (notes/pipes.md).
static mut EDITOR: line_editor::LineDisc = line_editor::LineDisc::new();

/// Bytes a raw read delivered after the one that ended a line, kept for the next line. A paste of
/// two lines arrives in one burst, and dropping the second would lose typed input.
static mut AHEAD: ([u8; 8], usize, usize) = ([0; 8], 0, 0);

/// **Whether the terminal is in raw mode on this shell's behalf.** On at the prompt and while a
/// plain command runs, so keystrokes typed ahead wait in the terminal's raw queue for the next
/// prompt. Off only while a supervised job is watched, because §24 (interrupting the foreground
/// process) counts `^C` in the terminal's line discipline, which raw mode bypasses.
static RAW: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Ask the terminal for raw mode, or leave it. `false` when the terminal refused, which a terminal
/// without raw mode does; the caller then reads lines the old way.
fn set_raw(on: bool) -> bool {
    let (r, _) = call(TERM, proto::req(proto::OPERATION_RAWMODE, on as u64), 0);
    let ok = r == 0;
    RAW.store(on && ok, core::sync::atomic::Ordering::Relaxed);
    ok
}

/// Leave raw mode before a supervised job, so its `^C` is counted where [`watch`] polls for it.
/// Switching discards whatever was typed ahead into the raw queue, which is `OPERATION_RAWMODE`'s contract.
pub(super) fn leave_raw() {
    if RAW.load(core::sync::atomic::Ordering::Relaxed) {
        set_raw(false);
    }
}

/// **Echo, staged in the output page and sent with `OPERATION_WRITE`.** The terminal translates `\n` to
/// `\r\n` on output, and the engine already writes `\r\n`, so a `\r` right before a `\n` is dropped
/// here rather than doubled there.
struct Echo {
    used: usize,
}

impl Echo {
    fn flush(&mut self) {
        if self.used > 0 {
            call(
                TERM,
                proto::req(proto::OPERATION_WRITE, self.used as u64),
                0,
            );
            self.used = 0;
        }
    }
}

impl line_editor::Sink for Echo {
    fn put(&mut self, bytes: &[u8]) {
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                continue;
            }
            if self.used == user_mode_runtime::mapped_window::PAGE as usize {
                self.flush();
            }
            OUT_WINDOW.w8(self.used as u64, b);
            self.used += 1;
        }
    }
}

/// **Read a command line, editing it here.** Same answer shape as [`read_line`]: the length and
/// the `FLAG_EOF` / `FLAG_INTERRUPTED` a terminal would have replied with. `^C` arrives as a byte
/// and the engine discards the line, exactly as the terminal's copy did.
pub(super) fn edit_line(nav: &mut Nav, prompt: &[u8], out: &mut [u8]) -> (usize, u64) {
    if !RAW.load(core::sync::atomic::Ordering::Relaxed) && !set_raw(true) {
        return read_line(prompt, out);
    }
    let editor = &raw mut EDITOR;
    let typed_ahead = &raw mut AHEAD;
    // SAFETY: single-threaded EL0, and nothing else in this program names `EDITOR`, so this is
    // the only reference to it while the line is read.
    let disc = unsafe { &mut *editor };
    // SAFETY: as above, for `AHEAD`.
    let ahead = unsafe { &mut *typed_ahead };
    let mut echo = Echo { used: 0 };
    disc.start_line(prompt, &mut echo);
    echo.flush();
    loop {
        if ahead.1 == ahead.2 {
            let (n, packed) = call(TERM, proto::req(proto::OPERATION_READRAW, 0), 0);
            // The terminal is being replaced and handed this read back (FLAG_RETRY, milestone 23
            // (a capability-routed component OS with live replacement)). The line lives in this
            // process, so nothing was lost and nothing needs painting: ask again. A replacement
            // that starts outside raw mode answers the next read `BAD_REQUEST`, handled below.
            if proto::is_retry(n, packed) {
                continue;
            }
            if n == proto::BAD_REQUEST {
                // Something this shell ran left raw mode (a program that holds the terminal turns
                // it off when it exits). Take it back, and paint the line again on a fresh row.
                if !set_raw(true) {
                    disc.abandon();
                    return read_line(prompt, out);
                }
                continue;
            }
            *ahead = (packed.to_le_bytes(), 0, (n as usize).min(8));
        }
        while ahead.1 < ahead.2 {
            let b = ahead.0[ahead.1];
            ahead.1 += 1;
            match disc.feed(b, &mut echo) {
                line_editor::Event::Line => {
                    echo.flush();
                    let line = disc.line();
                    let n = line.len().min(out.len());
                    out[..n].copy_from_slice(&line[..n]);
                    return (n, 0);
                }
                line_editor::Event::Eof => {
                    echo.flush();
                    return (0, proto::FLAG_EOF);
                }
                // At the prompt there is no foreground job, so ^Z abandons the line as ^C does.
                line_editor::Event::Interrupt | line_editor::Event::Suspend => {
                    echo.flush();
                    return (0, proto::FLAG_INTERRUPTED);
                }
                line_editor::Event::Tab => complete_word(nav, disc, &mut echo),
                line_editor::Event::None => {}
            }
        }
        echo.flush();
    }
}

/// **Tab**: finish the word under the cursor from what this shell can name (`swish::complete`).
/// A program or builtin name in command position; otherwise an entry of the directory the word's
/// lead names, read with the same `ENUMERATE` `ls` needs. No match rings the bell.
fn complete_word(nav: &mut Nav, disc: &mut line_editor::LineDisc, echo: &mut Echo) {
    use line_editor::Sink;
    use swish::complete::{Answer, Completing, Lister, Matches};
    let mut buf = [0u8; line_editor::LINE_MAX];
    let (line, cur) = disc.pending();
    let len = line.len();
    buf[..len].copy_from_slice(line);
    let line = &buf[..len];
    // Offer every candidate to `each`, from the source the word's position names. `false` when
    // there is nothing this shell can read there.
    let source = |nav: &mut Nav, which: &Completing<'_>, each: &mut dyn FnMut(&[u8], bool)| {
        match *which {
            Completing::Command { .. } => {
                for b in grant_plan::BUILTINS {
                    each(b, false);
                }
                for p in grant_plan::Program::ALL {
                    each(p.name().as_bytes(), false);
                }
                // And what the live installed-programs table names by bare word (§229 (how a bare
                // name at the prompt reaches an installed program), B2): the default each name's
                // pointer answers, never an owner's vouch, which claims no name, and never a
                // version somebody else picked.
                with_live_table(nav, |text, _| {
                    for entry in installed_programs::entries(text).flatten() {
                        // Once per name: a name the image also has is already offered, and is
                        // refused when run.
                        if installed_programs::lookup(text, entry.program)
                            .is_ok_and(|found| found == Some(entry))
                            && grant_plan::Program::from_name(entry.program.as_bytes()).is_none()
                        {
                            each(entry.program.as_bytes(), false);
                        }
                    }
                    Some(())
                });
                true
            }
            Completing::Name { dir, .. } => {
                // `ls docs` rather than `ls docs/`: the lead names a directory, and its trailing
                // slash is the word's, not the path's. The root keeps its one slash.
                let dir = match dir {
                    [b'/'] => dir,
                    [lead @ .., b'/'] => lead,
                    _ => dir,
                };
                nav.ls(dir, each) == Say::Nothing
            }
            Completing::Nothing => false,
        }
    };
    let which = swish::complete::completing(line, cur);
    let prefix = match which {
        Completing::Command { prefix } | Completing::Name { prefix, .. } => prefix,
        Completing::Nothing => return echo.put(&[0x07]),
    };
    let mut m = Matches::new(prefix);
    if !source(nav, &which, &mut |n, d| m.offer(n, d)) {
        return echo.put(&[0x07]);
    }
    match m.answer() {
        Answer::NoMatch => echo.put(&[0x07]),
        Answer::Insert { text, finished } => {
            disc.insert_text(text, echo);
            if let Some(f) = finished {
                disc.insert_text(&[f], echo);
            }
        }
        Answer::List => {
            let mut l = Lister::new();
            let wants = Matches::new(prefix);
            source(nav, &which, &mut |n, d| {
                if wants.wants(n) {
                    l.name(n, d, &mut |b| echo.put(b));
                }
            });
            l.finish(&mut |b| echo.put(b));
            disc.repaint(echo);
        }
    }
}
