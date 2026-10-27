//! **The editing engine**: [`LineDisc`], the limits it holds lines to, the [`Event`] each byte
//! amounts to, and the cursor-move sequences its echo writes. The crate docs say what it does and
//! does not do.
//!
//! The fields `handoff` saves and restores are `pub(super)` so that it can reach them.
//!
//! Name: provisional (milestone 878's lane, 2026-10-12 UTC; §295 (the line editor's crate root
//! splits into module files)), for the module and the file.

use crate::output::Sink;

/// The longest line the discipline will assemble. Longer input is refused with a bell.
pub const LINE_MAX: usize = 256;
/// How many history entries the ring holds.
pub const HIST: usize = 8;
/// The longest prompt the discipline will repaint. Longer prompts are truncated.
pub const PROMPT_MAX: usize = 64;

const BS: u8 = 0x08;
const BEL: u8 = 0x07;
const ESC: u8 = 0x1b;

/// What one input byte amounted to.
///
/// # Examples
///
/// `^D` ends input on an empty line and deletes on a non-empty one; `^C` discards the line.
///
/// ```
/// # use line_editor::{Event, LineDisc, Sink};
/// # struct Echo(Vec<u8>);
/// # impl Sink for Echo {
/// #     fn put(&mut self, bytes: &[u8]) { self.0.extend_from_slice(bytes); }
/// # }
/// let mut out = Echo(Vec::new());
/// let mut ld = LineDisc::new();
/// ld.start_line(b"$ ", &mut out);
///
/// // ^D on an empty line: end of input.
/// assert_eq!(ld.feed(0x04, &mut out), Event::Eof);
///
/// // ^C discards whatever was in progress.
/// ld.start_line(b"$ ", &mut out);
/// for b in b"rm -rf" {
///     ld.feed(*b, &mut out);
/// }
/// assert_eq!(ld.feed(0x03, &mut out), Event::Interrupt);
///
/// // On a non-empty line, ^D deletes at the cursor instead. ^A goes to the start first.
/// ld.start_line(b"$ ", &mut out);
/// for b in b"xls" {
///     ld.feed(*b, &mut out);
/// }
/// ld.feed(0x01, &mut out); // ^A: home
/// assert_eq!(ld.feed(0x04, &mut out), Event::None); // deletes the 'x', does NOT end input
/// assert_eq!(ld.feed(b'\r', &mut out), Event::Line);
/// assert_eq!(ld.line(), b"ls");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Nothing the caller needs to act on (echo may still have been emitted).
    None,
    /// A line is complete; read it with [`LineDisc::line`] before feeding the next byte.
    Line,
    /// ^D on an empty line: end of input.
    Eof,
    /// ^C: the line in progress was discarded.
    Interrupt,
    /// ^Z: the line in progress was discarded, and whoever runs a foreground job is asked to
    /// suspend it (milestone 48 (job control)). At a prompt it is a fresh line, as `^C` is.
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by lane/48-ctrl-z.
    Suspend,
    /// Tab: nothing was echoed and the line is unchanged. Whoever runs the engine decides what a
    /// Tab means, because completion needs a namespace the engine does not have (§227).
    Tab,
}

/// The ANSI parser's state. `Esc` has seen 0x1b; `Csi` is inside `ESC [`, accumulating numeric
/// parameters; `Ss3` is inside `ESC O` (application-mode cursor keys send SS3, not CSI).
enum EscState {
    Idle,
    Esc,
    Csi { params: [u16; 2], n: usize },
    Ss3,
}

/// The editing engine. One per terminal; a few KiB of fixed buffers, no allocation.
///
/// # Examples
///
/// Typing a line, editing it, and getting it back. The [`Sink`] is where echo goes; here a `Vec`,
/// in the running system an endpoint.
///
/// ```
/// use line_editor::{Event, LineDisc, Sink};
///
/// struct Echo(Vec<u8>);
/// impl Sink for Echo {
///     fn put(&mut self, bytes: &[u8]) {
///         self.0.extend_from_slice(bytes);
///     }
/// }
///
/// let mut out = Echo(Vec::new());
/// let mut ld = LineDisc::new();
/// ld.start_line(b"$ ", &mut out);
///
/// // Type "13", then fix it: ^B moves the cursor left one, onto the 3, and typing 2 **inserts**
/// // before it rather than overwriting it.
/// for b in b"13" {
///     assert_eq!(ld.feed(*b, &mut out), Event::None);
/// }
/// ld.feed(0x02, &mut out); // ^B
/// ld.feed(b'2', &mut out);
///
/// // Carriage return ends the line. The caller reads it before feeding the next byte.
/// assert_eq!(ld.feed(b'\r', &mut out), Event::Line);
/// assert_eq!(ld.line(), b"123");
///
/// // The prompt was echoed, and so was everything typed.
/// assert!(out.0.starts_with(b"$ "));
/// ```
pub struct LineDisc {
    pub(super) buf: [u8; LINE_MAX],
    pub(super) len: usize,
    pub(super) cur: usize,
    /// The completed line, snapshotted when [`Event::Line`] is returned.
    done: [u8; LINE_MAX],
    done_len: usize,
    pub(super) kill: [u8; LINE_MAX],
    pub(super) kill_len: usize,
    pub(super) hist: [([u8; LINE_MAX], usize); HIST],
    pub(super) hist_count: usize,
    pub(super) hist_next: usize,
    /// `Some(k)`: showing the k-th most recent history entry (k in `1..=hist_count`).
    pub(super) browse: Option<usize>,
    stash: ([u8; LINE_MAX], usize, usize),
    pub(super) prompt: [u8; PROMPT_MAX],
    pub(super) prompt_len: usize,
    esc: EscState,
    last_was_cr: bool,
}

impl Default for LineDisc {
    fn default() -> Self {
        Self::new()
    }
}

impl LineDisc {
    /// An empty discipline: no line in progress, no history, prompt unset. `const` on purpose: a
    /// user process here gets one 4 KiB page of stack (`kernel/src/user.rs`'s `USER_STACK_VA`),
    /// and this struct is a few KiB before the temporary a stack-local move would make, so
    /// `components/src/line_editor.rs` holds it in a `static` rather than in `_start`'s own frame
    /// (`display_terminal.rs`'s `TERMINAL` is the same fix for the same reason, one program over).
    pub const fn new() -> Self {
        LineDisc {
            buf: [0; LINE_MAX],
            len: 0,
            cur: 0,
            done: [0; LINE_MAX],
            done_len: 0,
            kill: [0; LINE_MAX],
            kill_len: 0,
            hist: [([0; LINE_MAX], 0); HIST],
            hist_count: 0,
            hist_next: 0,
            browse: None,
            stash: ([0; LINE_MAX], 0, 0),
            prompt: [0; PROMPT_MAX],
            prompt_len: 0,
            esc: EscState::Idle,
            last_was_cr: false,
        }
    }

    /// The completed line, valid after [`Event::Line`] until the next one.
    pub fn line(&self) -> &[u8] {
        &self.done[..self.done_len]
    }

    /// Discard whatever line is in progress: the edit buffer, the cursor, and any history
    /// browsing. History and the kill buffer are untouched, because neither belongs to the line
    /// that was in progress. No echo; the caller prints whatever tells the human why, or nothing
    /// (`^C`'s own `"^C\r\n"` is the caller side of that choice, and [`proto::OPERATION_RAWMODE`](crate::proto::OPERATION_RAWMODE)'s
    /// mode switch prints nothing at all).
    pub fn abandon(&mut self) {
        self.len = 0;
        self.cur = 0;
        self.browse = None;
    }

    /// **Resume a read the reader re-issued after [`proto::FLAG_RETRY`](crate::proto::FLAG_RETRY)**: remember `prompt` as
    /// [`start_line`](LineDisc::start_line) does, but paint nothing, because the screen already
    /// shows the prompt and the half-typed line. Painting again would print them twice.
    pub fn resume_line(&mut self, prompt: &[u8]) {
        let n = prompt.len().min(PROMPT_MAX);
        self.prompt[..n].copy_from_slice(&prompt[..n]);
        self.prompt_len = n;
    }

    /// The line being edited and the cursor's position in it, for a caller answering
    /// [`Event::Tab`]. Unlike [`LineDisc::line`], this is the line in progress, not a finished one.
    pub fn pending(&self) -> (&[u8], usize) {
        (&self.buf[..self.len], self.cur)
    }

    /// Insert `text` at the cursor as if it had been typed, echoing it. Printable bytes only; any
    /// other byte is skipped rather than inserted, so a completion cannot smuggle a control
    /// sequence into the line. What does not fit in [`LINE_MAX`] is refused with a bell.
    pub fn insert_text(&mut self, text: &[u8], out: &mut impl Sink) {
        for &b in text {
            if (0x20..=0x7e).contains(&b) {
                self.insert(b, out);
            }
        }
    }

    /// Paint the prompt and the line again on a fresh row, cursor where it was. For a caller that
    /// printed something between two keystrokes, as a completion listing does.
    pub fn repaint(&mut self, out: &mut impl Sink) {
        out.put(&self.prompt[..self.prompt_len]);
        out.put(&self.buf[..self.len]);
        csi_left(out, self.len - self.cur);
    }

    /// Begin a read: remember `prompt` (for repaints) and paint it, followed by whatever the
    /// user has already typed ahead. Called by the server when a READLINE request arrives.
    pub fn start_line(&mut self, prompt: &[u8], out: &mut impl Sink) {
        let n = prompt.len().min(PROMPT_MAX);
        self.prompt[..n].copy_from_slice(&prompt[..n]);
        self.prompt_len = n;
        out.put(&self.prompt[..n]);
        out.put(&self.buf[..self.len]);
        csi_left(out, self.len - self.cur);
    }

    /// Feed one wire byte; echo goes to `out`. See [`Event`] for what came of it.
    ///
    /// # Examples
    ///
    /// The LF of a CRLF pair is not a second, empty line.
    ///
    /// ```
    /// # use line_editor::{Event, LineDisc, Sink};
    /// # struct Echo(Vec<u8>);
    /// # impl Sink for Echo {
    /// #     fn put(&mut self, bytes: &[u8]) { self.0.extend_from_slice(bytes); }
    /// # }
    /// let mut out = Echo(Vec::new());
    /// let mut ld = LineDisc::new();
    /// ld.start_line(b"", &mut out);
    ///
    /// ld.feed(b'a', &mut out);
    /// assert_eq!(ld.feed(b'\r', &mut out), Event::Line);
    /// assert_eq!(ld.line(), b"a");
    ///
    /// // The LF of the CRLF pair is not a second, empty line.
    /// assert_eq!(ld.feed(b'\n', &mut out), Event::None);
    /// ```
    pub fn feed(&mut self, b: u8, out: &mut impl Sink) -> Event {
        // A bare LF right after a CR is the second half of a CRLF, not a second Enter.
        let was_cr = core::mem::replace(&mut self.last_was_cr, false);
        if b == b'\n' && was_cr {
            return Event::None;
        }

        let state = core::mem::replace(&mut self.esc, EscState::Idle);
        if let Some(event) = self.continue_escape(state, b, out) {
            return event;
        }

        match b {
            ESC => {
                self.esc = EscState::Esc;
                Event::None
            }
            b'\r' | b'\n' => {
                self.last_was_cr = b == b'\r';
                self.finish_line(out)
            }
            0x03 => {
                // ^C: discard the line in progress. The caller routes the interrupt (or, today,
                // merely fails a pending read); see design/interrupt-routing.md.
                out.put(b"^C\r\n");
                self.abandon();
                Event::Interrupt
            }
            0x1a => {
                // ^Z: the same discard as ^C, and the caller decides what a suspend means.
                out.put(b"^Z\r\n");
                self.abandon();
                Event::Suspend
            }
            b'\t' => Event::Tab,
            0x04 => self.eof_or_delete(out),
            0x01 => {
                self.home(out);
                Event::None
            }
            0x05 => {
                self.end(out);
                Event::None
            }
            0x02 => {
                self.left(out);
                Event::None
            }
            0x06 => {
                self.right(out);
                Event::None
            }
            0x0b => {
                // ^K: kill to end of line.
                self.kill_to_end(out);
                Event::None
            }
            0x15 => {
                // ^U: kill to start of line.
                self.kill_to_start(out);
                Event::None
            }
            0x17 => {
                // ^W: kill the word before the cursor (blanks, then non-blanks, like bash).
                self.kill_word_before(out);
                Event::None
            }
            0x19 => {
                self.yank(out);
                Event::None
            }
            0x0c => {
                // ^L: clear the screen, repaint prompt and line. The one operation that needs the
                // prompt, which is why READLINE carries it.
                self.clear_and_repaint(out);
                Event::None
            }
            BS | 0x7f => {
                self.backspace(out);
                Event::None
            }
            0x20..=0x7e => {
                self.insert(b, out);
                Event::None
            }
            _ => Event::None, // other control bytes: ignored, deliberately unecho'd
        }
    }

    /// The byte after an escape introducer, or inside a CSI sequence, in escape state `state`.
    /// `None` when no escape was in progress and the byte is an ordinary key.
    fn continue_escape(&mut self, state: EscState, b: u8, out: &mut impl Sink) -> Option<Event> {
        match state {
            EscState::Idle => None,
            EscState::Esc => {
                match b {
                    b'[' => {
                        self.esc = EscState::Csi {
                            params: [0; 2],
                            n: 0,
                        }
                    }
                    b'O' => self.esc = EscState::Ss3,
                    _ => {} // an escape we do not speak; drop it and the introducer
                }
                Some(Event::None)
            }
            EscState::Csi { mut params, mut n } => {
                match b {
                    b'0'..=b'9' => {
                        let i = n.min(1);
                        params[i] = params[i].saturating_mul(10) + (b - b'0') as u16;
                        self.esc = EscState::Csi { params, n };
                    }
                    b';' => {
                        n += 1;
                        self.esc = EscState::Csi { params, n };
                    }
                    // A final byte: dispatch. Unknown finals are dropped whole, which is what
                    // makes unrecognized keys (F-keys, mouse noise) harmless.
                    0x40..=0x7e => return Some(self.csi_final(b, params[0], out)),
                    _ => {} // malformed; abandon
                }
                Some(Event::None)
            }
            EscState::Ss3 => Some(self.csi_final(b, 0, out)),
        }
    }

    /// ^D: end of input on an empty line, otherwise delete the byte under the cursor.
    fn eof_or_delete(&mut self, out: &mut impl Sink) -> Event {
        if self.len == 0 {
            Event::Eof
        } else {
            self.delete_at(out);
            Event::None
        }
    }

    /// ^K: move everything from the cursor to the end of the line into the kill buffer.
    fn kill_to_end(&mut self, out: &mut impl Sink) {
        self.kill[..self.len - self.cur].copy_from_slice(&self.buf[self.cur..self.len]);
        self.kill_len = self.len - self.cur;
        self.len = self.cur;
        out.put(b"\x1b[K");
    }

    /// ^U: move everything before the cursor into the kill buffer.
    fn kill_to_start(&mut self, out: &mut impl Sink) {
        self.kill[..self.cur].copy_from_slice(&self.buf[..self.cur]);
        self.kill_len = self.cur;
        self.delete_range(0, self.cur, out);
    }

    /// ^W: move the word before the cursor (blanks, then non-blanks) into the kill buffer.
    fn kill_word_before(&mut self, out: &mut impl Sink) {
        let mut a = self.cur;
        while a > 0 && self.buf[a - 1] == b' ' {
            a -= 1;
        }
        while a > 0 && self.buf[a - 1] != b' ' {
            a -= 1;
        }
        self.kill[..self.cur - a].copy_from_slice(&self.buf[a..self.cur]);
        self.kill_len = self.cur - a;
        self.delete_range(a, self.cur, out);
    }

    /// ^L: clear the screen and paint the prompt and the line, cursor where it was.
    fn clear_and_repaint(&mut self, out: &mut impl Sink) {
        out.put(b"\x1b[2J\x1b[H");
        out.put(&self.prompt[..self.prompt_len]);
        out.put(&self.buf[..self.len]);
        csi_left(out, self.len - self.cur);
    }

    /// A CSI (or SS3) final byte, with its first numeric parameter.
    fn csi_final(&mut self, b: u8, p: u16, out: &mut impl Sink) -> Event {
        match (b, p) {
            (b'D', _) => self.left(out),
            (b'C', _) => self.right(out),
            (b'A', _) => self.hist_prev(out),
            (b'B', _) => self.hist_next_entry(out),
            (b'H', _) | (b'~', 1) | (b'~', 7) => self.home(out),
            (b'F', _) | (b'~', 4) | (b'~', 8) => self.end(out),
            (b'~', 3) => self.delete_at(out),
            _ => {} // unknown key; ignore
        }
        Event::None
    }

    fn finish_line(&mut self, out: &mut impl Sink) -> Event {
        out.put(b"\r\n");
        self.done[..self.len].copy_from_slice(&self.buf[..self.len]);
        self.done_len = self.len;
        // Record non-empty lines, skipping an exact repeat of the newest entry.
        if self.len > 0 && !self.repeats_newest() {
            self.hist[self.hist_next].0[..self.len].copy_from_slice(&self.buf[..self.len]);
            self.hist[self.hist_next].1 = self.len;
            self.hist_next = (self.hist_next + 1) % HIST;
            self.hist_count = (self.hist_count + 1).min(HIST);
        }
        self.len = 0;
        self.cur = 0;
        self.browse = None;
        // The prompt belonged to the read that just completed; the next read brings its own.
        self.prompt_len = 0;
        Event::Line
    }

    fn repeats_newest(&self) -> bool {
        if self.hist_count == 0 {
            return false;
        }
        let (entry, elen) = &self.hist[(self.hist_next + HIST - 1) % HIST];
        *elen == self.len && entry[..*elen] == self.buf[..self.len]
    }

    fn insert(&mut self, b: u8, out: &mut impl Sink) {
        if self.len >= LINE_MAX {
            out.put(&[BEL]);
            return;
        }
        self.buf.copy_within(self.cur..self.len, self.cur + 1);
        self.buf[self.cur] = b;
        self.len += 1;
        self.cur += 1;
        if self.cur == self.len {
            // The common case, typing at the end: echo the byte and nothing else.
            out.put(&[b]);
        } else {
            // Mid-line: echo the byte and the shifted tail, then walk back over the tail.
            out.put(&self.buf[self.cur - 1..self.len]);
            csi_left(out, self.len - self.cur);
        }
    }

    fn backspace(&mut self, out: &mut impl Sink) {
        if self.cur == 0 {
            return;
        }
        self.delete_range(self.cur - 1, self.cur, out);
    }

    fn delete_at(&mut self, out: &mut impl Sink) {
        if self.cur == self.len {
            return;
        }
        self.delete_range(self.cur, self.cur + 1, out);
    }

    /// Remove `buf[a..b]` and repaint what changed: step back to column `a`, print the shifted
    /// tail, erase the leftover columns, step back to the cursor. CSI K does the erasing, so no
    /// space-padding arithmetic. Works for both directions: a backward kill has the cursor at
    /// `b` (it moves left with the deletion), a forward delete has it at `a` (it stays put).
    fn delete_range(&mut self, a: usize, b: usize, out: &mut impl Sink) {
        debug_assert!(a <= b && b <= self.len && a <= self.cur);
        let removed = b - a;
        if removed == 0 {
            return;
        }
        let old_cur = self.cur;
        self.buf.copy_within(b..self.len, a);
        self.len -= removed;
        self.cur = if old_cur >= b {
            old_cur - removed
        } else {
            old_cur.min(a)
        };
        csi_left(out, old_cur - a);
        out.put(&self.buf[a..self.len]);
        out.put(b"\x1b[K");
        csi_left(out, self.len - self.cur);
    }

    fn yank(&mut self, out: &mut impl Sink) {
        let n = self.kill_len.min(LINE_MAX - self.len);
        if n == 0 {
            if self.kill_len > 0 {
                out.put(&[BEL]); // there was something to yank and no room for it
            }
            return;
        }
        self.buf.copy_within(self.cur..self.len, self.cur + n);
        // A yank of the kill buffer into the line it was cut from cannot overlap: kill is a
        // separate buffer, copied out at kill time.
        let kill = self.kill;
        self.buf[self.cur..self.cur + n].copy_from_slice(&kill[..n]);
        self.len += n;
        self.cur += n;
        out.put(&self.buf[self.cur - n..self.len]);
        csi_left(out, self.len - self.cur);
    }

    fn left(&mut self, out: &mut impl Sink) {
        if self.cur > 0 {
            self.cur -= 1;
            out.put(&[BS]);
        }
    }

    fn right(&mut self, out: &mut impl Sink) {
        if self.cur < self.len {
            self.cur += 1;
            out.put(b"\x1b[C");
        }
    }

    fn home(&mut self, out: &mut impl Sink) {
        csi_left(out, self.cur);
        self.cur = 0;
    }

    fn end(&mut self, out: &mut impl Sink) {
        csi_right(out, self.len - self.cur);
        self.cur = self.len;
    }

    /// Up arrow: show the previous (older) history entry. The first press stashes the line in
    /// progress so browsing is non-destructive.
    fn hist_prev(&mut self, out: &mut impl Sink) {
        let k = match self.browse {
            None => {
                if self.hist_count == 0 {
                    out.put(&[BEL]);
                    return;
                }
                self.stash.0[..self.len].copy_from_slice(&self.buf[..self.len]);
                self.stash.1 = self.len;
                self.stash.2 = self.cur;
                1
            }
            Some(k) if k < self.hist_count => k + 1,
            Some(_) => {
                out.put(&[BEL]); // already at the oldest
                return;
            }
        };
        self.browse = Some(k);
        let (entry, elen) = self.hist[(self.hist_next + HIST - k) % HIST];
        self.replace_line(&entry[..elen], out);
    }

    /// Down arrow: toward newer entries; past the newest, restore the stashed line.
    fn hist_next_entry(&mut self, out: &mut impl Sink) {
        match self.browse {
            None => out.put(&[BEL]),
            Some(1) => {
                self.browse = None;
                let (stash, slen, scur) = self.stash;
                self.replace_line(&stash[..slen], out);
                // replace_line leaves the cursor at the end; restore the stashed column.
                csi_left(out, slen - scur);
                self.cur = scur;
            }
            Some(k) => {
                self.browse = Some(k - 1);
                let (entry, elen) = self.hist[(self.hist_next + HIST - (k - 1)) % HIST];
                self.replace_line(&entry[..elen], out);
            }
        }
    }

    /// Replace the whole edit line with `text` and repaint the input region, cursor at the end.
    fn replace_line(&mut self, text: &[u8], out: &mut impl Sink) {
        csi_left(out, self.cur);
        let n = text.len().min(LINE_MAX);
        self.buf[..n].copy_from_slice(&text[..n]);
        self.len = n;
        self.cur = n;
        out.put(&self.buf[..n]);
        out.put(b"\x1b[K");
    }
}

/// Emit `CSI <n> D` (cursor left `n`), eliding the count when it is 1 and the whole sequence
/// when it is 0.
fn csi_left(out: &mut impl Sink, n: usize) {
    csi_move(out, n, b'D');
}

/// Emit `CSI <n> C` (cursor right `n`), same elisions.
fn csi_right(out: &mut impl Sink, n: usize) {
    csi_move(out, n, b'C');
}

fn csi_move(out: &mut impl Sink, n: usize, dir: u8) {
    match n {
        0 => {}
        1 => out.put(&[ESC, b'[', dir]),
        _ => {
            // One contiguous put per sequence: a sink must never see half an escape.
            let mut seq = [0u8; 11];
            seq[0] = ESC;
            seq[1] = b'[';
            let mut i = 10;
            let mut v = n;
            while v > 0 {
                i -= 1;
                seq[i] = b'0' + (v % 10) as u8;
                v /= 10;
            }
            seq.copy_within(i..10, 2);
            let digits = 10 - i;
            seq[2 + digits] = dir;
            out.put(&seq[..3 + digits]);
        }
    }
}

#[cfg(test)]
mod tests;
