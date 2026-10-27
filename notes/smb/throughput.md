# Throughput: the 64 KiB transfer, and where the rest of it went (2026-08-19)

An appendix to [notes/smb.md](../smb.md), which keeps the one number that survives. The code it
describes was removed on 2026-08-30; read every present tense as "as of `685900ec`".

Milestone 138 (close the read gap: a 4 KiB request must stop moving 128 KiB) step 3 grew the file
contract's transfer from one page to sixteen. It measured 8.02x on a sequential write and 5.67x on a
sequential read, against `fs_proto`, by a client that speaks that contract directly. None of it
reached a mounted share. This is what that cost, and what fixing it bought, under milestone 55 (Time
Machine).

## What was in the way

`FsShare::read` and `FsShare::write` chunked every SMB transfer into `fs_proto::PAGE`-sized requests:

```rust
let want = (out.len() - done).min(fs_proto::PAGE);      // read
let chunk = (data.len() - done).min(fs_proto::PAGE);    // write
```

So a Mac writing 64 KiB arrived at the store as sixteen separate 4 KiB writes. It writes exactly
64 KiB, because that is the `MaxWriteSize` this server negotiates. Each write paid the per-request
fixed term that milestone 138 step 1 measured at 87% of a 4 KiB write. The contract had permitted a
bigger request since step 3; this program had not asked for one.

The fix is two `min`s and a mapping. Both clamps read `fs::TRANSFER_MAX` from the contract rather than
a number of their own. So a future change to `fs::TRANSFER_PAGES` reaches this program without anyone
editing it. The kernel wiring maps all sixteen pages at `FS_VA`, because nothing checks that a client
asked for no more than it mapped (that constant's marked foot gun).

The other three places this program uses `fs_proto::PAGE` are untouched, and must stay so. A name, a
`READDIR` page, a `statfs` record and a rename's two names are lengths the *server* chooses. Step 3's
serve loop clamps those to one page precisely so they cannot land in a client's unmapped second page.

## Measured, through a real client

`bench/smb-throughput.sh` sweeps `fs::TRANSFER_PAGES` around a leg in xtask's SMB prober. A host
process, over the forwarded TCP connection, writes 1 MiB and reads it back in
`smb_proto::MAX_TRANSACT`-sized messages. Two rounds at each point, aarch64, debug build under QEMU.

| direction | 4 KiB file transfer | 64 KiB file transfer | speedup | against `fs_proto`'s own |
|---|---|---|---|---|
| write | 0.065 MiB/s | **0.31** | **4.8x** | 8.02x |
| read | 0.15 MiB/s | **0.36** | **2.4x** | 5.67x |

So most of the write speedup reached the customer path, and about half the read speedup did. The raw
rows are 0.07/0.06 and 0.31/0.31 for writes, 0.18/0.12 and 0.36/0.36 for reads. The machine was not
quiet (load 4.8 to 15.8). The interesting thing about that is how little it mattered at 16 pages. Two
rounds nine load-points apart agree to the last digit in both directions, because this path is
bounded by emulated device latency rather than by host CPU. The 4 KiB rows spread more, and the ratios
are taken from the means.

## Where the rest went, and it is one number in a different contract

The socket contract chunks at 4080 bytes. `socket_contract::DATA_MAX` is `4096 - OFF_PAYLOAD`,
because a client and `net_stack` share exactly one frame. So `send_all` and `recv_into` in
`smb_server` cross that contract about seventeen times in each direction per 64 KiB SMB message. That
did not change, and is now what a transfer costs. A 64 KiB write went from ~985 ms to ~206 ms per
message, and what remains is not the filesystem.

It is the same defect as milestone 138 step 3's, one contract over, with the same shape of fix
already demonstrated. The region is one page because nobody declared it otherwise, and the wire
already carries a length. See the BUGS entry, which is where the promotion trigger sits.

And SMB's own ceiling is 64 KiB, so raising `fs::TRANSFER_PAGES` past 16 buys this path nothing.
`smb_proto::MAX_TRANSACT` is the one value this server answers for `MaxTransactSize`, `MaxReadSize`
and `MaxWriteSize`, and no client asks for more than it is told. Any future transfer-size work that
means to help a mounted share has to raise both numbers. The reason 64 KiB is what `MAX_TRANSACT` says
is the SHOULD of [MS-SMB2] section 3.3.5.4, rather than anything in this tree.

## What that is worth to a backup, stated at the depth each number was measured

The write path alone: a 100 GiB first backup was 17.6 hours of sequential writing, at the 1.62 MiB/s
the record-level sweep measured. It is 40 minutes at step 3's 42.77 MiB/s. That is `fs_proto`'s
number in the release benchmark harness. This milestone is what puts a Mac's bytes on it, rather than
on sixteen 4 KiB requests.

End to end, no hours figure is offered, and refusing to give one is the honest result. The table is a
debug build under QEMU with user-mode networking, which is the wrong instrument for a wall clock. What
transfers from it is the ratio, not the rate. The rate a customer will see needs the same measurement
on the hardware. Until then, this says a Mac's backup got about five times faster to write and about
twice as fast to read, and does not say how long one takes.
