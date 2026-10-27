# The capability_plan crate, item by item

An appendix to [notes/component-manifest.md](../component-manifest.md), which keeps the design: what a
manifest is, why it differs from `grant_plan::Manifest`, and its limits. This file holds the reasons
behind each item of `crates/capability_plan`, which its doc comments link here for.

## Requests and authority

A manifest is a request and the provisions are the authority. The name is the component's and the
object is the supervisor's, which is what makes a component substitutable: one declaration, many
wirings. `Provisions` is per child rather than per supervisor, Fuchsia's routing rather than an
ambient environment. One supervisor may hand the same role name to two children and resolve it to
two different objects. A role it does not list resolves to nothing, so a manifest can never widen its
own authority. `Refusal::Unprovided` is the security-relevant refusal for that reason: it stops a
manifest from being a request the supervisor is obliged to honor. A supervisor may list roles nothing
needs, since refusing the surplus would make one table per child mandatory for no gain.

A `CapNeed`'s role is the component's word for what it needs the object for, never the supervisor's
word for the object. `swap_boundary_witness` asks to use `service`, and whether that is the shared
rendezvous or a queue broker's front rendezvous is the supervisor's business.

## Direction

`Direction` is the axis this crate adds over a program's manifest. It is a required field with no
default, in `writes_while_reading`'s shape (rung one of the ladder in `AGENTS.md`). A component cannot
declare a rendezvous without saying whether it answers on it, and the rights fall out of the answer,
so the supervisor never spells `READ` or `WRITE` and cannot spell one wrong.

The stakes are §41 (the endpoint is the broker)'s central refusal. `SEND` and `RECEIVE` are gated by
different rights on one object, so a rendezvous handed out two ways is a one-way pipe in whichever
direction each holder was trusted with. A client given `READ` by a typo could park in `RECEIVE_CAP`
and take its own server's requests. `a_client_of_the_stable_rendezvous_cannot_become_its_server`
would then fail for a reason no reader could see from the operator's source.

## Pages

A `MapNeed`'s address is in the declaration because it is a fact about the component's own code. The
component compiles it into a `read_volatile`, and a supervisor that mapped the page elsewhere would
produce a component that faults a long way from the mistake.

`PageKind::DeviceRegisters` is its own kind for §41's reason. Revocation is by physical page, so a
device the supervisor is about to take back cannot be installed at build time. A plan sorts device
pages last and `Plan::devices` hands them over separately, so the operator endows everything else
early and maps the registers after the revoke. That keeps the down window four syscalls wide. The
kernel maps device registers device-typed and read/write whatever mode is asked for. So the mode a
device need carries is the conservative one rather than a claim. `plan` makes two passes over the
mapping needs, so devices land at the end of one array. `maps_without_devices` is then a slice of
it, not a second copy a `no_std` caller has nowhere to put.

## State handoff

`STATE_HANDOFF_ROLE` is one fixed word rather than a role the component picks. A handoff page is the
one page a supervisor must route to the same frame in the outgoing and the incoming instance. A
supervisor wiring two builds of one contract has to find it without reading either build's code.

A `StateHandoff`'s bytes belong to the component. The supervisor never reads them and this crate never
names their shape. That keeps the refusal of §116 (live component state handoff is declined, for want
of a customer) to design one wire format for every kind of live state. The declaration gives the
supervisor two facts it could not otherwise know. One is that the contract carries state, so a swap
waits for the incoming instance to say it absorbed it before the outgoing one is retired. The other
is where both instances read and write it. The run of `pages` is minted as one frame capability with
`MemoryRegion::RETYPE`'s page count (§233 (`MemoryRegion::RETYPE` takes a page count)). The count is
the component's, since it is a fact about how much state it keeps.

`Requirements::state_handoff` is required with no default, for `Direction`'s reason: a contract
cannot be declared without saying whether it hands state off. `None` is the common answer (roughly
fifty of fifty-two components): kill and replace. With `Some`, `plan` routes the supervisor's
`handoff` object to the page and refuses with `Unprovided` when nothing is routed there. A supervisor
that cannot carry state must not silently kill and replace a component that declared some, because
the loss would look exactly like a successful swap. Under §209 (state handoff is an opaque blob over a
granted frame, and it is optional) a swap whose state was not absorbed does not commit. So
`Plan::state_handoff` tells the supervisor to wait for the incoming instance's verdict.

The handoff mapping is in `Plan::maps`, among the shared pages, installed at build time in both
instances. It is not deferred past a revoke and cannot be: `PageFrame::REVOKE` is symmetric and would
take the supervisor's own capability with everyone else's. The outgoing instance is quiesced and
blocked before the incoming one starts, so the two never run against the page at once.

## Slots and well-formedness

The order of `Requirements::caps` is the component's capability table slot order, because
`supervision_protocol::ChildEndowment::caps` lands its entries in slots 0, 1, 2 and on. The component
derives its own slot numbers with `slot_of`, a `const fn`, so a reordered manifest or an undeclared
role is a compile error rather than a component reading the wrong slot at run time. That is the rung
the crate is built to reach. `contract` is not matched against anything: what makes a build
substitutable is that it speaks the protocol, and a string cannot prove that.

`Requirements::problem` is a `const fn` that every real manifest asserts in a `const` item. Each of
its checks is a mistake that would otherwise be silent:

- A role declared twice. The component is handed two slots and reads one, and nothing at run time
  tells that from a correct wiring.
- Two pages at one address. One mapping silently wins, depending on installation order.
- Zero pages. A component nothing pays for cannot be built, and the failure would arrive as a region
  split refusing for no visible reason.

The handoff run is held to the same rules as a mapping, and a page anywhere inside the run collides.
An over-long declaration is not checked there, because `MAX_CAPS` is this crate's limit rather than
the declaration's defect, so it is `plan`'s refusal. `MAX_CAPS` is eight, about double the widest
manifest's four, and it leaves room below `abi::fault::FAULT_EP_SLOT` for placed slots.

`plan` refuses before the component exists, and the guest test asserts that by reporting the refusal
ahead of the first build step.

## Dependents

`dependents` is milestone 23 (a capability-routed component OS with live replacement)'s third
residual, in the roadmap's words: "if component B is a client of component A, swapping A means
quiesce B, swap, resume". Given the components a supervisor runs, it says which must be told before a
named contract is swapped. It does not run the swap, touch a capability, or decide how a dependent is
told, which is a per-contract protocol (`broker`'s warning page and signal).

`depends_on` names contracts, not roles. A role resolves to an object through the supervisor's
provisions, and that varies by wiring. `CLIENT`'s `service` is a console on the direct channel and a
broker's front rendezvous on the queued one; a contract name does not change. The field is populated
only for a component that itself serves others. A pure consumer calls through a `CALL`, and §41
showed that degrades for free. A `CALL` that finds nobody receiving parks on the rendezvous's sender
queue, and the next server to `RECEIVE_CAP` drains it. A component that serves while calling a
swappable dependency is different. `broker` blocks its one serving thread on a `CALL` to its backend.
So it must be told first to stop calling through and start buffering. The warning is a page and a
signal it is never asked to acknowledge (§231 (a swap's warning to a dependent is advisory, and the
supervisor never waits for it)).

Only direct dependents are returned. A dependent with its own decoupling (a queue, a cache) is not
assumed to pass the warning to its own dependents. Whether it must is a property of that mechanism,
which two contract names cannot show. `Dependents` makes no ordering claim; it is a filter, held as a
value so a supervisor does not re-derive it. `MAX_LIVE` is eight, matched to `MAX_CAPS`. A supervisor
running more live components than it could endow one of with capabilities is not a shape this tree
has.

## The proofs

The host tests pin the cases a reader cares about. The Kani harnesses in `src/proofs.rs` rule out
shapes a case-by-case test cannot, and what they leave symbolic is the routing table. `Requirements`
holds `&'static` slices, so a declaration is a compile-time constant by construction. That is what
makes `problem` and `slot_of` evaluable in a `const` item. It also means no manifest can be built at
run time out of bytes, which a shipped wire format would have to give up. So the declarations are a
bounded enumeration of `const` values, walked by a concrete loop: both directions, and all four
arrangements of the two page kinds. The slot numbers and routed names are symbolic.

The concrete loop is needed. Selecting a declaration with `SHAPES[kani::any()]` makes the slice CBMC
reads a symbolic pointer, and every slice access downstream becomes symbolic too. That formulation put
the solver past seventy thousand verification conditions for a fact about copying integers, and it did
not finish in ninety seconds. Walking the shapes concretely proves the same statement, for every
declaration of this width, and the difference is invisible from the assertion. The harness paths are
`proofs::<harness>`, which `script/falsifications` reads from the file's name.

`a_plan_never_grants_a_right_the_declaration_did_not_ask_for` is the property the crate exists for.
Nothing a supervisor routes turns a `Use` into a `READ`, and nothing carries `GRANT`: a component
that could pass its own authority on is not confined, and no declaration can ask for one.

## BUGS

The crate's `BUGS` section keeps one line per entry; notes/component-manifest.md has the long
account of most of them. Two have theirs here.

- `plan` does not refuse two roles routed to one supervisor slot. The child then holds one object
  twice, each copy with the rights its own role declared, so nothing is widened past the
  declaration. It is a provisioning mistake that should be a `Refusal`. The eighth outsider pass
  found it (notes/confinement-outsider-pass-8.md, near miss 3).
- `dependents` returns direct dependents only. A transitive version would need each decoupling
  mechanism to say whether it absorbs its dependency's downtime, which a contract name cannot
  carry, so it is not built.
