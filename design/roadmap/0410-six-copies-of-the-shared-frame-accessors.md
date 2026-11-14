---
status: BUILT
raised: 2026-09-14
built: 2026-10-16
promoted_from: six-copies-of-the-shared-frame-accessors
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 410. Six copies of the shared-frame accessors

Promoted from the proposal `six-copies-of-the-shared-frame-accessors`,
filed 2026-09-14 by milestone 290, which added the sixth copy and said so rather than hiding it.
*(Number provisional until the merge queue lands it.)*

A lane can close this. It is a refactor inside userspace with no wire format, no
syscall surface and no name calef has not already ruled on. If a new crate is wanted, the name is his.

Premise re-checked 2026-09-19: five copies, not six, and this file already records why.
`multicast_dns_responder` went at milestone 298 and the table below struck it out at the time. The
five that remain are `components/src/socket_test_client.rs`, `components/src/network_time_client.rs`
and `fixtures/src/network_time_test_server.rs`, which carry the absolute-VA accessors the title is
about. The remaining two, `components/src/entropy.rs` and `components/src/virtio_net_transport.rs`,
take an offset rather than a VA and are the virtio pair this file says to price separately. The title
keeps the count it was filed under.

Closed 2026-10-16 (UTC). The three socket-contract programs construct
`user_mode_runtime::socket_frame::Frame`, and their absolute-VA accessors are gone. The type had
this block's first home, `crates/socket_contract` beside the offsets it reads, and moved. It is
built on `user_mode_runtime::mapped_window`, which cannot compile for the host, and a dependency
from `socket_protocol` to `user_mode_runtime` lands `socket_protocol` in the host pass's derived
exclusion set. That costs the crate its own host tests. The gate's alternative, an optional
feature, does not help: the resolve graph unifies features across the workspace, so the edge
exists once any member turns it on. `user_mode_runtime` is already excluded everywhere, takes
`socket_protocol` for the offsets, and its opt-in modules are the §94 (what may live in a
library) shape. The virtio pair was priced and priced out. Its accessors are one-line forwards
over `MappedWindow` with no arithmetic to share, so lifting them would be the abstraction this
file refuses. The fact the pair does share, the kernel's ring layout spelled as literals, is
recorded as a `BUGS` entry in each driver, where the fix it wants is named. It is a `components` ->
`direct_memory_access_validator` dependency edge, §46 (thin primitives or whole subsystems).

Name: provisional. `Frame`, in the module `socket_frame`, which takes its name from the contract
it reads so `socket_protocol::` stays the one place a reader looks for the wire. Considered
`SharedFrame`, which doubles a word the contract's docs already put on "frame". Considered
`PageFrame`, which names the kernel object a frame is minted from, not the layout read through
it. Considered a second `base` field beside the window (the wording above), which would
duplicate the base `MappedWindow` already carries.

## Follow-on

- **Recorded.** The virtio pair's shared fact, the kernel's ring layout spelled as literals in
  `components/src/entropy.rs` and `components/src/virtio_net_transport.rs` (and in `crates/virtio`'s
  own net driver), is recorded as a `BUGS` entry on `components/src/virtio_net_transport.rs`,
  beside its own copy of the layout.

## What is duplicated

Six programs each carry their own copy of the same six functions over
`user_rt::mapped_window::MappedWindow`:

```rust
fn r8(va: u64) -> u8        { WINDOW.r8(va - PAGE_FRAME_VA) }
fn w8(va: u64, v: u8)       { WINDOW.w8(va - PAGE_FRAME_VA, v); }
fn r16le(va: u64) -> u16    { WINDOW.r16(va - PAGE_FRAME_VA) }
fn w16le(va: u64, v: u16)   { WINDOW.w16(va - PAGE_FRAME_VA, v); }
fn write_payload(bytes: &[u8]) { ... PAYLOAD_OFFSET_BYTES ... }
fn read_payload(n: usize, out: &mut [u8]) -> usize { ... PAYLOAD_OFFSET_BYTES ... }
```

| program | where | what the page is |
|---|---|---|
| `components/src/entropy.rs` | `components/` | a virtio DMA region |
| `components/src/virtio_net_transport.rs` | `components/` | a virtio DMA region |
| ~~`components/src/multicast_dns_responder.rs`~~ | retired 2026-09-15 (milestone 298) | the socket contract's frame |
| `components/src/socket_test_client.rs` | `components/` | the socket contract's frame |
| `components/src/network_time_client.rs` | `components/` | the socket contract's frame |
| `fixtures/src/network_time_test_server.rs` | `fixtures/` | the socket contract's frame |

**This is code, not a fact two binaries agree on**, which is why AGENTS.md rule 7 does not already
forbid it and why `script/lint` check 5 does not fire. The layout the accessors read is
`crates/socket_contract`'s, and it is a crate already. What is copied is the arithmetic that turns an
absolute virtual address back into the offset `MappedWindow` bounds-checks.

## Why it is worth closing anyway

Milestone 139 did the hard half. Before it, each of these programs hand-wrote one `// SAFETY:` comment
per accessor asserting the same invariant at every call site. `MappedWindow` holds that invariant once
and every call site is ordinary safe code. What is left is the boilerplate, and
`notes/unsafe-obligations.md` already records the tell: `ntp.rs`'s own comment had named the
duplication (*"the same shape net_stack and socket_test_client use"*) without anyone lifting it out.

The prize is small and honest: about forty lines per program, and one place rather than six to change
if the window abstraction moves again.

## The shape, and the one question a lane has to answer

Four of the six speak the socket contract, and for those the natural home is `crates/socket_contract`
beside the offsets they already use: a small type holding the window and the base address. Its
methods are `payload_write`, `payload_read`, `dst_ip`, `dst_port` and `len`, rather than free functions
over an absolute VA.

Two are virtio DMA users and are a different shape: their offsets are device descriptor rings, not
`socket_protocol`'s header, and `crates/virtio` is where that belongs if anywhere. **Price them
separately and do not force one abstraction over both**, which is the speculative trait-ification
AGENTS.md refuses.

**The question that is not mechanical**: each program picks its own `PAGE_FRAME_VA` and the constant
is genuinely per-program (each address space is its own). So the type has to be constructed with the
base rather than reading a shared constant. A lane that "simplifies" by hoisting `PAGE_FRAME_VA`
into the crate would be inventing an agreement that does not exist.

## What would close it

Four programs constructing one type from `socket_protocol`, their local accessor functions gone, and the
existing socket and network time tests green on both ISAs with no test changed. If the virtio
pair is done in the same lane, `crates/virtio`'s own host tests too. Any new type is a name calef has
not ruled on, so it ships provisional and says so.

## Index row

Six programs each carried their own copy of the same accessors over
`user_mode_runtime::mapped_window::MappedWindow`, turning an absolute virtual address back into the
offset the window bounds-checks. The retirement of `multicast_dns_responder` at milestone 298 (retire
the multicast DNS responder) left
five. The three that speak the socket contract belong beside the offsets they already use in
`crates/socket_contract`. The two virtio DMA users read descriptor rings instead, and this block's
own instruction is to price them separately rather than force one abstraction over both. The part
that is not mechanical is that `PAGE_FRAME_VA` is genuinely per-program, since each address space is
its own. So the type is constructed with its base, and a lane that hoisted the constant into the
crate would be inventing an agreement that does not exist.
