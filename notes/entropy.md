# Entropy

Where random numbers come from on this machine, who is allowed to reach the device, and what
`std::random` does when nobody granted you any. Milestone 56, the entropy half; the decision and its
argument are [DECISIONS §44 (entropy is a capability, `std::random` improves transparently, and the refusal is loud)](../design/decisions/0044-entropy-capability.md), the contract is `crates/entropy_contract`.

## The thing this replaced

`std::random` was splitmix64 seeded from the virtual counter, and the file said so in its
first line: *"not cryptographic, and saying so is the point."* The counter is the ABI's one ambient
readable, so the stream was predictable to anyone who could guess boot-relative time. Fine for a
`HashMap` seed. Useless for a key, a token, or a challenge.

That is not a small caveat sitting in one file. It taints anything security-adjacent anywhere in the
tree, because a caller cannot tell a weak source from a strong one by looking at the call. And it
blocked SMB authentication outright: an NTLMv2 server challenge that an attacker can guess is a
challenge an attacker can precompute a response for, so the whole exchange stops proving anything.

The old file named its own replacement ("a virtio-rng driver feeding a capability-granted service is
future work and would replace this file, not patch it"), which is what this is.

## The shape

```text
   virtio-rng ──virtio (mmio or PCIe, §18)──► entropy ──an endpoint──► clients
    (a device)                                  │      (CALL, bytes in the reply)
                                                └── its DMA page: nobody else maps it
```

One process holds the device. Everything else holds one endpoint that names no device, and that
is the entire difference between "may obtain randomness" and "may reach the random-number
generator". A client cannot program the queue, cannot map the page the device writes into, and
cannot ask the device for anything the service did not ask on its behalf.

The service's whole authority is four things placed before it ran: the request endpoint (RECEIVE), an
`Interrupt`, a `Virtio` transport, and a readiness endpoint. No initrd, no budget, no filesystem, no
network. A compromised entropy service is a machine whose random numbers an attacker chooses, which
is exactly as much damage as owning the entropy source should be worth, and it is the reason the
device is not sitting inside every program.

It needed nothing new in the kernel. No capability type, no syscall, no method number: an
`Rendezvous` with WRITE is the whole grant, and `caps` already prints it. That is the third time this
pattern has paid (the framebuffer, the clock, now this), and it is worth noticing that a capability
system's answer to "add a new privilege" keeps being "hand out a smaller object you already have".

## Attenuation by operation, not by object

This is the fourth appearance of one idea and it deserves the name the roadmap gives it:

| milestone | the object | the attenuated power |
|---|---|---|
| 51 | the wall clock | an NTP client may **propose** a time, not set it |
| 51 (§43) | the clock page | read is a read-only mapping, set is a writable one |
| 56 (§44) | the virtio-rng | **obtain** bytes, without reaching the device |
| 56 (planned) | a password hash | **use** a credential, without reading it |

In each case the narrow power is a different object rather than a flag on the wide one, so the
narrowing survives delegation. A client that passes its endpoint on passes the small power, because
the small power is all it ever held.

## The bytes are the device's bytes

No pool, no whitening, no mixing, no DRBG. There is no cryptographic primitive in this tree yet,
and without a one-way function every transformation available is a reversible permutation. It would
change the bytes without adding an unpredictability an attacker could not undo, while making the
security claim harder to state. So the claim stays one sentence long, and it is auditable:

> These are the bytes the device produced.

What the service does keep is a 256-byte buffer, and the distinction from a pool matters. Byte
*i* out is byte *i* in, unmodified, handed to exactly one client, and zeroed behind the cursor. It
is a cache for round trips, not an entropy transformation, and it turns thirty-two device requests
into one.

### Short reads

virtio-rng is allowed to return fewer bytes than the buffer holds, and the used ring's `len` is
where it says so. QEMU's really does, which is worth recording as a measurement rather than a spec
allowance the code defends against on principle. The first version of the service passed a
short buffer straight through to the client, and the milestone-56 test caught a five-byte reply to
an eight-byte request thirty draws in.

So the service gathers across the buffer boundary, and a count below what was asked means one thing
only: the device went dry part-way through. It never pads, never repeats a byte it has already
served, and never substitutes a pseudo-random stand-in. A device that produces nothing across four
attempts gets `NO_ENTROPY` back to the caller, because a caller who cannot be given randomness must
find out.

### Why the bytes ride in the reply and not in a shared page

[§10](../design/decisions/0010-capability-microkernel.md) says bulk rides in a page and control rides in the message, and this contract
deliberately does not. A page shared with a client is a place the bytes persist and a second
party can read, and random bytes are the one payload whose entire value is that nobody else has seen
them. Registers and the client's own stack are a smaller footprint than a page both parties map.

The cost is real and is not hidden: one round trip per eight bytes, so a 32-byte key is four round
trips. `bench/` prices a round trip; nothing about this is free.

### One number that could not collide

The reply's first word is a byte count in `0..=8`. Every failure the kernel can return from a `CALL`
is one of its small negatives (-1..-8), which read as enormous `u64`s. So "there is no entropy
service" and "the service has no entropy" are distinguishable with no probe request and no ambiguity
to reason about. `filesystem_contract` could not manage that (its errno space collides with the kernel's, a
wart notes/std.md records), and a contract this new had no excuse to inherit the collision.

## What `std::random` does

Transparent, and split where std already splits it:

| std entry point | what std promises | granted | not granted |
|---|---|---|---|
| `std::random::SystemRng` / `fill_bytes` | "random data suitable for cryptographic purposes such as key generation" | the device's bytes | **panic**, naming the reason |
| `RandomState` / `hashmap_random_keys` | DoS resistance for a hash table | the device's bytes | the old counter-seeded splitmix64 |

A program that calls `SystemRng` gets real entropy the moment it is granted the capability, with no
nife-specific API to learn. `fill_bytes` has no error channel, so the only loud refusal available
is a panic, which is [§43 (reading the clock is a page)](../design/decisions/0043-clock-authority.md)'s `SystemTime::now()`
decision applied a second time. A program that never asks is unaffected, and a program that asks
gets told rather than quietly
stamping a key with something guessable.

`hashmap_random_keys` is the one place a fallback is right. A `HashMap` in a program nobody granted
entropy must still work, and std's own `unsupported` backend degrades that same function (to
allocation addresses) rather than failing, so a platform is permitted to. The splitmix64 stream
survives there, clearly labeled, and nothing in the file lets it reach `fill_bytes`. That
separation is the milestone's point: the caller that promises cryptographic strength refuses when it
cannot keep the promise, and the caller that promises nothing degrades and says so.

The grant is slot 6 of the std slot convention (`sys/pal/nife/rt.rs`), and it is an endpoint
with no mapping alongside it, unlike the clock, whose read authority *is* a page.

## Both transports, and a finding about interrupts

virtio-mmio and PCIe, one binary, chosen by the wiring ([§18](../design/decisions/0018-pcie-transport.md)'s seam). The PCIe
instance sits behind the IOMMU and the test asserts it: the buffer this device writes is where the
machine's key material comes from, so an unconfined device writing it is the last thing you would
leave unchecked.

The driver looks at the used ring before it blocks, which is a change from the disk driver's
shape and a fact about the board rather than an optimization. `pci::intx_interrupt_number` swizzles INTx by
device number modulo four; `sched::bind_interrupt` routes an intid to exactly one endpoint; the test leg
now attaches five PCI functions. There is no unshared line left, so a driver that blocked before
looking would be betting on owning its line, and losing that bet is a hang rather than a wrong
answer. The interrupt wait is still there and is what a genuinely asynchronous device gets. QEMU
completes a virtio request inside `NOTIFY`, so the fast path never blocks in practice.

## What is proven, and where

Four kernel tests, not arch-gated and not transport-gated, so aarch64 and riscv64 run literally the
same assertions over both buses (`system_tests/src/user/entropy_tests.rs`):

- 512 bytes across a refill boundary, all 64 words distinct and none zero, over mmio and over
  PCIe. A stuck device, a buffer served twice, and a driver reading a stale used ring all present
  exactly as a repeat, and a collision among 64 draws from a real source is a 2^-58 event.
- Two devices do not agree. The mmio and PCIe services draw disjoint bytes, which is what says
  they came from the devices rather than from something shared underneath.
- The count in a reply is the truth about the reply: a three-byte request writes three bytes and
  leaves the rest of the caller's buffer alone. An oversized one is clamped and answered rather than
  refused, and an unimplemented opcode is answered with nothing rather than killing the service.
- A std program gets there: `std_exerciser` draws two 32-byte values through `std::random` and asserts
  they differ (`entropy ok` in the pinned transcript), on both ISAs.

**And since 2026-08-03 the endpoint is load-bearing for something a machine cannot do without it.**
Milestone 57's write half made this service the thing that decides whether a disk can be partitioned
or formatted at all. A GPT partition and a RedoxFS volume each carry an identifier that must be
globally unique, and neither `crates/globally_unique_identifier_partition_table` nor a `no_std`
RedoxFS has any randomness of its own. So
`disk_partitioner` and `mkfs` hold this endpoint beside their disk. Withholding it is what the test
does to prove the pair is necessary: the same binary, the same disk, one capability fewer,
and a disk that afterwards still reads as unpartitioned. That is a stronger statement of "an
endpoint is the authority to obtain randomness" than a client that merely draws bytes and compares
them, because here the refusal is visible on the platter.

It is also the first client whose *correct* behavior on `NO_ENTROPY` is to do nothing at all.
`std::random` has no way to fail, which notes/std.md records as a wart; these two do, and they take
it.

## Honest limits

- **Under QEMU the device is backed by the host's `/dev/urandom`.** That is what makes these bytes
  real, and it is a fact about the emulator rather than a property of the driver. On hardware the
  quality of the bytes is the quality of the board's generator, full stop, because nothing here
  conditions them.
- **No hardware TRNG yet, but "needs verifying" is now precise rather than open-ended** (milestone
  159, 2026-08-24). The StarFive JH7110's TRNG is documented, from the chip's own datasheet and
  Linux's mainline driver, but nothing about it has run. `crates/jh7110_entropy` holds the register
  layout (`CTRL`/`STAT`/`MODE`/`SMODE`/`IE`/`ISTAT`/`RAND0..RAND7`/`AUTO_RQSTS`/`AUTO_AGE`,
  transcribed from `drivers/char/hw_random/jh7110-trng.c`, mainline as of 2026-08-24) and a DTB
  discovery query (`starfive,jh7110-trng`, `reg = <0x1600C000 0x4000>`, PLIC interrupt 30, from the
  device-tree binding's own worked example). Both are host-tested against fixtures, never against
  silicon. `components/src/jh7110_entropy.rs` is a full `entropy_contract` backend built on that logic, over a
  raw device mapping rather than a virtqueue (this device has no DMA and no queue, only registers).
  **Wired on 2026-09-01**: `entropy_service`'s `Bus` enum grew a `Jh7110` variant. The riscv64 boot
  tour spawns the driver when the machine's device tree describes the device. On every machine this
  repository boots it does not, so what CI exercises is the skip. Its authority is the
  smallest of any backend here: two rendezvous capabilities and one page of device memory, no DMA
  page, no `Interrupt` capability, no `Virtio` capability. The buffer that hands bytes to clients moved
  into the crate the same day (`jh7110_entropy::Pool`), so the one part of the driver that could serve
  a byte twice is host-tested rather than merely written. What is left in the program is the
  volatile accesses and two unmeasured polling bounds. What the datasheet still does not settle:
  whether radon's own shipped device tree carries the TRNG node the mainline one does (nobody has
  captured one from the board to check). Nor whether the block's clocks and reset are left running
  by
  U-Boot (this tree drives neither, and Linux's driver takes two clocks and a reset line before it
  touches a register; see `components/src/jh7110_entropy.rs`'s `BUGS`). And the whole question
  below.
- **The health-test story got sharper, not answered.** The datasheet (§2.8.2) documents "Support
  LFSR based digital post process" and "Support self re-seeding" but claims no NIST SP 800-90B,
  FIPS 140, or AIS-31 compliance anywhere reachable. The Linux driver names exactly one hardware
  fault signal, `ISTAT.LFSR_LOCKUP` (an SEU in the post-processing stage), which is cheap to read,
  and the new driver reads it. It treats a lockup the way this service already treats a dry
  virtio-rng device: retry, bounded, then tell the caller the truth. It is not decided whether that
  hardware bit is *enough* before trusting these bytes for anything security-shaped. The alternative
  is a software statistical test (repetition-count, adaptive-proportion) over and above it. See
  [entropy/jh7110-trng.md](entropy/jh7110-trng.md), "Health testing", and
  `design/roadmap/0159-jh7110-trng-driver.md` for the argument, which a lane deliberately did not
  resolve on its own initiative.
- **A second backend exists on aarch64 and x86_64, milestone 162.** `entropy` can now be spawned in
  an instruction mode that needs no virtio device at all. It uses `RNDRRS` (aarch64, `FEAT_RNG`) or
  `RDSEED` (x86_64), not the DRBG-buffered `RNDR`/`RDRAND`, since `entropy`'s whole discipline ("no
  pool, no whitening, no mixing, no DRBG") rules out a buffered instruction the same way it rules out
  software conditioning. `RNDRRS`'s register encoding is `S3_3_C2_C4_1`; success/failure rides
  `PSTATE.NZCV` after the `MRS`, the same idiom Linux's `arch/arm64/include/asm/archrandom.h` uses.
  `RDSEED`'s carry flag and Intel's DRNG Software Implementation Guide (rev. 2.2, §5.3.1.2) give the
  retry bound (100 attempts, a `pause` between). Both instructions' own specifications describe
  SP800-90B-shaped on-die conditioning as part of their architectural contract, so bytes pass
  through unmodified, same as virtio-rng. The health-test question above does not apply to either.
  Proven
  end to end under QEMU on aarch64, but only with `--cpu neoverse-n2`. The suite's default CPU
  (`cortex-a72`) predates `FEAT_RNG`. QEMU's `max` model, despite carrying `FEAT_RNG`, cannot even
  boot this kernel (a missing 4 KiB stage-1 granule, checked and confirmed 2026-08-24 against QEMU
  11.0.2, unrelated to entropy). On x86_64 the instruction itself is proven (a kernel-side boot
  probe; see `design/roadmap/0162-cpu-instruction-entropy.md`). But the service cannot be spawned
  into
  userspace at all yet: there is no ring 3 on this port (milestone 161 item 3).
- **No health test.** A device that started returning a constant would be passed straight through.
  The kernel tests would catch it, a running system would not. NIST SP 800-90B's repetition-count and
  adaptive-proportion tests are the cheap standard answer and are not implemented.
- **No rate limit and no quota.** A client holding the endpoint can drain the service as fast as it
  can `CALL`. Eight bytes per round trip is a cost, not a defense.
- **A program at the prompt can hold entropy, since milestone 111** (2026-09-05). This entry used to
  read "`init` does not endow the shell with entropy", and its own last sentence ("future work with
  no design problem in it") turned out to be right: no new mechanism, no new right, nothing on the
  wire. `grant_plan::Manifest::entropy` joined `clock`, `domain` and `config`. The progenitor reads
  the declaration and places a `WRITE` view of the entropy service's request endpoint at
  `grant_plan::ENTROPY_SLOT`. A program that did not declare it holds an empty slot there and
  `entropy_contract::delivered` answers `None` rather than a short count. `components/src/uuid.rs` is the
  first consumer a person can type, and `caps uuid` prints the row.

  **Ambient entropy would still be ambient authority**, and nothing here made randomness ambient:
  the endowment is per program, declared, and previewed before anything is spawned. The shell
  itself holds none, deliberately, because nothing it does as a builtin is unpredictable;
  `config` and `domain` make the same call, and `clock` is the exception only because `time`
  measures with it.

  What it cost: one permanent capability slot in the progenitor, which used to release the endpoint once
  `credentialer` held its own copy. `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` moved 21 to 22 of
  24, caught by milestone 231's own gate on the first boot after the wiring landed.
- **No cryptography anywhere.** No hash, no cipher, no key derivation. Milestone 56's other half
  (vendoring RustCrypto) is a separate decision, and this lane deliberately did not pre-empt it.
