# The sink protocol: benchmarks

*Name: provisional. After the main page's own heading.*

An appendix to [`notes/sink-protocol.md`](../sink-protocol.md), which states the result.

`println!` is on the ABI fastpath and `relay_rtt` / `call_reply` price exactly the kind of hop this
lane could have added. It added none, and the numbers say so rather than the argument doing it.
Deterministic icount ticks against `bench/baseline-aarch64.txt`, on the pinned QEMU:

| | aarch64 | riscv64 |
|---|---|---|
| `ipc_rtt_el0` (an EL0 send/receive round trip, the closest thing to a `println!`) | +0.13% | -0.63% |
| `relay_rtt` (a hop through a userspace relay) | +0.01% | +0.02% |
| `call_reply` | 0.00% | +0.00% |
| `null_syscall` | +0.00% | -0.04% |

Everything moved by less than 1% and in both directions, which is build noise on an instruction
count. The baseline file was deliberately not updated, because updating it is a statement that a
performance change was intended, and there was none.

Two honest caveats. There is no benchmark that prices `println!` itself; what these price is the
`SEND` that `println!` is, so a regression inside the PAL's chunking loop would not appear here. And
the reason the fastpath is untouched is structural rather than lucky. `OPERATION_BYTES == 0` keeps
the wire identical, `SEND` keeps the message count identical, and the kernel's only change is on the
*failure* return of an aborted send, which no benchmark takes.