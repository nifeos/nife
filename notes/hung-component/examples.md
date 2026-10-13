# The hung component: the code examples

An appendix to the EXAMPLES section of [`notes/hung-component.md`](../hung-component.md), under
§212 (a prose budget). The parent keeps the shell examples.

Watch the domain of a running supervisor from a program that cannot touch it. The pattern a
`liveness_watch` would use, and it is `ps`'s loop with the acting half absent:

```rust
let mut cursor = abi::survey::DONE;          // 0, which is also "start here"
loop {
    let (next, tid, state) = user_mode_runtime::survey(domain, cursor);
    if next < 0 { break }                    // refused: NOT an empty domain. Say so.
    let next = next as u64;
    if next == abi::survey::DONE || next <= cursor { break }
    // `state` is READY | RUNNING | BLOCKED | DEAD, and BLOCKED is where a hang hides.
    cursor = next;
}
```

Endow it so it cannot act. One right, and the emptiness of the rest is the claim:

```rust
sched::tcb_insert_cap(tid, cap::endpoint_cap(deaths, Rights::ENUMERATE), None)
```

`crates/system_initializer` already does exactly this for `ps`, with the reasoning beside it:
"`ENUMERATE` alone, deliberately. Granting `READ` here would hand a viewer [the authority] to
collect a child rather than to name one."

Restore a service under a hung component, which is the four steps minus one:

```rust
// 1 BUILT: the replacement, endowed without the device, unconfigured.
// 2 DRAINED: skipped. A component that is not receiving needs no quiesce.
// 3 REVOKED: take the device back from a holder that consents to nothing.
invoke(DEVICE, abi::frame::REVOKE, 0, 0, 0);
// 4 STARTED: map, configure, start. It drains what queued behind the silence.
```
