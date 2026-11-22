---
status: PROPOSED
raised: 2026-11-22
---

# 327. What a service answers for a verb it does not carry

*(The section number and title are provisional until the merge lands them. Every code name below,
§326's and this file's, is provisional until ruled.)*

The survey reads `nifeos/nife@af1ed9df6`.

## Background

A request to a nife service carries an operation number, the verb, in its first word. A service
knows a fixed set of verbs. When a request names a number outside that set, the service still has
to answer, because the caller waits in a blocking `CALL` until it does. That answer is a word from
the table that §326 (the names and numbers of the services' wire error codes, #3923, ruled
2026-11-22) drafts: 19 codes from -256 down, spelled as `io::ErrorKind` spells them.

§326 renames each site's present answer and leaves this question open (its item 4). Today the
services disagree. The file caretaker answers an unknown verb with `EBADF`, the number it also gives
a forged handle, so its answer becomes `BadHandle`. Ten other sites answer `EINVAL`, so theirs
becomes `InvalidInput`, the code they also give a malformed argument. None answers `Unsupported`,
"this server or capability does not do that", though two servers already send it for a verb they
know and do not offer.

It matters now because §326's renaming touches every one of these lines anyway. A later change to
the unknown-verb answer would be a second pass over the same lines and tests.

## The decision

What every service that speaks §326's table answers when a request's operation number is not one
its protocol carries, and which check comes first when a request is wrong in more than one way.
Services with their own reply vocabulary (entropy, sockets, clock and reset, the terminal and
others below) stay out of scope: §325 (the services' wire error vocabulary) leaves those to each
protocol.

## Is the premise true

Mostly. Three corrections.

- No test pins the file caretaker's number. The test
  `a_file_grants_refusals_keep_their_separate_meanings` in `filesystem_contract`'s `verb/tests.rs`
  pins that an opcode past `verb::LAST` has no row in `verb::of` or `verb::file_grant::of`. That is
  what lets one `let ... else` cover both lookups.
- The caretaker's own contract contradicts its choice. The doc of `verb::file_grant::EBADF` reads
  "no such handle, and only that", and the same test's comment says `EBADF` "stays available to
  mean the one thing it should". The caretaker then gives it a second meaning, and §326's table had
  to write `BadHandle` as "a handle or a verb this capability never minted" to cover it.
- Most `EINVAL` sites give no reason (the table below). Only the subtree caretaker argues for
  `EINVAL` over another code.

## Every site that answers an unknown verb

Found with `git grep -nE "_ =>|verb::of\(" -- components crates redoxfs_server ':!vendor'`, then
each serve loop read by hand.

| site | answer today | under §326 | stated reason | pinned by |
|---|---|---|---|---|
| `fs_file_caretaker.rs`, `serve` | `EBADF` -9 | `BadHandle` | "One refusal site: an opcode this contract does not carry gets `EBADF`, as a forged handle does." | only the shared `None`, above |
| `fs_subtree_caretaker.rs`, `serve` | `EINVAL` -22 | `InvalidInput` | "a word this contract cannot resolve is a malformed request rather than a capability's refusal" | none |
| `fs_nameset_caretaker.rs`, `serve` | `EINVAL` -22 | `InvalidInput` | the doc of its `EINVAL`: "for the verbs this contract does not carry" | none |
| `disk_tree`, `handle` | `EINVAL` -22 | `InvalidInput` | none | none |
| `redoxfs_server`, `dispatch_other_verbs` | `EINVAL` -22 | `InvalidInput` | its doc: "An unknown code is `EINVAL` here." | none |
| `non_volatile_memory_express_data_plane.rs`, `serve` | `EINVAL` -22 | `InvalidInput` | "an opcode this server has no verb for" | a `system_tests` case asserts only `< 0` |
| `designware_mobile_storage.rs`, `serve` | `EINVAL` -22 | `InvalidInput` | none | none |
| `block_driver_roles.rs`, `run_block_server` | -22, a literal | `InvalidInput` | "an opcode this server does not implement" | none |
| `gpu_driver.rs`, `start` | `graphics_contract::EINVAL` -22 | `InvalidInput` | the constant's doc: "the errno a refused request replies with" | `the_refusal_is_negative` pins -22 |
| `framebuffer_driver.rs`, `start` | `graphics_contract::EINVAL` -22 | `InvalidInput` | none | as above |
| `compositor.rs`, `start` | `compositor::proto::EBADOP` -22 | `InvalidInput` | the constant's doc: "a word whose opcode the compositor does not implement" | `the_request_words_round_trip` pins -22 |

Eleven sites in eleven programs: one `EBADF`, ten `EINVAL`. The two graphics drivers also refuse a
rectangle off the screen with the same `EINVAL`.

Two servers already answer `EOPNOTSUPP`, which becomes `Unsupported`, for a verb the protocol
carries and the server declines. `disk_tree::handle` does so for `STATFS`, the modification-time
verbs and the binding verbs: "a disk has no modification time or free count this server could
truthfully give". `run_block_server` does so for `FLUSH` on a device without one.

The order of the checks differs. The file caretaker and `disk_tree` decode the verb first, so an
unknown verb with a forged handle reads as an unknown verb. The subtree and nameset caretakers check
the handle first, so the same request reads `EBADF` there.

## What the tree already does in the analogous case

The kernel gives an unknown method a code of its own. `abi::Error::BadMethod` (-5) is "no such
method on that object", every object's dispatch default, and `BadSystemCall` (-6) is "no such
system call". Neither is `NotPermitted` or `BadPointer`. The `std` overlay relies on this: its
clock-page probe invokes `NO_SUCH_METHOD`, "a method number no object type defines", and reads
`BadMethod` as proof that a real object is there.

The newer contracts with enums of their own do the same. Across every serve loop in `components/`
and `crates/`:

- Four keep an unknown operation apart. `jh7110_clock_and_reset_contract` has
  `Status::UnknownOperation` (2) beside `Status::MalformedRequest`: "An operation the server does
  not serve is answered `Status::UnknownOperation` first." The power-management route has
  `Status::UnknownOperation` (1), the timetable `STATUS_UNKNOWN_OPERATION` (4). The progenitor's
  `ActivationStatus::Unknown` (6) sits beside `NotServedHere` (14), a verb it knows and does not
  serve on that endpoint: exactly the split between "no such verb" and "not offered here".
- Seven fold it into a general bad request: the clock service (`BAD_REQUEST`), the terminal,
  broker and hot-swap contracts (a `BAD_REQUEST` sentinel), the credentialer and login
  (`MALFORMED`), the resolver (`BAD_REQUEST`), sockets (`REPLY_ERROR`) and the spawner
  (`REP_FAILED`).
- Two answer a value that a success could carry. Entropy servers answer zero bytes on purpose: "an
  unknown opcode is answered 'you got nothing', which is true". The NVMe admin service answers 1,
  the same word as a failed retire.

The JH7110 contract also shows what the code is for. Until milestone 1000 (message contracts carry
bind-time version checks) defines a real reply, its server answers `VERSION_QUERY` with
`UnknownOperation`. An unknown-operation answer is how a client learns that its server is older
than itself.

The overlay shows what an ambiguous answer costs. A file server older than milestone 606 (a
directory walk costs what it does on Linux) answers `EINVAL` to a path with a `/`. So
`path_request.rs` sends the path, walks it on `EINVAL`, and only then learns whether the server's
age or the name caused the `EINVAL`. A clear "I do not know that" would have saved the second
request.

## Prior art

Read, except where marked as recalled.

- Linux system calls (man7.org `syscall(2)` and `errno(3)`, kernel `sys_ni.c`). An unimplemented
  system call number returns `ENOSYS` from `sys_ni_syscall`. `checkpatch.pl` warns that "ENOSYS
  means 'invalid syscall nr' and nothing else", so drivers do not borrow it.
- POSIX and Linux `ioctl` (opengroup `ioctl()`, man7.org `ioctl(2)`, `fs/ioctl.c`). `ENOTTY` means
  the operation "does not apply to the kind of object", apart from `EINVAL`, "op or argp is not
  valid". A driver returns the internal `ENOIOCTLCMD` ("No ioctl command"), and `vfs_ioctl`
  rewrites it to `ENOTTY` before user space sees it.
- Fuchsia (`zircon/errors.h`, RFC-0138 on unknown interactions). `ZX_ERR_NOT_SUPPORTED` (-2) covers
  "not implemented, supported, or enabled". FIDL closes the channel on an unknown strict method. An
  unknown flexible method gets a reply whose result union selects the framework error
  `UNKNOWN_METHOD`, "the same as `ZX_ERR_NOT_SUPPORTED`". Fuchsia keeps unknown apart from a
  method's own errors by the variant, not the number. A nife reply has one error word and no
  framework variant, so the number is the only place left to keep them apart.
- Windows (learn.microsoft.com system error codes, `MS-ERREF`). `ERROR_INVALID_FUNCTION` (1) sits
  apart from `ERROR_NOT_SUPPORTED` (50). `STATUS_NOT_IMPLEMENTED` and
  `STATUS_INVALID_DEVICE_REQUEST` ("not a valid operation for the target device") are separate.
  That drivers return the last for an unhandled control code is recalled.
- Rust `std` (`ErrorKind` docs, `sys/io/error/unix.rs`). `Unsupported` means "the operation can
  never succeed". `decode_error_kind` maps `ENOSYS`, `EOPNOTSUPP` and `ENOTSUP` all to
  `Unsupported`, and `ENOTTY` and `EBADF` to `Uncategorized`.
- gRPC (`doc/statuscodes.md`). A method the server lacks answers `UNIMPLEMENTED`, which also covers
  "not supported/enabled in this service".
- HTTP (RFC 9110, sections 15.6.2 and 15.5.6). `501 Not Implemented` is a method the server "does
  not recognize". `405 Method Not Allowed` is one "known by the origin server but not supported by
  the target resource".
- seL4 (`libsel4` `errors.h`). `seL4_IllegalOperation` sits beside `seL4_InvalidArgument`. That the
  kernel returns it for an unknown invocation label is recalled.
- Plan 9 (`intro(5)`). `Rerror` carries a string and no code, and states no rule for an unknown
  message type.

No system reports an unknown operation as a bad handle, and only nife's ten sites report it as bad
input. Linux, Windows, HTTP, FIDL and nife's own kernel keep "no such operation" apart from "not
supported". `std`, gRPC and Zircon's status number fold the two together.

## Whether a forged handle and an unknown verb should stay one answer

The case for one answer would be an information leak: two answers that let a client learn something
its capability does not grant. The leak is not real.

- An unknown-verb answer reveals which numbers the protocol carries. `filesystem_contract`
  publishes that in `verb::TABLE` and `verb::LAST`, and every client links it.
- A bad-handle answer reveals whether the client named a handle it holds. A file grant holds
  exactly one, `grant::HANDLE`, the constant 0. A known verb with a forged handle already gets this
  answer, so folding the unknown verb into it hides nothing.
- Both refusals concern fields of a word the client wrote itself. Telling them apart tells the
  client which of its own fields was wrong.
- A server's version is not a secret. A client that sends a newer verb to an older server needs to
  know that the server is older.

The one-site choice is a convenience of structure, one `let ... else` over two lookups, and the code
gives no other reason.

## Options

**(C) A code of its own for "no such verb" (recommended).** The provisional name is
`UnknownOperation`, the spelling the JH7110 and power-management contracts use. It takes -275, the
next number after §326's table, with `doc(alias = "ENOTTY")`. Every site above answers it, and the
neighboring codes then each mean one thing:

| the request | answer |
|---|---|
| names a verb the protocol does not carry | `UnknownOperation` |
| names a known verb this server or capability does not offer | `Unsupported`, as `disk_tree` and virtio block answer now |
| names a known verb with a handle this capability never minted | `BadHandle` |
| names a known verb with an argument it cannot take | `InvalidInput` |

Every server decodes the verb before any other field. The other fields are the verb's to define, so
a word with an unknown verb has no handle to check. The overlay maps `UnknownOperation` to
`ErrorKind::Unsupported`, as `std` maps `ENOSYS`. A `std` caller sees one kind, and the wire keeps
the difference for clients that act on it.

It wins on three grounds. The kernel does this with `BadMethod`, and so do the tree's newer
contracts and Linux, Windows, HTTP and FIDL. It is the version signal: a client newer than its
server learns so in one reply, which is the job the JH7110 contract gives the same status. And it
is the restrictive choice. If the split proves useless, a later ruling can have servers answer
`Unsupported` for both, and every older client still handles what it gets. Splitting one code into
two later is not compatible. An older client reads the new code as `Unrecognized`, and its fallback
for `Unsupported` never fires.

**(A) Every unknown verb answers `Unsupported`.** One rule and no new code, as Zircon's status
number, gRPC and `std` read it. It loses the version signal: a client cannot tell "this server is
older than me" from "this server is current and declines". The progenitor and the JH7110 contract
both chose to tell those apart. It is the close second, and at equal cost the margin is the
restrictive direction above.

**(B) Leave each site as it is.** Free. It keeps three answers for one fact: `BadHandle` at one
site, `InvalidInput` at ten, and `Unsupported` for the neighboring case. A client of two services
handles one situation three ways. The overlay reports the commonest as `InvalidInput`, "the FS
server refused the request", which reads as a bug in the caller. It loses because the renumbering
is the cheapest moment to settle this.

**(D) (C) or (A), except where an answer would leak.** The same as its base rule at every site
today, because no site leaks. As a rule it adds a judgment that every new service must make and a
reviewer must check, against a case nobody has found. A service that finds one can carry a recorded
exception.

**(E) `InvalidInput` everywhere**, the majority's present answer. It makes an unknown verb look like
a bad argument to a known one, the ambiguity `path_request.rs` pays a second request for. No prior
art read here reports an unknown operation as bad input.

**(F) No reply: drop the request or close the client's channel**, as FIDL does for strict methods.
A nife client waits in a blocking `CALL`, and a server that never replies leaves it waiting until
the server dies. Revoking a client's endpoint is a supervisor's act, not a server's.

## What each option costs, measured

For (C), on top of §326's renaming, which touches each of these lines already:

| where | change |
|---|---|
| §326's table | one row, `UnknownOperation` -275 |
| the eleven sites | the answered constant, 11 lines |
| `fs_file_caretaker.rs` | the constant's doc; the one `let ... else` stays, since both lookups refuse exactly the same opcodes |
| `fs_subtree_caretaker.rs`, `fs_nameset_caretaker.rs` | the verb check moves above the handle check, about 8 lines each |
| `graphics_contract`, `compositor` | `EBADOP` retires for the shared code; `graphics_contract::EINVAL` keeps the bad rectangle; two tests' expected words |
| `system_tests` | the NVMe case can assert the exact code |
| the overlay | one arm, `UnknownOperation` to `ErrorKind::Unsupported` |

About 40 production lines and four tests. (A) costs the same less one table row, (B) nothing, and
(E) the file caretaker's constant alone. (D) costs what its base rule does.

## How reversible it is

Two-way. Nothing outside the tree reads these words yet. nife has no installs, and no server or
client outside nifeos speaks these protocols. Nothing is on crates.io, since the ruling of
2026-11-18 holds every first publish, and no code sends a §326 code yet. It turns one-way at
§325's trigger, the first outside reader. Within that, (C) is the end a later ruling can undo
compatibly, and (A) is not.

## Would we still choose this if both cost the same

Yes. (C) is the dearer of the two close options by one table row and one overlay arm, and (B) is the
cheapest, so the recommendation is not about effort.

## A concrete example

A client built against a newer `filesystem_contract` sends a verb past today's `verb::LAST` (a
hypothetical one; none is planned) to an older server.

Today, through a per-file grant, it reads `EBADF`. The overlay reports that as "the FS server does
not honor that handle", though the handle was right. Through a directory grant it reads `EINVAL`,
"the FS server refused the request". Neither tells it the server is older.

Under (C) every server answers one word, and the client falls back in one step:

```rust
// fs_file_caretaker.rs: the same check in the same place; only the answer changes.
let (Some(v), Some(policy)) = (verb::of(code), verb::file_grant::of(code)) else {
    reply(reply_slot, Code::UnknownOperation.word()); // -275
    continue;
};
// The handle checks below are unchanged and still answer Code::BadHandle (-260).

// A newer client.
match Code::from_word(r0) {
    Decoded::Code(Code::UnknownOperation) => old_way(), // the server predates this verb
    Decoded::Code(Code::Unsupported) => not_here(),      // it knows the verb and declines
    // ...
}
```

## What waits on this

- §326's renaming lane. With a ruling here it changes each of the eleven sites once, to the ruled
  answer, instead of twice. Under (C), §326's table gains one row.
- Nothing else. §326 can be ruled without this, since it renames each site as it stands.

## What was not checked

Whether any client sends an operation number outside its protocol on purpose: grep found none. The
reasoning of each own-vocabulary service beyond its comment. The console reads an untagged first
word as a write length, so it has no unknown verb to answer; whether that is right is the console
protocol's question. The NVMe admin service's answer of 1 for both an unknown word and a failed
retire is ambiguous in the same way, outside this table.
