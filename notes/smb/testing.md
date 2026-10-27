# How the SMB adapter was tested, and what running it looked like

An appendix to [notes/smb.md](../smb.md). The code it describes was removed on 2026-08-30; read every
present tense as "as of `685900ec`". It holds the host tests, the two-ISA QEMU gate leg by leg, and
the commands that no longer exist.

## Host tests

`cargo test -p smb_proto` drives the state machine through a full client session, the compound path,
the read-only refusals with their statuses, the listing walk, SPNEGO round trips, and the transport
framing.

Identity's nine are in there too, against a `#[cfg(test)]` authenticator that holds the password.
That is the *credential service's* position, and the one the SMB server is never in. Two of them are
the ones that would go green on a decorative implementation, and so are worth naming. An anonymous
AUTHENTICATE must be refused by a share with an authenticator. And a refused session must leave the
share *unreachable*, rather than merely answer a status. The others pin these:

- the guest label clear on a proven session and set on an unproven one;
- a retry succeeding on the same connection after a refusal;
- a captured proof failing against the next connection's challenge;
- a proof derived over a different domain failing;
- and `LOGOFF` forgetting that anybody was named.

## The QEMU gate, both ISAs

It proves bytes crossing in both directions, with a different process witnessing each.

The read leg asserts a file `fs_test_client`'s seed role put on the filesystem. The write leg has
xtask's prober create a file over SMB2 and write it in two chunks at two offsets plus a tail. It cuts
the tail off with `SET_INFO`, stamps its timestamps, closes, and then deliberately does not read it
back. A second in-guest process (`fs_test_client`'s verify role, holding a directory capability and
nothing that names the network) reads it through the FS server after the adapter has stopped serving.
It reports a classification: exact, absent, wrong size (the truncate leg), or wrong bytes (an offset
or chunking bug). A prober that read back its own write would prove only that the adapter remembers,
which an adapter can do with no filesystem under it at all.

The subdirectory leg is the same discipline one level up, and is the one check the prober genuinely
cannot make for itself. It makes a directory over the wire with `FILE_DIRECTORY_FILE`, writes a file
inside it by its full path, and lists the directory back. A listing is a fact about the server's own
view, so a leaf shown as a full path is a bug this side *can* see. What it cannot see is whether a
directory reached RedoxFS. A share that ignored the separator would create a file literally called
`tm_bands\band0` in the share root, and that is indistinguishable from success on the wire. The verify
role descends with `fs::OPENDIR`, and reports `DIR_IS_A_FILE` when the answer is `ENOTDIR`, which is
exactly that failure named.

The prober asserts `FileFsFullSizeInformation` is not the nominal constant. That is the `STATFS` half
arriving where Time Machine will read it.

The Apple leg rides the first CREATE, because that is where a Mac puts it. The prober's open of the
seeded file carries the `AAPL` context. The same response that has to report the file's real size has
to carry the answering context. What that proves over a host test is the whole adapter. The context
had to be chunked out through the socket contract, reassembled by a real TCP stack, and arrive with
its `CreateContextsOffset` still measured from the right place. The prober names each claim
separately, so a bit that goes missing says which one it was, rather than "the bytes differ".

The identity leg is three AUTHENTICATE messages down one connection, in the order that makes each one
mean something. First an anonymous login, refused; that is what this prober itself sent until identity
landed, and what the guest used to admit. Then a real proof with one bit flipped, refused. Then the
real thing, accepted and not flagged guest. After each refusal it tries a `TREE_CONNECT` and requires
`STATUS_USER_SESSION_DELETED`, because a refusal that only changes a status word is not a gate.

## What that arrangement proves, and no unit test could

The password exists only on the host. Inside the guest it exists only as an `NTOWFv2`, inside a sealed
credential store held by a process with no network. The adapter that answers the exchange holds one
endpoint to that store, and cannot compute any of the three proofs. The bytes it then serves come from
a third process that holds no network either. Four processes, four authorities, one `ls`.

And the kernel closes it from the outside. `assert_smb_held_no_key` reads the frame the adapter and
the store share through the direct map, which no userspace program could do. It requires the
published `NTOWFv2`, the published `SessionBaseKey`, and every other nonzero byte to be absent. That
is the check the adapter could not make about itself. It is what turns
`an_smb_server_authenticates_a_session_without_ever_holding_the_key`, from milestone 65 (a secrets
service), from a claim about a stand-in into a claim about the real SMB server.

## How the gate is wired

The adapter rides the spawn of the milestone-107 inbound test
(`a_host_process_connects_to_the_guest_and_is_answered`), as a second client of the same `Stack`
endpoint, from milestone 107 (the socket contract learns to accept). A second `net_stack` does not fit
the test boot: its 192-page region is never reclaimed; see `virtio::MAX_DEVICES` for the recorded
failure.

The test wires the FS service, seeds the gate's file through it, and grants the adapter the directory
capability. It hands the adapter the credential service's verify endpoint, the same sealed store the
milestone-56 tests use, latched once per boot. The runner adds a second `hostfwd`, on an SMB-specific
host-forward environment variable, removed with the runners' SMB block. xtask's SMB prober performs
the mount-shaped exchange end to end, asserting the seeded file's bytes, while the echo prober runs
beside it. Both verdicts gate.

This is the first boot that holds the block server, the FS server, `net_stack`, the SMB adapter and
the credentialer at once. So the test prints the free-frame count where it wires them; the day the
budget stops fitting, the number is already in the transcript.

## EXAMPLES

None of these commands exist any more. They are kept because what they *were* is part of the record.
This is what running the thing looked like, and the second one is the only place the mount
instructions a real Mac was given were ever written down.

```sh
script/test               # both ISAs; the smb check reported beside the inbound check
cargo xtask smb-serve     # booted the kernel under QEMU, SMB forwarded to 127.0.0.1:10445
```

With `smb-serve` running, the Mac side was either Finder's Go > Connect to Server (Cmd-K) at
`smb://127.0.0.1:10445/share`, choosing Guest, or:

```sh
mkdir /tmp/nife-share && mount_smbfs -N //GUEST@127.0.0.1:10445/share /tmp/nife-share
```

The share was the RedoxFS image, and it was read-write. So `cat /tmp/nife-share/motd` read bytes that
had come off a virtual block device, through the block server, the FS server and the SMB adapter,
over this kernel's own TCP stack. That sentence is the demonstration this note exists to preserve.

To read the code, check out `685900ec`, the last commit that holds it:

```sh
git show 685900ec:user/src/smb_server.rs
git log --oneline 685900ec -- crates/smb_proto user/src/smb_server.rs   # 35 commits
```
