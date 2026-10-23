//! **The preview, which is the visibility surface**: [`write_capabilities`] over a whole line,
//! [`write_preview`] for one stage, `caps <path>` for an image
//! ([`write_image_capabilities`]), and the row writer all three share. Why each row is printed:
//! notes/shell/caps-preview.md.
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
    /// A `grant` line gives these bytes the network (`installed_programs::network_granted`).
    pub network_granted: bool,
}

/// **`caps <path>`: what running a file's bytes would grant, and on whose word** (DECISIONS §219
/// option D and gate D2; milestone 597 (a program carries its manifest in an ELF note)).
///
/// `digest` is the file's `sha256:<hex>`, `vouched_by` the live generation that lists it (or
/// `None`), `holds` whether this session holds the run-unvouched capability, `declared` the
/// manifest its note carries, and `e` the line bound against it. The rows are what
/// [`grant_plan::image_manifest`] would endow, beside what the note asks. See
/// notes/shell/caps-preview.md, "`caps <path>`: what a file's bytes would be granted".
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
// Each argument is one fact the caller already holds; a struct used nowhere else would not help.
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
    let vouch = vouched_by.map_or(grant_plan::Vouch::Unvouched, |v| {
        grant_plan::Vouch::Vouched {
            network: v.network_granted,
        }
    });
    let verdict = (vouched || holds).then(|| grant_plan::image_manifest(declared, vouch));
    out(b"  ");
    out(path);
    match verdict {
        // An unvouched `std` image's pages sit at the `std` layout's slots, so the native rows
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
        OutputDeclaration::Words => b"a word of output".as_slice(),
        OutputDeclaration::Silent => b"no output",
        _ => b"output bytes",
    });
    let items: [(bool, &[u8]); 9] = [
        (
            m.program_argument == ProgramArgumentDeclaration::Required,
            b"an argument",
        ),
        (m.program_argument.hears_words(), b"its words"),
        (
            !matches!(
                m.memory_grant,
                grant_plan::MemoryGrantDeclaration::Forbidden
            ),
            b"memory",
        ),
        (m.clock, b"the clock"),
        (m.configuration, b"the configuration page"),
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
/// With an empty tail this is [`write_holdings`]. With a tail it is a whole line, operators
/// included, because which slot holds what depends on where the stage is. Nothing here moves any
/// authority. See notes/shell/caps-preview.md, "A whole line, not a stage at a time".
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
    // `caps time <command>` previews the command. A loop, not recursion: this frame carries a
    // whole split `Line` by value, and `caps time time time date` is a line a person can type.
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
                // Narrowed by the line's verb, as the progenitor narrows it reading the same word
                // off the argv (milestone 809 (the package client becomes a program), option V2).
                let mut verb = [0u8; grant_plan::MAX_VERB];
                let verb = grant_plan::verb_of(stage, &mut verb);
                let m = grant_plan::narrow_by_verb(&e.program.manifest(), e.program.verbs(), verb);
                out(b"  ");
                out(e.program.name().as_bytes());
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
/// ([`grant_plan::SHELL_CONFIGURATION_SLOT`]). It is the frame the progenitor endows a child declaring
/// [`grant_plan::Manifest::configuration`] with, so its values are printed as what that child will read.
/// `None` prints the row without values and says why.
pub fn write_preview(
    e: &Endowment,
    holdings: &Holdings,
    config: Option<ConfigPage>,
    out: &mut dyn FnMut(&[u8]),
) {
    out(b"  ");
    out(e.program.name().as_bytes());
    out(b" would grant the new process, and nothing else:\n");
    write_preview_rows(e, &e.program.manifest(), holdings, config, out);
}

/// [`write_preview`]'s rows, for an endowment bound against `m`. Separate because a file run by its
/// path is bound against the manifest its note carries, and its endowment's row is a stand-in
/// (`grant_plan::IMAGE_ROW`) that must not be read.
fn write_preview_rows(
    e: &Endowment,
    m: &grant_plan::Manifest,
    holdings: &Holdings,
    config: Option<ConfigPage>,
    out: &mut dyn FnMut(&[u8]),
) {
    // A `std` program's slots are fixed by `crates/std_runtime_contract`, not by listing order.
    let std = m.startup == grant_plan::StartupContract::Std;
    write_result_and_memory_rows(e, m, std, out);
    write_file_and_directory_rows(e, m, holdings, std, out);
    write_page_rows(m, config, std, out);
    write_service_rows(m, std, out);
    write_resolver_row(e, m, std, out);
    write_machine_rows(m, out);
    write_index_rows(e, m, out);
    write_activation_rows(e, m, out);
    write_stream_rows(e, out);
    out(b"    arg    ");
    // From the manifest, never a second list: this line must not under-report an argument.
    if m.program_argument == ProgramArgumentDeclaration::Required {
        write_num(e.program_argument, out);
        out(b"\n");
    } else if m.program_argument.hears_words() {
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
    if e.memory_grant_pages > 0 {
        out(b"    cap 1  untyped   ");
        write_num(e.memory_grant_pages, out);
        out(b" pages  split from this shell's budget\n");
    }
}

/// The rows for what the line designated: a file and its direction, or a directory, its names and
/// whether `-r` grants the walk (notes/shell/caps-preview.md, "The rows, in the order they print").
fn write_file_and_directory_rows(
    e: &Endowment,
    m: &grant_plan::Manifest,
    holdings: &Holdings,
    std: bool,
    out: &mut dyn FnMut(&[u8]),
) {
    if let Some(g) = e.file {
        out(b"    cap 2  endpoint  file     ");
        out(g.name.as_bytes());
        out(if g.writable {
            b"  (read+write, and nothing else on the disk)\n".as_slice()
        } else {
            b"  (read-only, and nothing else on the disk)\n".as_slice()
        });
    }
    if let Some(g) = e.directory {
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
        let n = holdings.place(g.which, &g.directory).render(&mut buf);
        out(&buf[..n]);
        if let grant_plan::ProgramArgumentDeclaration::Words(w) = m.program_argument {
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
            // All of them: the same expansion `echo *.txt` renders.
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

/// The read-only pages the progenitor endows, which no token on the line designates: the clock,
/// the inert configuration with its values (as §111 (inert configuration is a validated page)
/// asks), and the argv, which is bytes and grants nothing (notes/shell/caps-preview.md).
fn write_page_rows(
    m: &grant_plan::Manifest,
    config: Option<ConfigPage>,
    std: bool,
    out: &mut dyn FnMut(&[u8]),
) {
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
    // The values come from the shell's view of the same frame the child is handed, never a copy
    // of the defaults, so the preview cannot drift from what the child reads.
    if m.configuration {
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
    if m.program_argument.hears_words() {
        write_slot(std_runtime_contract::ARGS_SLOT, out);
        out(b"frame     args     read-only. the words on the line, as bytes; they\n");
        out(b"                              name things and grant none of them\n");
    }
}

/// The service endpoints a program may call: entropy, the network, and its supervision domain,
/// each with the right it holds and what that right cannot do (notes/shell/caps-preview.md).
fn write_service_rows(m: &grant_plan::Manifest, std: bool, out: &mut dyn FnMut(&[u8])) {
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
    if m.network {
        if std {
            // Slot 3 is the region the program is built in; the net PAL retypes sockets from it.
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
    if m.domain {
        // `ENUMERATE`, never `READ`, which would describe a wider grant than the one being made.
        out(b"    cap 7  endpoint  domain   ENUMERATE. the processes this shell's jobs are\n");
        out(b"                              supervised by, and no others. it can name them and\n");
        out(b"                              do nothing to them: not receive their deaths, not\n");
        out(
            b"                              collect them, and not learn anything about a process\n",
        );
        // Not "that it exists": `SURVEY`'s machine-wide cursor lets two members count the threads
        // between them (notes/process-view.md's BUGS).
        out(b"                              outside this domain but that it exists\n");
    }
}

/// **The name resolver, beside the network** (milestone 892 (a std program at the prompt holds the
/// network and a resolver)): the zone is the table's (`Program::resolver_zone`), so a person reads
/// which names the program may look up before it runs, where on Unix every process may read
/// `/etc/resolv.conf` and ask about anything.
fn write_resolver_row(
    e: &Endowment,
    m: &grant_plan::Manifest,
    std: bool,
    out: &mut dyn FnMut(&[u8]),
) {
    let (true, true, Some(zone)) = (m.network, std, e.program.resolver_zone()) else {
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
/// and the catalog (notes/shell/caps-preview.md). `m` is the manifest the progenitor endows from.
fn write_machine_rows(m: &grant_plan::Manifest, out: &mut dyn FnMut(&[u8])) {
    // `m`, not `e.program.manifest()`: for an image run from a file, `e.program` is a placeholder.
    if m.machine {
        out(b"    cap 11 frame     machine  read-only. memory, run queue, interrupts, context\n");
        out(b"                              switches and busy time for the whole machine.\n");
        out(b"                              granted by default; the machine's owner can withhold it\n");
    }
    if m.share {
        out(b"    cap 12 region    share    ENUMERATE. this shell's job budget: how much is spent\n");
        out(b"                              and on what. it cannot spend, split or destroy it\n");
    }
    // The reboot row names the sync done first (milestone 805 (`reboot` at the prompt)).
    if m.reboot {
        out(b"    cap 13 reboot    WRITE. restart the machine (the kernel asks the firmware).\n");
    }
    if m.sync {
        out(b"    cap 14 endpoint  sync   WRITE. the file server answers SYNC on it and refuses\n");
        out(b"                              everything else: it flushes the device, and reaches no file\n");
    }
    // The installer endpoint is a copy of one that can do all three things these lines refuse
    // (§270 (a package manager holds an installer endpoint, not the spawn endpoint)).
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
        grant_plan::IndexStoreDeclaration::Forbidden => return,
        _ if e.directory.is_some() => return,
        grant_plan::IndexStoreDeclaration::ReadOnly => false,
        grant_plan::IndexStoreDeclaration::ReadWrite => true,
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

/// **The activation directory, read only** (milestone 809 (the package client becomes a program),
/// `jig list -u`), at the `std` layout's one directory slot in place of the index store
/// (`grant_plan::Manifest::activation_directory`). Printed only when the line designated no
/// directory of its own, as the store's row is.
fn write_activation_rows(e: &Endowment, m: &grant_plan::Manifest, out: &mut dyn FnMut(&[u8])) {
    if !m.activation_directory || e.directory.is_some() {
        return;
    }
    write_slot(std_runtime_contract::FS_DIR_SLOT, out);
    out(
        b"endpoint  dir      /activation  read-only. what is installed, and the index copy
",
    );
    out(
        b"                              beneath it; only the owner's console grants it
",
    );
}

/// Where the output, the second stream and the input go: each a capability `caps` can name
/// (notes/shell/caps-preview.md).
fn write_stream_rows(e: &Endowment, out: &mut dyn FnMut(&[u8])) {
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
        // The program holds this shell's result endpoint, not the file.
        line::Sink::File(g, mode) => {
            out(b"    output   ");
            out(g.name.as_bytes());
            out(b"  (this shell writes the bytes there; the program holds\n");
            out(b"             an endpoint and cannot seek, truncate, re-read or stat)\n");
            // `>` and `>>` hand the child the same endpoint; only what the shell does differs.
            out(if matches!(mode, line::Mode::Append) {
                b"             this shell keeps what is already in it and writes after it\n"
                    .as_slice()
            } else {
                b"             this shell empties it first, before the command runs\n".as_slice()
            });
        }
    }
    // Printed even with no `2>` on the line; no row for a program that declares no second stream.
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
