# Finding 9 was five copies, not four, and the fix is one function

*Name: provisional. After the function finding 9 became.*

An appendix to [`notes/architecture-list-sweep.md`](../architecture-list-sweep.md), which states the
result.

The sweep named `crates/virtio`, `gpu_driver.rs`, `keyboard_driver.rs` and `net_transport.rs`. The
fifth is `components/src/entropy.rs`, found by the fatal-risk-9 lane on 2026-09-23 and confirmed
by re-running the sweep's own method 3. It had the same two arms, no third, and no `BUGS` note, so
it was the one copy nobody had even recorded. A sixth `barrier()` exists in
`components/src/non_volatile_memory_express.rs` and was never in this class: it has all three arms.

They are now one function, `user_mode_runtime::virtio::virtio_ring_barrier` (name provisional),
in the module where those four programs already get `virtio::read_register` and its siblings
(`virtio_read_reg` and so on until 2026-10-12). That is a higher rung than five `compile_error!`
arms would have been, which is what option B priced. The fallback now exists once, and a seventh driver gets the third architecture by calling the
function rather than by remembering to write an arm.

What x86_64 needed, which the sweep left open as a rule 4 call. A compiler fence, and the
reasoning is per-site rather than a default. Every call site in those five programs asks for one of
two orderings: store-store (publish the descriptor before the available index; publish the index
before the notify) or load-load (read the used index before the payload it gates). x86_64 gives both
for the write-back memory a coherent DMA agent shares. The store-load case TSO does not give is
unreachable here, because every notify and every register access in these drivers leaves the process
through a syscall instruction, which is serializing. So the compiler is the entire exposure.

`non_volatile_memory_express.rs` is the control that makes this argument falsifiable rather than
convenient. It keeps `dmb sy` and a real `mfence`, because its doorbell is a direct store to a
device-typed page in this address space with no syscall between the ring stores and it. Same
hardware, different answer, for a stated reason.
