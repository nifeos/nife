# What the first two lanes built, 2026-08-24 and 2026-09-01

An appendix to [milestone 159 (a real hardware entropy source: the JH7110's
TRNG)](../0159-jh7110-trng-driver.md). The main page is complete without it.

## Why the status did not move on 2026-08-24 or 2026-09-01

Both lanes left the status where it was, and this section was their report of why, per the
roadmap's rule that a branch touching nothing else in a block must at least say so.

The first lane (2026-08-24) confirmed the TRNG from documentation. It wrote and host-tested the
register and DTB-discovery logic (`crates/jh7110_entropy`), and it wrote an unwired driver program
(`components/src/jh7110_entropy.rs`). The second lane (2026-09-01) wired that program to a spawner
and gave it a boot-tour step that says something falsifiable. It also moved the one remaining piece
of untested logic into the host-tested crate.

Neither cleared this tree's bar. Milestone 53 (the board's own peripherals: network and storage on
real silicon) uses `PARTIAL` for a phase that runs end to end, proven in QEMU. The phase here was
"read real bits off a real TRNG", and QEMU has no JH7110 TRNG to run it against, so what ran in QEMU
was the *absence* path. `NOT-STARTED` ("specified, nothing built") was not right either, and less
right than before. It was still the closer of the two available tokens, and the honest gap was
recorded in prose rather than folded into a status word the vocabulary does not have.

## What the second lane built (2026-09-01), and what it proved

The device is on radon, the `StarFive` VisionFive 2 (JH7110, riscv64). radon booted nife with
userspace on 2026-09-01, reaching `init/build`, `device IRQ` and four cores online. So a userspace
process holding a capability runs there; what had never happened is one talking to a real
non-virtio device.

- A spawner. `kernel/src/user/entropy_service.rs` grew `Bus::Jh7110`, beside `Mmio`, `Pci` and
  `Instruction`. The fork the first lane held open turned out not to be one. It had declined to add
  the variant because "adding one needs a real decision about how the service that wires it locates
  this binary in an initrd". But `entropy_service::ensure` already takes the program's bytes from
  its caller, exactly as it does for `entropy`. The caller reads `user::program("jh7110_trng")`.
  Nothing about the interactive boot changed, so the stopgap question of §120 (a QEMU-only
  virtio-rng stopgap for the interactive boot) is untouched.
- The authority, which is the risk-6 claim. The driver is granted two rendezvous capabilities (a
  request endpoint it RECVs on, a readiness endpoint it SENDs once) and one page of device memory:
  the TRNG's register block, device-typed. There is no DMA page (the device writes nothing to
  memory), no `Irq` capability (it polls) and no `Virtio` capability (there is no transport). The
  binding's `reg` window is `0x4000` and the spawner maps `0x1000` of it, because
  `jh7110_trng::regs` reaches only `0x68`. An earlier draft of the program named a third slot for a
  `DeviceFrame` capability. The spawner that exists grants the page as a `Mapping`, so the slot
  described something nobody hands over, and it is gone.
- A falsifiable boot-tour line. The riscv64 tour's new `hw entropy` step plays a client over the
  same request endpoint any client would hold, and draws 32 bytes twice. It prints the success line
  only when the bring-up report says `READY`, both draws come back full, the first is not all
  zeros, and the two differ. Anything else prints `FAILED` with the numbers. The kernel never reads
  a `RAND` register.
- The skip, which is what CI actually exercises. QEMU's riscv64 `virt` board has no
  `starfive,jh7110-trng` node, so on every machine this repository boots the step prints
  `hw entropy  : skipped`. `kernel::user::entropy_tests`'
  `the_jh7110_backend_refuses_to_wire_where_there_is_no_jh7110` pins that the wiring *refuses*. It
  does not spawn a driver holding a device mapping of an address nobody named.
- The buffer, moved somewhere testable. `jh7110_trng::Pool` is the one part of the driver that can
  serve a byte twice, lose the seam between two generations, or pad a short answer with zeros a
  client would mistake for entropy. None of that is visible in the register decode. It is now in
  the crate with five host tests:
  - no byte served twice across ten generations;
  - a request straddling the seam;
  - a dry device shortening the count rather than padding;
  - a served byte zeroed behind the cursor;
  - an oversized request clamped.

  Writing the first of them found `take` shifting left by `8 * n` for unclamped `n`. That is
  unreachable through the wire format, because `entropy_proto::want` clamps to 8 first, but it was
  a real overflow at the API. `take` clamps, and a `const` assert in the program ties the two 8s
  together.

## What none of that established

Nothing at that point had read a bit off a TRNG. Every register offset was still transcribed from
Linux's driver rather than observed. The polling bounds were still guesses with no board
measurement behind them. And the rate question this milestone opens ("a rate and quality argument,
measured rather than assumed") was untouched, because a rate cannot be measured against a device
that has not run.
