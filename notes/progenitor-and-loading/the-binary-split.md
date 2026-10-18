# Splitting hello into the system's binaries: milestones 19f.2 to 19f.6

The record behind [`notes/progenitor-and-loading.md`](../progenitor-and-loading.md)'s section "The
programs leave hello". The parent keeps the loader, the archive, the stripped ELFs and where the
claim lives now. This file keeps how each program left the multi-role `hello` binary, what proved
each split, and the shared runtime that followed. It is history: the paths it names were current
when each step landed.

## The first distinct binary: the least_authority_demo (milestone 19f.2)

The least_authority_demo is the first program that is its own binary, not a role of `hello`. It
lives in `components/src/least_authority_demo.rs`: its own `_start`, its own panic handler, ~30
lines, and not one line of hello's code. It shares the `user` package's `link.ld` (so it links at
`0x40_0000` like hello), which is not a conflict because each program runs in its own address space.
`initial_ramdisk_aarch64` packs it as a second archive entry, `"least_authority_demo"`, beside the boot
program's.

Every consumer that used to spawn "a role-6 least_authority_demo of hello" now loads
`"least_authority_demo"` by name and starts it with `x0 = 0` (a standalone binary needs no role
selector) and the input in `x1`:

- init's `init_least_authority_demo` and the initboot spawn service (`hello.rs`), for `run <n>`.
  (`initboot` is the name that path had when this was written. It was a kernel Cargo feature
  and a `script/initboot`, both deleted by milestone 296, which found the feature selected nothing
  `shell` did not. Read `initboot` here as today's `shell`; the account is otherwise unchanged.)
- the kernel-side `shell_service` (the pre-initboot interactive shell), same `run <n>`. (Retired as
  a boot path by DECISIONS §28 and deleted by milestone 41; it is described here as it was.)

Removing the least_authority_demo from `hello` is what proved the split was real: it broke every one
of those call sites (a role-6 spawn fell through to hello's default arm and *faulted*), and fixing
each to load `"least_authority_demo"` is the migration. The hello binary no longer contains a
least_authority_demo at all. Two headless tests pin it:
`a_spawned_least_authority_demo_computes_and_reports` (kernel spawns the least_authority_demo
binary, gets 81) and `the_least_authority_demo_builder_passes_its_demo_an_argument` (init loads it by name, gets
49).

The tiny syscall runtime in `least_authority_demo.rs` (`invoke`/`send`/`exit`) is duplicated from
hello on purpose. When 19f.3 splits the next binary, that second copy is the signal to lift a shared
user-runtime crate, with the requirements known rather than guessed (DECISIONS: don't build the
abstraction before the requirements are).

## The console server, its own binary (milestone 19f.3)

"Console server as its own binary" was the headline 19f was aiming at, and here it is:
`components/src/console.rs`, a distinct ELF init loads by the name `"console"`. It owns the UART and
one request/reply channel, loops (receive a length, copy that many bytes from the shared page to the
UART, ack), and holds nothing else. Same shape as the least_authority_demo split: every consumer
that entered hello at the console role now loads `"console"` and starts it with `x0 = 0`. There were
three, in two domains:

- the kernel-side `console_service::start` (`kernel/src/user/console_service.rs`), used by the
  milestone-8 tour and the pre-initboot shell;
- init's `init_console` (the userspace-built-console test) and the `init_boot` console child.

hello lost the console entirely: the `console_server` function, `uart_put`, and the PL011 register
constants are gone from it. hello keeps only the *printing client* that drives a console (a role,
for the tour), because a client is agnostic to who serves it. The test
`the_console_server_builder_brings_up_a_console_that_prints_for_it` still passes, now proving the `"console"` binary works
end to end: init builds it, wires a channel, delegates the UART, and the line comes out.

## The input driver, its own binary (milestone 19f.4)

The receive half of the terminal, `components/src/input.rs`, lifted out of hello the same way. It
owns the PL011 receive side and its RX interrupt, assembles a line character by character (echoing
as it goes), and hands each completed line to the shell over IPC. Its consumer is init's `init_boot`
child, which loads `"input"` by name and starts it with `x0 = 0`. A kernel-side
`input_service::spawn_wired` did the same until milestone 41 (dead code) deleted it with the rest of
the retired kernel-wired shell. `hello` lost the `input` module and role entirely.

With this, the whole interactive stack runs on distinct binaries. It was verified end to end by
piping real keystrokes into QEMU's serial and typing `run 6` at the prompt. The input binary read
and echoed the line, and the shell (still a hello role) parsed it. init built the
least_authority_demo binary with the argument 6, the least_authority_demo computed 36 and reported,
and the console binary printed `6*6 = 36` back. Input, least_authority_demo, and console, three
separate programs, plus init and the shell, cooperating through capabilities and shared pages.

## The shell, its own binary (milestone 19f.5): the split is complete

The last and most-wired program, `components/src/swish.rs`, lifted out of hello. It holds five
capability slots (console request/reply, the input line endpoint, and the spawn/result endpoints)
and two shared pages, reads a line, and prints. Its consumer is init's `init_boot`, which loads
`"swish"` by name and starts it with `x0 = 0`.

With the shell out, hello contains none of the system's programs. Every service is its own binary in
the archive: `least_authority_demo`, `console`, `input`, `swish`. hello keeps only init and the
milestone-tour demo roles (the printing client, the virtio driver, the capability demos). On the
`initboot` path init loads nothing of hello into a child at all; it builds the whole system from the
four distinct binaries.

It was proven end to end by piping keystrokes into QEMU's serial on both interactive paths
(kernel-side `shell` feature and `initboot`), typing `run 9` at the prompt. The input binary read
and echoed it, and the shell parsed it. init built the least_authority_demo binary with 9, the
least_authority_demo computed 81, and the console binary printed `9*9 = 81`. Four separate programs
plus init, cooperating through capabilities.

One honest wrinkle surfaced: a line burst-piped before the prompt appears loses its first
character. The input driver arms its RX interrupt a few instructions after it starts, and a
pre-armed poll only narrows that window, it does not close it. The input driver's comment used to
claim it "never loses the first character"; it does, under burst-piping. A user typing after the
prompt never hits it, and every line after the first is interrupt-driven and intact. The comment is
corrected; fully closing the window is a separate input-driver fix, not part of the split.

## The shared runtime, `user_mode_runtime` (milestone 19f.6)

With the split done, the `invoke`/`send`/`receive`/`exit` runtime was copied verbatim into five
binaries (hello and the four it shed). `crates/user_mode_runtime` is that runtime, lifted into one
library crate all five now depend on: one `invoke` (the single syscall), and `send`/`receive`/`exit`
built on it. The extraction waited on purpose until the split was complete, so the shared surface
was known rather than guessed (the DECISIONS rule about not building an abstraction before its
requirements exist).

Two things deliberately stayed out of `user_mode_runtime`:

- The `#[panic_handler]`. A panic handler is per-final-binary, and one in the shared library would
  be forced on every program that links it and collide with any program (like hello) that wants its
  own. Each binary keeps its own one-line handler; it is trivial, and it keeps the linking simple.
- Device helpers (the UART `putc` and echo logic in the console and input drivers). Those are not
  runtime, they are the program: they belong to the driver that owns the hardware.
