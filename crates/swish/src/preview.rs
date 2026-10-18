//! **The preview, which is the visibility surface**: [`write_capabilities`] over a whole line,
//! [`write_preview`] for one stage, `caps <path>` for an image
//! ([`write_image_capabilities`]), and the row writer all three share.
//!
//! Moved out of the crate root by milestone 884 (the shell logic crate is split along its seams).
//!
//! Name: provisional (milestone 884, 2026-10-12 UTC), for calef's naming pass.

use super::*;

/// **Who vouched for an image's digest**, for [`write_image_capabilities`]: the live generation that lists
/// it, and whether its entry is the owner's own vouch (`installed_programs::OWNER`) rather than an
/// installed package's. Name: provisional (2026-09-26).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vouched {
    /// The live generation's number.
    pub generation: u32,
    /// The entry names the owner rather than a package.
    pub by_owner: bool,
}

/// **`caps <path>`: what running a file's bytes would grant, and on whose word**
/// (DECISIONS §219 option D and gate D2, milestone 198 rung 3a; the manifest note, milestone 597,
/// provisional).
///
/// The shell computes every input from the bytes it read and the installed-programs table it can read:
/// `digest` is the file's SHA-256 in its text form, `sha256:<hex>` (`measured_boot::digest_text`), `vouched_by` is the live generation that lists
/// that digest and whether that entry is the owner's own vouch (DECISIONS §221 (the boot prompt
/// is the owner's console)), or `None`, `declared` is the manifest the file's note carries (`None` for no note),
/// and `e` is the line bound against it. `holds` is whether this session holds the run-unvouched
/// capability. The rows are the endowment the progenitor would build, decided by the same
/// [`grant_plan::image_manifest`] it calls: a vouched file's own manifest (or
/// [`grant_plan::NO_NOTE_MANIFEST`]'s when it carries none), and [`grant_plan::UNVOUCHED_MANIFEST`]'s
/// three slots when not, or a refusal. **What the note asks is printed beside what is granted**,
/// because for unvouched bytes the two differ and that difference is §219's rule made visible.
///
/// It is a preview, and it says so where it could be wrong: the progenitor hashes its own copy
/// when the line runs, so a file changed in between is judged on what arrives then.
///
/// # EXAMPLES
///
/// ```
/// let grant_plan::Command::Run(spec) = grant_plan::parse(b"./a.out") else { panic!() };
/// let m = grant_plan::Manifest { network: true, ..grant_plan::NO_NOTE_MANIFEST };
/// let holds = grant_plan::Holdings::default();
/// let e = grant_plan::plan_against(&spec, grant_plan::IMAGE_ROW, m, holds, grant_plan::expand::Expansion::none()).unwrap();
/// let mut said = Vec::new();
/// swish::write_image_capabilities(b"./a.out", b"sha256:ab12", None, true, Some(m), &e, &holds, &mut |b| said.extend_from_slice(b));
/// let said = String::from_utf8(said).unwrap();
/// assert!(said.contains("provenance: unvouched (digest sha256:ab12)"));
/// assert!(said.contains("cap 1  page      clock"));
/// assert!(said.contains("its manifest note asks for: output bytes, the network"));
/// ```
// Eight arguments since milestone 154 added `holdings`, which a directory grant's row needs to
// print a path under a mount point. Each is one fact the caller already holds, and bundling them
// into a struct used nowhere else would be more to read, not less.
#[allow(clippy::too_many_arguments)]
pub fn write_image_capabilities(
    path: &[u8],
    digest: &[u8],
    vouched_by: Option<Vouched>,
    holds: bool,
    declared: Option<grant_plan::Manifest>,
    e: &Endowment,
    holdings: &Holdings,
    out: &mut dyn FnMut(&[u8]),
) {
    let vouched = vouched_by.is_some();
    let verdict = (vouched || holds).then(|| grant_plan::image_manifest(declared, vouched));
    out(b"  ");
    out(path);
    match verdict {
        // **An unvouched `std` image is previewed from its manifest too** (milestone 205): its
        // two pages sit at the `std` layout's slots, and its words at slot 8, so the native rows
        // below would name slots it does not get.
        Some(Ok(m)) if vouched || m.startup == grant_plan::StartupContract::Std => {
            out(b" would grant the new process, and nothing else:\n");
            write_preview_rows(e, &m, holdings, None, out);
        }
        Some(Ok(_)) => {
            out(b" would grant the new process, and nothing else:\n");
            out(b"    cap 0  endpoint  result   report its answer back\n");
            out(b"    cap 1  page      clock    read-only; unvouched bytes may read the time\n");
            out(b"    cap 2  page      config   read-only; and the configuration page\n");
        }
        Some(Err(r)) => {
            out(b" would not run here:\n");
            out(b"  ");
            out(image_refusal_sentence(r));
        }
        None => out(b" would not run here:\n"),
    }
    write_note_asks(declared, out);
    out(b"    provenance: ");
    match vouched_by {
        Some(v) => {
            out(if v.by_owner {
                b"vouched by the owner in activation generation "
            } else {
                b"vouched by activation generation "
            });
            write_num(u64::from(v.generation), out);
            out(b" (digest ");
        }
        None => out(b"unvouched (digest "),
    }
    out(digest);
    out(b")\n");
    match (vouched_by, holds) {
        (Some(_), _) => {}
        (None, true) => {
            out(b"    runs on this session's capability to run unvouched bytes (slot ");
            write_num(spawnproto::RUN_UNVOUCHED_SLOT, out);
            out(b")\n");
            if declared.is_some() {
                out(b"    and its note grants nothing: bytes nobody vouched for are not asked\n");
            }
        }
        (None, false) => {
            out(b"    refused if run: this session holds no capability to run unvouched bytes\n");
        }
    }
}

/// **The one line that says what a file's own manifest asks for**, in words, so it can be read
/// beside the rows of what will be granted. Every authority a manifest can declare that an image
/// can carry is named; the ones it cannot carry are refused before this matters.
fn write_note_asks(declared: Option<grant_plan::Manifest>, out: &mut dyn FnMut(&[u8])) {
    let Some(m) = declared else {
        out(b"    it carries no manifest note, so it asks for its output and nothing else\n");
        return;
    };
    out(b"    its manifest note asks for: ");
    out(match m.output {
        OutputSpec::Words => b"a word of output".as_slice(),
        OutputSpec::Silent => b"no output",
        _ => b"output bytes",
    });
    let items: [(bool, &[u8]); 9] = [
        (m.arg == ArgSpec::Required, b"an argument"),
        (m.arg.hears_words(), b"its words"),
        (!matches!(m.mem, grant_plan::MemSpec::Forbidden), b"memory"),
        (m.clock, b"the clock"),
        (m.config, b"the configuration page"),
        (m.entropy, b"entropy"),
        (m.network, b"the network"),
        (m.domain, b"the process domain"),
        (m.interruptible, b"an interrupt"),
    ];
    for (asked, what) in items {
        if asked {
            out(b", ");
            out(what);
        }
    }
    out(b"\n");
}

/// **Preview what a command line would grant**, which is the whole of `caps <command>`.
///
/// With an empty tail this is [`write_holdings`]. With a tail it is **a whole line, operators
/// included** (milestone 50), because what a pipeline grants is not the sum of what its stages grant
/// read one at a time: which slot holds what depends on where the stage is. Previewing the line the
/// user would run is the only preview worth having.
///
/// Nothing here moves any authority. That is the point of it being reachable from a host test at
/// all: the sentences a user reads before they decide to run something are checked without an
/// emulator, and without a shell that holds anything.
pub fn write_capabilities(
    tail: &[u8],
    budget_pages: u64,
    holdings: Holdings,
    clock: Option<u64>,
    config: Option<ConfigPage>,
    expand: &mut dyn FnMut(&[u8]) -> Result<NameSet, Say>,
    out: &mut dyn FnMut(&[u8]),
) {
    let mut tail = grant_plan::trim(tail);
    if tail.is_empty() {
        return write_holdings(budget_pages, holdings, clock, config, out);
    }
    // **`caps time <command>` previews the command**, because that is what would run and `time`
    // moves no authority to it: the shell times with its own clock and the child is spawned with the
    // endowment the tail names, unchanged. A preview that stopped at the prefix word would answer
    // "this is not an invocation" about a line that is one, which is the drift between what you
    // inspect and what you run that `caps` exists to close.
    //
    // A loop rather than a recursive call, because `caps time time time date` is a line a person can
    // type and this function's frame carries a whole split `Line` by value. Bounded recursion on a
    // shell stack is a bug waiting for a long line (notes/pipes.md has two of those already).
    let mut timed = false;
    while let Command::Time(inner) = grant_plan::parse(tail) {
        timed = true;
        tail = grant_plan::trim(inner);
        if tail.is_empty() {
            return write_untimed(Untimed::NothingToTime, out);
        }
    }
    if timed {
        out(b"  time grants nothing; what it would run:\n");
    }
    let l = match line::split(tail) {
        Ok(l) => l,
        Err(r) => return write_say(Say::Cannot(r), out),
    };
    let sink_file = match l.output {
        Some(t) => match grant_plan::redirect_target(t, holdings, true) {
            Ok(g) => Some(g),
            Err(r) => return write_say(Say::Cannot(r), out),
        },
        None => None,
    };
    let source_file = match l.input {
        Some(t) => match grant_plan::redirect_target(t, holdings, false) {
            Ok(g) => Some(g),
            Err(r) => return write_say(Say::Cannot(r), out),
        },
        None => None,
    };
    let diag_file = match l.diagnostics {
        Some(t) => match grant_plan::redirect_target(t, holdings, true) {
            Ok(g) => Some((g, l.diagnostics_mode())),
            Err(r) => return write_say(Say::Cannot(r), out),
        },
        None => None,
    };
    for (i, stage) in l.stages().iter().enumerate() {
        // Only a program invocation carries a grant to preview; `caps help` has nothing to say.
        let Command::Run(spec) = grant_plan::parse(stage) else {
            out(b"  caps previews a command's grant; try: caps memory_grant_depleter --mem 16\n");
            return;
        };
        let expanded = match expansion(&spec, expand) {
            Ok(e) => e,
            Err(Say::Cannot(r)) => return write_refusal(&spec, r, out),
            Err(said) => return write_say(said, out),
        };
        let streams = Streams {
            sink: l.sink_for(i, sink_file),
            source: l.source_for(i, source_file),
            diagnostics: diag_file,
        };
        match grant_plan::plan_stage(&spec, holdings, expanded, streams) {
            Err(refusal) => return write_refusal(&spec, refusal, out),
            Ok(e) => {
                // **Narrowed by the line's verb** (milestone 809 (the package client becomes a
                // program), option V2): `caps jig list` shows what `jig list` would hold, which is
                // what the progenitor will endow, reading the same word off the argv.
                let mut verb = [0u8; grant_plan::MAX_VERB];
                let verb = grant_plan::verb_of(stage, &mut verb);
                let m = grant_plan::narrow_by_verb(&e.prog.manifest(), e.prog.verbs(), verb);
                out(b"  ");
                out(e.prog.name().as_bytes());
                out(b" would grant the new process, and nothing else:\n");
                write_preview_rows(&e, &m, &holdings, config, out);
            }
        }
    }
}

/// Write the endowment a resolved invocation would hand the new process. `holdings` is what the
/// planning shell held, which is how a directory grant's position is printed as a path in the one
/// tree even when the grant is in a mounted second tree ([`Holdings::place`]).
///
/// `config` is this shell's own view of the inert-configuration page, when it holds one
/// ([`grant_plan::SHELL_CONFIG_SLOT`]). It is the frame the progenitor endows a child declaring
/// [`grant_plan::Manifest::config`] with, so its values are printed as what that child will read.
/// `None` prints the row without values and says why.
pub fn write_preview(
    e: &Endowment,
    holdings: &Holdings,
    config: Option<ConfigPage>,
    out: &mut dyn FnMut(&[u8]),
) {
    out(b"  ");
    out(e.prog.name().as_bytes());
    out(b" would grant the new process, and nothing else:\n");
    write_preview_rows(e, &e.prog.manifest(), holdings, config, out);
}

/// [`write_preview`]'s rows, for an endowment bound against `m`. Separate because a file run by its
/// path (milestone 597, provisional) is bound against the manifest its note carries, and its
/// endowment's row is a stand-in (`grant_plan::IMAGE_ROW`) that must not be read.
fn write_preview_rows(
    e: &Endowment,
    m: &grant_plan::Manifest,
    holdings: &Holdings,
    config: Option<ConfigPage>,
    out: &mut dyn FnMut(&[u8]),
) {
    // **A `std` program's slots are fixed by its runtime, not by the order they are listed in**
    // (milestone 595 (provisional), `crates/std_runtime_contract`). The same grants land somewhere
    // else, and slot 0 is not even the same kind of object, so a preview that printed the native
    // positions for one would be describing a child the progenitor does not build.
    let std = m.startup == grant_plan::StartupContract::Std;
    write_result_and_memory_rows(e, m, std, out);
    write_file_and_directory_rows(e, m, holdings, std, out);
    write_page_rows(m, config, std, out);
    write_service_rows(m, std, out);
    write_resolver_row(e, m, std, out);
    write_machine_rows(m, out);
    write_index_rows(e, m, out);
    write_stream_rows(e, out);
    out(b"    arg    ");
    // **Read the manifest, do not keep a second list of which programs take an argument.** This
    // was `matches!(e.prog, Prog::LeastAuthorityDemo)` until 2026-08-16, so every other argument-taking program
    // previewed `arg (none)` while the shell went on to hand it the argument anyway. That is the
    // worst possible direction for this particular line to be wrong in: the next thing it prints is
    // that reading the command is reading its whole authority, and a preview that under-reports
    // authority is one a reader would trust. Found by someone adding their first program, who hit
    // it because the manifest already knew the answer.
    if m.arg == ArgSpec::Required {
        write_num(e.arg, out);
        out(b"\n");
    } else if m.arg.hears_words() {
        out(b"(the words on the line, at cap ");
        write_num(std_runtime_contract::ARGS_SLOT, out);
        out(b")\n");
    } else {
        out(b"(none)\n");
    }
    out(b"  reading the command is reading its whole authority.\n");
}

/// The `cap N` column of a preview row, padded so the kind that follows lines up.
fn write_slot(n: u64, out: &mut dyn FnMut(&[u8])) {
    out(b"    cap ");
    write_num(n, out);
    out(if n < 10 {
        b"  ".as_slice()
    } else {
        b" ".as_slice()
    });
}

/// The result row (or a `std` program's heap and stdout rows) and the memory grant.
fn write_result_and_memory_rows(
    e: &Endowment,
    m: &grant_plan::Manifest,
    std: bool,
    out: &mut dyn FnMut(&[u8]),
) {
    if std {
        write_slot(std_runtime_contract::MEMORY_REGION_SLOT, out);
        out(b"untyped   heap. the ");
        write_num(grant_plan::named_std_region_pages(m), out);
        out(b"-page region it is built in, less the\n");
        out(if m.installer {
            b"                              build; from the progenitor's image pool, which holds\n                              one package beside the progenitor's copy of it, not\n".as_slice()
        } else {
            b"                              build; from the progenitor's job pool, not this\n"
                .as_slice()
        });
        out(b"                              shell's budget, and all of it goes back at exit\n");
        write_slot(std_runtime_contract::STDOUT_SLOT, out);
        out(b"endpoint  result   stdout and stderr, both\n");
    } else {
        out(b"    cap 0  endpoint  result   report its answer back\n");
    }
    if e.mem_pages > 0 {
        out(b"    cap 1  untyped   ");
        write_num(e.mem_pages, out);
        out(b" pages  split from this shell's budget\n");
    }
}

/// The rows for what the line designated: a file, or a directory and its names.
fn write_file_and_directory_rows(
    e: &Endowment,
    m: &grant_plan::Manifest,
    holdings: &Holdings,
    std: bool,
    out: &mut dyn FnMut(&[u8]),
) {
    // A file endowment reads as one line naming the file and the direction, because that IS the
    // whole authority: an endpoint served by a file caretaker that will answer for this name and no
    // other. The direction comes from the program's manifest, not from anything typed, which is why
    // it is worth printing: the line you typed plus this table is the child's complete authority.
    if let Some(g) = e.file {
        out(b"    cap 2  endpoint  file     ");
        out(g.name.as_bytes());
        out(if g.writable {
            b"  (read+write, and nothing else on the disk)\n".as_slice()
        } else {
            b"  (read-only, and nothing else on the disk)\n".as_slice()
        });
    }
    // **A directory endowment is the subtree at risk, printed before anything happens**, which is
    // the argument for `rm` being a program rather than a builtin: a builtin would have run with
    // this shell's entire endowment and there would have been nothing to print. The `-r` line is
    // the load-bearing half, because typing that option is what widens the capability from "may
    // take a name out of this directory" to "may walk everything under it".
    if let Some(g) = e.dir {
        write_slot(
            if std {
                std_runtime_contract::FS_DIR_SLOT
            } else {
                2
            },
            out,
        );
        out(b"endpoint  dir      ");
        let mut buf = [0u8; nav::RENDER_MAX];
        let n = holdings.place(g.which, &g.dir).render(&mut buf);
        out(&buf[..n]);
        // **What a foreign program's words designated** (milestone 205): the names, or the whole
        // directory for `.`, and what it may do with them, which is its manifest's word grant (the
        // progenitor makes it read-only for bytes nobody vouched for, and says so in the manifest
        // this preview was given).
        if let grant_plan::ArgSpec::Words(w) = m.arg {
            if g.names.is_empty() {
                out(b"  (this directory itself)\n");
            } else {
                out(b"  (only ");
                write_set(&g.names, out);
                out(b" in it, named on the line)\n");
            }
            out(match w {
                grant_plan::WordGrant::ReadOnly => {
                    b"           ...read-only, and everything under what was named\n".as_slice()
                }
                grant_plan::WordGrant::ReadWrite => {
                    b"           ...read and write, and everything under what was named\n"
                }
                grant_plan::WordGrant::Create => {
                    b"           ...read, write and create what was named, and everything under it\n"
                }
            });
        } else {
            out(b"  (the directory holding ");
            // **The names, all of them, and this is the point of previewing a set at all.** `caps rm
            // *.txt` prints exactly what `echo *.txt` prints, because both render the same
            // expansion: the authority about to move is on the screen before anything moves it,
            // which is a claim Unix cannot make about its own `rm`.
            write_set(&g.names, out);
            out(b")\n");
            if g.subtree {
                out(b"           ...and everything under it: -r grants the walk\n");
            } else {
                out(b"           ...and nothing under it: no -r, so it cannot even look\n");
            }
        }
    }
}

/// The read-only pages the progenitor endows: the clock, the inert configuration, and the argv.
fn write_page_rows(
    m: &grant_plan::Manifest,
    config: Option<ConfigPage>,
    std: bool,
    out: &mut dyn FnMut(&[u8]),
) {
    // **The clock, which no token on the line designates.** It is the progenitor's to endow rather than the
    // shell's, and it is still part of this child's complete authority, so `caps` prints it: a
    // reader who took the command line for the whole story would be wrong by exactly one capability.
    // The row says *read-only* because that is the entire reason `date` cannot set the time
    // (DECISIONS §43): there is no flag it could pass and no method it could call.
    if m.clock {
        write_slot(
            if std {
                std_runtime_contract::CLOCK_SLOT
            } else {
                1
            },
            out,
        );
        out(b"frame     clock    read-only. it can read the time and not set it,\n");
        out(b"                              and no token on the line could have asked for more\n");
    }
    // **The inert-configuration page, `clock`'s twin** (milestone 47, DECISIONS §111). No token on
    // the line could designate it either, so it is the progenitor's to endow and this is where a
    // reader learns the authority exists at all.
    //
    // **And its values, which is what §111 asked this preview for**: "print the actual values of
    // declared inert config, not just the key names, so a misclassified value is visible to whoever
    // is about to run something." They come from the shell's own read-only view of the *same frame*
    // the child will be handed, never from a copy of the defaults, so the preview cannot drift from
    // what the child reads. A shell given no view (a `login` session, a test role) says so rather
    // than guessing.
    if m.config {
        write_slot(
            if std {
                std_runtime_contract::CONFIG_SLOT
            } else {
                1
            },
            out,
        );
        match config {
            Some(page) => {
                out(b"frame     config   read-only, the page this shell reads too:\n");
                write_config_values(&page, b"                              ", out);
            }
            None => {
                out(b"frame     config   read-only. TZ, LANG and TERM as this boot's inert\n");
                out(b"                              defaults; this shell holds no view of the page,\n");
                out(b"                              so it cannot show their values\n");
            }
        }
        out(b"                              nothing here can change what a shell hands\n");
        out(b"                              its children\n");
    }
    // **The argv, which is bytes and not authority** (milestone 205, DECISIONS §170). Printed
    // because it is a capability the child holds, and worded so nobody reads it as more: a path
    // among these words reaches only what a directory row above already granted.
    if m.arg.hears_words() {
        write_slot(std_runtime_contract::ARGS_SLOT, out);
        out(b"frame     args     read-only. the words on the line, as bytes; they\n");
        out(b"                              name things and grant none of them\n");
    }
}

/// The service endpoints a program may call: entropy, the network, and its supervision domain.
fn write_service_rows(m: &grant_plan::Manifest, std: bool, out: &mut dyn FnMut(&[u8])) {
    // **The row milestone 111 exists to print, and it is the point of that milestone rather than a
    // decoration on it.** Randomness is the one authority a program can hold whose use leaves no
    // trace at all: a process that draws a key and a process that hardcodes one look identical from
    // outside, run the same syscalls, and produce output of the same shape. So "does this program
    // depend on unpredictable bytes" is a question no observation of a running system answers, and
    // this line answers it before anything is spawned.
    //
    // `WRITE` is the whole grant, and on an endpoint that is the right to `CALL`: this child may
    // ask the service for bytes. It may not receive another client's request (that would be `READ`,
    // which the service holds and nothing else does), and it holds no `GRANT`, so it cannot pass
    // randomness on to anything at all.
    if m.entropy {
        write_slot(
            if std {
                std_runtime_contract::ENTROPY_SLOT
            } else {
                grant_plan::ENTROPY_SLOT
            },
            out,
        );
        out(b"endpoint  entropy  WRITE. it may ask the entropy service for random\n");
        out(
            b"                              bytes, and nothing else: it cannot reach the device,\n",
        );
        out(b"                              cannot receive another client's request, and cannot\n");
        out(b"                              hand a random source to anything it spawns\n");
        out(b"                              a program without this row draws no randomness at all\n");
    }
    // **The network, which no token on the line designates either** (milestone 590 (the booted
    // system starts its network stack)).
    // The entropy row's shape and its reason: whether a program can reach the network is a fact
    // about what it holds, so a person reads it here before anything runs rather than learning it
    // from a firewall afterward. The fourth line says what it does not reach: since milestone 649
    // (every client of a network stack shares its socket numbers) each socket is its own capability
    // (§255 (each socket is its own capability)), so one declaring program cannot reach another's
    // sockets. Until then this line said the opposite, because it was true.
    if m.network {
        if std {
            // The stack at slot 2 and, at slot 3, the region the program is built in, which the
            // net PAL retypes each socket's shared page from (milestone 809).
            write_slot(std_runtime_contract::STACK_SLOT, out);
            out(b"endpoint  network  WRITE. it may open outbound sockets through the network\n");
            write_slot(std_runtime_contract::NET_MEMORY_REGION_SLOT, out);
            out(b"untyped   sockets. the heap's region again, which each socket's page is\n");
            out(b"                              made from\n");
        } else {
            out(b"    cap 10 endpoint  network  WRITE. it may open outbound sockets through the network\n");
        }
        out(b"                              stack, and nothing else: it cannot reach the card, cannot\n");
        out(b"                              listen, and cannot hand the network to anything it spawns.\n");
        out(b"                              each socket it opens is a capability of its own; it cannot\n");
        out(b"                              reach another program's sockets\n");
        out(
            b"                              a program without this row reaches no network at all\n",
        );
    }
    // **The row this milestone exists to print.** On Linux there is nothing here to say: `ps` reads
    // /proc and the answer is "every process on the machine", which no command line chose and no
    // tool can narrow. Here the scope is a capability, so it is a line a person can read before
    // anything is spawned, and a wider grant would be a different line rather than an invisible one.
    if m.domain {
        // `ENUMERATE`, and the word is the point of the line rather than decoration: it is the
        // right that lets this program *name* the domain's members and not the one that would let
        // it receive their deaths or collect them. Printing `READ` here would describe a wider
        // grant than the one being made, which is the failure a `caps` output has available.
        out(b"    cap 7  endpoint  domain   ENUMERATE. the processes this shell's jobs are\n");
        out(b"                              supervised by, and no others. it can name them and\n");
        out(b"                              do nothing to them: not receive their deaths, not\n");
        out(
            b"                              collect them, and not learn anything about a process\n",
        );
        // **The last line was an overclaim until 2026-08-17**, and the audit that found it
        // (design/audit-reports/) fixed the sentence rather than the mechanism, on purpose. It read
        // "and not learn that a process outside this domain exists", which is false: `SURVEY`
        // returns a cursor that is a machine-wide thread-table slot index, and a tid whose low half
        // is the same index, so a viewer with two members can subtract and count the threads
        // created between them. It still cannot name one. Narrowing the claim to what the mechanism
        // actually delivers is the honest half-step; scoping the cursor to the domain is a
        // milestone, and notes/process-view.md's `BUGS` carries the disposition.
        out(b"                              outside this domain but that it exists\n");
    }
}

/// **The name resolver, beside the network** (milestone 892 (a std program at the prompt holds the
/// network and a resolver)): the zone is the table's (`Prog::resolver_zone`), so a person reads
/// which names the program may look up before it runs, where on Unix every process may read
/// `/etc/resolv.conf` and ask about anything.
fn write_resolver_row(
    e: &Endowment,
    m: &grant_plan::Manifest,
    std: bool,
    out: &mut dyn FnMut(&[u8]),
) {
    let (true, true, Some(zone)) = (m.network, std, e.prog.resolver_zone()) else {
        return;
    };
    write_slot(std_runtime_contract::RESOLVER_SLOT, out);
    out(b"endpoint  resolver  WRITE. it may look up the addresses of ");
    if zone.is_empty() {
        out(b"any name\n");
    } else {
        out(b"names in ");
        out(zone.as_bytes());
        out(b"\n");
    }
    out(b"                              and nothing else: a name outside that is refused unasked.\n");
    out(b"                              held only on a boot whose lease named a name server\n");
}

/// The machine-level grants: the statistics page, the budget view, reboot, sync, the installer
/// and the catalog.
fn write_machine_rows(m: &grant_plan::Manifest, out: &mut dyn FnMut(&[u8])) {
    // **The machine, and this prompt's share of it** (milestone 126 (the `procps` package), DECISIONS §225 (`free` sees the machine and your share)). Two rows
    // because they are two grants and one can be held without the other: `vmstat` sees the machine
    // and not the budget, `slabtop` the budget and not the machine, `free` both. The first says the
    // owner's switch exists, because a person reading `caps free` on a machine whose owner withheld
    // the page should learn why the program cannot see it.
    // `m`, not `e.prog.manifest()`: for an image run from a file `e.prog` is the placeholder row
    // and `m` is what its note declared, and the progenitor endows from the note. Reading the
    // row here under-reported a packaged `free` as holding neither (the 2026-10-03 security
    // audit's follow-up, item (d)).
    if m.machine {
        out(b"    cap 11 frame     machine  read-only. memory, run queue, interrupts, context\n");
        out(b"                              switches and busy time for the whole machine.\n");
        out(b"                              granted by default; the machine's owner can withhold it\n");
    }
    if m.share {
        out(b"    cap 12 region    share    ENUMERATE. this shell's job budget: how much is spent\n");
        out(b"                              and on what. it cannot spend, split or destroy it\n");
    }
    // **The reboot object** (milestone 805 (`reboot` at the prompt), DECISIONS §251 (restarting the
    // machine is a kernel object the progenitor hands out)): the row exit criterion 3 asks `caps
    // reboot` to print. It names the authority and the sync it does first, because the
    // second is what makes the first safe and neither shows anywhere else before the machine stops.
    if m.reboot {
        out(b"    cap 13 reboot    WRITE. restart the machine (the kernel asks the firmware).\n");
    }
    if m.sync {
        out(b"    cap 14 endpoint  sync   WRITE. the file server answers SYNC on it and refuses\n");
        out(b"                              everything else: it flushes the device, and reaches no file\n");
    }
    // **The installer endpoint** (milestone 809 (the package client becomes a program), DECISIONS
    // §270 (a package manager holds an installer endpoint, not the spawn endpoint)). Three lines
    // for what it cannot do, because the endpoint it is a copy of can do all three.
    if m.installer {
        write_slot(grant_plan::INSTALLER_SLOT, out);
        out(b"endpoint  installer  WRITE. it may ask the progenitor to install, remove or\n");
        out(b"                              roll back a package, and nothing else: it cannot spawn,\n");
        out(b"                              cannot vouch for bytes, and cannot hand this on.\n");
        out(b"                              only the owner's console grants it\n");
        write_slot(grant_plan::INSTALLER_REPLY_SLOT, out);
        out(b"endpoint  replies    READ. the progenitor's answers to those requests\n");
    }
    if m.catalog {
        write_slot(grant_plan::CATALOG_SLOT, out);
        out(b"frame     catalog  read-only. the packages this image vouches for, by name,\n");
        out(b"                              version and digest; knowledge, and no authority\n");
    }
}

/// **The machine's index store** (milestone 801 (packages over the internet)): the directory the
/// index copy lives in, at the `std` layout's one directory slot, and the image's index sources.
/// Printed only when the line designated no directory of its own, because the progenitor then
/// places the line's and not this one (`grant_plan::Manifest::indexes`).
fn write_index_rows(e: &Endowment, m: &grant_plan::Manifest, out: &mut dyn FnMut(&[u8])) {
    let write = match m.indexes {
        grant_plan::IndexStore::Forbidden => return,
        _ if e.dir.is_some() => return,
        grant_plan::IndexStore::ReadOnly => false,
        grant_plan::IndexStore::ReadWrite => true,
    };
    write_slot(std_runtime_contract::FS_DIR_SLOT, out);
    out(if write {
        b"endpoint  dir      /activation/indexes  read and write. the index copy this
"
        .as_slice()
    } else {
        b"endpoint  dir      /activation/indexes  read-only. the index copy this
"
    });
    out(
        b"                              machine installs from; only the owner's console grants it
",
    );
    write_slot(grant_plan::INDEX_SOURCES_SLOT, out);
    out(
        b"frame     sources  read-only. where this image's index is, and the root its TLS
",
    );
    out(
        b"                              must chain to; knowledge, and no authority
",
    );
}

/// Where the output, the second stream and the input go.
fn write_stream_rows(e: &Endowment, out: &mut dyn FnMut(&[u8])) {
    // **Where its output goes**, which is the demonstration milestone 50 owed: the destination is a
    // capability rather than an integer with a convention attached, so `caps` can name it. On Unix
    // the same question has no answer at this point, because fd 1 is whatever the shell's fd 1
    // happened to be and nothing records what that was.
    match e.sink {
        line::Sink::Report => {
            out(
                b"    output   this shell's result endpoint (it reads the bytes and prints them)\n",
            );
        }
        line::Sink::Pipe => {
            out(b"    output   an endpoint into the next stage. no file, no buffer, no object:\n");
            out(b"             the rendezvous IS the pipe\n");
        }
        // The row names the file, and the parenthesis names what the program actually holds, which
        // is not the file. Its slot 0 is this shell's result endpoint either way; `>` is where the
        // shell puts the bytes, not something the child was handed. So a redirected program still
        // cannot seek, truncate, re-read or stat, and this is where a reader can see why.
        line::Sink::File(g, mode) => {
            out(b"    output   ");
            out(g.name.as_bytes());
            out(b"  (this shell writes the bytes there; the program holds\n");
            out(b"             an endpoint and cannot seek, truncate, re-read or stat)\n");
            // The one line where `>` and `>>` differ, and it is about the shell rather than about
            // the child: whichever was typed, what the child holds is the same endpoint. Printed
            // because it is the only visible consequence of the operator, and because a person
            // about to overwrite a file should be able to see that from the preview.
            out(if matches!(mode, line::Mode::Append) {
                b"             this shell keeps what is already in it and writes after it\n"
                    .as_slice()
            } else {
                b"             this shell empties it first, before the command runs\n".as_slice()
            });
        }
    }
    // **The second stream, for a program that declares one** (DECISIONS §67). It is printed even
    // when no `2>` is on the line, and that is the row worth having: a reader who took `date >
    // when.txt` for "everything date says goes into when.txt" would be wrong, and this is where they
    // can see it. A program that declares none gets no row, because it has no second stream to hide.
    match e.diagnostics {
        line::Diagnostics::None => {}
        line::Diagnostics::Printed => {
            out(b"    diags    the terminal's own sink, a component this shell does not hold.\n");
            out(b"             declared by the program, so a > cannot swallow them, 2> can\n");
            out(b"             name them, and they reach the screen without passing here\n");
        }
        line::Diagnostics::File(g, mode) => {
            out(b"    diags    ");
            out(g.name.as_bytes());
            out(b"  (this shell writes them there; the program holds a\n");
            out(b"             second endpoint and still cannot seek, truncate or stat)\n");
            out(if matches!(mode, line::Mode::Append) {
                b"             this shell keeps what is already in it and writes after it\n"
                    .as_slice()
            } else {
                b"             this shell empties it first, before the command runs\n".as_slice()
            });
        }
    }
    match e.source {
        line::Source::None => {}
        line::Source::Pipe => out(b"    input    the previous stage's output\n"),
        line::Source::File(g) => {
            out(b"    input    ");
            out(g.name.as_bytes());
            out(b"  (this shell reads it and streams it in; the program\n");
            out(b"             holds an endpoint, not a file)\n");
        }
    }
}

#[cfg(test)]
mod tests;
