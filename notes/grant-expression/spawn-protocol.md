# The spawn protocol: the exchange, and the reasons behind its words

An appendix to [notes/grant-expression.md](../grant-expression.md). `grant_plan::spawnproto` is
the wire between the shell and the progenitor. Its doc comments say what each word and bit holds.
This page holds the order of the whole exchange and the reasons behind the choices that are not
visible in the code.

## The exchange, in order

1. Request. Three words on the spawn endpoint: the program id, the integer argument, and the
   memory-grant page count (`request`, `prog_id`, `program_argument`, `memory_grant_pages`).
2. The directory grant, if announced (`Wiring::directory_grant`): `GRANT_WORDS` plain `SEND`s.
   They come before the capabilities because they are data, and mixing the orders would put a
   `RECEIVE` where a `RECEIVE_CAP` belongs.
3. Delegation, in a fixed order both sides read from the same `Wiring`. First the supervised
   job's pair, then the sink, the source, the diagnostic endpoint and the screen-narrowed tail's
   completion endpoint. Then the `--mem` untyped, split from the shell's own budget, and the
   machine statistics page last. The order stands in for tags, so a promise nobody receives would
   deadlock both sides.
4. Outcome. The progenitor builds, endows and starts the child, which answers on the result
   endpoint. If it cannot, it sends `SPAWN_FAILED` there, so the shell's single read completes.
5. Death. For a killed child, `job_undertaker` sends `JOB_FAULTED` on that same read.
6. Reaped. After every collect, `job_undertaker` sends `reaped` on the spawn endpoint through a
   copy badged `UNDERTAKER_BADGE`. That tells the progenitor the job's memory and file-service
   window are back.

## Data messages and frames

A directory grant's two messages are opaque to `grant_plan`. They are
`filesystem_contract::grant`'s packing, and `grant_plan` has no non-dev dependency on that crate on
purpose: the shell must be able to check a command line without linking the filesystem contract.
The shell packs them, the progenitor forwards them, and nothing in between reads them. A change to
how a grant is packed is then a change in one crate rather than in this wire.

The screen bit's capability is not a sink. The progenitor builds the default sink from its own
`term_sink`, unprompted, as it builds a diagnostic default. What travels is a fresh endpoint the
shell minted, which becomes the child's fault target under §26 (the fault endpoint: thread death
becomes a message a supervisor holds). The kernel then delivers the child's exit there instead of
to the progenitor's reaper.

An image's frames are copied before they are hashed, because the caller keeps a mapping of them and
could change the bytes between a hash and a build. The progenitor maps each frame through the
loader's scratch window (`supervision_protocol::map_scratch`) and copies it into a page of its own.
It deletes the capability before taking the next, so a request of any size costs its capability
table one transient slot. A digest found in the installed-programs table is vouched and runs with
that entry's manifest. An install copies its frames the same way, and a vouch sends its frames
before its name because the progenitor stages the capabilities before it reads the data.

The argv page is copied for the same reason, and its bytes are not read on the way. §170 (how a
foreign program is told what to do) ruled that they carry no authority. What a word may reach is
the directory grant's business, and nothing on that page can widen it. The name-set frame is how a
program that hears words gets exactly the names its words designated. `rm *.txt` would ride it too,
but `components/src/swish.rs`'s `dir_grant` does not send it for `rm` yet.

## Why the progenitor serves activation requests

§208 (installing a package is granting it, and the installed programs are versioned) argued that
"the supervisor that performs a live swap and the thing that decides which version is active are
the same authority". §219 (how the shell names an installed program to the spawner) already gave
the progenitor the read half: it looks every image's digest up in the live generation. It holds the
file service with `WRITE`, the image's catalog in its archive, and the frame-staging path an image
request built. The client is `jig`, which sends on its installer endpoint, and the owner's console
sends `vouch` on the spawn endpoint. An installer endpoint's answers go to the holder's own reply
slot, because the shell is reading the result endpoint while the holder runs.

## The run-unvouched capability is a message

The capability is granted without `GRANT`, so its holder cannot pass it on (§219 (how the shell
names an installed program to the spawner)'s recorded limitation 2), and the kernel refuses to
delegate a capability that lacks `GRANT`. What a holder can do with a `WRITE`-only endpoint is
send on it. The progenitor holds the only `READ` on that endpoint and takes one message from it, at
a fixed point in a request that claimed it. A message there came from a holder, because nothing else
can reach the endpoint. It is sent after every delegated capability, so the progenitor knows the
digest's verdict and the whole delegation before it decides.

The progenitor rather than the kernel gives the message its meaning. That is why it is a bit on a
userspace word and not a method, and the surface of §10 (process model: capability-based,
microkernel) is unchanged.

## Why the machine page travels with the request

The progenitor's capability table peaks during the login block at one slot under the table's size
(`kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED`). A page held for the life of the boot would spend
that last slot, and the first CI run that tried it measured the table full. So the progenitor hands
the page to the shell before the login block and keeps no copy. What reaches a program is then
decided by what its session holds, which is also how §225 (`free` sees the machine and your share)
reads: granted to every login and withholdable by the owner.

## Why a faulted job is reported by the supervisor

A child the kernel killed sends nothing, so before `JOB_FAULTED` the prompt never came back. It was
measured on 2026-09-02 with `least_authority_demo` patched to trap. §26 delivers every death to
exactly one endpoint, so the three ways to report it are a choice of who holds that endpoint.

The shell asking loses first. It would have to decide when to ask, and with no non-blocking receive
in the ABI that is a poll interval. A poll interval cannot tell a slow job from a dead one, which is
what the hang already could not tell.

The endpoint carrying the death loses on the ordinary path. §26 flows exits down the same endpoint,
so every ordinary job would leave a second message behind its answer for the next command's read.
It would also move collection into the shell and take every job out of the progenitor's supervision
domain, which is what `ps` and `pgrep` read. §106 (the `terminal_sink_caretaker` narrowing) accepts
that cost for one narrow stage, not for all of them.

The supervisor telling costs one capability and one word. `job_undertaker` already receives the
death and collects the corpse, and the word lets it say so.

## The reaped message's layout

The kernel delivers a badge on a plain `SEND` too, in `x3` for `RECEIVE` and `RECEIVE_CAP` alike,
under milestone 613 (a system log service: the in-memory half)'s amendment to §230 (badged endpoint
capabilities). That is why the progenitor can check the badge on every receive of a request.

`w0` is the label the progenitor put on the job's supervision capability, which the kernel returns
beside the death (§148 (resolves by asking the kernel)). The progenitor keys on it because it chose
it before the job ran; it holds the job's thread capability, not its thread id, so a tid alone would
name nothing it knows. The tid rides along because the ruling asked for it and a trace reader wants
it. `w0` and `w1` are the two data words a `RECEIVE_CAP` returns (`x0`, `x2`), so a reaped message
arriving where a delegated capability was expected is still read whole.

## BUGS

- Any promised message a caller withholds hangs the progenitor: claiming run-unvouched
  (`Wiring::run_unvouched`) without holding `RUN_UNVOUCHED_SLOT`, or a shell that cannot retype a
  frame mid-request. Nothing lets either side abandon a half-sent request. Today the boot shell is
  the only holder, and a holder can also lend its presentation to another's claim.
- An image's manifest note (`grant_plan::image_manifest`) reaches only what these words carry, so
  a note declaring a file, directory, input, option, job, second stream, silent output or `std` is
  refused (`grant_plan::image_can_carry`). Its `Endowment` names a stand-in row
  (`grant_plan::IMAGE_ROW`), and reading that row instead of the manifest is the foot gun.
- The shell reads the note twice, and the progenitor judges what arrived and refuses a misfit with
  `SPAWN_REFUSED_BY_MANIFEST`. Only a plain line runs an image, never interruptibly.
- Every activation verb, `Activation::Vouch` included, is open to any spawn-endpoint holder. That
  is safe only while the owner's boot prompt is the one, under §221 (the boot prompt is the owner's
  console). Install, remove and rollback have `INSTALLER_BADGE`, and vouch has none.
