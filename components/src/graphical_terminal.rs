//! The graphical terminal: a prompt on the display, launched from the swish prompt (milestone 632
//! (graphics on demand)).
//!
//! The progenitor builds the stack from the display devices the shell delegated when this program's
//! name was typed. `x0` picks one of two arms, both a prompt that echoes what is typed: arm 0, when
//! a virtio keyboard came with the gpu, reads cooked lines from the session's own line discipline;
//! arm 1 reads the boot discipline raw over the UART and paints the echo itself, the shell's §227
//! (how Tab reaches the shell) shape. `^C` or a line reading `quit` ends a session. It holds no
//! device, interrupt or DMA, only the endpoints and pages at [`REPORT`], [`WRITE`], [`KEYS`],
//! [`OUT_VA`] and [`IN_VA`]. The two arms, drawn, are in notes/line-discipline.md.

//! # BUGS
//!
//! - A session runs no program: its lines are echoed and nothing else, so a full-screen program
//!   cannot reach `display_terminal`, which milestone 705 (the graphical terminal runs full-screen
//!   programs) needs.
//! - Arm 1 has no line editing: backspace and the arrows are painted as the control bytes they are.
//! - Arm 1 can still switch the boot discipline's mode under the shell: a disruption, not
//!   authority (notes/line-discipline.md).
//!
//! Name: ratified 2026-10-03 (calef, #1493). Refused `screen` (clashes with `SCREEN_BIT` of §106
//! (take the `terminal_sink_caretaker` narrowing), and with GNU screen), `gate` (the tree's word
//! for a check that fails loudly, and the x86 IDT's interrupt gate), `display_session` and
//! `display_console` (`console` is the UART server). It names the whole stack, so it survives any
//! later change of VT engine behind `display_terminal`.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates: one lint policy)'s ratchet tracks
// (§107 (`missing_docs` moves to the workspace lints, opt-out rather than opt-in)): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use line_editor::proto;
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{call, exit, send};

/// The result endpoint (slot 0): one word when the session ends, the shape every reporting program
/// answers.
const REPORT: u64 = 0;
/// **The screen's write endpoint** (slot 1): arm 0's line discipline, arm 1's `display_terminal`.
/// On either arm the contract is `OPERATION_WRITE` over one page, `line_editor::proto`'s
/// control-by-message, bulk-by-shared-page split.
const WRITE: u64 = 1;
/// **The keystroke endpoint** (slot 2): arm 0's line discipline again (it serves both classes on
/// one endpoint, `display_terminal`'s own one-wait-point rule), arm 1's boot line discipline,
/// which this program reads raw.
const KEYS: u64 = 2;

/// The page the write endpoint's server reads. Which frame backs it is the progenitor's business;
/// on arm 0 it is the page the discipline reads (`line_editor`'s `APP_OUT`), on arm 1 the page
/// `display_terminal` reads (its `OUT_VA`). Must match the spawn wiring; the address is ours.
const OUT_VA: u64 = address_space_map::pair_page(0x0000_0000_00a0_0000);
/// The page the discipline delivers completed lines into, read-only. Arm 0 only; arm 1's reads
/// are register-only and no second page is mapped.
const IN_VA: u64 = address_space_map::pair_page(0x0000_0000_00a1_0000);

// SAFETY: the wiring maps OUT_VA read/write before this program runs, on either arm; nothing
// below writes it before a flush-sized batch is composed and never past one page.
const OUT_WINDOW: MappedWindow = unsafe { MappedWindow::new(OUT_VA, 4096) };
// SAFETY: arm 0's wiring maps IN_VA read-only; arm 1 never reads through it at all.
const IN_WINDOW: MappedWindow = unsafe { MappedWindow::new(IN_VA, 4096) };

/// The prompt. Two bytes, the same `$ ` every nife prompt prints, so a reader decoding the
/// scanout needs no second spelling of one program's name.
const PROMPT: &[u8] = b"$ ";

/// The line that ends a session, judged after the editing is done so a half-typed `qui` is just
/// input, exactly as it would be at the swish prompt.
const QUIT: &[u8] = b"quit";

/// Batch the screen's echo the way the shell's own `Echo` does: fill the page, flush with one
/// `CALL`, so a burst of typing is one round trip per page rather than one per byte.
struct Echo {
    used: usize,
}

impl Echo {
    fn put(&mut self, bytes: &[u8]) {
        for &b in bytes {
            if self.used == 4096 {
                self.flush();
            }
            OUT_WINDOW.w8(self.used as u64, b);
            self.used += 1;
        }
    }

    fn flush(&mut self) {
        if self.used > 0 {
            call(
                WRITE,
                proto::req(proto::OPERATION_WRITE, self.used as u64),
                0,
            );
            self.used = 0;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(arm: u64, _a1: u64, _a2: u64) -> ! {
    match arm {
        1 => raw_arm(),
        _ => discipline_arm(),
    }
}

/// **Arm 0: a keyboard came with the gpu, so a line discipline did too.** The discipline does
/// the terminal's whole job, cooked `OPERATION_READLINE` (milestone 28 (a solid terminal: the line discipline as a component)'s original contract): it paints
/// the prompt, echoes each keystroke, moves on enter, and hands this program the completed line,
/// so this arm's loop is a prompt delivered, a line read, and nothing painted by the program
/// itself. `quit` and `^C`/EOF (the discipline's flags) are the two ways out.
fn discipline_arm() -> ! {
    loop {
        // The prompt rides the READLINE request's own page: the discipline reads it from there
        // and paints it, which is what keeps the prompt and the echoing cursor in one place.
        for (i, &b) in PROMPT.iter().enumerate() {
            OUT_WINDOW.w8(i as u64, b);
        }
        let (len, flags) = call(
            KEYS,
            proto::req(proto::OPERATION_READLINE, PROMPT.len() as u64),
            0,
        );
        if flags & (proto::FLAG_EOF | proto::FLAG_INTERRUPTED) != 0 {
            // EOF or `^C`: the session is over either way. The discipline already moved the
            // cursor for the interrupted line's sake; say nothing and end cleanly.
            break;
        }
        let len = (len as usize).min(4096);
        let mut line = [0u8; 4096];
        for (i, b) in line.iter_mut().enumerate().take(len) {
            *b = IN_WINDOW.r8(i as u64);
        }
        if line[..len] == QUIT[..] {
            break;
        }
    }
    end()
}

/// **Arm 1: no keyboard, so the keystrokes are the boot prompt's own, over the UART.** Raw mode
/// on the boot discipline (`OPERATION_RAWMODE`), register-only reads (`OPERATION_READRAW`), echo painted here:
/// the shell's own §227 shape, run against a screen instead of a UART console. The discipline's
/// cooked-mode echo is bypassed in raw mode, so nothing double-echoes on either surface.
fn raw_arm() -> ! {
    // The shell keeps the discipline raw at the prompt (§227 option D), so this is usually
    // already the state; asking anyway is what makes the session correct when it is not.
    call(KEYS, proto::req(proto::OPERATION_RAWMODE, 1), 0);
    let mut echo = Echo { used: 0 };
    echo.put(PROMPT);
    echo.flush();
    let mut line = [0u8; 4096];
    let mut n = 0usize;
    'session: loop {
        let (got, packed) = call(KEYS, proto::req(proto::OPERATION_READRAW, 0), 0);
        if got == proto::BAD_REQUEST {
            // Somebody left the discipline cooked between our reads. Take raw mode back and ask
            // again, the same recovery the shell's own edit loop performs.
            call(KEYS, proto::req(proto::OPERATION_RAWMODE, 1), 0);
            continue;
        }
        let got = (got as usize).min(8);
        let bytes = packed.to_le_bytes();
        for &b in bytes.iter().take(got) {
            if b == 0x03 {
                // `^C` ends the session, and this arm feels it as the byte it is.
                break 'session;
            }
            if b == b'\r' || b == b'\n' {
                if line[..n] == QUIT[..] {
                    break 'session;
                }
                echo.put(b"\n");
                n = 0;
            } else if n < line.len() {
                // Echo as the byte arrives: the round trip this arm exists to prove.
                echo.put(&[b]);
                line[n] = b;
                n += 1;
            }
        }
        echo.flush();
    }
    // Put the boot discipline back the way the prompt found it. A session the kernel killed
    // skips this, and the shell's `BAD_REQUEST` recovery covers it from its side.
    call(KEYS, proto::req(proto::OPERATION_RAWMODE, 0), 0);
    end()
}

/// One word on the result endpoint, then exit: the session ran and is over.
fn end() -> ! {
    send(REPORT, 0, 0, 0);
    exit()
}

user_mode_runtime::panic_handler!();
