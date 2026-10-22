//! **`component_plan`: what a component declares it needs, and how a supervisor wires it.** Under
//! §41 (the endpoint is the broker), any program that speaks the protocol and holds the right
//! capabilities is the component; this is the capability half. No IO and no syscalls: a [`Requirements`] (what the component says it needs) and
//! a [`Provisions`] (what the supervisor routes to this child) become a [`Plan`] (exactly what to
//! endow) or a typed [`Refusal`]. notes/component-manifest.md has the design, including why this is
//! not `grant_plan::Manifest`: that declares what a person at a prompt may designate, this declares
//! what a supervisor must route, and [`Direction`] (`Serve` is `READ`, `Use` is `WRITE`) is what
//! stops a client of a stable rendezvous becoming its server.
//!
//! A manifest is a request; the provisions are the authority. A name the supervisor did not list
//! resolves to [`Refusal::Unprovided`] before the component is built, so a manifest can fail to be
//! satisfiable but cannot widen authority. The name is the component's and the object is the
//! supervisor's, which is what makes a component substitutable: one declaration, many wirings.
//!
//! A malformed declaration (a duplicate role, two mappings at one address, zero pages) is caught by
//! the `const fn` [`Requirements::problem`], which every real manifest asserts in a `const` item,
//! so it does not compile. A provisioning mismatch is only known at run time, and [`plan`] reports
//! it.
//!
//! # BUGS
//!
//! - A manifest is compiled in, not shipped with the build. calef ruled on 2026-09-26 that it
//!   travels in the executable as an ELF note (§197 (a package is one archive file)); the note's
//!   format is still the architect's, and nothing reads one yet.
//! - [`Requirements::pages`] is a property of a build, not of the contract: two builds can honestly
//!   differ, which is the strongest argument for shipping the manifest with the binary.
//! - A malformed declaration's compile error names the manifest, not the role;
//!   [`Requirements::problem`] in a host test prints the role.
//! - [`MAX_CAPS`] and [`MAX_MAPS`] are hard bounds, about double any manifest here.
//! - Nothing checks that a routed object is the kind the role wants; the kernel refuses the
//!   `CAP_INSERT` or the first `RECEIVE` instead.
//! - [`plan`] does not refuse two roles routed to one supervisor slot. The child then holds one
//!   object twice, each copy with the rights its own role declared, so nothing is widened past the
//!   declaration; it is a provisioning mistake that should be a [`Refusal`]. Found by the eighth
//!   outsider pass (`notes/confinement-outsider-pass-8.md`, near miss 3).
//!
//! Name: provisional, and ruled: calef ruled **`capability_plan`** on 2026-09-13. The block stays
//! `provisional` until the rename is performed.

#![no_std]

/// **How many capabilities one component may declare.** A [`Plan`] is a value in a `no_std` program
/// with no allocator, so this is a hard bound rather than a hint: a manifest past it is
/// [`Refusal::TooManyCaps`].
///
/// Eight, which is roughly double the four that the widest manifest in this tree asks for, and which
/// leaves room below `abi::fault::FAULT_EP_SLOT` for the placed slots a component might also want.
pub const MAX_CAPS: usize = 8;

/// **How many pages one component may declare.** [`MAX_CAPS`]'s twin, for the mapping half. Four,
/// against the two that the widest manifest here asks for.
pub const MAX_MAPS: usize = 4;

/// **Which way an rendezvous points**, which is the axis this crate adds over a program's manifest and
/// the reason a component's declaration is a different object from a program's.
///
/// It is a required field with no default, and that is deliberate in `writes_while_reading`'s shape
/// (AGENTS.md's rung one): a component **cannot declare an rendezvous without saying whether it
/// answers on it**. The rights fall out of the answer, so the supervisor never spells `READ` or
/// `WRITE` and cannot spell one wrong.
///
/// The stakes are §41 (the endpoint is the broker)'s central refusal. `SEND` and `RECEIVE` are gated by different rights on the same
/// object, so the same rendezvous handed out two ways is a one-way pipe in whichever direction each
/// holder was trusted with. A typo that gave a client `READ` would let it park in `RECEIVE_CAP` and take
/// its own server's requests, and the test that catches that
/// (`a_client_of_the_stable_rendezvous_cannot_become_its_server`) would fail for a reason no reader
/// could see from the operator's source. There is now nothing to typo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    /// **The component answers here.** Requests arrive on this rendezvous and it replies: `READ`.
    Serve,
    /// **The component asks here, and only asks.** It sends and never receives: `WRITE`.
    Use,
}

impl Direction {
    /// The rights a child is endowed with for an rendezvous it declared this way. The only place in
    /// the tree a component's rendezvous rights are decided.
    pub const fn rights(self) -> u64 {
        match self {
            Direction::Serve => abi::rights::READ,
            Direction::Use => abi::rights::WRITE,
        }
    }
}

/// **One capability a component declares it needs**, named by the component and routed by the
/// supervisor.
///
/// The `role` is the component's own word for what it needs the object *for*, never the
/// supervisor's word for the object. That is what lets one declaration be wired two ways:
/// `swap_boundary_witness` asks to use `service`, and whether that is the shared rendezvous or a
/// queue broker's front rendezvous is the supervisor's business and invisible to the component.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CapNeed {
    /// The component's name for this need. Unique within one [`Requirements`]; a repeat is
    /// [`Refusal::DuplicateRole`] at compile time.
    pub role: &'static str,
    /// Which way it points. See [`Direction`] for why this has no default.
    pub direction: Direction,
}

/// **What kind of page a mapping need is for.** Two kinds, because the two behave differently at the
/// one moment a live replacement cares about.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageKind {
    /// Ordinary memory the supervisor shares with the component, mapped read/write.
    Shared,
    /// Ordinary memory the component may read and must not write: a client's outgoing page, which
    /// the client fills and the component only reads (milestone 23 (a capability-routed component
    /// OS with live replacement), for `line_editor`'s view of the shell's output page). Mapped
    /// read-only, and installed at build time like [`Shared`](PageKind::Shared).
    ///
    /// Name: provisional (the lane for milestone 23, 2026-09-27).
    ReadOnly,
    /// A device's registers.
    ///
    /// Called out as its own kind for one reason, and it is §41's: revocation is by physical page, so
    /// a device the supervisor is about to take back **cannot be installed at build time**. A plan
    /// sorts these last and [`Plan::devices`] hands them over separately, so the operator can endow
    /// everything else early and map the registers on the far side of the revoke, which is what keeps
    /// the down window four syscalls wide.
    DeviceRegisters,
}

impl PageKind {
    /// The mapping mode this kind is installed with. Device registers are mapped device-typed
    /// read/write by the kernel whatever mode is asked for, so the mode a device need carries is
    /// the conservative one rather than a claim.
    pub const fn mode(self) -> u64 {
        match self {
            PageKind::Shared => abi::address_space::MAP_RW,
            PageKind::ReadOnly | PageKind::DeviceRegisters => abi::address_space::MAP_RO,
        }
    }
}

/// **One page a component declares it needs, and the address it needs it at.**
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapNeed {
    /// The component's name for this need, routed exactly as a [`CapNeed`]'s is.
    pub role: &'static str,
    /// The virtual address the component reads it at. It is in the declaration rather than in the
    /// supervisor because it is a fact about the component's own code: the component compiles the
    /// address into a `read_volatile`, and a supervisor that mapped it somewhere else would produce
    /// a component that faults for a reason a long way from the mistake.
    pub va: u64,
    /// Shared memory or a device's registers. See [`PageKind`].
    pub kind: PageKind,
}

/// **The role name a supervisor routes a component's handoff page under** (DECISIONS §209 (state handoff is an
/// opaque blob over a granted frame, and it is optional)). One fixed
/// word rather than a role the component picks, because a handoff page is not a need the component
/// names for its own purposes: it is the one page a supervisor must route to the **same** frame in the
/// outgoing and the incoming instance, and a supervisor wiring two builds of one contract has to be
/// able to find it without reading either build's code.
///
/// Name: ratified 2026-10-18 (calef, on the five `handoff` names of this crate: "Approve all five").
pub const STATE_HANDOFF_ROLE: &str = "handoff";

/// The page size a [`StateHandoff`] run is counted in, for the overlap check. Every architecture this
/// tree runs on maps 4 KiB pages at this level.
const HANDOFF_PAGE: u64 = 4096;

/// **Where a component that carries state across a live replacement wants its handoff pages**
/// (DECISIONS §209: state handoff is an opaque blob over a granted frame, and it is optional).
///
/// The page's bytes are the component's business and nobody else's. The supervisor never reads them
/// and this crate never names their shape, which is the refusal of §116 (live component state handoff is declined, for want of a customer) to design one wire format for
/// every kind of live state, kept. What the declaration buys the supervisor is two facts it could not
/// otherwise know: **that** this contract carries state (so a swap must wait for the incoming
/// instance to say it absorbed it before the outgoing one is retired), and **where** both instances
/// read and write it (so the supervisor can route one frame to both).
///
/// A run of [`pages`](StateHandoff::pages) pages, which the supervisor mints as one frame capability
/// with `MemoryRegion::RETYPE`'s page count, per DECISIONS §233 (`MemoryRegion::RETYPE` takes a page count). The count is the
/// component's to declare, because it is a fact about how much state that component keeps.
///
/// Name: ratified 2026-10-18 (calef, on the five `handoff` names of this crate: "Approve all five").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StateHandoff {
    /// The virtual address both instances read and write the blob at. In the declaration for
    /// [`MapNeed::va`]'s reason: it is compiled into the component's own code.
    pub va: u64,
    /// How many pages the blob may occupy, starting at [`va`](StateHandoff::va). At least one.
    pub pages: u64,
}

/// **What a component declares it needs.** The capability half of a contract, beside the wire half.
///
/// The order of [`caps`](Requirements::caps) is load-bearing: it **is** the component's capability table slot
/// order, because `supervision_protocol::ChildEndowment::caps` lands its entries in slots 0, 1, 2, ...
/// Before this crate that agreement was a comment in two files. Now the component derives its own
/// slot numbers from this list with [`slot_of`], so a reordered manifest is a compile error rather
/// than a component reading the wrong slot at run time.
///
/// What is **not** here is as deliberate as what is. Supervision is not declared, because a
/// component does not choose whether it is watched: that is the supervisor's decision and it is done
/// *to* the component (§32). Start arguments are not declared either, because they are
/// configuration rather than authority: which log entries this instance writes and whether it was
/// handed a device are facts about the wiring, and the component is told them rather than asking.
/// A manifest declares what a component **needs**, not everything about how it is started.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Requirements {
    /// The contract this component implements, for a reader and for a diagnostic. Not matched
    /// against anything: what makes a build substitutable is that it speaks the protocol, and a
    /// string it carries cannot prove that.
    pub contract: &'static str,
    /// The capabilities it needs, **in capability table slot order**. See the struct docs.
    pub caps: &'static [CapNeed],
    /// The pages it needs mapped, each at the address its own code reads.
    pub maps: &'static [MapNeed],
    /// How many pages of the supervisor's budget one instance is built out of: its segments, its
    /// stack, its address-space root, its TCB, its page tables and its revocation log.
    ///
    /// The one field here that is a property of the **build** rather than of the contract, which is
    /// the crate's second `BUGS` entry and the strongest argument for eventually shipping a manifest
    /// with the binary that satisfies it.
    pub pages: u64,
    /// **Which contracts this component cannot silently tolerate the absence of** (milestone 23's
    /// dependency-aware orchestration residual). Named by contract, not by role: a role name is
    /// resolved to an object by the supervisor's [`Provisions`] and that resolution can vary by
    /// wiring (`CLIENT`'s `service` role is a console on the direct channel and a broker's front
    /// rendezvous on the queued one), so a role cannot say which contract will answer it. A contract
    /// name can, and does not change across wirings.
    ///
    /// **Populated only for a component that itself serves others**, and empty otherwise, because
    /// that is the whole of what this field is for: deciding who [`dependents`] must warn before a
    /// swap. A pure consumer (no `Direction::Serve` need at all, like `CLIENT`) never needs warning.
    /// It calls through a `CALL`, and DECISIONS §41 already proved that call degrades for free: a
    /// `CALL` that finds nobody receiving parks on the rendezvous's own sender queue, and the next
    /// server to `RECEIVE_CAP` drains it. Nothing has to tell a pure consumer anything, so nothing here
    /// claims one needs telling. A component that *serves* others while itself calling through to a
    /// swappable dependency is different: `broker` blocks its one serving thread on a `CALL` to its
    /// backend, so if the backend is what is being swapped, `broker` cannot go on answering its own
    /// producers unless it is told first to stop calling through and start buffering (a page and a
    /// signal it is never asked to acknowledge, DECISIONS §231 (a swap's warning to a dependent is
    /// advisory, and the supervisor never waits for it)).
    /// That is the edge this field exists to name.
    ///
    /// **Direct dependents only.** A dependent that has its own decoupling mechanism (a queue, a
    /// cache) is not assumed to propagate the warning to *its* dependents, because whether it needs
    /// to is a property of that mechanism and not something this crate can infer from two contract
    /// names. See [`dependents`]'s docs and this crate's `BUGS` for what that leaves unbuilt.
    pub depends_on: &'static [&'static str],
    /// **Whether this contract carries state across a live replacement, and where** (DECISIONS §209,
    /// milestone 23's last residual). `None` is the default answer and the common one: a component
    /// that declares nothing is killed and replaced, which is what every component in this tree was
    /// before §209 and what roughly fifty of fifty-two still are.
    ///
    /// A required field with no default, for [`Direction`]'s reason: a contract **cannot be declared
    /// without saying whether it hands state off**, so "we forgot" is not a state the tree can be in.
    ///
    /// `Some` does two things. [`plan`] routes the supervisor's [`STATE_HANDOFF_ROLE`] object to the page
    /// at [`StateHandoff::va`], and refuses with [`Refusal::Unprovided`] when the supervisor routes
    /// nothing there. That refusal is the point: **a supervisor that cannot carry state must not
    /// silently kill-and-replace a component that declared some**, because the loss would look
    /// exactly like a successful swap. And [`Plan::state_handoff`] tells the supervisor to wait for the
    /// incoming instance's verdict before it retires the outgoing one, since under §209 a swap
    /// whose state was not absorbed does not commit.
    ///
    /// Name: ratified 2026-10-18 (calef, on the five `handoff` names of this crate: "Approve all five").
    pub state_handoff: Option<StateHandoff>,
}

impl Requirements {
    /// **Whether this declaration is well formed at all**, independent of any supervisor.
    ///
    /// A `const fn`, and that is the point: every real manifest in this tree asserts on it in a
    /// `const` item, so a malformed declaration does not compile. Three things are checked, and each
    /// one is a mistake that would otherwise be silent:
    ///
    /// - **A role declared twice.** The component would be handed two slots and read one of them,
    ///   and nothing at run time distinguishes that from a correct wiring.
    /// - **Two pages at one address.** One mapping silently wins; which one depends on the order the
    ///   supervisor happened to install them.
    /// - **Zero pages.** A component nothing pays for cannot be built, and the failure would arrive
    ///   as a region split refusing for no visible reason.
    ///
    /// Over-long declarations are **not** checked here, because [`MAX_CAPS`] is this crate's limit
    /// rather than the declaration's defect: it is [`plan`]'s refusal, where a reader meets the
    /// bound.
    pub const fn problem(&self) -> Option<Refusal> {
        if self.pages == 0 {
            return Some(Refusal::NoPages);
        }
        let mut i = 0;
        while i < self.caps.len() {
            let mut j = i + 1;
            while j < self.caps.len() {
                if str_eq(self.caps[i].role, self.caps[j].role) {
                    return Some(Refusal::DuplicateRole {
                        role: self.caps[i].role,
                    });
                }
                j += 1;
            }
            i += 1;
        }
        let mut i = 0;
        while i < self.maps.len() {
            let mut j = i + 1;
            while j < self.maps.len() {
                if str_eq(self.maps[i].role, self.maps[j].role) {
                    return Some(Refusal::DuplicateRole {
                        role: self.maps[i].role,
                    });
                }
                if self.maps[i].va == self.maps[j].va {
                    return Some(Refusal::OverlappingVa {
                        va: self.maps[i].va,
                    });
                }
                j += 1;
            }
            i += 1;
        }
        // The handoff page is one more mapping, so it is held to the two rules every mapping is:
        // its role word may not be declared a second time, and its address may not be shared.
        if let Some(h) = self.state_handoff {
            if h.pages == 0 {
                return Some(Refusal::EmptyHandoff);
            }
            let mut i = 0;
            while i < self.caps.len() {
                if str_eq(self.caps[i].role, STATE_HANDOFF_ROLE) {
                    return Some(Refusal::DuplicateRole {
                        role: STATE_HANDOFF_ROLE,
                    });
                }
                i += 1;
            }
            let mut i = 0;
            while i < self.maps.len() {
                if str_eq(self.maps[i].role, STATE_HANDOFF_ROLE) {
                    return Some(Refusal::DuplicateRole {
                        role: STATE_HANDOFF_ROLE,
                    });
                }
                // The handoff is a run, so a page anywhere inside it collides, not only at its base.
                let end = h.va.saturating_add(h.pages.saturating_mul(HANDOFF_PAGE));
                if self.maps[i].va >= h.va && self.maps[i].va < end {
                    return Some(Refusal::OverlappingVa {
                        va: self.maps[i].va,
                    });
                }
                i += 1;
            }
        }
        None
    }

    /// How many pages a plan of this declaration maps: its declared pages, plus the handoff page if
    /// it declares one.
    const fn mapped(&self) -> usize {
        self.maps.len() + if self.state_handoff.is_some() { 1 } else { 0 }
    }
}

/// **What a supervisor will route to one child**, as its own slot per role name.
///
/// Per child rather than per supervisor, and that is Fuchsia's routing rather than an ambient
/// environment: the same supervisor hands the same role name to two children and may resolve it to
/// two different objects, which is exactly what makes a component's peer substitutable. A role the
/// supervisor does not list resolves to nothing, so a manifest can never widen its own authority.
///
/// A supervisor may list roles nothing needs. That is not an error: it holds more than any one child
/// requires, and refusing the surplus would make one table per child mandatory for no gain.
#[derive(Clone, Copy, Debug)]
pub struct Provisions<'a> {
    /// `(role name, the supervisor's own capability table slot holding the object)`.
    pub held: &'a [(&'a str, u64)],
}

impl<'a> Provisions<'a> {
    /// The slot this supervisor routes `role` to, or `None`.
    pub fn slot_for(&self, role: &str) -> Option<u64> {
        let mut i = 0;
        while i < self.held.len() {
            if self.held[i].0 == role {
                return Some(self.held[i].1);
            }
            i += 1;
        }
        None
    }
}

/// **Why a component could not be wired.** A value the supervisor reports, never a panic: a
/// supervisor that cannot satisfy a manifest has a legible thing to say and a child it did not
/// build.
///
/// # Examples
///
/// A supervisor that does not hold what the component asks for is refused, and nothing is built:
///
/// ```
/// # use component_plan::{CapNeed, Direction, Provisions, Refusal, Requirements};
/// # const TALKER: Requirements = Requirements {
/// #     contract: "talker",
/// #     caps: &[CapNeed { role: "service", direction: Direction::Serve }],
/// #     maps: &[],
/// #     pages: 32,
/// #     depends_on: &[],
/// #     state_handoff: None,
/// # };
/// let refused = component_plan::plan(&TALKER, &Provisions { held: &[("report", 1)] });
/// assert_eq!(refused, Err(Refusal::Unprovided { role: "service" }));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// The supervisor routes nothing to a role the component declared. **The security-relevant
    /// one**: it is what stops a manifest from being a request the supervisor is obliged to honour.
    Unprovided {
        /// The component's name for the need that went unrouted.
        role: &'static str,
    },
    /// One role name appears twice in a declaration.
    DuplicateRole {
        /// The name that repeats.
        role: &'static str,
    },
    /// Two pages are declared at one virtual address.
    OverlappingVa {
        /// The address they collide at.
        va: u64,
    },
    /// The declaration asks for more capabilities than [`MAX_CAPS`].
    TooManyCaps {
        /// How many it asked for.
        asked: usize,
    },
    /// The declaration asks for more pages than [`MAX_MAPS`].
    TooManyMaps {
        /// How many it asked for.
        asked: usize,
    },
    /// The declaration asks to exist in zero pages.
    NoPages,
    /// The declaration asks for a handoff of zero pages, which is a blob with nowhere to go.
    /// Name: provisional (the lane for milestone 23, 2026-09-26).
    EmptyHandoff,
}

impl Refusal {
    /// **One word, so a refusal can be reported over IPC.** A supervisor inside a test has two
    /// registers and no way to send a string, and a refusal that only exists as a `&'static str` in
    /// a dead process is a refusal nothing can assert on.
    pub const fn code(self) -> u64 {
        match self {
            Refusal::Unprovided { .. } => 1,
            Refusal::DuplicateRole { .. } => 2,
            Refusal::OverlappingVa { .. } => 3,
            Refusal::TooManyCaps { .. } => 4,
            Refusal::TooManyMaps { .. } => 5,
            Refusal::NoPages => 6,
            Refusal::EmptyHandoff => 7,
        }
    }

    /// What a person is told. `grant_plan::Refusal::message`'s convention: a sentence about this
    /// system's model, not a Unix errno translated.
    pub const fn message(self) -> &'static str {
        match self {
            Refusal::Unprovided { .. } => "this supervisor routes nothing to a role it declares",
            Refusal::DuplicateRole { .. } => "it declares one role name twice",
            Refusal::OverlappingVa { .. } => "it declares two pages at one address",
            Refusal::TooManyCaps { .. } => "it declares more capabilities than a plan can carry",
            Refusal::TooManyMaps { .. } => "it declares more pages than a plan can carry",
            Refusal::NoPages => "it declares no memory to be built in",
            Refusal::EmptyHandoff => "it declares a handoff with no pages to hold the state",
        }
    }
}

/// **Exactly what to endow one child with**, in the form `supervision_protocol::ChildEndowment` takes.
///
/// A value with no allocation, bounded by [`MAX_CAPS`] and [`MAX_MAPS`], so a supervisor holds it on
/// its stack and hands out slices of it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Plan {
    caps: [(u64, u64); MAX_CAPS],
    ncaps: usize,
    maps: [(u64, u64, u64); MAX_MAPS],
    nmaps: usize,
    ndevices: usize,
    pages: u64,
    state_handoff: Option<StateHandoff>,
}

impl Plan {
    /// `(the supervisor's slot, the child's rights)` per declared capability, **in declared order**,
    /// which is the child's slot order. Feeds `ChildEndowment::caps` directly.
    pub fn caps(&self) -> &[(u64, u64)] {
        &self.caps[..self.ncaps]
    }

    /// `(child va, the supervisor's slot, mode)` per declared page. Feeds `ChildEndowment::maps`.
    ///
    /// Device registers sort last. See [`maps_without_devices`](Plan::maps_without_devices) for the
    /// reason and for the swap that needs it.
    pub fn maps(&self) -> &[(u64, u64, u64)] {
        &self.maps[..self.nmaps]
    }

    /// **Everything but the device registers**, for a supervisor that is about to revoke a device.
    ///
    /// §41: revocation is by physical page, so a replacement endowed with the registers before the
    /// revoke would lose its copy to the revoke, and since the kernel mints a device capability once
    /// at boot nothing could hand one back. A supervisor building a replacement therefore endows it
    /// with this, revokes, and then installs [`devices`](Plan::devices) into the child's address
    /// space. That is what keeps the down window four syscalls wide.
    pub fn maps_without_devices(&self) -> &[(u64, u64, u64)] {
        &self.maps[..self.nmaps - self.ndevices]
    }

    /// The device mappings [`maps_without_devices`](Plan::maps_without_devices) left out, to be
    /// installed on the far side of a revoke.
    pub fn devices(&self) -> &[(u64, u64, u64)] {
        &self.maps[self.nmaps - self.ndevices..self.nmaps]
    }

    /// How many pages of the supervisor's budget one instance costs.
    pub fn pages(&self) -> u64 {
        self.pages
    }

    /// **Where this component's handoff page is, if it carries state across a swap** (DECISIONS
    /// §209). `None` means kill-and-replace: retire the outgoing instance as soon as the incoming one
    /// is running. `Some` means the swap does not commit until the incoming instance says it absorbed
    /// the blob, and a supervisor that retires the outgoing instance first has thrown the state away
    /// with nothing to show for it.
    ///
    /// The mapping itself is already in [`maps`](Plan::maps), among the shared pages and installed
    /// at build time in both instances, so a supervisor never installs it by hand. Unlike a device it
    /// is not deferred past a revoke, and it cannot be: `PageFrame::REVOKE` is symmetric and would take
    /// the supervisor's own capability with everyone else's. The outgoing instance is quiesced and
    /// blocked before the incoming one is started, so the two never run against the page at once.
    ///
    /// Name: ratified 2026-10-18 (calef, on the five `handoff` names of this crate: "Approve all five").
    pub fn state_handoff(&self) -> Option<StateHandoff> {
        self.state_handoff
    }
}

/// **Check a component's declaration against what a supervisor will route to it**, and yield the
/// endowment or say why not.
///
/// Nothing here spawns, maps or invokes: a refusal happens **before the component exists**, which
/// the guest test asserts by reporting it ahead of the first build step. Rights come from each
/// role's declared direction, and a component's slot from its own declaration ([`slot_of`]).
///
/// # Examples
///
/// ```
/// use component_plan::{CapNeed, Direction, MapNeed, PageKind, Provisions, Requirements};
///
/// pub const TALKER: Requirements = Requirements {
///     contract: "talker",
///     caps: &[
///         CapNeed { role: "service", direction: Direction::Serve },
///         CapNeed { role: "report", direction: Direction::Use },
///     ],
///     maps: &[MapNeed { role: "witness", va: 0x0300_0000, kind: PageKind::Shared }],
///     pages: 32,
///     depends_on: &[],
///     state_handoff: None,
/// };
///
/// const _: () = assert!(TALKER.problem().is_none());
///
/// pub const SERVICE: u64 = component_plan::slot_of(&TALKER, "service");
/// assert_eq!(SERVICE, 0);
///
/// let plan = component_plan::plan(
///     &TALKER,
///     &Provisions { held: &[("service", 5), ("report", 1), ("witness", 9)] },
/// )
/// .expect("every declared role is routed");
///
/// assert_eq!(plan.caps(), &[(5, abi::rights::READ), (1, abi::rights::WRITE)]);
/// assert_eq!(plan.maps(), &[(0x0300_0000, 9, abi::address_space::MAP_RW)]);
/// assert_eq!(plan.pages(), 32);
/// ```
pub fn plan(reqs: &Requirements, provisions: &Provisions<'_>) -> Result<Plan, Refusal> {
    if let Some(problem) = reqs.problem() {
        return Err(problem);
    }
    if reqs.caps.len() > MAX_CAPS {
        return Err(Refusal::TooManyCaps {
            asked: reqs.caps.len(),
        });
    }
    if reqs.mapped() > MAX_MAPS {
        return Err(Refusal::TooManyMaps {
            asked: reqs.mapped(),
        });
    }

    let mut out = Plan {
        caps: [(0, 0); MAX_CAPS],
        ncaps: reqs.caps.len(),
        maps: [(0, 0, 0); MAX_MAPS],
        nmaps: reqs.mapped(),
        ndevices: 0,
        pages: reqs.pages,
        state_handoff: None,
    };

    for (i, need) in reqs.caps.iter().enumerate() {
        let Some(slot) = provisions.slot_for(need.role) else {
            return Err(Refusal::Unprovided { role: need.role });
        };
        out.caps[i] = (slot, need.direction.rights());
    }

    // Two passes over the mapping needs so device registers land at the end of the array whatever
    // order they were declared in. That ordering is what lets `maps_without_devices` be a slice of
    // this same array rather than a second copy a `no_std` caller has nowhere to put.
    let mut n = 0;
    for pass in [PageKind::Shared, PageKind::DeviceRegisters] {
        // A read-only page is installed with the shared ones: only a device is deferred.
        let in_pass = |m: &&MapNeed| match pass {
            PageKind::DeviceRegisters => m.kind == PageKind::DeviceRegisters,
            _ => m.kind != PageKind::DeviceRegisters,
        };
        for need in reqs.maps.iter().filter(in_pass) {
            let Some(slot) = provisions.slot_for(need.role) else {
                return Err(Refusal::Unprovided { role: need.role });
            };
            out.maps[n] = (need.va, slot, need.kind.mode());
            n += 1;
            if pass == PageKind::DeviceRegisters {
                out.ndevices += 1;
            }
        }
        // The handoff page goes in after the shared pages and before any device, so it is part of
        // `maps_without_devices` and a replacement is built holding it. See `Plan::state_handoff`.
        if pass == PageKind::Shared
            && let Some(h) = reqs.state_handoff
        {
            let Some(slot) = provisions.slot_for(STATE_HANDOFF_ROLE) else {
                return Err(Refusal::Unprovided {
                    role: STATE_HANDOFF_ROLE,
                });
            };
            out.maps[n] = (h.va, slot, PageKind::Shared.mode());
            out.state_handoff = Some(h);
            n += 1;
        }
    }

    Ok(out)
}

/// **The capability table slot a component reads one of its own needs in**, derived from its declaration.
///
/// This is the rung the whole crate is built to reach. The slot agreement used to be a pair of
/// constants written twice, once as `pub const SVC: u64 = 0` in the contract and once as the position
/// of an entry in a literal array in the operator, with a comment in each file asking the reader to
/// check. Now the constant *is* the position, computed at compile time, and a role that the manifest
/// does not declare **does not compile**.
///
/// # Panics
///
/// If `role` is not declared. In a `const` item, which is the only way this is meant to be called,
/// that is a compile error rather than a panic.
///
/// ```compile_fail
/// # use component_plan::{CapNeed, Direction, Requirements};
/// const R: Requirements = Requirements {
///     contract: "talker",
///     caps: &[CapNeed { role: "service", direction: Direction::Serve }],
///     maps: &[],
///     pages: 32,
///     depends_on: &[],
///     state_handoff: None,
/// };
/// // There is no `control` role, so this constant cannot be evaluated.
/// const CONTROL: u64 = component_plan::slot_of(&R, "control");
/// ```
pub const fn slot_of(reqs: &Requirements, role: &str) -> u64 {
    let mut i = 0;
    while i < reqs.caps.len() {
        if str_eq(reqs.caps[i].role, role) {
            return i as u64;
        }
        i += 1;
    }
    panic!("this component declares no capability by that role name")
}

/// Byte equality on two strings, in `const` context. `str`'s own `==` is not a `const fn`, and every
/// check in this crate that has to run at compile time needs one.
const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

// ===================================================================================================
// Dependency-aware orchestration (milestone 23's third residual, roadmap's own words: "if component
// B is a client of component A, swapping A means quiesce B, swap, resume").
//
// This is the other half of [`Requirements::depends_on`]'s doc comment, made into a decision rather
// than a fact a supervisor would otherwise have to work out by hand. It answers exactly one
// question: **given the components a supervisor is currently running, which of them must be told
// before a named contract is swapped?** It does not run the swap, does not touch a capability, and
// does not decide *how* a dependent is told (that is a per-contract protocol, `broker`'s warning
// page and signal today); it only says *who*.
//
// **Direct dependents only, and that is a scope decision recorded rather than an oversight.** A
// dependent that itself absorbs its own dependency's downtime (a queue, in `broker`'s case) is not
// assumed to need telling about *its* dependents in turn, because whether it does is a property of
// that dependent's own mechanism and this crate has no way to know it from two contract names. See
// this crate's `BUGS` for what a transitive version would need and why it is not built.
// ===================================================================================================

/// **How many running components one orchestration query may consider.** The same shape as
/// [`MAX_CAPS`]: a [`Dependents`] is a value on the stack in a `no_std` program with no allocator, so
/// this is a real bound rather than a hint. Eight, matched to [`MAX_CAPS`] because a supervisor
/// managing more live components than it could ever endow one of with capabilities is not a shape
/// this tree has.
pub const MAX_LIVE: usize = 8;

/// **One component a supervisor is currently running**, as far as dependency-aware orchestration
/// needs to know about it: an id the caller chose (a tid, or any label stable for the query) and the
/// contract it is an instance of.
///
/// Not a general process-registry type. It exists to be the input to [`dependents`] and carries
/// nothing that function does not read.
#[derive(Clone, Copy)]
pub struct LiveInstance<'a> {
    /// The caller's own name for this running instance. Opaque to this crate; returned unchanged in
    /// [`Dependents::quiesce_order`].
    pub id: u64,
    /// Which contract this instance implements, and so whose [`Requirements::depends_on`] is
    /// consulted.
    pub reqs: &'a Requirements,
}

/// Why a [`dependents`] query was refused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OrchestrationRefusal {
    /// More live instances were named than [`MAX_LIVE`] can hold.
    TooManyLive {
        /// How many were asked about.
        asked: usize,
    },
}

/// **The live instances that must be told before a swap, in the order they were found.**
///
/// Not a general ordering claim: nothing here says quiescing one takes longer than another, or that
/// they must be told serially. It is a filter over [`dependents`]'s input, and its shape exists so a
/// supervisor holds the answer as a value rather than re-deriving it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Dependents {
    ids: [u64; MAX_LIVE],
    n: usize,
}

impl Dependents {
    /// The ids to quiesce, in the order [`dependents`] found them (the order `live` was given in).
    /// Empty is a real, common answer: it is what a target with no server-shaped clients returns,
    /// which is every target in this tree's own direct channel.
    pub fn quiesce_order(&self) -> &[u64] {
        &self.ids[..self.n]
    }
}

/// **Which of the running components in `live` must be quiesced before `target_contract` is
/// swapped**, because they declared it in their own [`Requirements::depends_on`].
///
/// An instance of `target_contract` itself is never returned, even from a malformed declaration
/// that lists its own contract: a component cannot be its own dependent.
///
/// # EXAMPLES
///
/// ```
/// use component_plan::{dependents, CapNeed, Direction, LiveInstance, Requirements};
///
/// const BACKEND: Requirements = Requirements {
///     contract: "backend",
///     caps: &[CapNeed { role: "service", direction: Direction::Serve }],
///     maps: &[], pages: 32, depends_on: &[], state_handoff: None,
/// };
/// const BROKER: Requirements = Requirements {
///     contract: "broker",
///     caps: &[
///         CapNeed { role: "requests", direction: Direction::Serve },
///         CapNeed { role: "backend", direction: Direction::Use },
///     ],
///     maps: &[], pages: 32, depends_on: &["backend"], state_handoff: None,
/// };
///
/// let live = [
///     LiveInstance { id: 1, reqs: &BACKEND },
///     LiveInstance { id: 2, reqs: &BROKER },
/// ];
///
/// // Swapping the backend means telling the broker first: it forwards to it synchronously.
/// let d = dependents("backend", &live).unwrap();
/// assert_eq!(d.quiesce_order(), &[2]);
///
/// // Nothing here declares "broker" in its own `depends_on`.
/// let d = dependents("broker", &live).unwrap();
/// assert!(d.quiesce_order().is_empty());
/// ```
pub fn dependents(
    target_contract: &str,
    live: &[LiveInstance],
) -> Result<Dependents, OrchestrationRefusal> {
    if live.len() > MAX_LIVE {
        return Err(OrchestrationRefusal::TooManyLive { asked: live.len() });
    }
    let mut out = Dependents {
        ids: [0; MAX_LIVE],
        n: 0,
    };
    for inst in live {
        if str_eq(inst.reqs.contract, target_contract) {
            continue; // an instance of the target is not its own dependent
        }
        let mut i = 0;
        while i < inst.reqs.depends_on.len() {
            if str_eq(inst.reqs.depends_on[i], target_contract) {
                out.ids[out.n] = inst.id;
                out.n += 1;
                break;
            }
            i += 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERVICE: CapNeed = CapNeed {
        role: "service",
        direction: Direction::Serve,
    };
    const REPORT: CapNeed = CapNeed {
        role: "report",
        direction: Direction::Use,
    };
    const WITNESS: MapNeed = MapNeed {
        role: "witness",
        va: 0x0300_0000,
        kind: PageKind::Shared,
    };
    const UART: MapNeed = MapNeed {
        role: "uart",
        va: 0x0310_0000,
        kind: PageKind::DeviceRegisters,
    };

    const CONSOLE: Requirements = Requirements {
        contract: "console",
        caps: &[SERVICE, REPORT],
        maps: &[WITNESS, UART],
        pages: 32,
        depends_on: &[],
        state_handoff: None,
    };

    fn everything() -> Provisions<'static> {
        Provisions {
            held: &[("service", 5), ("report", 1), ("witness", 9), ("uart", 2)],
        }
    }

    #[test]
    fn a_serve_gets_read_and_a_use_gets_write() {
        let p = plan(&CONSOLE, &everything()).unwrap();
        assert_eq!(p.caps(), &[(5, abi::rights::READ), (1, abi::rights::WRITE)]);
    }

    /// The one that would otherwise be a comment: the plan's cap order is the declaration's order,
    /// which is the component's slot order.
    #[test]
    fn the_declared_order_is_the_slot_order() {
        assert_eq!(slot_of(&CONSOLE, "service"), 0);
        assert_eq!(slot_of(&CONSOLE, "report"), 1);
        let p = plan(&CONSOLE, &everything()).unwrap();
        assert_eq!(p.caps()[slot_of(&CONSOLE, "report") as usize].0, 1);
    }

    #[test]
    fn a_role_the_supervisor_does_not_route_is_refused() {
        let partial = Provisions {
            held: &[("report", 1), ("witness", 9), ("uart", 2)],
        };
        assert_eq!(
            plan(&CONSOLE, &partial),
            Err(Refusal::Unprovided { role: "service" })
        );
    }

    /// A missing *page* refuses exactly as a missing capability does. Worth its own test because the
    /// two are checked in different loops.
    #[test]
    fn a_page_the_supervisor_does_not_route_is_refused_too() {
        let no_device = Provisions {
            held: &[("service", 5), ("report", 1), ("witness", 9)],
        };
        assert_eq!(
            plan(&CONSOLE, &no_device),
            Err(Refusal::Unprovided { role: "uart" })
        );
    }

    /// A supervisor holding more than the component asks for is wired, not refused: it holds what it
    /// holds, and one table per child would be mandatory otherwise.
    #[test]
    fn a_surplus_provision_is_not_an_error() {
        let extra = Provisions {
            held: &[
                ("service", 5),
                ("report", 1),
                ("witness", 9),
                ("uart", 2),
                ("nothing_needs_this", 77),
            ],
        };
        assert!(plan(&CONSOLE, &extra).is_ok());
    }

    /// §41's device deferral, which is the reason [`PageKind::DeviceRegisters`] is a kind at all.
    #[test]
    fn the_device_is_separable_from_the_rest_of_the_mappings() {
        let p = plan(&CONSOLE, &everything()).unwrap();
        assert_eq!(
            p.maps_without_devices(),
            &[(0x0300_0000, 9, abi::address_space::MAP_RW)]
        );
        assert_eq!(p.devices(), &[(0x0310_0000, 2, abi::address_space::MAP_RO)]);
    }

    /// And it is separable whatever order the declaration used, because a plan sorts rather than
    /// trusting the manifest's author to put the device last.
    #[test]
    fn a_device_declared_first_still_sorts_last() {
        const DEVICE_FIRST: Requirements = Requirements {
            contract: "console",
            caps: &[SERVICE],
            maps: &[UART, WITNESS],
            pages: 32,
            depends_on: &[],
            state_handoff: None,
        };
        let p = plan(&DEVICE_FIRST, &everything()).unwrap();
        assert_eq!(p.devices(), &[(0x0310_0000, 2, abi::address_space::MAP_RO)]);
        assert_eq!(p.maps_without_devices().len(), 1);
    }

    /// A read-only page is mapped read-only, and early: only a device waits for the far side of a
    /// revoke.
    #[test]
    fn a_read_only_page_is_mapped_read_only_and_not_deferred() {
        const READER: Requirements = Requirements {
            contract: "reader",
            caps: &[SERVICE],
            maps: &[
                UART,
                MapNeed {
                    role: "witness",
                    va: 0x0300_0000,
                    kind: PageKind::ReadOnly,
                },
            ],
            pages: 32,
            depends_on: &[],
            state_handoff: None,
        };
        let p = plan(&READER, &everything()).unwrap();
        assert_eq!(
            p.maps_without_devices(),
            &[(0x0300_0000, 9, abi::address_space::MAP_RO)]
        );
        assert_eq!(p.devices().len(), 1);
    }

    /// A component with no device at all: the empty-device case must not be a panic in the
    /// arithmetic that splits the array.
    #[test]
    fn a_component_with_no_device_has_no_deferred_mapping() {
        const PLAIN: Requirements = Requirements {
            contract: "backend",
            caps: &[SERVICE],
            maps: &[WITNESS],
            pages: 32,
            depends_on: &[],
            state_handoff: None,
        };
        let p = plan(&PLAIN, &everything()).unwrap();
        assert!(p.devices().is_empty());
        assert_eq!(p.maps_without_devices(), p.maps());
    }

    // ===========================================================================================
    // State handoff (DECISIONS §209).
    // ===========================================================================================

    /// A console that also carries state: the widest shape a handoff can land in, with a device on
    /// the far side of it.
    const STATEFUL: Requirements = Requirements {
        contract: "stateful_backend",
        caps: &[SERVICE],
        maps: &[UART, WITNESS],
        pages: 32,
        depends_on: &[],
        state_handoff: Some(StateHandoff {
            va: 0x0320_0000,
            pages: 2,
        }),
    };

    fn with_handoff() -> Provisions<'static> {
        Provisions {
            held: &[
                ("service", 5),
                ("report", 1),
                ("witness", 9),
                ("uart", 2),
                (STATE_HANDOFF_ROLE, 11),
            ],
        }
    }

    /// **The handoff page is built in, not deferred.** It lands among the shared pages and ahead of
    /// the device, so a replacement built from `maps_without_devices` already holds it, and the
    /// device split the direct swap depends on is exactly what it was.
    #[test]
    fn the_handoff_page_is_mapped_early_and_the_device_still_sorts_last() {
        let p = plan(&STATEFUL, &with_handoff()).unwrap();
        assert_eq!(
            p.state_handoff(),
            Some(StateHandoff {
                va: 0x0320_0000,
                pages: 2
            })
        );
        assert_eq!(
            p.maps_without_devices(),
            &[
                (0x0300_0000, 9, abi::address_space::MAP_RW),
                (0x0320_0000, 11, abi::address_space::MAP_RW),
            ]
        );
        assert_eq!(p.devices(), &[(0x0310_0000, 2, abi::address_space::MAP_RO)]);
    }

    /// **The refusal §209 needs and does not spell out.** A supervisor that routes no handoff page
    /// cannot carry state, and wiring the component anyway would be a kill-and-replace that looks
    /// exactly like a successful swap while the state is thrown away. So it is refused before
    /// anything is built, the same way an unrouted capability is.
    #[test]
    fn a_supervisor_that_cannot_carry_state_is_refused_a_component_that_declares_some() {
        let no_state = Provisions {
            held: &[("service", 5), ("witness", 9), ("uart", 2)],
        };
        assert_eq!(
            plan(&STATEFUL, &no_state),
            Err(Refusal::Unprovided {
                role: STATE_HANDOFF_ROLE
            })
        );
    }

    /// And a component that declares nothing is wired exactly as before, whatever the supervisor
    /// holds: optional is the half of §209 that most of the tree lives in.
    #[test]
    fn a_component_with_no_handoff_ignores_a_supervisor_that_offers_one() {
        let p = plan(&CONSOLE, &with_handoff()).unwrap();
        assert_eq!(p.state_handoff(), None);
        assert_eq!(p.maps().len(), 2);
    }

    /// The handoff page is one more mapping and obeys both mapping rules: no shared address, and its
    /// role word is not declared twice.
    #[test]
    fn a_handoff_page_is_held_to_the_mapping_rules() {
        const COLLIDES: Requirements = Requirements {
            state_handoff: Some(StateHandoff {
                va: 0x0300_0000,
                pages: 1,
            }),
            ..STATEFUL
        };
        assert_eq!(
            COLLIDES.problem(),
            Some(Refusal::OverlappingVa { va: 0x0300_0000 })
        );
        const TWICE: Requirements = Requirements {
            maps: &[MapNeed {
                role: STATE_HANDOFF_ROLE,
                va: 0x0400_0000,
                kind: PageKind::Shared,
            }],
            ..STATEFUL
        };
        assert_eq!(
            TWICE.problem(),
            Some(Refusal::DuplicateRole {
                role: STATE_HANDOFF_ROLE
            })
        );
    }

    /// **A handoff is a run, so a page declared anywhere inside it collides**, not only one at its
    /// base, and a run of zero pages is refused outright.
    #[test]
    fn a_handoff_run_is_checked_as_a_run() {
        const INSIDE: Requirements = Requirements {
            maps: &[MapNeed {
                role: "witness",
                va: 0x0320_1000,
                kind: PageKind::Shared,
            }],
            ..STATEFUL
        };
        assert_eq!(
            INSIDE.problem(),
            Some(Refusal::OverlappingVa { va: 0x0320_1000 }),
            "a page on the handoff run's second page must collide",
        );
        const BESIDE: Requirements = Requirements {
            maps: &[MapNeed {
                role: "witness",
                va: 0x0320_2000,
                kind: PageKind::Shared,
            }],
            ..STATEFUL
        };
        assert_eq!(
            BESIDE.problem(),
            None,
            "the page after a two-page run is free"
        );
        const EMPTY: Requirements = Requirements {
            state_handoff: Some(StateHandoff {
                va: 0x0320_0000,
                pages: 0,
            }),
            ..STATEFUL
        };
        assert_eq!(EMPTY.problem(), Some(Refusal::EmptyHandoff));
    }

    /// The bound counts the handoff page: four declared pages plus a handoff is five mappings, and a
    /// plan with room for four must refuse rather than overflow its array.
    #[test]
    fn the_handoff_page_counts_against_the_mapping_bound() {
        const FULL: Requirements = Requirements {
            maps: &[
                MapNeed {
                    role: "a",
                    va: 0x1000,
                    kind: PageKind::Shared,
                },
                MapNeed {
                    role: "b",
                    va: 0x2000,
                    kind: PageKind::Shared,
                },
                MapNeed {
                    role: "c",
                    va: 0x3000,
                    kind: PageKind::Shared,
                },
                MapNeed {
                    role: "d",
                    va: 0x4000,
                    kind: PageKind::Shared,
                },
            ],
            ..STATEFUL
        };
        assert_eq!(
            plan(&FULL, &with_handoff()),
            Err(Refusal::TooManyMaps { asked: 5 })
        );
    }

    #[test]
    fn a_role_declared_twice_is_malformed() {
        const TWICE: Requirements = Requirements {
            contract: "confused",
            caps: &[SERVICE, SERVICE],
            maps: &[],
            pages: 32,
            depends_on: &[],
            state_handoff: None,
        };
        assert_eq!(
            TWICE.problem(),
            Some(Refusal::DuplicateRole { role: "service" })
        );
        assert_eq!(plan(&TWICE, &everything()).unwrap_err().code(), 2);
    }

    #[test]
    fn two_pages_at_one_address_are_malformed() {
        const COLLIDE: Requirements = Requirements {
            contract: "confused",
            caps: &[],
            maps: &[
                WITNESS,
                MapNeed {
                    role: "other",
                    va: WITNESS.va,
                    kind: PageKind::Shared,
                },
            ],
            pages: 32,
            depends_on: &[],
            state_handoff: None,
        };
        assert_eq!(
            COLLIDE.problem(),
            Some(Refusal::OverlappingVa { va: 0x0300_0000 })
        );
    }

    #[test]
    fn a_component_nothing_pays_for_is_malformed() {
        const FREE: Requirements = Requirements {
            contract: "confused",
            caps: &[],
            maps: &[],
            pages: 0,
            depends_on: &[],
            state_handoff: None,
        };
        assert_eq!(FREE.problem(), Some(Refusal::NoPages));
    }

    /// Every refusal has a distinct wire code, because the guest reports the code and a collision
    /// would make two different failures indistinguishable in a test.
    #[test]
    fn the_refusal_codes_are_distinct_and_never_zero() {
        let all = [
            Refusal::Unprovided { role: "x" },
            Refusal::DuplicateRole { role: "x" },
            Refusal::OverlappingVa { va: 0 },
            Refusal::TooManyCaps { asked: 0 },
            Refusal::TooManyMaps { asked: 0 },
            Refusal::NoPages,
            Refusal::EmptyHandoff,
        ];
        for (i, a) in all.iter().enumerate() {
            assert_ne!(a.code(), 0, "zero would read as \"no refusal\"");
            assert!(!a.message().is_empty());
            for b in &all[i + 1..] {
                assert_ne!(a.code(), b.code());
            }
        }
    }

    #[test]
    fn the_well_formed_declaration_is_the_common_case() {
        assert_eq!(CONSOLE.problem(), None);
    }

    // ===========================================================================================
    // Dependency-aware orchestration.
    // ===========================================================================================

    const BACKEND: Requirements = Requirements {
        contract: "backend",
        caps: &[SERVICE],
        maps: &[],
        pages: 32,
        depends_on: &[],
        state_handoff: None,
    };
    const BROKER: Requirements = Requirements {
        contract: "broker",
        caps: &[
            CapNeed {
                role: "requests",
                direction: Direction::Serve,
            },
            CapNeed {
                role: "backend",
                direction: Direction::Use,
            },
        ],
        maps: &[],
        pages: 32,
        depends_on: &["backend"],
        state_handoff: None,
    };
    const CLIENT: Requirements = Requirements {
        contract: "client",
        caps: &[REPORT],
        maps: &[],
        pages: 32,
        depends_on: &[],
        state_handoff: None,
    };

    /// The one case this residual exists for, in its smallest true form: a component that forwards
    /// synchronously to another must be told before that other is swapped.
    #[test]
    fn a_component_that_calls_through_to_a_swap_target_is_a_dependent() {
        let live = [
            LiveInstance {
                id: 1,
                reqs: &BACKEND,
            },
            LiveInstance {
                id: 2,
                reqs: &BROKER,
            },
        ];
        let d = dependents("backend", &live).unwrap();
        assert_eq!(d.quiesce_order(), &[2]);
    }

    /// A pure consumer never appears, because it has nothing declared in `depends_on`: §41 already
    /// proved a `CALL` to an absent server degrades for free (the kernel's own sender queue), so a
    /// component with no `Serve` need of its own never needs telling.
    #[test]
    fn a_pure_consumer_is_never_a_dependent() {
        let live = [
            LiveInstance {
                id: 1,
                reqs: &BACKEND,
            },
            LiveInstance {
                id: 3,
                reqs: &CLIENT,
            },
        ];
        let d = dependents("backend", &live).unwrap();
        assert!(
            d.quiesce_order().is_empty(),
            "CLIENT declares no depends_on, so it must never be returned",
        );
    }

    /// Swapping a contract that nothing depends on is the common case in this tree's own direct
    /// channel, and it must be a real empty answer rather than a refusal.
    #[test]
    fn a_target_with_no_dependents_returns_an_empty_order() {
        let live = [LiveInstance {
            id: 1,
            reqs: &BACKEND,
        }];
        let d = dependents("backend", &live).unwrap();
        assert!(d.quiesce_order().is_empty());
    }

    /// An instance of the target contract is never its own dependent, even if asked about directly:
    /// a component cannot need warning about its own absence.
    #[test]
    fn an_instance_of_the_target_itself_is_excluded() {
        let live = [
            LiveInstance {
                id: 1,
                reqs: &BACKEND,
            },
            LiveInstance {
                id: 2,
                reqs: &BROKER,
            },
        ];
        let d = dependents("broker", &live).unwrap();
        assert!(
            d.quiesce_order().is_empty(),
            "nothing in this registry declares broker as a dependency, and the broker instance \
             itself must not be returned even though it exists",
        );
    }

    /// More live instances than `MAX_LIVE` is refused rather than silently truncated: a supervisor
    /// that got a short answer back would quiesce fewer components than it needed to.
    #[test]
    fn too_many_live_instances_is_refused() {
        let entries: [LiveInstance; MAX_LIVE + 1] = [LiveInstance {
            id: 0,
            reqs: &BACKEND,
        }; MAX_LIVE + 1];
        assert_eq!(
            dependents("backend", &entries),
            Err(OrchestrationRefusal::TooManyLive {
                asked: MAX_LIVE + 1
            })
        );
    }

    /// Order is preserved from the caller's own `live` slice, not sorted or otherwise rearranged:
    /// [`dependents`] is a filter, and a supervisor that wants a particular quiesce order gets it by
    /// choosing the order it lists its live instances in.
    #[test]
    fn the_order_returned_is_the_order_given() {
        const OTHER_BROKER: Requirements = Requirements {
            contract: "other_broker",
            caps: &[],
            maps: &[],
            pages: 32,
            depends_on: &["backend"],
            state_handoff: None,
        };
        let live = [
            LiveInstance {
                id: 20,
                reqs: &OTHER_BROKER,
            },
            LiveInstance {
                id: 10,
                reqs: &BROKER,
            },
        ];
        let d = dependents("backend", &live).unwrap();
        assert_eq!(d.quiesce_order(), &[20, 10]);
    }

    /// A dependent is found whichever position in its `depends_on` names the target, and a
    /// registry that fills `MAX_LIVE` exactly is answered, not refused. Two bounds in one fixture:
    /// the scan's step past a non-matching entry, and the live-count comparison's edge.
    #[test]
    fn a_target_named_second_is_found_and_a_full_registry_is_answered() {
        const LATE: Requirements = Requirements {
            contract: "late",
            caps: &[],
            maps: &[],
            pages: 32,
            depends_on: &["unrelated", "backend"],
            state_handoff: None,
        };
        let live = [LiveInstance { id: 7, reqs: &LATE }; MAX_LIVE];
        let d = dependents("backend", &live).unwrap();
        assert_eq!(d.quiesce_order(), &[7; MAX_LIVE]);
    }

    /// Exactly `MAX_CAPS` capabilities and exactly `MAX_MAPS` pages is a full plan, and one more of
    /// either is refused. The refusal tests above sit one past each bound; these sit on it.
    #[test]
    fn a_plan_exactly_at_each_bound_is_built_and_one_over_is_refused() {
        const fn cap(role: &'static str) -> CapNeed {
            CapNeed {
                role,
                direction: Direction::Use,
            }
        }
        const NINE: [CapNeed; MAX_CAPS + 1] = [
            cap("c0"),
            cap("c1"),
            cap("c2"),
            cap("c3"),
            cap("c4"),
            cap("c5"),
            cap("c6"),
            cap("c7"),
            cap("c8"),
        ];
        const FULL_CAPS: Requirements = Requirements {
            contract: "wide",
            caps: &[
                NINE[0], NINE[1], NINE[2], NINE[3], NINE[4], NINE[5], NINE[6], NINE[7],
            ],
            maps: &[],
            pages: 32,
            depends_on: &[],
            state_handoff: None,
        };
        const OVER_CAPS: Requirements = Requirements {
            caps: &NINE,
            ..FULL_CAPS
        };
        let held = Provisions {
            held: &[
                ("c0", 0),
                ("c1", 1),
                ("c2", 2),
                ("c3", 3),
                ("c4", 4),
                ("c5", 5),
                ("c6", 6),
                ("c7", 7),
                ("c8", 8),
            ],
        };
        assert_eq!(plan(&FULL_CAPS, &held).unwrap().caps().len(), MAX_CAPS);
        assert_eq!(
            plan(&OVER_CAPS, &held),
            Err(Refusal::TooManyCaps {
                asked: MAX_CAPS + 1
            })
        );

        const FOUR_PAGES: Requirements = Requirements {
            contract: "paged",
            caps: &[],
            maps: &[
                MapNeed {
                    role: "a",
                    va: 0x1000,
                    kind: PageKind::Shared,
                },
                MapNeed {
                    role: "b",
                    va: 0x2000,
                    kind: PageKind::Shared,
                },
                MapNeed {
                    role: "c",
                    va: 0x3000,
                    kind: PageKind::Shared,
                },
                MapNeed {
                    role: "d",
                    va: 0x4000,
                    kind: PageKind::Shared,
                },
            ],
            pages: 32,
            depends_on: &[],
            state_handoff: None,
        };
        let pages = Provisions {
            held: &[("a", 1), ("b", 2), ("c", 3), ("d", 4)],
        };
        assert_eq!(plan(&FOUR_PAGES, &pages).unwrap().maps().len(), MAX_MAPS);
    }

    /// A capability role that spells the handoff's reserved word is a duplicate of the handoff page,
    /// the same as a mapping that does (`a_handoff_page_is_held_to_the_mapping_rules`), and it is
    /// found whichever capability position carries it.
    #[test]
    fn a_capability_named_for_the_handoff_page_is_a_duplicate() {
        const CAP_TWICE: Requirements = Requirements {
            caps: &[
                SERVICE,
                CapNeed {
                    role: STATE_HANDOFF_ROLE,
                    direction: Direction::Use,
                },
            ],
            ..STATEFUL
        };
        assert_eq!(
            CAP_TWICE.problem(),
            Some(Refusal::DuplicateRole {
                role: STATE_HANDOFF_ROLE
            })
        );
    }

    /// A role nobody declared is a compile-time or run-time panic that says so, not an index error.
    #[test]
    #[should_panic(expected = "declares no capability by that role name")]
    fn asking_for_a_slot_nobody_declared_names_the_mistake() {
        let _ = slot_of(&CONSOLE, "control");
    }

    /// Each refusal explains itself differently, so a person reading one learns which it was.
    #[test]
    fn no_two_refusals_say_the_same_thing() {
        let all = [
            Refusal::Unprovided { role: "x" },
            Refusal::DuplicateRole { role: "x" },
            Refusal::OverlappingVa { va: 0 },
            Refusal::TooManyCaps { asked: 0 },
            Refusal::TooManyMaps { asked: 0 },
            Refusal::NoPages,
            Refusal::EmptyHandoff,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.message(), b.message());
            }
        }
    }
    // The eighth outsider pass's host attacks (claim 18): the shapes around the cited tests.
    // The test names are provisional (this pass's lane, 2026-10-13 UTC).

    const PEER: CapNeed = CapNeed {
        role: "peer",
        direction: Direction::Serve,
    };
    const SINK: CapNeed = CapNeed {
        role: "sink",
        direction: Direction::Use,
    };

    const FOUR_WAY: Requirements = Requirements {
        contract: "four_way",
        caps: &[SERVICE, REPORT, PEER, SINK],
        maps: &[],
        pages: 4,
        depends_on: &[],
        state_handoff: None,
    };

    /// **A four-role plan grants exactly each declaration's direction and nothing beside it**
    /// (claim 18), with the expected rights written as literals the way the Kani harness writes
    /// them, so a `Direction::rights` that drifted widens this test's answer and not its premise.
    #[test]
    fn a_four_role_plan_grants_exactly_the_declared_directions() {
        let provisions = Provisions {
            held: &[("peer", 3), ("sink", 7), ("service", 0), ("report", 9)],
        };
        let p = plan(&FOUR_WAY, &provisions).unwrap();
        assert_eq!(
            p.caps(),
            &[
                (0, abi::rights::READ),
                (9, abi::rights::WRITE),
                (3, abi::rights::READ),
                (7, abi::rights::WRITE)
            ],
            "the plan's grants are not exactly one per declaration, in the declaration's order, \
             with the direction's right",
        );
        assert_eq!(p.maps(), &[], "a plan with no maps granted a map");
        assert_eq!(p.pages(), 4);
    }

    /// **A provisions table carrying a surplus role with a fat right injects nothing.** The claim
    /// is about widening; the hostile shape is a supervisor that holds more than the component
    /// asked for and misroutes it, and the plan must still answer with the declaration's caps and
    /// only those.
    #[test]
    fn a_surplus_role_with_a_right_the_declaration_never_asked_for_grants_nothing() {
        let greedy = Provisions {
            held: &[
                ("service", 5),
                ("report", 1),
                ("witness", 9),
                ("uart", 2),
                ("peer", 4),
                ("sink", 6),
            ],
        };
        let p = plan(&CONSOLE, &greedy).unwrap();
        assert_eq!(
            p.caps(),
            &[(5, abi::rights::READ), (1, abi::rights::WRITE)],
            "a role the declaration never asked for reached the plan's grants",
        );
    }

    /// **A need routed by a role the provisions table names twice still grants one capability.**
    /// The hostile shape is a provisions table repeating a role at two slots: the plan resolves
    /// each declared need once, so the duplicate buys nothing.
    #[test]
    fn a_provisions_table_repeating_a_role_grants_one_capability() {
        let repeated = Provisions {
            held: &[
                ("service", 5),
                ("service", 6),
                ("report", 1),
                ("witness", 9),
                ("uart", 2),
            ],
        };
        let p = plan(&CONSOLE, &repeated).unwrap();
        assert_eq!(
            p.caps(),
            &[(5, abi::rights::READ), (1, abi::rights::WRITE)],
            "a repeated role changed what the plan granted",
        );
    }
}

// Machine-checked properties of the wiring (Kani), in proofs.rs. Behind `#[cfg(kani)]`, so an
// ordinary build or test never sees them; `script/verify` runs them.
#[cfg(kani)]
mod proofs;
