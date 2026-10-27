---
status: NOT-STARTED
raised: 2026-09-21
promoted_from: one-rate-mechanism-or-three
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 567. One mechanism for the counter rate, or three

The number is **provisional**: the integrator mints it at merge. Promoted from the proposal `one-rate-mechanism-or-three` on 2026-09-22, filed 2026-09-21. Raised by the `cntfrq` lane, which built the riscv64 half and was
told to price this rather than decide it.

The page's address is a fixed virtual address every process on an architecture
would inherit, which puts it in *anything two programs agree on*.

## The state after the riscv64 work

One fact, how many ticks make a second, now reaches userspace three different ways:

| architecture | mechanism | can it report "unknown"? |
|---|---|---|
| aarch64 | `CNTFRQ_EL0`, an architected register EL0 reads directly | no, by construction |
| riscv64 | a page the kernel fills from the device tree | yes |
| x86_64 | a page the kernel fills from `CPUID` leaf `0x15` or PIT calibration | yes |

## What collapsing them onto the page would buy, and cost

Buys: one refusal path. Today aarch64 literally cannot say "unknown", so its `cntfrq_checked` is
`Some` by construction. That is honest rather than wrong, but it means the three architectures do not
share a failure mode. A caller written against one of them can be surprised by another.

Costs, measured rather than asserted. aarch64's read is a single `mrs` with `nomem`. The page is
a load that can miss, and `monotonic_ns` calls it per measurement. Collapsing costs a frame per
machine plus a mapping per process, spending intermediate table pages from each process's own budget.
It would put aarch64 on the same footing the other two have, where a path that forgets to map it
faults. The riscv64 rollout touched seven kernel call sites and the first run hung because two were
missed; aarch64 would find the same class.

## The lane's recommendation, which is a recommendation because this is reversible until it ships

Keep the register. A fact the machine architecturally states is a better source than a fact the
kernel copies, and §19 (architectural parity) asks for the capability on every architecture rather
than for the same implementation on every architecture.

## BUGS

- The asymmetry is a real foot gun and this proposal does not remove it. If the answer is "keep
  the register", the thing that should change is documentation: `cntfrq_checked`'s aarch64 arm should
  say out loud that it cannot return `None`, where a reader meets it.

## Index row

Three mechanisms now answer one question. Collapsing them onto the page buys a uniform refusal and
costs aarch64 its architected register read, and the address would be an ABI fact for every process
on that architecture forever.
