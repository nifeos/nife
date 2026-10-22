# What the alternatives to a framebuffer console cost

An appendix to [`notes/serial-less-output.md`](../serial-less-output.md). Milestone 243 (a machine
with no serial port)'s block listed candidates and endorsed none. This file prices each against the
two halves, the human watching and the gate reading.

The block listed candidates and endorsed none. Each was priced against both halves.

| Candidate | The human | The gate | Verdict |
|---|---|---|---|
| **Firmware framebuffer** (chosen) | yes, from the first tour line | under QEMU only | the only one that answers the human half at all, and the only one whose address survives `ExitBootServices` with no driver of ours |
| **Keep `ConOut` alive longer** | until `ExitBootServices` and no further | no | not a candidate, it is a fact: boot services *are* the console, and a kernel that never exits them never gets its memory map. The loader already prints there and this milestone kept that |
| **Postmortem to the boot medium** | no (it is postmortem) | **yes, and on real hardware** | the gate half's real answer, and it needs a **USB mass-storage driver**, which nife does not have. Milestone 242 is USB host. Deferred as a proposal, not refused |
| **A network console** | no | yes, and unattended | needs a NIC driver per board and says nothing until the stack is up, which excludes every failure before it. The block's own objection stands: a machine that can only report over a network cannot report a network failure |
| **A photograph of the screen** | it *is* the human | no | not pixel-aligned and not a screendump; `serial_console::screen` cannot read one, and saying so is cheaper than discovering it |

The honest summary is that the two halves want different mechanisms, which the block anticipated
and this lane confirms. The framebuffer is the human's answer and is the right first increment
because it is the one a person needs standing in front of a machine that will not boot. The gate's
answer on real hardware is postmortem to storage, and it is blocked on a driver.
