//! **The programs the shell can spawn, declared once**: the macro that turns the rows into the
//! `Program` enum and its lookups, the compile-time checks on the rows, and the rows themselves.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// **Declare the programs the shell can spawn, once** (milestone 150; name provisional).
///
/// Each row is `Variant { id: N, name: "archive_name" }`, and from the rows this generates the enum,
/// [`Program::name`], [`Program::id`], [`Program::from_id`], [`Program::from_name`],
/// [`Program::ALL`] and [`PROGRAM_COUNT`]. A row needs its [`Program::manifest`] arm (`E0004`).
///
/// The id is written, never derived from position, because the shell and the progenitor agree on
/// it: removal leaves a hole, [`PROGRAM_COUNT`] is one past the highest id, and
/// `the_wire_ids_already_shipped_never_move` pins the shipped ids. A repeated id or name fails the
/// build at the `const` assertions below. See notes/adding-a-program/why-it-works-this-way.md.
macro_rules! programs {
    (
        $(#[$enum_meta:meta])*
        $vis:vis enum Program {
            $(
                $(#[$meta:meta])*
                $variant:ident { id: $id:literal, name: $name:literal },
            )*
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        $vis enum Program {
            $(
                $(#[$meta])*
                $variant,
            )*
        }

        /// One past the highest wire id [`Program::id`] can answer, which is the size of the table
        /// the progenitor indexes with it (`[Option<Elf>; PROGRAM_COUNT]`). **Not the number of
        /// programs**: a removed program's id stays a hole rather than being reused, so the two
        /// differ as soon as one is removed, and [`Program::from_id`] answers `None` for a hole.
        /// Computed from the declarations, so it cannot be forgotten; until milestone 150 it was a
        /// literal a person widened, and forgetting to was the one edit nothing caught.
        ///
        /// Name: ratified 2026-10-18 (calef: "Yes"), with [`Program`].
        pub const PROGRAM_COUNT: usize = one_past_highest(&[$($id),*]);

        const _: () = assert!(all_distinct_ids(&[$($id),*]), "two programs share a wire id");
        const _: () = assert!(all_distinct_names(&[$($name),*]), "two programs share a name");

        impl Program {
            /// Every program, in declaration order. Sweep this rather than `0..PROGRAM_COUNT`, which
            /// has holes once a program has been removed.
            pub const ALL: &'static [Program] = &[$(Program::$variant),*];

            /// Resolve a program by the name typed on the command line.
            ///
            /// Builtins are matched first by [`parse`](crate::parse), so a program named `help`, `echo`, `caps`
            /// or `time` would be unreachable. The program namespace must not contain any of those
            /// names.
            pub fn from_name(name: &[u8]) -> Option<Program> {
                $(
                    if name == $name.as_bytes() {
                        return Some(Program::$variant);
                    }
                )*
                None
            }

            /// The name the progenitor loads it by in the initial ramdisk (nifefs), and the shell
            /// prints. It is also the `[[bin]]` name in `components/` or `fixtures/`, which `xtask`
            /// checks every time it packs an archive.
            pub fn name(self) -> &'static str {
                match self {
                    $(Program::$variant => $name,)*
                }
            }

            /// The stable wire id the shell sends and the progenitor decodes ([`spawnproto`](crate::spawnproto)).
            pub fn id(self) -> u64 {
                match self {
                    $(Program::$variant => $id,)*
                }
            }

            /// The inverse of [`id`](Program::id): the progenitor turns the wire id back into a
            /// program.
            pub fn from_id(id: u64) -> Option<Program> {
                match id {
                    $($id => Some(Program::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

/// [`PROGRAM_COUNT`]'s arithmetic, `const` so the table size is known where the progenitor declares
/// its array.
pub(super) const fn one_past_highest(ids: &[u64]) -> usize {
    let mut next = 0;
    let mut i = 0;
    while i < ids.len() {
        if ids[i] as usize >= next {
            next = ids[i] as usize + 1;
        }
        i += 1;
    }
    next
}

pub(super) const fn all_distinct_ids(ids: &[u64]) -> bool {
    let mut i = 0;
    while i < ids.len() {
        let mut j = i + 1;
        while j < ids.len() {
            if ids[i] == ids[j] {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

pub(super) const fn all_distinct_names(names: &[&str]) -> bool {
    let mut i = 0;
    while i < names.len() {
        let mut j = i + 1;
        while j < names.len() {
            if same_bytes(names[i].as_bytes(), names[j].as_bytes()) {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

/// Name: provisional, flagged 2026-09-24 by the boolean-predicate pass
/// (design/naming/functions-that-answer-yes-or-no-worklist.md). It does not yet follow the Rust predicate rule
/// calef ratified 2026-09-24; recommended `bytes_eq`, because std's `ptr_eq` and `addr_eq` shape.
pub(super) const fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut k = 0;
    while k < a.len() {
        if a[k] != b[k] {
            return false;
        }
        k += 1;
    }
    true
}

programs! {
    /// A program the shell can spawn. The set is small and closed in phase 1; each variant carries a
    /// static [`Manifest`](crate::Manifest) and a stable wire id for [`spawnproto`](crate::spawnproto).
    ///
    /// This is deliberately an enum and not a string lookup at the grant boundary: the shell resolves
    /// a typed program once, and everything downstream (the manifest check, the wire id the progenitor decodes)
    /// speaks the type, not the name. A name that does not resolve is [`Refusal::NoSuchProgram`](crate::Refusal::NoSuchProgram), the
    /// "there is nothing there to name" shape of no-ambient-authority applied to programs themselves.
    ///
    /// Name: ratified 2026-10-18 (calef: "Yes"), with `PROGRAM_COUNT` and `Endowment::program`.
    pub enum Program {
        /// Squares its integer argument and reports the answer. Needs no memory grant.
        LeastAuthorityDemo { id: 0, name: "least_authority_demo" },
        /// Spends a granted untyped budget: maps pages until the budget is exhausted and reports how
        /// many it got. The program that makes `--mem` *real* rather than parsed-and-ignored: the
        /// number it reports is the authority the command line handed it.
        MemoryGrantDepleter { id: 1, name: "memory_grant_depleter" },
        /// A long-running job that *heeds* the cooperative interrupt: it works forever, polling its
        /// interrupt flag between work units, and on `^C` cleans up and exits (DECISIONS §24 (interrupting the foreground process, two-tier and shell-held)). The
        /// cooperative tier made visible: the first `^C` stops it gracefully.
        InterruptHeeder { id: 2, name: "interrupt_heeder" },
        /// A runaway that ignores the interrupt entirely: a tight loop that never checks its flag. Only
        /// the forcible tier (the shell tearing its region down) ends it. The case the cooperative tier
        /// cannot reach, and the reason the second `^C` exists.
        InterruptIgnorer { id: 3, name: "interrupt_ignorer" },
        /// Print the wall-clock time (milestone 51, `components/src/date.rs`). It takes nothing from the
        /// command line: no argument, no memory, no file. **Its whole authority is a read-only mapping
        /// of the clock page, which the progenitor endows and this shell cannot**, and that asymmetry is why
        /// [`Manifest::clock`](crate::Manifest::clock) exists: the grant is real, it is just not something a person designates.
        /// The interactive boot starts a clock service and hands the progenitor the page, so `date` at the prompt
        /// prints a time; on a machine whose RTC the service did not believe it prints "the time is
        /// unknown: the machine has no clock it believes", which is the other true sentence.
        Date { id: 4, name: "date" },
        /// **Remove a name, and with `-r` the tree under it** (milestone 47 (navigation and naming),
        /// `components/src/rm.rs`).
        ///
        /// A **program, not a builtin**, and that is Unix's shape rather than a divergence from it.
        /// `cd`, `pwd` and `ls` are builtins here because the shell is rebinding what it already holds;
        /// `rm -r` is a destructive loop, not a rebinding. A builtin would run with the shell's **entire
        /// endowment**, while a program takes an explicit attenuated grant, so `caps rm -r logs` prints
        /// the subtree at risk before anything happens and a bug in the recursion can only reach what it
        /// was handed. See [`DirectoryGrantDeclaration`](crate::DirectoryGrantDeclaration).
        // `rm` stopped being a builtin in milestone 47's rmdir lane, which is what makes this
        // name reachable: a builtin would have shadowed it, because `parse` matches those first.
        Rm { id: 5, name: "rm" },
        /// **Count what arrives on its input** (milestone 50, `components/src/wc.rs`): lines, words and
        /// bytes, printed as one line of text.
        ///
        /// It declares [`InputDeclaration::Required`](crate::InputDeclaration::Required), so a line
        /// that feeds it nothing is [`Refusal::InputRequired`](crate::Refusal::InputRequired) at the
        /// prompt, and [`OutputDeclaration::Bytes`](crate::OutputDeclaration::Bytes), so `echo a |
        /// wc | wc` composes. With an input and no file, `wc report.txt` is `wc < report.txt`
        /// ([`line::Source::File`](crate::line::Source::File)); see
        /// notes/grant-expression/per-file-grants.md.
        Wc { id: 6, name: "wc" },
        /// **Render markdown for a terminal** (milestone 40, `components/src/mdr.rs`,
        /// notes/documentation.md).
        ///
        /// The same manifest as [`Program::Wc`]: a stream in, a stream out, and nothing else. `doc
        /// notes/glob.md` reads like Unix's `man` and is not: the name is a designation the *shell*
        /// resolves against the directory it holds, and what arrives at the program is bytes. A viewer
        /// that opened the page it renders would be a viewer that could open any page.
        ///
        /// Name: provisional.
        Mdr { id: 7, name: "mdr" },
        /// **List the processes in the supervision domain it was spawned into** (milestone 126 (the `procps` package),
        /// `components/src/ps.rs`, notes/process-view.md).
        ///
        /// The reason [`Manifest::domain`](crate::Manifest::domain) exists, and the same asymmetry [`Program::Date`] made for the
        /// clock: the grant is real and it is not something a person designates on the line. There is
        /// no `/proc` here to name and no pid space to scan, so what `ps` can see is decided entirely
        /// by which supervision endpoint the progenitor put in its capability table, and `caps ps` prints that.
        Ps { id: 8, name: "ps" },
        /// **Name the members of that same domain that match, and do nothing to them** (milestone 126,
        /// `components/src/pgrep.rs`, notes/process-view.md).
        ///
        /// [`Program::Ps`]'s manifest exactly: a domain names its members and does not act on them
        /// (calef, 2026-08-17), so there is no `pkill`. `ProgramArgumentDeclaration::Forbidden`, as
        /// for [`Program::Date`]: the state mask it reads is a register the shell cannot set, so at
        /// this prompt it names every member. See notes/process-view.md ("`pgrep`, and the `pkill`
        /// that cannot exist") and `crates/pgrep`'s `BUGS`.
        Pgrep { id: 9, name: "pgrep" },
        /// **Print how long the ambient monotonic counter has been running** (milestone 126,
        /// `components/src/uptime.rs`, `crates/uptime`).
        ///
        /// [`Program::LeastAuthorityDemo`]'s manifest, not [`Program::Date`]'s: `user_mode_runtime::monotonic_nanos` is granted to
        /// **every** process unconditionally (`kernel/src/arch/*/timer.rs`'s documented, deliberate
        /// exception to DECISIONS §10's no-ambient-authority rule), so this program needed no clock
        /// capability, no domain, no memory, no file, nothing beyond the report channel every spawn
        /// carries. The one member of milestone 126's "machine-wide statistics" row that turned out to
        /// be pure wiring rather than a design fork; see design/roadmap/0126-who-else-is-running.md.
        Uptime { id: 10, name: "uptime" },
        /// **Print the inert-configuration page** (milestone 47's environment-variable fork, DECISIONS
        /// §111; `components/src/printenv.rs`).
        ///
        /// Its whole authority is the read-only page ([`Manifest::configuration`](crate::Manifest::configuration)).
        /// `TZ`, `LANG` and `TERM` print as `KEY=value`, or `KEY (unset)` when a valid page does not
        /// carry the key; a page nobody assembled reads as "no configuration was granted".
        ///
        /// Name: provisional, Unix's own for exactly this (`printenv(1)`/`env(1)` with no
        /// arguments): a term of art already right, per this tree's own naming convention for
        /// standard terms.
        Printenv { id: 11, name: "printenv" },
        /// **Print a version-4 UUID drawn from the entropy service** (milestone 111, `components/src/uuid.rs`).
        ///
        /// Its whole authority is the entropy service's client view ([`Manifest::entropy`](crate::Manifest::entropy)).
        /// It is `disk_partitioner`'s draw without the disk: both call
        /// `uuid::Builder::from_random_bytes` over sixteen bytes from the service. Its second stream
        /// exists so a boot with no entropy service leaves `uuid > id.txt` empty and says why on the
        /// terminal, since a predictable identifier is worse than none. See notes/entropy.md.
        ///
        /// Name: provisional. RFC 9562's own term for the object, which
        /// `uuid` crate also uses; the partition table says
        /// GUID only for the UEFI fields that carry one.
        Uuid { id: 12, name: "uuid" },
        /// **Rank the members of that same domain by the CPU time each has been scheduled for**
        /// (milestone 282 (a thread's CPU time, and the `top` it makes possible), `components/src/top.rs`, `crates/top`).
        ///
        /// [`Program::Ps`]'s manifest plus the machine statistics page, which its summary needs
        /// (DECISIONS §225 (`free` sees the machine and your share)); whether that makes it a program
        /// apart from `ps` is calef's, and `crates/top`'s module docs carry the argument. No count
        /// argument: there is no optional one, and a `top` that cannot be typed bare is not a `top`.
        ///
        /// Name: provisional.
        Top { id: 13, name: "top" },
        /// **Reach the network from the prompt** (milestone 590 (the booted system starts its
        /// network stack),
        /// `fixtures/src/network_echo_client.rs`): open a TCP connection to the QEMU runners'
        /// echo peer (10.0.2.9:7777, `helpers/qemu-runner-aarch64.sh`'s `guestfwd`), send a
        /// fixed line, and print what came back.
        ///
        /// A fixture declaring [`Manifest::network`](crate::Manifest::network): the echo peer exists
        /// only inside a QEMU runner, so it proves a person at the prompt reaches the network through
        /// the stack the progenitor built, and nothing more.
        ///
        /// Name: provisional.
        NetworkEchoClient { id: 14, name: "network_echo_client" },
        /// **Try to reach the network without declaring it, and say what the kernel answered**
        /// (milestone 590 (provisional), `fixtures/src/unreachable_network_witness.rs`).
        ///
        /// The negative control for [`Program::NetworkEchoClient`], and the reason it is a program
        /// rather than a sentence: its manifest declares nothing, it `CALL`s [`NETWORK_SLOT`](crate::NETWORK_SLOT) with a
        /// real socket-contract request anyway, and prints the error. A progenitor that endowed the
        /// network to every child rather than to the declaring one would turn its line from a
        /// refusal into a reply. `unwritable_clock_witness`'s shape, one authority over.
        ///
        /// Name: provisional.
        UnreachableNetworkWitness { id: 15, name: "unreachable_network_witness" },
        /// **The `std` demonstration program, spawned from the prompt** (milestone 595
        /// (provisional), the shell runs a `std` program). `std_exerciser/src/main.rs`, built with
        /// `-Zbuild-std` against nife's patched standard library rather than as a `[[bin]]` in
        /// `components/` or `fixtures/`, which is why it is in an archive only when `cargo xtask
        /// std-exerciser` ran first (`script/test` does).
        ///
        /// It proves the [`StartupContract::Std`](crate::StartupContract::Std) layout at the prompt:
        /// its transcript asserts the heap, clock, randomness and configuration page each arrived at
        /// its fixed slot. Listing it here settles nothing about §219 (how the shell names an
        /// installed program to the spawner).
        StdExerciser { id: 16, name: "std_exerciser" },
        /// **How much memory the machine has, and how much of this prompt's budget is spent**
        /// (milestone 126, DECISIONS §225 (`free` sees the machine and your share), `components/src/free.rs`, `crates/free`).
        ///
        /// Two grants, and they are the ruling's two lines. [`Manifest::machine`](crate::Manifest::machine) is the machine
        /// statistics page, which owner policy grants by default and can withhold; [`Manifest::share`](crate::Manifest::share)
        /// is this prompt's job budget narrowed to `ENUMERATE`, which answers "yours". Neither can
        /// change anything: the page is read-only and the budget view cannot spend.
        ///
        /// Name: provisional, milestone 126's `free` lane, 2026-09-26. Upstream `procps`'s, for the
        /// program a reader types to ask this.
        Free { id: 17, name: "free" },
        /// **The machine's counters since boot**: run queue, memory, interrupts, context switches
        /// and the busy/idle split (milestone 126, DECISIONS §225, `components/src/vmstat.rs`,
        /// `crates/vmstat`).
        ///
        /// The machine statistics page and nothing else. Not `free`'s manifest, because it does not
        /// hold the budget view: `vmstat` describes the machine and says nothing about "yours".
        ///
        /// Name: provisional, milestone 126's `free` lane, 2026-09-26. Upstream `procps`'s.
        Vmstat { id: 18, name: "vmstat" },
        /// **Where this prompt's job budget went, by kind of kernel object** (milestone 126,
        /// DECISIONS §225, `components/src/slabtop.rs`, `crates/slabtop`).
        ///
        /// Upstream `slabtop` lists the kernel's slab caches. This kernel has none (milestone 14 (kernel objects from untyped)
        /// removed its heap and slab); kernel objects are carved from regions their holders own,
        /// so §225 turned the question into `MemoryRegion::USAGE` asked per object type. It holds
        /// the budget view alone, which is why it is not `free`: it does not see the machine.
        ///
        /// Name: provisional, milestone 126's `free` lane, 2026-09-26, flagged harder than the other
        /// two: the upstream name promises a kernel-wide cache view, and this is one budget's
        /// breakdown.
        Slabtop { id: 19, name: "slabtop" },
        /// **Take the screen and the keyboard, and run a prompt on them** (milestone 632
        /// (provisional), `components/src/graphical_terminal.rs`; calef's 2026-09-30 ruling that graphics is
        /// launched from the swish prompt rather than built at boot).
        ///
        /// The shell asks (`spawnproto::Wiring::graphics`, which carries no capability) and the
        /// progenitor builds `gpu_driver`, `display_terminal`, the line discipline and, with a
        /// keyboard, `keyboard_driver`, then starts this program wired to them; with none, keystrokes
        /// come over the UART. It declares nothing a line designates, so every operator on it is
        /// refused.
        ///
        /// Name: ratified 2026-10-03 (calef, #1493). Refused `screen` (clashes with `SCREEN_BIT`,
        /// the screen-narrowed tail of §106 (an unredirected tail stage's output goes to the
        /// screen, not the shell), and with GNU screen), `gate` (the tree's word for a check that
        /// fails loudly, about 3,700 uses, and the x86 IDT's interrupt gate), `display_session` and
        /// `display_console` (`console` is the UART server). It names the whole stack
        /// (`gpu_driver`, `display_terminal` and the keyboard), so it survives any later change of
        /// VT engine behind `display_terminal`.
        GraphicalTerminal { id: 20, name: "graphical_terminal" },
        /// **Restart the machine** (milestone 805 (`reboot` at the prompt), DECISIONS §251
        /// (restarting the machine is a kernel object the progenitor hands out)),
        /// `components/src/reboot.rs`.
        ///
        /// The one program that declares [`Manifest::reboot`](crate::Manifest::reboot) and [`Manifest::sync`](crate::Manifest::sync). It syncs
        /// the writable filesystem through its sync-only capability, prints the answer, then
        /// invokes the reboot object, which does not return when the reset works. When the firmware
        /// refuses, it says so on its second stream.
        ///
        /// Name: provisional. A verb, which design/naming.md passes only as a term of art, like
        /// `bind`; every Unix since V7 has spelled this one `reboot`.
        Reboot { id: 21, name: "reboot" },
        /// **The package manager** (milestone 809 (the package client becomes a program)): it
        /// installs, removes and rolls back packages, and lists what the image vouches for.
        /// `jig/src/main.rs`, a `std` program built beside `std_exerciser`, which is why it is in
        /// an archive only when `cargo xtask std-exerciser` ran (`script/test` and CI do).
        ///
        /// The one program that declares [`Manifest::installer`](crate::Manifest::installer)
        /// (DECISIONS §270 (a package manager holds an installer endpoint, not the spawn endpoint)).
        /// It fetches a package over the network itself, so the progenitor parses no network input.
        ///
        /// Name: ratified 2026-10-06 (calef): the guide that makes flat-pack assembly come out the
        /// same every time. Refused `package`, `pkg`, `bpm`, `knap` and `flatpak`; `kit` and `cam`
        /// collide with shipped commands. Its verbs are apt's where apt has one
        /// (design/naming/command-line-rulings.md, 2026-10-10).
        Jig { id: 22, name: "jig" },
    }
}
