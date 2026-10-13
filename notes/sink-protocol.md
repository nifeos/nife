# The sink protocol: one way to write bytes somewhere

*Milestone 50, the protocol lane. `crates/byte_sink_protocol`, `fixtures/src/sink.rs`, the std PAL's
`sys/stdio/nife.rs`, and `abi::Error::Gone`.*

## The problem, which was not the one anybody expected

The shell has no `|`, `>` or `<`, so the obvious reading is that a pipe is missing. It is not.
`patches/std-nife/.../pal/nife/rt.rs` fixes `STDOUT_SLOT = 1` and `sys/stdio/nife.rs`
implements `println!` as a SEND on that slot, which means a program's output destination is
already a capability its spawner chose. Redirection is putting a different capability in that
slot. No kernel change, no pipe object, no `dup2`.

What blocked it was that we had four different protocols for "write these bytes there":

| Sink | Protocol before this lane |
|---|---|
| std `println!` | SEND, register-only, 16 bytes per message, `w0` = length, `w1`\|`w2` = bytes |
| `line_editor` | CALL, shared page, `OPERATION_WRITE`, `r0` = bytes consumed |
| `filesystem_protocol` | CALL, handle plus offset plus shared page, `WRITE` |
| console server | shared page, SEND the length, ACK on a second endpoint |

A child cannot be indifferent to what is in its output slot until those are one protocol, and that
unification is this lane. `>` and `|` are parser work afterwards.

## The shape, and the two decisions inside it

A sink is an `Endpoint` capability with `WRITE`, and nothing else.

### Register-only, not a shared page

This is forced, not chosen. Milestone 50's finding is that redirection *is* substituting one
capability in one slot. The moment a sink requires a page mapped at an agreed virtual address,
substitution stops being one grant and becomes a spawn-time negotiation between the shell, the
writer and the sink, and the finding evaporates. It decides the pipe: for `a | b` the shell
creates an endpoint, hands SEND to `a` and RECEIVE to `b`, and that is the entire construction. A
page-based sink would make a pipe cost a frame, a mapping in each of two address spaces, and a
revocation record for each.

The cost is 16 bytes per message. That is the three-word fastpath (notes/abi.md §2) and it is what
std's stdout already used, so nothing on the default path got slower; see the benchmarks below.

### SEND, not CALL

The difference is whether the writer learns anything. A CALL would return "bytes consumed", which
for a self-framing message is always "all of them", and it would pay a second IPC hop on the hottest
path in the system to say so. Back-pressure does not need the reply: SEND blocks until a receiver
takes the message, so the rendezvous is the flow control, which is the property
`line_editor::proto::OPERATION_BYTES` had already written down.

SEND makes the reader of a pipe an ordinary program that does nothing but `receive`. With a CALL
protocol every pipe reader would owe a reply, which means every program on the right of a `|` would
have to know it was on the right of a `|`.

### What the protocol deliberately does not carry

A per-write error code. A sink that can no longer accept bytes says so by ceasing to exist: it
destroys its receiving end and the writer's next SEND fails. That collapses "the reader exited",
"the device is gone" and "the filesystem is full" into one fact, and the honest caveat is that the
writer learns that the sink is over and not why. It is the right trade for a byte stream, whose only
available response to any of the three is to stop, and it is the trade Unix makes as well: a writer
gets `EPIPE` and never the reader's reason. A destination whose failures a client must tell apart is
not a sink, it is a service, and it should be a CALL protocol like `filesystem_protocol`.

Types. This carries bytes. Typed pipelines are a separate and larger fork, recorded as one in
design/roadmap/0050-pipes-and-redirection.md, and nothing in this framing is a step toward one.

Seek, truncate, re-read, stat. A sink appends, and that is the payoff milestone 50 (pipes and
redirection) claims over Unix. `> report.txt` hands a program strictly less than fd 1 with full file
semantics does, and it is the opcode list that makes that true rather than policy.

## The wire

```text
  w0 = (op << 56) | len          w1, w2 = up to 16 bytes, little-endian, low word first

  OPERATION_BYTES = 0    len = 1..=16   the bytes are in w1|w2
  OPERATION_EOF   = 1    len = 0        the writer is finished
```

`OPERATION_BYTES` is zero on purpose. With the opcode at zero a bytes message's first word is
exactly its byte count, which is bit for bit the framing std's stdout already sent. So unifying the
protocol changed no instruction on the fastpath, cost no message, and the benchmark that prices
`println!` cannot tell that anything happened. An opcode is a claim about what a message means; the
cheapest claim to make is the one the wire was already making.

`OPERATION_EOF` is new and it is not decoration. Without it a pipe's reader blocks forever after the
writer exits, and "the producer is done" would have to be inferred from a death notification the
reader may not even be the supervisor for. That is a fact about process supervision standing in for
a fact about a stream. std sends it from the PAL's `cleanup`, which std's runtime calls after `main`
returns and after it has flushed stdout.

## "Gone" versus "never had one", which is the point of doing this now

The old code swallowed a failed SEND, with a comment saying a program without a console "just prints
into the void, which is what every OS does to a process whose stdout is closed". That is right for a
program with no console and wrong for a pipeline, where `yes | head` must end when `head` exits.
So the protocol needs to tell two failures apart:

- never had one: the slot is empty. Keep running; the bytes go nowhere.
- gone: there was a sink and it has been destroyed. End the program.

### The kernel could not tell them apart, and that was the actual finding

Both arrived as `abi::Error::NoSuchSlot`. A destroyed endpoint leaves the holder's capability in
place (endpoints are named generationally, `crates/generational_table`), and the failure surfaces
when `sched::take_ipc_aborted` is set; `syscall.rs` mapped that to `NoSuchSlot`, the same value an
empty slot returns. The only available behavior was therefore the wrong one for a pipeline, and no
amount of userspace protocol design could have recovered the distinction, because the fact lives in
the kernel.

So the ABI grew one variant, `abi::Error::Gone` (-11): *the capability names an object that no
longer exists*. It applies to all five endpoint IPC paths (SEND, RECEIVE, SEND_CAP, RECEIVE_CAP,
CALL), because the fact is about the endpoint and not about the direction.

This is the second time a distinction like this came up and it was resolved the other way the first
time. DECISIONS §32 deliberately makes `Endpoint::REAP` return one error for "already collected" and
"not your child", because telling them apart would let a supervisor probe the tid space of children
it has no relationship with. `Gone` carries no such risk: the capability is one the caller already
holds, in its own capability table, so learning that its object died reveals nothing it was not
entitled to know.

### SIGPIPE, arriving through std's own seam

std already has exactly this two-way split and we had been defeating it. `io::stdio`'s
`handle_ebadf` swallows an error for which `is_ebadf` returns true and propagates everything else,
and a propagated error makes `println!` panic. The old PAL returned `true` unconditionally, so every
failure was swallowed. Now:

- never had one maps to `ErrorKind::Unsupported`, which `is_ebadf` accepts. That is the same
  answer every other ungranted capability gives in this PAL (`std::fs` without a directory,
  `std::net` without a stack), and it is the honest one: this program was not given an output
  stream.
- gone maps to `ErrorKind::BrokenPipe`, which propagates, so `println!` panics with "failed
  printing to stdout: broken pipe". The target is panic=abort, the panic reaches `rt::abort`, and
  the kernel kills the process and attributes the fault.

That is byte for byte what a Rust program on Linux does when its reader exits, because Rust sets
`SIGPIPE` to `SIG_IGN` and lets the `EPIPE` reach the same panic. A third signal disappears, on
the same grounds milestone 48 retired `SIGTTIN` and `SIGTSTP`: the question the signal answered is
already answered by who holds what.

## The sinks

Three programs in `fixtures/`, and the sink among them is the `fs_file_caretaker` shape: something
that speaks the sink contract to its client and the underlying protocol to whatever is behind it.

- `file_sink`: holds a `filesystem_protocol` endpoint and a shared page, creates or opens one name,
  and appends every message's bytes at a running offset. `OPERATION_EOF` closes the handle and
  reports the total. Its client holds an endpoint to this process and nothing that names the FS
  server, so it cannot seek, truncate, re-read or stat, which is milestone 50's "grants strictly
  less than Unix" made structural rather than promised.
- `file_source`: the read-back. It opens the same name in its own FS session and streams the
  contents out over this contract, which is what makes `<` the same shape as `|` for whatever is
  reading.
- **`sink_transcript_writer`**: the writer that cannot tell what it is writing to, used by the
  tests. It writes a fixed transcript to whatever is in slot 0 and reports the classification it got
  back, which is how the "gone" path is asserted by value.

They are three programs, not one binary dispatching `ROLE_FILE`, `ROLE_VERIFY` and `ROLE_WRITER`
out of `arg0`. Role numbers would live in three files (the program, the kernel's `sink_tests`, and
`fs_service`) with nothing keeping them equal, and a mismatch spawns the wrong role and hangs. Three
programs have no role numbers to keep equal.

## What the indifference test proves

`kernel::user::sink_tests`, both ISAs.

The same `std_exerciser` ELF is spawned twice with identical grants except for what is behind
slot 1. Once it gets an endpoint the kernel test receives on directly (the pipe shape: the reader is
an ordinary receiver). Once it gets an endpoint served by `file_sink`, which writes the bytes into a
file on the real RedoxFS image through the real FS server. The test then reads that file back and
compares it, byte for byte, with the transcript the first arm received.

Same binary, same transcript, two destinations that share nothing but the sixteen bytes of a
message. The program is not told which one it has and has no way to find out.

The `Gone` half is asserted separately and by value, because it is a claim about a number. The
kernel creates an endpoint out of a region it owns, spawns `sink_transcript_writer` with WRITE on
it, takes some messages, destroys the region, and the writer reports that its next SEND classified
as `Sent::Gone` rather than `Sent::NoSink`. Without the ABI variant that assertion is not
expressible.

## Benchmarks

The lane added no hop to `println!`'s path, and the icount benchmarks say so: `ipc_rtt_el0`,
`relay_rtt`, `call_reply` and `null_syscall` all moved by less than 1%, in both directions, on
aarch64 and riscv64. That is build noise, so the baseline file was deliberately not updated. No
benchmark prices `println!` itself, so a regression inside the PAL's chunking loop would not show.
The table and both caveats are in [sink-protocol/benchmarks.md](sink-protocol/benchmarks.md).

## What is still missing, named where a reader meets it

- **stdin.** `Stdin::read` still returns honest EOF. Both `< file` and a pipe's read end need an
  input-slot convention that does not exist, and this lane did not invent one: the sink contract is
  one-directional by construction and a source contract is its own design.
  *(Answered: a source is the sink contract received rather than sent. See notes/pipes.md.)*
- **The terminal is a sink** (`components/src/terminal_sink_caretaker.rs`), at the cost of one new
  opcode and one process; see "The terminal's sink adapter" below.
- **The console server's page-plus-ack channel is untouched**, and after building the adapter that
  looks like the right answer rather than a gap. `line_editor` is its only client and now speaks for
  two writers; a second client of the *console* would hit the same one-page wall one layer down,
  with nothing gained.

## The terminal's sink adapter, and the wall it hit

The last of milestone 50's remainders. A program's output slot can now hold the terminal, and it
still cannot tell that from a pipe or a file.

```text
  a declaring child ──sink_proto SEND──► terminal_sink_caretaker ──OPERATION_PRINT CALL──► line_editor ──► console
```

### It is a process for a capability reason, which was known

The cheap move is to have `line_editor` serve the sink contract on the endpoint it already has: a
`SEND` arrives there with no reply capability, so it is trivially distinguishable from the `CALL`s
it serves. That is wrong. That endpoint also carries `OPERATION_READLINE`, and `WRITE` on an
endpoint is the right to `CALL`, so a child handed it as its output slot would hold the terminal's
keyboard. A sink capability that can read the keyboard is not a sink capability. This kernel offers
no receive-on-a-set, so the terminal cannot serve two endpoints itself, and the answer is a separate
process holding both contracts and handing out only one.

### And it needed a new opcode, which was not known

Building the adapter found that `OPERATION_WRITE` reads from the client's one output page, and a
second page-based client would need a second frame. So the terminal contract grew `OPERATION_PRINT`:
up to eight bytes carried in the request's own words, replied when they are on the wire, sharing
`expand_output` with `OPERATION_WRITE`. Two writers, one terminal, one set of manners, no second
frame. Why eight, and the page problem it avoided, are in
[sink-protocol/print-opcode.md](sink-protocol/print-opcode.md).

### What it is for

DECISIONS §67's declared second stream. A program that declares diagnostics gets this endpoint in
its declared slot by default, endowed by the progenitor from the manifest exactly as the clock is,
and `2>` replaces it with a file the shell backs. So a `date` complaining about a missing clock
reaches the screen without passing through the shell at all, which is stronger than the shell
printing it. Nothing the shell does to the output can touch those bytes, and `caps` says so in its
`diags` row.

`kernel::user::sink_tests::the_terminal_is_a_sink_like_any_other_and_the_writer_cannot_tell` is the
proof, and it is the indifference claim made a third time: the same `sink_transcript_writer` ELF,
the same transcript, a pipe and a file and now a terminal, and the program holds one capability in
each case.

### BUGS

- **It is one process per system, not one per client.** Every declaring child writes to the same
  endpoint, so two of them talking at once interleave at message granularity. That is what a shared
  fd 2 does on Unix too, and there is nothing here that could do better without a second adapter and
  a way to decide which one a child gets.
- **The bytes bypass the shell entirely, which is the feature and also the limitation.** A shell
  cannot capture, indent, count or truncate them. `2>` is the only way to put them anywhere else,
  and it works by handing the child a *different* endpoint rather than by intercepting this one.
- **Building it found the progenitor's sixteen-slot capability table for the third time.** One more
  endpoint held across the shell's `build_child` made the boot print nothing at all, so the adapter
  is built after the shell (order: notes/pipes/the-boot.md). "Built last" was this note's phrase,
  and merging milestone 22 (trusted init) proved it wrong. The progenitor now builds a
  `job_undertaker` after it and the capability table has room either way. The real constraint was
  never the ordinal, it was the shell's build. Where the adapter does have to sit is before the
  progenitor gives the construction budget away, because it is a system component and that budget is
  what the system is built from. Building it afterwards would spend the progenitor's scratch pool on
  a whole program. See notes/trusted-init.md.
- **`date` was already speaking the contract before it existed**, which is the `OPERATION_BYTES ==
  0` decision paying out immediately: its hand-rolled framing is bit for bit a `BYTES` message. It
  announced no end of stream, because nothing yet read its output as a stream. `|` has since landed
  and it does: every exit path in `components/src/date.rs` goes through an `end()` that sends
  `byte_sink_protocol::eof()` first, because a reader downstream of a `|` has no other way to learn
  the producer is finished. See notes/pipes.md.
- **No buffering, and it is now measured rather than argued.** A pipe built from this contract is
  full lockstep, where Unix's 64 KB buffer lets a producer run ahead. `bench: sink_throughput`
  (`kernel/src/bench.rs`, two EL0 processes over one endpoint) and
  `bench/host/pipe_throughput.rs` are the two halves of the comparison; the numbers and what they
  decided are in notes/pipes.md.

- **A second stream is a declaration** (DECISIONS §67, 2026-08-03). A program that has diagnostics
  declares a second output in its manifest and gets a second endpoint speaking this same
  contract; `2>` names where those bytes go. Nothing about the framing changed, which is the
  measure of whether the contract was right: a diagnostic is bytes, and the only thing that makes
  one a diagnostic is which endpoint it is on. See notes/pipes.md.
- **`>>`.** Append is a property of `file_sink`'s wiring (it starts at the file's current size)
  rather than a mode a client can ask for, because a client of a sink cannot ask for anything.
  Whether append is a mode on open or a property of the sink is milestone 50's later question.
