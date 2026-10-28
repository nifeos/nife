//! **The terminal replay harness of milestone 705 (the graphical terminal runs full-screen
//! programs)'s step 0a: recordings in, match counts out, a failure the moment a count drops.**
//!
//! It replays each recording's output bytes through [`video_terminal::Vt`], snapshots the text grid
//! just before every recorded keystroke, compares each snapshot with the oracle's screen committed
//! beside the recording, and holds every session's match count to `manifest.toml`'s. A count that
//! drops fails the gate; a count that rises fails until the manifest is raised, so the tripwire
//! always stands at the newest high-water mark. Grading needs no pty, no installed program and no
//! emulator; it runs in milliseconds.
//!
//! The oracle is `alacritty_terminal` 0.26.0, calef's ruling of 2026-10-21 03:37 UTC. His
//! 2026-10-19 hold on its §46 (thin primitives or whole subsystems; we write everything in between)
//! take keeps its generator out of the tree, so the oracle's screens are committed artifacts with
//! provenance in `crates/video_terminal/recordings/README.md`, beside the recording format, the
//! manifest's fields and the recording recipe.
//!
//! # EXAMPLES
//!
//! ```text
//! $ cargo xtask vt-replay                 # grade every recording, hold every count
//! $ cargo xtask vt-replay --record vim    # capture a session; needs the program and script(1)
//! $ cargo xtask vt-replay --list
//! ```
//!
//! # BUGS
//!
//! - **The comparison is text only.** Attributes, colors and the cursor are not compared, so the
//!   counts flatter every engine, ours included, exactly as the census's counts do.
//! - **A wide character loses the column match by construction.** `Vt` holds one `char` per cell
//!   until the plan's step 7; the oracle gives a wide character two columns. Any checkpoint whose
//!   screen carries one fails, and the census's counts carry the same caveat.
//! - **Recording is macOS-only** (`script(1)` and the census's Homebrew programs); grading runs
//!   anywhere.
//! - **A recording is one run.** Sessions are timing-shaped, so re-recording a session produces a
//!   different byte stream; a recording, its expected screens and its manifest row are one
//!   self-consistent triple and move together or not at all.
//!
//! Name: provisional. The subcommand (`vt-replay`), the module, the recording format's magic, the
//! `recordings` directory and every type here are the step 0a lane's wording, 2026-10-28 (UTC);
//! calef names things.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use video_terminal::{Vt, script};

use crate::host::workspace_root;

/// Where the recordings, the expected screens and the manifest live: beside the engine they grade.
fn recordings_dir() -> PathBuf {
    workspace_root().join("crates/video_terminal/recordings")
}

/// The file name of one session's recording.
fn recording_path(name: &str) -> PathBuf {
    recordings_dir().join(format!("{name}.rec"))
}

/// The file name of one session's committed oracle screens.
fn expected_path(name: &str) -> PathBuf {
    recordings_dir().join(format!("{name}.expected.txt"))
}

/// The file name of the manifest.
fn manifest_path() -> PathBuf {
    recordings_dir().join("manifest.toml")
}

/// [`script::COLS`] as the `usize` the text rows are built with.
const COLS: usize = script::COLS as usize;
/// [`script::ROWS`] as the `usize` the text rows are built with.
const ROWS: usize = script::ROWS as usize;

/// `vt-replay`'s entry: grade by default, `--record <session>` to capture a new one, `--list` to
/// print the manifest.
pub(crate) fn vt_replay() -> bool {
    let args: Vec<String> = std::env::args().skip(2).collect();
    match args.first().map(String::as_str) {
        None => grade_all(),
        Some("--list") => list(),
        Some("--record") => match args.get(1) {
            Some(name) => record(name),
            None => {
                eprintln!("vt-replay: --record needs a session name (`--list` prints them)");
                false
            }
        },
        Some(other) => {
            eprintln!(
                "vt-replay: unknown argument {other}; grade (no argument), --record <name> or --list"
            );
            false
        }
    }
}

// --- the recording format ----------------------------------------------------------------------

/// The recording file's magic and the version this reader takes.
const MAGIC: &[u8; 4] = b"NVTR";
/// The only recording version this reader accepts.
const VERSION: u16 = 1;
/// The event tag preceding a run of the program's output.
const TAG_OUTPUT: u8 = 0x00;
/// The event tag preceding one keystroke's input bytes.
const TAG_KEYSTROKE: u8 = 0x01;

/// One keystroke of a recorded session: where the output stream stood when it was sent, and what
/// was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Keystroke {
    /// How many output bytes the program had written when the keystroke was sent.
    at: usize,
    /// The bytes typed.
    bytes: Vec<u8>,
}

/// A recorded session: the bytes a program wrote, plus the offsets of its keystrokes.
///
/// [`Recording::to_bytes`] and [`Recording::from_bytes`] are the file format of the module doc.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Recording {
    /// The session's name, as the manifest names it.
    name: String,
    /// Every byte the program wrote, in order.
    output: Vec<u8>,
    /// One entry per keystroke, in the order sent, each `at` no greater than `output.len()`.
    keystrokes: Vec<Keystroke>,
}

impl Recording {
    /// Assemble the recording from an output stream and the keystrokes taken against it.
    ///
    /// Panics if a keystroke's offset exceeds the output written, which only a caller bug can
    /// arrange: the capture helper reads its offsets from the same counter that bounds the output.
    fn new(name: &str, output: Vec<u8>, keystrokes: Vec<Keystroke>) -> Recording {
        assert!(
            keystrokes.iter().all(|k| k.at <= output.len()),
            "keystroke offset past the recorded output"
        );
        Recording {
            name: name.to_string(),
            output,
            keystrokes,
        }
    }

    /// The recording as the file format's bytes.
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.output.len() + 16 * self.keystrokes.len() + 6);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        let mut fed = 0usize;
        for key in &self.keystrokes {
            write_event(&mut out, TAG_OUTPUT, &self.output[fed..key.at]);
            fed = key.at;
            write_event(&mut out, TAG_KEYSTROKE, &key.bytes);
        }
        write_event(&mut out, TAG_OUTPUT, &self.output[fed..]);
        out
    }

    /// Parse the file format's bytes, refusing anything a reader of this version cannot trust.
    fn from_bytes(name: &str, bytes: &[u8]) -> Result<Recording, String> {
        if bytes.len() < 6 || &bytes[0..4] != MAGIC {
            return Err("not a recording (the magic is missing)".to_string());
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != VERSION {
            return Err(format!(
                "recording version {version}, this reader takes {VERSION}"
            ));
        }
        let mut output = Vec::new();
        let mut keystrokes = Vec::new();
        let mut at = 6usize;
        while at < bytes.len() {
            let tag = bytes[at];
            at += 1;
            let Some(len) = read_u32(bytes, &mut at) else {
                return Err("the file ends inside an event's length".to_string());
            };
            let body = bytes
                .get(at..at + len as usize)
                .ok_or("the file ends inside an event's body")?;
            at += len as usize;
            match tag {
                TAG_OUTPUT => output.extend_from_slice(body),
                TAG_KEYSTROKE => keystrokes.push(Keystroke {
                    at: output.len(),
                    bytes: body.to_vec(),
                }),
                other => return Err(format!("unknown event tag {other:#04x}")),
            }
        }
        Ok(Recording::new(name, output, keystrokes))
    }
}

/// Append one tagged event.
fn write_event(out: &mut Vec<u8>, tag: u8, body: &[u8]) {
    out.push(tag);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
}

/// Read a `u32` little-endian at `at`, advancing it, or `None` at the end of the buffer.
fn read_u32(bytes: &[u8], at: &mut usize) -> Option<u32> {
    let end = *at + 4;
    let slice = bytes.get(*at..end)?;
    *at = end;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

// --- the expected screens ----------------------------------------------------------------------

/// The oracle's screens: one block of [`ROWS`] rows per graded checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedScreens {
    blocks: Vec<Vec<String>>,
}

impl ExpectedScreens {
    /// Parse an expected file: `@ checkpoint N` lines in order from 0, each followed by exactly
    /// `rows` rows. Refuses a file whose shape has drifted, since a silent misalignment would
    /// grade one session against another's screens.
    fn parse(text: &str, rows: usize) -> Result<ExpectedScreens, String> {
        let mut blocks = Vec::new();
        let mut current: Option<Vec<String>> = None;
        let mut want = 0usize;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("@ checkpoint ") {
                if let Some(open) = current.take() {
                    if open.len() != rows {
                        return Err(format!(
                            "checkpoint {} has {} rows, it takes {rows}",
                            want - 1,
                            open.len()
                        ));
                    }
                    blocks.push(open);
                }
                if rest.trim() != want.to_string() {
                    return Err(format!(
                        "checkpoint marker says {rest:?}, the file wants {want}"
                    ));
                }
                current = Some(Vec::with_capacity(rows));
                want += 1;
            } else if !line.is_empty() || current.is_some() {
                match current.as_mut() {
                    Some(open) => open.push(normalize_row(line)),
                    None => return Err("a row before the first checkpoint marker".to_string()),
                }
            }
        }
        if let Some(open) = current.take() {
            if open.len() != rows {
                return Err(format!(
                    "checkpoint {} has {} rows, it takes {rows}",
                    want - 1,
                    open.len()
                ));
            }
            blocks.push(open);
        }
        Ok(ExpectedScreens { blocks })
    }
}

/// One grid row as the comparison sees it: control characters as blanks, trailing blanks trimmed.
fn normalize_row(row: &str) -> String {
    row.chars()
        .map(|c| if c < ' ' || c == '\x7f' { ' ' } else { c })
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// The engine grid's [`ROWS`] rows, normalized the way an expected screen's rows are.
fn row_texts(vt: &Vt) -> Vec<String> {
    (0..ROWS)
        .map(|row| {
            (0..COLS)
                .map(|col| vt.cell(col as u32, row as u32).ch)
                .map(|c| if c < ' ' || c == '\x7f' { ' ' } else { c })
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

// --- the manifest ------------------------------------------------------------------------------

/// One session's committed row: its provenance, its checkpoint count, and the match count the gate
/// holds.
#[derive(Debug, Clone)]
struct SessionRow {
    /// The session's name, the manifest table's key, which is also the file names' stem.
    name: String,
    /// `census` (a full-screen program's session) or `vttest` (one vttest menu's walk).
    kind: String,
    /// What ran, with its version, as the census names it.
    program: String,
    /// The TERM the session ran under.
    term: String,
    /// The capture date, UTC.
    captured: String,
    /// What the session did, in the census's shorthand.
    session: String,
    /// The keystroke count the recording carries.
    checkpoints: usize,
    /// The match count the gate holds: drops fail, rises fail until this is raised.
    matches: usize,
    /// Per-checkpoint screen labels for a `vttest` session; absent for a census session, where
    /// every checkpoint is graded.
    screens: Option<Vec<String>>,
}

/// The manifest: every session's row, in file order.
#[derive(Debug, Clone)]
struct Manifest {
    sessions: Vec<SessionRow>,
}

impl Manifest {
    /// Parse the manifest's TOML, refusing a file whose rows do not carry every field the gate
    /// reads.
    fn parse(text: &str) -> Result<Manifest, String> {
        let table: toml::Table =
            toml::from_str(text).map_err(|e| format!("manifest.toml is not valid TOML: {e}"))?;
        let sessions = table
            .get("sessions")
            .and_then(|v| v.as_table())
            .ok_or("manifest.toml has no [sessions] table")?;
        let mut rows = Vec::new();
        for (name, value) in sessions {
            let row = value
                .as_table()
                .ok_or_else(|| format!("session {name} is not a table"))?;
            let field = |key: &str| -> Result<String, String> {
                row.get(key)
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .ok_or_else(|| format!("session {name} has no {key}"))
            };
            let count = |key: &str| -> Result<usize, String> {
                row.get(key)
                    .and_then(|v| v.as_integer())
                    .map(|v| v as usize)
                    .ok_or_else(|| format!("session {name} has no integer {key}"))
            };
            rows.push(SessionRow {
                name: name.clone(),
                kind: field("kind")?,
                program: field("program")?,
                term: field("term")?,
                captured: field("captured")?,
                session: field("session")?,
                checkpoints: count("checkpoints")?,
                matches: count("matches")?,
                screens: row.get("screens").and_then(|v| v.as_array()).map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                }),
            });
        }
        Ok(Manifest { sessions: rows })
    }

    /// Read the manifest from the tree.
    fn load() -> Result<Manifest, String> {
        let path = manifest_path();
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Manifest::parse(&text)
    }
}

/// Is this vttest screen label one the gate grades? `skip` is the 132-column screens the census
/// excludes; `menu` is vttest's own menu screens. A label the gate grades names the screen's place
/// in its menu, `N.M` as conformance.md numbers them, 132-column screens included.
fn is_graded(label: &str) -> bool {
    let bytes = label.as_bytes();
    !bytes.is_empty()
        && bytes.iter().filter(|b| **b == b'.').count() == 1
        && bytes[0].is_ascii_digit()
        && bytes[bytes.len() - 1].is_ascii_digit()
}

// --- grading -----------------------------------------------------------------------------------

/// Grade one session: replay the recording through `Vt`, compare every graded checkpoint with the
/// oracle's committed screen, and report the count.
fn grade_session(
    recording: &Recording,
    row: &SessionRow,
    expected: &ExpectedScreens,
) -> Result<(usize, usize), String> {
    if recording.keystrokes.len() != row.checkpoints {
        return Err(format!(
            "the recording has {} keystrokes, the manifest says {} checkpoints",
            recording.keystrokes.len(),
            row.checkpoints
        ));
    }
    let labels: &[String] = match &row.screens {
        Some(screens) => {
            if screens.len() != row.checkpoints {
                return Err(format!(
                    "the manifest lists {} screen labels for {} checkpoints",
                    screens.len(),
                    row.checkpoints
                ));
            }
            screens.as_slice()
        }
        None => &[],
    };
    // A census session grades every checkpoint; a vttest session grades the labeled ones.
    let graded_labels: Vec<&str> = if labels.is_empty() {
        (0..row.checkpoints).map(|_| "").collect()
    } else {
        labels
            .iter()
            .map(String::as_str)
            .filter(|label| is_graded(label))
            .collect()
    };
    if expected.blocks.len() != graded_labels.len() {
        return Err(format!(
            "{}.expected.txt holds {} blocks, the manifest grades {}",
            recording.name,
            expected.blocks.len(),
            graded_labels.len()
        ));
    }

    let mut vt = Vt::new(script::COLS, script::ROWS);
    let mut fed = 0usize;
    let mut block = 0usize;
    let mut matched = 0usize;
    for (i, key) in recording.keystrokes.iter().enumerate() {
        vt.feed(&recording.output[fed..key.at]);
        fed = key.at;
        let graded = match labels.get(i) {
            Some(label) => is_graded(label),
            None => true,
        };
        if graded {
            let want = expected
                .blocks
                .get(block)
                .ok_or("a graded checkpoint with no expected block")?;
            if row_texts(&vt) == *want {
                matched += 1;
            }
            block += 1;
        }
    }
    // The bytes after the last keystroke still belong to the session (the program's exit), but no
    // checkpoint sits there: nothing follows to grade them against.
    vt.feed(&recording.output[fed..]);
    Ok((matched, block))
}

/// Grade one manifest row: read its recording and its expected screens, replay and compare.
fn grade_one(row: &SessionRow) -> Result<(usize, usize), String> {
    let bytes = std::fs::read(recording_path(&row.name))
        .map_err(|_| format!("{}: the recording file is missing", row.name))?;
    let recording =
        Recording::from_bytes(&row.name, &bytes).map_err(|e| format!("{}: {e}", row.name))?;
    let expected_text = std::fs::read_to_string(expected_path(&row.name)).map_err(|_| {
        format!(
            "{}: the expected screens are missing ({} does not exist)",
            row.name,
            expected_path(&row.name).display()
        )
    })?;
    let expected =
        ExpectedScreens::parse(&expected_text, ROWS).map_err(|e| format!("{}: {e}", row.name))?;
    grade_session(&recording, row, &expected)
}

/// The gate: grade every session in the manifest and hold every count.
fn grade_all() -> bool {
    let manifest = match Manifest::load() {
        Ok(manifest) => manifest,
        Err(e) => {
            eprintln!("vt-replay: {e}");
            return false;
        }
    };
    if manifest.sessions.is_empty() {
        eprintln!("vt-replay: manifest.toml records no sessions");
        return false;
    }
    let dir = recordings_dir();
    println!(
        "==> vt-replay: grading {} recordings from {}",
        manifest.sessions.len(),
        dir.strip_prefix(workspace_root())
            .unwrap_or(dir.as_path())
            .display()
    );
    let mut census = (0usize, 0usize);
    let mut vttest = (0usize, 0usize);
    let mut drifted = Vec::new();
    for row in &manifest.sessions {
        let (matched, graded) = match grade_one(row) {
            Ok(counts) => counts,
            Err(e) => {
                eprintln!("vt-replay: {e}");
                drifted.push(row.name.clone());
                continue;
            }
        };
        let totals = if row.kind == "vttest" {
            &mut vttest
        } else {
            &mut census
        };
        totals.0 += matched;
        totals.1 += graded;
        if matched < row.matches {
            eprintln!(
                "{}: {matched} of {graded} checkpoints match the oracle, fewer than the committed {}: \
                 an engine change lost ground",
                row.name, row.matches
            );
            drifted.push(row.name.clone());
        } else if matched > row.matches {
            eprintln!(
                "{}: {matched} of {graded} checkpoints match the oracle, more than the committed {}: \
                 raise `matches` in manifest.toml so the next drop is caught from the new mark",
                row.name, row.matches
            );
            drifted.push(row.name.clone());
        } else {
            println!(
                "{name}: {matched} of {graded} checkpoints match the oracle",
                name = row.name
            );
        }
    }
    println!(
        "==> census total: {} of {}, vttest total: {} of {}",
        census.0, census.1, vttest.0, vttest.1
    );
    if drifted.is_empty() {
        println!("vt-replay: every count holds");
        true
    } else {
        eprintln!("vt-replay: FAILED ({})", drifted.join(", "));
        false
    }
}

/// `--list`: the manifest as the reader meets it.
fn list() -> bool {
    let manifest = match Manifest::load() {
        Ok(manifest) => manifest,
        Err(e) => {
            eprintln!("vt-replay: {e}");
            return false;
        }
    };
    for row in manifest.sessions {
        let screens = match &row.screens {
            Some(screens) => format!(
                ", {} graded of {} checkpoints",
                screens.iter().filter(|s| is_graded(s)).count(),
                screens.len()
            ),
            None => format!(", {} checkpoints", row.checkpoints),
        };
        println!(
            "{}: {} {} under {} on {}, holds {},{}",
            row.name, row.program, row.session, row.term, row.captured, row.matches, screens
        );
    }
    true
}

// --- the capture helper ------------------------------------------------------------------------

/// One session's capture spec: what the pty runs and what is typed at it.
///
/// The specs are code rather than manifest rows because the keystrokes are a schedule, not data:
/// each entry waits its own time before the next, and the waits are what make a program settle
/// between keystrokes. `{sample}` stands for the prepared sample file.
struct SessionSpec {
    /// The command the pty runs, handed to `sh -c` after the environment is set.
    command: &'static str,
    /// The TERM the session runs under.
    term: &'static str,
    /// Milliseconds to wait before the first keystroke: the program's start-up and first paint.
    startup_ms: u64,
    /// `(bytes, wait after)` per keystroke.
    keys: &'static [(&'static [u8], u64)],
    /// End the session by closing the pty and killing the program rather than waiting for it to
    /// exit, for the session the census's harness also had to end.
    force_end: bool,
}

/// The sessions the tree commits: the census's ten programs and vttest's menus 1, 2 and 8.
///
/// The keystroke schedules are this lane's own, written to the census's shape (the census's exact
/// schedules were throwaway and are not kept); what each session does is in the manifest's
/// `session` field.
const SESSIONS: &[(&str, SessionSpec)] = &[
    (
        "vim",
        SessionSpec {
            command: "vim '{sample}'",
            term: "xterm-256color",
            startup_ms: 1500,
            keys: &[
                (b"\x06", 1000),             // Ctrl-F, one page down
                (b"\x06", 1000),             // again
                (b"\x02", 1000),             // Ctrl-B, one page up
                (b":split\r", 1600),         // horizontal split
                (b"\x17v", 1600),            // Ctrl-W v, vertical split
                (b"\x17w", 1200),            // Ctrl-W w, next window
                (b"dd", 1000),               // delete the line under the cursor
                (b"/glyph\r", 1400),         // search forward
                (b"n", 1000),                // the next match
                (b"i", 900),                 // insert mode
                ("你好😀".as_bytes(), 1100), // the wide and emoji text
                (b"\x1b", 900),              // back to normal mode
                (b":q\r", 1400),             // close one window
                (b":q!\r", 1800),            // leave vim
            ],
            force_end: false,
        },
    ),
    (
        "nvim",
        SessionSpec {
            command: "nvim '{sample}'",
            term: "xterm-256color",
            startup_ms: 2000,
            // The same keys as the vim session, as the census's Neovim session used the same keys
            // as its vim session. The last wait covers Neovim's one-second DA1 wait at exit.
            keys: &[
                (b"\x06", 1000),
                (b"\x06", 1000),
                (b"\x02", 1000),
                (b":split\r", 1600),
                (b"\x17v", 1600),
                (b"\x17w", 1200),
                (b"dd", 1000),
                (b"/glyph\r", 1400),
                (b"n", 1000),
                (b"i", 900),
                ("你好😀".as_bytes(), 1100),
                (b"\x1b", 900),
                (b":q\r", 1400),
                (b":q!\r", 1800),
            ],
            force_end: false,
        },
    ),
    (
        "less",
        SessionSpec {
            command: "less '{sample}'",
            term: "xterm-256color",
            startup_ms: 1200,
            keys: &[
                (b" ", 1000), // page forward
                (b" ", 1000),
                (b" ", 1000),
                (b"b", 1000), // page back: the reverse-index case
                (b"b", 1000),
                (b"G", 1000), // the end of the file
                (b"k", 900),  // one line up: reverse index again
                (b"k", 900),
                (b"/glyph\r", 1300), // search
                (b"n", 1000),
                (b"q", 800),
            ],
            force_end: false,
        },
    ),
    (
        "top",
        SessionSpec {
            command: "top",
            term: "xterm-256color",
            startup_ms: 2500,
            // macOS top: '?' is the help screen, 'o' prompts for a sort key.
            keys: &[
                (b"?", 1300),  // the help screen
                (b" ", 1200),  // any character leaves it
                (b"o", 1000),  // the sort-key prompt
                (b"cpu", 900), // sort by CPU
                (b"\r", 1600), // apply it
                (b"q", 800),
            ],
            force_end: false,
        },
    ),
    (
        "htop",
        SessionSpec {
            command: "htop",
            term: "xterm-256color",
            startup_ms: 2000,
            keys: &[
                (b"t", 1400),     // tree view
                (b"\x1b[B", 800), // Down
                (b"\x1b[B", 800),
                (b"\x1b[A", 800),   // Up
                (b"h", 1600),       // the help window
                (b"\x1b", 1000),    // close it
                (b"\x1b[6~", 1000), // PageDown
                (b"q", 800),
            ],
            force_end: false,
        },
    ),
    (
        "nano",
        SessionSpec {
            command: "nano '{sample}'",
            term: "xterm-256color",
            startup_ms: 1500,
            keys: &[
                (b"\x16", 1200),  // Ctrl-V, one page down
                (b"\x1b[B", 600), // Down
                (b"\x1b[B", 600),
                (b"\x1b[B", 600),
                ("你好😀".as_bytes(), 1000), // typed text
                (b"\x0b", 1000),             // Ctrl-K, cut the line
                (b"\x18", 1000),             // Ctrl-X, the exit prompt
                (b"n", 1200),                // refuse to save; nano exits
            ],
            force_end: false,
        },
    ),
    (
        "ncurses-test",
        SessionSpec {
            command: "'{ncurses_test}'",
            term: "xterm-256color",
            startup_ms: 1500,
            // The program draws its whole screen, flashes, and exits on the one key; the census's
            // session also graded one checkpoint.
            keys: &[(b"x", 1200)],
            force_end: false,
        },
    ),
    (
        "emacs",
        SessionSpec {
            command: "emacs -nw -Q '{sample}'",
            term: "xterm-256color",
            startup_ms: 2500,
            keys: &[
                (b"\x16", 1200), // Ctrl-V
                (b"\x16", 1200),
                (b"\x182", 1300),            // Ctrl-X 2, split
                (b"\x18o", 1100),            // Ctrl-X o, the other window
                (b"\x16", 1100),             // Ctrl-V down there
                ("你好😀".as_bytes(), 1100), // self-insert
                (b"\x18\x18", 1500),         // Ctrl-X Ctrl-C, the save prompt
                (b"n", 1500),                // refuse; emacs exits
            ],
            force_end: false,
        },
    ),
    (
        "mc",
        SessionSpec {
            command: "mc -u",
            term: "xterm-256color",
            startup_ms: 2000,
            // The census's Midnight Commander session also did not exit; the harness ended it.
            keys: &[
                (b"\x1b[B", 900), // Down
                (b"\x1b[B", 900),
                (b"\t", 900),     // Tab, the other panel
                (b"\x1b[A", 900), // Up
            ],
            force_end: true,
        },
    ),
    (
        "tmux",
        SessionSpec {
            command: "tmux -f /dev/null new-session",
            term: "xterm-256color",
            startup_ms: 1800,
            keys: &[
                (b"\x02%", 1600),             // prefix, split vertically
                (b"\x02\"", 1600),            // prefix, split horizontally
                (b"\x02\x1b[A", 1000),        // prefix Up, the pane above
                (b"less '{sample}'\r", 2000), // the pager in the pane
                (b"q", 1100),                 // out of the pager
                (b"\x02\x1b[C", 1000),        // prefix Right, the pane to the right
                (b"vim '{sample}'\r", 2400),  // the editor in the pane
                (b":q\r", 1400),              // out of the editor
                (b"\x02x", 1000),             // prefix x, the kill-pane prompt
                (b"y", 1400),                 // confirm; the last pane closes and tmux exits
            ],
            force_end: false,
        },
    ),
    (
        "vttest-1",
        SessionSpec {
            command: "vttest",
            term: "xterm",
            startup_ms: 1500,
            // Menu 1, cursor movement: six screens numbered 1.1 to 1.6 as conformance.md numbers
            // them, of which 1.2 and 1.4 are the 132-column second pass.
            keys: &[
                (b"1\r", 1500),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"0\r", 900),
            ],
            force_end: false,
        },
    ),
    (
        "vttest-2",
        SessionSpec {
            command: "vttest",
            term: "xterm",
            startup_ms: 1500,
            // Menu 2, screen features: fifteen screens numbered 2.1 to 2.15, of which 2.3 and 2.5
            // are 132-column.
            keys: &[
                (b"2\r", 1500),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"0\r", 900),
            ],
            force_end: false,
        },
    ),
    (
        "vttest-8",
        SessionSpec {
            command: "vttest",
            term: "xterm",
            startup_ms: 1500,
            // Menu 8, VT102 insert and delete: the seven 80-column screens numbered 8.1 to 8.7,
            // then the menu's 132-column second pass, 8.8 to 8.14.
            keys: &[
                (b"8\r", 1500),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"\r", 1100),
                (b"0\r", 900),
            ],
            force_end: false,
        },
    ),
];

/// The line prepended to the file the editors and pagers open, so every session meets wide
/// characters, a combining accent and an emoji, the census's own construction.
const SAMPLE_HEADLINE: &str = "你好 e\u{301} 😀 (the census's wide, combining and emoji line)\n";

/// `--record <name>`: run the session's spec in an 80x24 pty, record what the program wrote and
/// where the keystrokes fell, and write `<name>.rec`.
fn record(name: &str) -> bool {
    let Some((spec_name, spec)) = SESSIONS.iter().find(|(n, _)| *n == name) else {
        eprintln!(
            "vt-replay: no session named {name}; the recorded ones are: {}",
            SESSIONS
                .iter()
                .map(|(n, _)| *n)
                .collect::<Vec<_>>()
                .join(", ")
        );
        return false;
    };
    match run_pty_session(spec_name, spec) {
        Ok(recording) => {
            let path = recording_path(name);
            if let Err(e) = std::fs::write(&path, recording.to_bytes()) {
                eprintln!("vt-replay: cannot write {}: {e}", path.display());
                return false;
            }
            println!(
                "vt-replay: wrote {} ({} output bytes, {} keystrokes)",
                path.display(),
                recording.output.len(),
                recording.keystrokes.len()
            );
            println!(
                "next: generate {} with the oracle (recordings/README.md has the recipe), then set \
                 `checkpoints` in manifest.toml; the recording, its expected screens and its row \
                 move together",
                expected_path(name)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
            true
        }
        Err(e) => {
            eprintln!("vt-replay: recording {name} failed: {e}");
            false
        }
    }
}

/// Run one session in an 80x24 pty and record it.
///
/// The pty comes from macOS's `script(1)`, which needs no dependency: it runs the session's command
/// under a fresh pty and forwards this process's stdin to it. The inner shell fixes the pty's size
/// with `stty` before the program starts, so the program's `TIOCGWINSZ` reads 80x24 whatever the
/// calling terminal is.
fn run_pty_session(name: &str, spec: &SessionSpec) -> Result<Recording, String> {
    let work = std::env::temp_dir().join(format!("vt-replay-record-{}", std::process::id()));
    std::fs::create_dir_all(&work).map_err(|e| format!("cannot make {}: {e}", work.display()))?;
    let home = work.join("home");
    std::fs::create_dir_all(&home).map_err(|e| format!("cannot make the empty home: {e}"))?;
    let ncurses_test = prepare_ncurses_test(&work)?;

    // The sample file: the engine's own source with the census's headline line prepended, so the
    // editors and pagers meet wide characters by construction.
    let mut sample = String::from(SAMPLE_HEADLINE);
    sample.push_str(
        &std::fs::read_to_string(workspace_root().join("crates/video_terminal/src/vt.rs"))
            .map_err(|e| format!("cannot read the sample's source: {e}"))?,
    );
    let sample_path = work.join("vt_sample.rs");
    std::fs::write(&sample_path, &sample)
        .map_err(|e| format!("cannot write the sample file: {e}"))?;

    // A fixed locale, because vttest reads it to choose its UTF-8 mode and the programs read it to
    // choose their widths; a capture from a differently set shell would record different bytes.
    let inner = format!(
        "export HOME='{}'; export TERM='{}'; export LANG=en_US.UTF-8; export LC_ALL=en_US.UTF-8; \
         stty rows 24 cols 80; exec {}",
        home.display(),
        spec.term,
        spec.command
            .replace("{sample}", &sample_path.display().to_string())
            .replace("{ncurses_test}", &ncurses_test.display().to_string())
    );

    let mut child = Command::new("script")
        .args(["-q", "/dev/null", "sh", "-c", &inner])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("cannot spawn script(1): {e}"))?;
    let mut stdin = child.stdin.take().ok_or("no stdin to the child")?;
    let stdout = child.stdout.take().ok_or("no stdout from the child")?;

    // The reader owns the output stream; the sender reads its count when it needs an offset. A
    // byte that lands between the count and the write belongs to the next checkpoint, which is
    // what the census's gaps (0.3 to 3.5 s) make rare: nothing else sets an offset.
    let buffer: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let reader = spawn_reader(stdout, Arc::clone(&buffer));

    let mut keystrokes = Vec::new();
    std::thread::sleep(Duration::from_millis(spec.startup_ms));
    for (key, wait) in spec.keys {
        let at = buffer
            .lock()
            .map_err(|_| "the reader panicked".to_string())?
            .len();
        stdin
            .write_all(key)
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("the pty closed before the session finished: {e}"))?;
        keystrokes.push(Keystroke {
            at,
            bytes: key.to_vec(),
        });
        std::thread::sleep(Duration::from_millis(*wait));
    }
    // The session's typed half is over: closing the pipe makes script(1) close the pty, which
    // hangs up the program unless it has already exited.
    drop(stdin);
    finish_child(&mut child, spec.force_end);
    let _ = reader.join();
    let guarded = Arc::try_unwrap(buffer).map_err(|_| "the reader still holds the output")?;
    let output = guarded
        .lock()
        .map_err(|_| "the reader panicked".to_string())?
        .clone();

    let _ = std::fs::remove_dir_all(&work);
    Ok(Recording::new(name, output, keystrokes))
}

/// Take the reader half of the pty into a thread that appends to `buffer`.
fn spawn_reader(
    mut stdout: impl Read + Send + 'static,
    buffer: Arc<Mutex<Vec<u8>>>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        loop {
            match stdout.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let Ok(mut guard) = buffer.lock() {
                        guard.extend_from_slice(&chunk[..n]);
                    }
                }
            }
        }
    })
}

/// Wait for the child, killing it when it will not go: promptly for a force-ended session, after a
/// grace period otherwise.
fn finish_child(child: &mut Child, force_end: bool) {
    let grace = if force_end { 1500 } else { 8000 };
    let began = Instant::now();
    while began.elapsed() < Duration::from_millis(grace) {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Compile the census's ncurses screen test into `work` when a session needs it, or return an empty
/// path when none does.
fn prepare_ncurses_test(work: &Path) -> Result<PathBuf, String> {
    let needed = SESSIONS
        .iter()
        .any(|(_, spec)| spec.command.contains("{ncurses_test}"));
    if !needed {
        return Ok(PathBuf::new());
    }
    let source = recordings_dir().join("ncurses_screen_test.c");
    let binary = work.join("ncurses_screen_test");
    // Homebrew's ncurses 6.6, the census's library; a macPorts or source install sets
    // NIFE_NCURSES_HOME to its prefix.
    let prefix =
        std::env::var("NIFE_NCURSES_HOME").unwrap_or_else(|_| "/opt/homebrew/opt/ncurses".into());
    let output = Command::new("cc")
        .arg("-o")
        .arg(&binary)
        .arg(&source)
        .arg(format!("-I{prefix}/include"))
        .arg(format!("-L{prefix}/lib"))
        .arg("-lncursesw")
        .output()
        .map_err(|e| format!("cannot run cc for the ncurses test: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "compiling ncurses_screen_test.c failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(binary)
}

// --- tests -------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A recording round-trips through the file format with its checkpoints intact.
    #[test]
    fn recording_round_trips() {
        let recording = Recording::new(
            "test",
            b"hello \r\nworld".to_vec(),
            vec![
                Keystroke {
                    at: 0,
                    bytes: b"x".to_vec(),
                },
                Keystroke {
                    at: 7,
                    bytes: b"\x1b[A".to_vec(),
                },
            ],
        );
        let bytes = recording.to_bytes();
        let back = Recording::from_bytes("test", &bytes).unwrap();
        assert_eq!(back, recording);
    }

    /// A truncated file and an unknown tag are refused, not partially read.
    #[test]
    fn recording_refuses_bad_files() {
        let recording = Recording::new("test", b"hello".to_vec(), vec![]);
        let bytes = recording.to_bytes();
        assert!(Recording::from_bytes("test", &bytes[..bytes.len() - 1]).is_err());
        let mut odd = bytes.clone();
        odd.push(0x7f);
        assert!(Recording::from_bytes("test", &odd).is_err());
        assert!(Recording::from_bytes("test", b"nope").is_err());
    }

    /// Expected files parse strictly: a short block or a misnumbered marker is refused.
    #[test]
    fn expected_parses_strictly() {
        let text = "@ checkpoint 0\nrow one\nrow two\n@ checkpoint 1\nrow three\nrow four\n";
        let expected = ExpectedScreens::parse(text, 2).unwrap();
        assert_eq!(expected.blocks.len(), 2);
        assert_eq!(
            expected.blocks[0],
            vec!["row one".to_string(), "row two".to_string()]
        );
        // A block short one row, and a marker out of order.
        assert!(ExpectedScreens::parse("@ checkpoint 0\nonly\n", 2).is_err());
        assert!(ExpectedScreens::parse("@ checkpoint 1\na\nb\n", 2).is_err());
    }

    /// Rows compare after trailing blanks are trimmed and control characters become blanks.
    #[test]
    fn rows_normalize() {
        let vt = Vt::new(script::COLS, script::ROWS);
        let mut fed = Vt::new(script::COLS, script::ROWS);
        fed.feed(b"tail\x07   ");
        // The untouched grid trims to nothing; a row of text with a trailing bell and blanks trims
        // to its text.
        assert!(row_texts(&vt)[0].is_empty());
        assert_eq!(row_texts(&fed)[0], "tail");
        assert_eq!(normalize_row("ab\x07  "), "ab");
    }

    /// Grading counts matches checkpoint by checkpoint, and the manifest holds them.
    #[test]
    fn grading_counts_and_holds() {
        let recording = Recording::new(
            "test",
            b"alpha\r\nbeta\r\ngamma".to_vec(),
            vec![
                Keystroke {
                    at: 0,
                    bytes: b"k".to_vec(),
                },
                Keystroke {
                    at: 18,
                    bytes: b"k".to_vec(),
                },
            ],
        );
        // The engine's own screens, as the oracle saw them. The second keystroke's offset is the
        // output's whole length: every byte was written before it was sent.
        let mut vt = Vt::new(script::COLS, script::ROWS);
        let first = row_texts(&vt);
        vt.feed(b"alpha\r\nbeta\r\ngamma");
        let second = row_texts(&vt);
        let expected = ExpectedScreens {
            blocks: vec![first.clone(), second.clone()],
        };
        let row = |matches: usize| SessionRow {
            name: "test".to_string(),
            kind: "census".to_string(),
            program: "test".to_string(),
            term: "xterm-256color".to_string(),
            captured: "2026-10-28".to_string(),
            session: "test".to_string(),
            checkpoints: 2,
            matches,
            screens: None,
        };
        let (matched, graded) = grade_session(&recording, &row(2), &expected).unwrap();
        assert_eq!((matched, graded), (2, 2));
        // A dropped screen is a dropped match, and the manifest's row is what holds it.
        let drifted = ExpectedScreens {
            blocks: vec![first, vec![String::new(); ROWS]],
        };
        assert_eq!(grade_session(&recording, &row(2), &drifted).unwrap().0, 1);
        // Stale shapes are refused rather than graded askew.
        assert!(grade_session(&recording, &row(2), &ExpectedScreens { blocks: vec![] }).is_err());
    }

    /// A vttest row's labels decide which checkpoints are graded, and the manifest parses.
    #[test]
    fn manifest_and_labels() {
        let text = "
[sessions.vim]
kind = \"census\"
program = \"vim 9.1\"
term = \"xterm-256color\"
captured = \"2026-10-28\"
session = \"test\"
checkpoints = 14
matches = 1

[sessions.vttest-1]
kind = \"vttest\"
program = \"vttest 2.7\"
term = \"xterm\"
captured = \"2026-10-28\"
session = \"menu 1\"
checkpoints = 8
matches = 3
screens = [\"menu\", \"1.1\", \"skip\", \"1.3\", \"skip\", \"1.5\", \"1.6\", \"menu\"]
";
        let manifest = Manifest::parse(text).unwrap();
        assert_eq!(manifest.sessions.len(), 2);
        assert_eq!(manifest.sessions[0].matches, 1);
        let screens = manifest.sessions[1].screens.clone().unwrap();
        assert_eq!(screens.iter().filter(|s| is_graded(s)).count(), 4);
        assert!(!is_graded("menu"));
        assert!(!is_graded("skip"));
        assert!(is_graded("1.1"));
        assert!(is_graded("2.10"));
        assert!(!is_graded("1.1.1"));
        assert!(!is_graded("a.1"));
    }
}
