# The native ABI: the appendices to notes/abi.md

[`notes/abi.md`](../abi.md) carries the syscall convention, the object surface, the entry contract,
how a program meets its capabilities, the fault endpoint and the one ambient read. These files carry
what its prose budget has no room for.

- [`cycle-counter-grant.md`](cycle-counter-grant.md): why the cycle counter is granted rather than
  ambient (milestone 229 (build the cycle-counter grant DECISIONS 139 decided)), per architecture, and what the build gate does not cover.

*Name: provisional. The directory follows the siting default of §212 (a prose budget), `notes/X/`
beside `notes/X.md`. calef names directories.*

- `cycle-counter-grant` Name: provisional. Named for the grant milestone 229 built.
