//! **`rmle`, the Rust multi-line editor: the smallest real text editor** (milestone 169 (`kilo`:
//! the smallest real text editor)), on the raw-keystroke primitive that milestone added to the
//! terminal contract (`OPERATION_RAWMODE`/`OPERATION_READRAW`, `crates/line_editor`). Modelled on
//! antirez's public-domain `kilo` (<https://github.com/antirez/kilo>): a fixed screen, a row array,
//! a cursor, insert and delete, save. Rust rather than a port of `kilo.c`, because DECISIONS §31
//! (the foreign-language seam) calls C **once**, with a buffer and a length, and the C makes no
//! syscalls; `kilo` is a blocking event loop doing its own I/O, and a call per keystroke would be a
//! new seam.

//! # Capability contract
//!
//! | slot | what | why |
//! |---|---|---|
//! | 0 | the terminal, `WRITE` (`CALL`) | `OPERATION_RAWMODE`, `OPERATION_READRAW`, `OPERATION_WRITE` |
//! | 1 | a directory capability, `WRITE` (`CALL`) | `OPEN`/`CREATE`/`READ`/`WRITE`/`TRUNCATE`/`CLOSE` on the one file it edits |
//! | 2 | a report endpoint, `WRITE` | one message, sent right before `exit()` |
//!
//! `_start(spec, name_lo, name_hi)`: the filename arrives as `rm` takes one
//! (`filesystem_contract::grant::{spec, pack_name}`); no listing right, and [`load`] opens only it.

//! # What real `kilo` has that this does not
//!
//! - **No terminal size negotiation**: the serial contract carries none
//!   (`notes/terminal-contract.md`), so [`SCREEN_COLS`]/[`SCREEN_ROWS`] are a fixed 80x24.
//! - **A bounded document.** [`MAX_ROWS`] rows of [`MAX_COLS`] bytes, fixed arrays and no
//!   allocator; a file past either bound is silently truncated on load.
//! - **No incremental search (`^F`), no syntax highlighting.** `^C` is a literal byte in raw mode
//!   and bound to nothing; quit is `^Q`, as in `kilo`.
//! - **Quit confirmation is one extra `^Q`, not three** (`KILO_QUIT_TIMES`).
//!
//! Name: ratified 2026-08-27 (calef): Rust Multi-Line Editor, chosen over `kilo` so a real C port of
//! `kilo.c` through the foreign-language seam would not share the name, and distinct on sight from
//! `line_editor` (a single-line editor).

#![no_std]
#![allow(missing_docs)] // program entry point, not library surface (DECISIONS §107)
#![no_main]

use filesystem_contract::{fs, grant};
use line_editor::proto;
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{call, exit, send};

/// The terminal endpoint: `CALL` for `OPERATION_RAWMODE` / `OPERATION_READRAW` / `OPERATION_WRITE`.
const TERM: u64 = 0;
/// The directory capability: `CALL` for the `filesystem_contract::fs` verbs.
const DIR: u64 = 1;
/// Where the one closing report goes.
const REPORT: u64 = 2;

/// The fixed screen this program assumes; see the module doc's honest limit.
const SCREEN_COLS: usize = 80;
const SCREEN_ROWS: usize = 24;
/// Rows left for text once the status bar and the message bar take one each.
const TEXT_ROWS: usize = SCREEN_ROWS - 2;

/// The document's fixed capacity; see the module doc's honest limit.
const MAX_ROWS: usize = 32;
const MAX_COLS: usize = 100;

/// The terminal's own output page, mapped read/write here and read-only on the terminal's side
/// (`components/src/line_editor.rs`'s `APP_OUT_VA`). Chosen not to collide with [`FS_VA`] below, in the
/// same address space.
const TERM_OUT_VA: u64 = address_space_map::pair_page(0x0000_0000_0080_0000);
/// The page shared with the FS server, `filesystem_contract`'s own transfer unit. The same
/// conventional address `swish` and `fs_test_client` use; nothing requires it match theirs, since
/// each program is its own address space, but a reader who knows one FS client's layout should not
/// have to learn a second one for no reason.
const FS_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);

// SAFETY: the wiring (`kernel/src/user/rmle_service.rs`) maps one page read/write at each VA
// before this program runs, the same convention `fs_file_caretaker.rs`'s own `WINDOW` documents.
const TERM_OUT_WINDOW: MappedWindow = unsafe { MappedWindow::new(TERM_OUT_VA, 4096) };
// SAFETY: see above.
const FS_WINDOW: MappedWindow =
    unsafe { MappedWindow::new(FS_VA, filesystem_contract::PAGE as u64) };

// ---- the terminal half ----

fn rawmode(on: bool) {
    let w0 = proto::req(proto::OPERATION_RAWMODE, on as u64);
    call(TERM, w0, 0);
}

/// Block for the next batch of raw bytes (1..=8), exactly as they arrived.
fn readraw() -> ([u8; 8], usize) {
    let w0 = proto::req(proto::OPERATION_READRAW, 0);
    loop {
        let (r0, r1) = call(TERM, w0, 0);
        // The terminal is being replaced and handed this read back (FLAG_RETRY, milestone 23 (a capability-routed component OS with live replacement)).
        if !proto::is_retry(r0, r1) {
            return (r1.to_le_bytes(), r0 as usize);
        }
    }
}

/// Stage `bytes` in the output page and `OPERATION_WRITE` it, chunked at 4096 (the page's own size; the
/// whole-screen redraw this program ever sends is well under that, chunking is defensive).
fn term_write(bytes: &[u8]) {
    for chunk in bytes.chunks(4096) {
        for (i, &b) in chunk.iter().enumerate() {
            TERM_OUT_WINDOW.w8(i as u64, b);
        }
        let w0 = proto::req(proto::OPERATION_WRITE, chunk.len() as u64);
        call(TERM, w0, 0);
    }
}

// ---- the filesystem half ----

fn put_fs_page(bytes: &[u8]) {
    for (i, &b) in bytes.iter().enumerate() {
        FS_WINDOW.w8(i as u64, b);
    }
}

fn get_fs_page(n: usize, out: &mut [u8]) {
    for (i, b) in out.iter_mut().take(n).enumerate() {
        *b = FS_WINDOW.r8(i as u64);
    }
}

/// Stage `name` and call a name-taking verb against the granted directory's root.
fn name_call(verb: u64, name: &[u8]) -> i64 {
    put_fs_page(name);
    call(DIR, fs::req(verb, fs::ROOT, name.len() as u64), 0).0 as i64
}

fn read_at(handle: u64, len: usize, off: u64) -> i64 {
    call(DIR, fs::req(fs::READ, handle, len as u64), off).0 as i64
}

fn write_at(handle: u64, len: usize, off: u64) -> i64 {
    call(DIR, fs::req(fs::WRITE, handle, len as u64), off).0 as i64
}

fn truncate_to(handle: u64, size: u64) -> i64 {
    call(DIR, fs::req(fs::TRUNCATE, handle, 0), size).0 as i64
}

fn close(handle: u64) {
    call(DIR, fs::req(fs::CLOSE, handle, 0), 0);
}

// ---- the document ----

#[derive(Clone, Copy)]
struct Row {
    buf: [u8; MAX_COLS],
    len: usize,
}

impl Row {
    const fn empty() -> Self {
        Row {
            buf: [0; MAX_COLS],
            len: 0,
        }
    }
}

struct Editor {
    rows: [Row; MAX_ROWS],
    nrows: usize,
    /// Cursor column, a byte index into `rows[cy]`.
    cx: usize,
    /// Cursor row, an index into `rows`.
    cy: usize,
    /// The topmost row currently drawn, for vertical scrolling past [`TEXT_ROWS`].
    rowoff: usize,
    dirty: bool,
    /// Set by one `^Q` on a dirty document; a second `^Q` while this is set actually quits. Any
    /// other key clears it, matching real `kilo`'s reset-on-any-other-key behaviour if not its
    /// exact count (see the module doc).
    quit_pending: bool,
    handle: u64,
    message: [u8; 64],
    message_len: usize,
}

impl Editor {
    /// `const` so the one instance can live in `.bss` rather than on the stack: a user process
    /// here gets one 4 KiB page of stack by default (`kernel/src/user.rs`'s `USER_STACK_VA`), and
    /// even with `rmle_service.rs`'s extra pages this struct plus a debug build's per-call overhead
    /// overflows it. `display_terminal.rs`'s `TERMINAL` is the precedent.
    const fn new() -> Self {
        Editor {
            rows: [Row::empty(); MAX_ROWS],
            nrows: 0,
            cx: 0,
            cy: 0,
            rowoff: 0,
            dirty: false,
            quit_pending: false,
            handle: 0,
            message: [0; 64],
            message_len: 0,
        }
    }

    fn set_message(&mut self, text: &[u8]) {
        let n = text.len().min(self.message.len());
        self.message[..n].copy_from_slice(&text[..n]);
        self.message_len = n;
    }

    fn row_len(&self, y: usize) -> usize {
        if y < self.nrows { self.rows[y].len } else { 0 }
    }

    fn clamp_cursor(&mut self) {
        if self.cy >= self.nrows {
            self.cy = self.nrows.saturating_sub(1);
        }
        let len = self.row_len(self.cy);
        if self.cx > len {
            self.cx = len;
        }
    }

    fn insert_char(&mut self, b: u8) {
        let row = &mut self.rows[self.cy];
        if row.len >= MAX_COLS {
            return; // the row's own bound; see the module doc's honest limit
        }
        row.buf.copy_within(self.cx..row.len, self.cx + 1);
        row.buf[self.cx] = b;
        row.len += 1;
        self.cx += 1;
        self.dirty = true;
    }

    /// Enter: split the current row at the cursor into two.
    fn insert_newline(&mut self) {
        if self.nrows >= MAX_ROWS {
            self.set_message(b"document is at its row limit");
            return; // the document's own bound; see the module doc's honest limit
        }
        let tail_len = self.rows[self.cy].len - self.cx;
        let mut tail = Row::empty();
        tail.buf[..tail_len].copy_from_slice(&self.rows[self.cy].buf[self.cx..self.cx + tail_len]);
        tail.len = tail_len;
        self.rows[self.cy].len = self.cx;
        // Shift every row after cy down one to make room, then place the tail.
        for i in (self.cy + 1..self.nrows).rev() {
            let (left, right) = self.rows.split_at_mut(i + 1);
            right[0].buf = left[i].buf;
            right[0].len = left[i].len;
        }
        self.rows[self.cy + 1].buf = tail.buf;
        self.rows[self.cy + 1].len = tail.len;
        self.nrows += 1;
        self.cy += 1;
        self.cx = 0;
        self.dirty = true;
    }

    /// Backspace: delete before the cursor, joining with the previous row at column 0.
    fn backspace(&mut self) {
        if self.cx > 0 {
            let row = &mut self.rows[self.cy];
            row.buf.copy_within(self.cx..row.len, self.cx - 1);
            row.len -= 1;
            self.cx -= 1;
            self.dirty = true;
        } else if self.cy > 0 {
            let prev_len = self.rows[self.cy - 1].len;
            let cur_len = self.rows[self.cy].len;
            if prev_len + cur_len <= MAX_COLS {
                let cur = self.rows[self.cy].buf;
                self.rows[self.cy - 1].buf[prev_len..prev_len + cur_len]
                    .copy_from_slice(&cur[..cur_len]);
                self.rows[self.cy - 1].len = prev_len + cur_len;
            }
            for i in self.cy..self.nrows - 1 {
                let (left, right) = self.rows.split_at_mut(i + 1);
                left[i].buf = right[0].buf;
                left[i].len = right[0].len;
            }
            self.nrows -= 1;
            self.cy -= 1;
            self.cx = prev_len;
            self.dirty = true;
        }
    }

    /// Delete: remove under the cursor, joining with the next row at end of line.
    fn delete_at_cursor(&mut self) {
        let len = self.rows[self.cy].len;
        if self.cx < len {
            self.rows[self.cy]
                .buf
                .copy_within(self.cx + 1..len, self.cx);
            self.rows[self.cy].len -= 1;
            self.dirty = true;
        } else if self.cy + 1 < self.nrows {
            self.cy += 1;
            self.cx = 0;
            self.backspace();
        }
    }
}

// ---- loading and saving ----

/// A whole document's worth of scratch, for [`load`] and [`save`] to stage into before it goes to
/// (or comes from) the FS page: another `.bss` static rather than a stack local, [`Editor::new`]'s
/// own reason. Never used concurrently (one thread, and `save` never nests inside `load`), so one
/// buffer serves both.
///
/// **The `+ 1` is the row separator**: [`save`] joins the rows with `\n`, so a full document costs
/// `MAX_ROWS - 1` bytes more than its text, and one separator per row covers that for one byte.
/// notes/user-proofs.md has the defect leaving it out caused (milestone 197).
const FILE_SCRATCH_LEN: usize = MAX_ROWS * (MAX_COLS + 1);

// **Rung one, and deliberately not a proof** (AGENTS.md's ladder; milestone 197): raise `MAX_ROWS`
// or `MAX_COLS` without raising the buffer and this build stops at the wrong line. A Kani harness
// over a symbolic document is a bounded sum CBMC did not finish (notes/user-proofs.md).
const _: () = assert!(FILE_SCRATCH_LEN >= MAX_ROWS * MAX_COLS + (MAX_ROWS - 1));

static mut FILE_SCRATCH: [u8; FILE_SCRATCH_LEN] = [0; FILE_SCRATCH_LEN];

/// Open the granted name, creating it if absent. Loads its content (bounded, see the module doc)
/// on the existing-file path; a fresh file starts with one empty row.
fn load(ed: &mut Editor, name: &[u8]) {
    let mut handle = name_call(fs::OPEN, name);
    let existed = handle >= 0;
    if !existed {
        handle = name_call(fs::CREATE, name);
    }
    if handle < 0 {
        // Nothing to edit and nowhere to create it: report and die, the same as real `kilo`'s
        // `die()` on an unopenable file.
        send(REPORT, STATUS_OPEN_FAILED, (-handle) as u64, 0);
        exit();
    }
    ed.handle = handle as u64;

    if existed {
        let scratch_p = &raw mut FILE_SCRATCH;
        // SAFETY: one thread, for good (built without its own process); see `Editor::new`'s doc.
        let buf = unsafe { &mut *scratch_p };
        let n = read_at(ed.handle, buf.len(), 0).max(0) as usize;
        get_fs_page(n.min(filesystem_contract::PAGE), buf);
        // A read past one page needs more rounds; the whole scratch buffer is under one page today
        // (`FILE_SCRATCH_LEN` is 32 * 101 = 3232), so this is exact rather than an approximation
        // that happens to work.
        let mut row = 0usize;
        let mut col = 0usize;
        for &b in &buf[..n] {
            if b == b'\n' {
                ed.rows[row].len = col;
                row += 1;
                col = 0;
                if row >= MAX_ROWS {
                    break;
                }
            } else if col < MAX_COLS {
                ed.rows[row].buf[col] = b;
                col += 1;
            }
        }
        if row < MAX_ROWS {
            ed.rows[row].len = col;
            row += 1;
        }
        ed.nrows = row.max(1);
    } else {
        ed.nrows = 1;
    }
}

/// **How many bytes staging this document takes**: every row's text, plus one `\n` between each
/// adjacent pair.
///
/// Its own function because it is the *claim* rather than a step: [`FILE_SCRATCH`] has to be at
/// least this large for every document the editor's own limits allow, which is what the `const`
/// assertion beside that buffer says. Naming the quantity is what let the two constants be compared
/// at all, and comparing them is how the defect recorded there was found.
///
/// Name: provisional (milestone 197 (`user/` and `xtask` are out of reach of the prover)): calef
/// names functions (AGENTS.md, extended 2026-08-23).
fn staged_len(ed: &Editor) -> usize {
    let mut n = 0usize;
    for i in 0..ed.nrows {
        n += ed.rows[i].len;
        if i + 1 < ed.nrows {
            n += 1;
        }
    }
    n
}

/// **Stage the document into `buf` as `\n`-joined rows**, and return how many bytes that took.
///
/// [`save`]'s loop, with the two capability calls left behind it: arithmetic over the document's
/// own bounds. **The bounds check is hoisted to the top on purpose**: `buf[..staged_len(ed)]` is
/// one check against a number that has a name and a `const` assertion about it, rather than a dozen
/// implicit ones inside the copies. If that slice exists the writes fit, because `n` never runs
/// past the sum it is accumulating toward.
///
/// Name: provisional (milestone 197). `stage` is the verb [`save`]'s own doc already used for this.
fn stage(ed: &Editor, buf: &mut [u8]) -> usize {
    let out = &mut buf[..staged_len(ed)];
    let mut n = 0usize;
    for i in 0..ed.nrows {
        let row = &ed.rows[i];
        out[n..n + row.len].copy_from_slice(&row.buf[..row.len]);
        n += row.len;
        if i + 1 < ed.nrows {
            out[n] = b'\n';
            n += 1;
        }
    }
    n
}

/// `^S`: write every row back, `\n`-joined, and truncate to exactly that length.
fn save(ed: &mut Editor) {
    let scratch_p = &raw mut FILE_SCRATCH;
    // SAFETY: one thread, for good (built without its own process); see `Editor::new`'s doc.
    let buf = unsafe { &mut *scratch_p };
    let n = stage(ed, buf);
    put_fs_page(&buf[..n]);
    let w = write_at(ed.handle, n, 0);
    if w < 0 || w as usize != n {
        ed.set_message(b"save failed");
        return;
    }
    if truncate_to(ed.handle, n as u64) < 0 {
        ed.set_message(b"save failed (truncate)");
        return;
    }
    ed.dirty = false;
    ed.set_message(b"saved");
}

// ---- rendering ----

fn push(buf: &mut [u8; 4096], n: &mut usize, bytes: &[u8]) {
    let room = buf.len() - *n;
    let k = bytes.len().min(room);
    buf[*n..*n + k].copy_from_slice(&bytes[..k]);
    *n += k;
}

fn push_num(buf: &mut [u8; 4096], n: &mut usize, mut v: usize) {
    if v == 0 {
        push(buf, n, b"0");
        return;
    }
    let mut digits = [0u8; 20];
    let mut i = 20;
    while v > 0 {
        i -= 1;
        digits[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    push(buf, n, &digits[i..]);
}

/// The screen buffer `redraw` stages one frame into before the single `OPERATION_WRITE` that sends it: a
/// `.bss` static rather than a stack local, called every loop iteration, so this is the hottest of
/// the three buffers `Editor::new`'s doc explains moving off the stack.
static mut SCREEN: [u8; 4096] = [0; 4096];

fn redraw(ed: &mut Editor) {
    // Keep the cursor's row on screen.
    if ed.cy < ed.rowoff {
        ed.rowoff = ed.cy;
    } else if ed.cy >= ed.rowoff + TEXT_ROWS {
        ed.rowoff = ed.cy - TEXT_ROWS + 1;
    }

    let screen_p = &raw mut SCREEN;
    // SAFETY: one thread, for good (built without its own process); see `Editor::new`'s doc.
    let buf = unsafe { &mut *screen_p };
    let mut n = 0usize;
    push(buf, &mut n, b"\x1b[?25l\x1b[H"); // hide cursor, home

    for screen_row in 0..TEXT_ROWS {
        let file_row = ed.rowoff + screen_row;
        if file_row < ed.nrows {
            let row = &ed.rows[file_row];
            let show = row.len.min(SCREEN_COLS);
            push(buf, &mut n, &row.buf[..show]);
        } else {
            push(buf, &mut n, b"~");
        }
        push(buf, &mut n, b"\x1b[K\r\n");
    }

    // Status bar: filename-free (rmle never learns its own name past the OPEN call), row count
    // and dirty marker, which is what a status bar can honestly show here.
    push(buf, &mut n, b"\x1b[7m"); // reverse video
    push(buf, &mut n, b"rmle -- ");
    push_num(buf, &mut n, ed.nrows);
    push(buf, &mut n, b" lines");
    if ed.dirty {
        push(buf, &mut n, b" (modified)");
    }
    push(buf, &mut n, b"\x1b[K\x1b[m\r\n");

    // Message bar.
    push(buf, &mut n, &ed.message[..ed.message_len]);
    push(buf, &mut n, b"\x1b[K");

    // Position the real cursor, then show it again.
    push(buf, &mut n, b"\x1b[");
    push_num(buf, &mut n, ed.cy - ed.rowoff + 1);
    push(buf, &mut n, b";");
    push_num(buf, &mut n, ed.cx + 1);
    push(buf, &mut n, b"H\x1b[?25h");

    term_write(&buf[..n]);
}

// ---- keys ----

/// The escape parser's state, byte to byte across `OPERATION_READRAW` batches: the same reason
/// `line_editor::LineDisc` keeps its own `EscState` rather than assuming a whole sequence arrives
/// in one read.
enum Esc {
    Idle,
    Seen,
    Csi(u16),
}

/// One `^Q` too many without saving reports this and quits; `STATUS_OPEN_FAILED` reports the errno
/// of an OPEN/CREATE that both failed. Kept as plain constants rather than an enum so the report
/// word is exactly what a kernel test reads off the wire, the same convention `login_contract`'s
/// `RPT_*` constants use.
const STATUS_QUIT: u64 = 1;
const STATUS_OPEN_FAILED: u64 = 2;

/// The one document this process ever edits. `.bss`, not `_start`'s own frame; see `Editor::new`'s
/// doc for why.
static mut EDITOR: Editor = Editor::new();

user_mode_runtime::program_entry!(start);

fn start(spec: u64, name_lo: u64, name_hi: u64) -> ! {
    let mut name_buf = [0u8; grant::WORDS_NAME];
    let name_len = grant::unpack_name(name_lo, name_hi, grant::spec_len(spec), &mut name_buf);
    let name = &name_buf[..name_len];

    rawmode(true);

    let ed_p = &raw mut EDITOR;
    // SAFETY: this program has one thread and cannot be given a second, since it is built without
    // its own process (milestone 812 (`std::thread::spawn` runs real threads in one address
    // space)), so this pointer is the only route to `EDITOR` and there is no aliasing question,
    // the same reasoning `display_terminal.rs`'s `term()` documents.
    let ed = unsafe { &mut *ed_p };
    ed.set_message(b"^S save  ^Q quit");
    load(ed, name);

    let mut esc = Esc::Idle;
    loop {
        redraw(ed);
        let (bytes, n) = readraw();
        for &b in &bytes[..n] {
            let clear_quit_pending = handle_byte(ed, &mut esc, b);
            if clear_quit_pending {
                ed.quit_pending = false;
            }
        }
    }
}

/// Feed one raw byte through the escape parser and then the editor. Returns whether this byte
/// should clear a pending `^Q` confirmation (everything except `^Q` itself does).
fn handle_byte(ed: &mut Editor, esc: &mut Esc, b: u8) -> bool {
    match core::mem::replace(esc, Esc::Idle) {
        Esc::Idle => {}
        Esc::Seen => {
            if b == b'[' {
                *esc = Esc::Csi(0);
            }
            return true;
        }
        Esc::Csi(param) => {
            if b.is_ascii_digit() {
                *esc = Esc::Csi(param.saturating_mul(10) + (b - b'0') as u16);
                return true;
            }
            csi_final(ed, b, param);
            return true;
        }
    }

    match b {
        0x1b => {
            *esc = Esc::Seen;
            true
        }
        0x11 => {
            // ^Q
            if !ed.dirty || ed.quit_pending {
                close(ed.handle);
                rawmode(false);
                send(REPORT, STATUS_QUIT, ed.dirty as u64, 0);
                exit();
            }
            ed.quit_pending = true;
            ed.set_message(b"unsaved changes: ^Q again to quit without saving");
            false
        }
        0x13 => {
            // ^S
            save(ed);
            true
        }
        b'\r' | b'\n' => {
            ed.insert_newline();
            true
        }
        0x7f => {
            ed.backspace();
            true
        }
        0x20..=0x7e => {
            ed.insert_char(b);
            true
        }
        _ => true, // other control bytes: ignored, exactly like real kilo's default case
    }
}

fn csi_final(ed: &mut Editor, b: u8, param: u16) {
    match b {
        b'A' => ed.cy = ed.cy.saturating_sub(1),
        b'B' => {
            if ed.cy + 1 < ed.nrows {
                ed.cy += 1;
            }
        }
        b'D' => {
            if ed.cx > 0 {
                ed.cx -= 1;
            } else if ed.cy > 0 {
                ed.cy -= 1;
                ed.cx = ed.row_len(ed.cy);
            }
        }
        b'C' => {
            if ed.cx < ed.row_len(ed.cy) {
                ed.cx += 1;
            } else if ed.cy + 1 < ed.nrows {
                ed.cy += 1;
                ed.cx = 0;
            }
        }
        b'H' => ed.cx = 0,
        b'F' => ed.cx = ed.row_len(ed.cy),
        b'~' if param == 1 => ed.cx = 0, // vt220 Home
        b'~' if param == 4 => ed.cx = ed.row_len(ed.cy), // vt220 End
        b'~' if param == 3 => ed.delete_at_cursor(), // Delete
        _ => {}
    }
    ed.clamp_cursor();
}

user_mode_runtime::panic_handler!();
