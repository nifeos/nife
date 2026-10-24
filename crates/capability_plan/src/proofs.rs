//! Machine-checked properties of the wiring. The host tests in `lib.rs` pin the cases a reader
//! cares about; these rule out shapes a case-by-case test cannot. A declaration holds `&'static`
//! slices, so the declarations are a bounded enumeration of `const` values walked by a concrete
//! loop, and what is symbolic is the routing table: slot numbers, and which roles are routed.
//! Selecting a declaration with `SHAPES[kani::any()]` instead does not finish. The harness paths
//! are `proofs::<harness>`, which `script/falsifications` reads from this file's name. See
//! notes/component-manifest/capability-plan.md and notes/verification.md.
//!
//! Name: provisional (milestone 882's lane, 2026-10-12 UTC).

use super::*;

const SERVE_A: CapNeed = CapNeed {
    role: "a",
    direction: Direction::Serve,
};
const USE_A: CapNeed = CapNeed {
    role: "a",
    direction: Direction::Use,
};
const SERVE_B: CapNeed = CapNeed {
    role: "b",
    direction: Direction::Serve,
};
const USE_B: CapNeed = CapNeed {
    role: "b",
    direction: Direction::Use,
};

/// Every arrangement of two capability needs over the two directions. Four, and that is the whole
/// space for two roles, so "for every declaration of this width" is exact rather than sampled.
const SHAPES: [Requirements; 4] = [
    Requirements {
        contract: "c",
        caps: &[SERVE_A, SERVE_B],
        maps: &[],
        pages: 1,
        depends_on: &[],
        state_handoff: None,
    },
    Requirements {
        contract: "c",
        caps: &[SERVE_A, USE_B],
        maps: &[],
        pages: 1,
        depends_on: &[],
        state_handoff: None,
    },
    Requirements {
        contract: "c",
        caps: &[USE_A, SERVE_B],
        maps: &[],
        pages: 1,
        depends_on: &[],
        state_handoff: None,
    },
    Requirements {
        contract: "c",
        caps: &[USE_A, USE_B],
        maps: &[],
        pages: 1,
        depends_on: &[],
        state_handoff: None,
    },
];

/// **A slot a supervisor could route, bounded to the capability table that exists.**
///
/// A slot number at or past `abi::CAPABILITY_TABLE_SLOTS` (sixty-four since 2026-10-04, thirty-two before) is not
/// a routing a supervisor could make: the kernel refuses it. Raising the table from twenty-four
/// did not move these harnesses' times past their noise (measured by milestone 126 (the `procps` package), in
/// notes/process-view/the-machine-and-your-share.md). Bounding it here is not a weakening of the property, which
/// is about *which* slot a need is wired to rather than how large the number is, and it is what
/// makes these harnesses a viable gate: unbounded `u64` slots put the solver past seventy
/// thousand verification conditions for a fact about copying two integers through.
fn any_slot() -> u64 {
    let s: u64 = kani::any();
    kani::assume(s < abi::CAPABILITY_TABLE_SLOTS);
    s
}

/// No routing table ever gives a component a right its declaration did not ask for. For every
/// declaration of two needs and every pair of slot numbers a supervisor could route, the rights
/// word at slot `i` is exactly `caps[i].direction.rights()`: no `Use` becomes a `READ` (§41 (the
/// endpoint is the broker)), and nothing carries `GRANT`, which no declaration can ask for.
/// The unwind bound is a termination proof: ten, against real bounds of two capability needs, two
/// page needs, two routed names and a one-byte role name. `SHAPES[i]` makes the slice CBMC reads a
/// symbolic pointer, so the lengths it walks are symbolic and `str_eq` would unwind forever.
/// Falsification: replayable `crates/capability_plan/falsifications/proofs.a_plan_never_grants_a_right_the_declaration_did_not_ask_for.patch`
#[kani::proof]
#[kani::unwind(10)]
fn a_plan_never_grants_a_right_the_declaration_did_not_ask_for() {
    let held: [(&str, u64); 2] = [("a", any_slot()), ("b", any_slot())];
    for reqs in &SHAPES {
        let p = plan(reqs, &Provisions { held: &held }).expect("both roles are routed");
        assert!(p.caps().len() == 2);
        let mut i = 0;
        while i < 2 {
            // **The mapping written out, not `rights()` compared with itself** (milestone
            // 211, and milestone 202 found this shape here first). `plan` fills the rights
            // word from `direction.rights()`, so an equality against that same call is
            // satisfied by any consistently wrong `rights()`: one that returned
            // `READ | GRANT` for both directions passes it. The two assertions below it
            // were already independent, and they are what caught the defect; this makes the
            // first one independent too, so the harness states the whole claim rather than
            // two thirds of it.
            let expected = match reqs.caps[i].direction {
                Direction::Serve => abi::rights::READ,
                Direction::Use => abi::rights::WRITE,
            };
            assert!(
                p.caps()[i].1 == expected,
                "a plan granted a right the declaration did not ask for",
            );
            // **The two assertions that used to follow are gone, and which one they were is
            // the point** (milestone 307). They were `== READ || == WRITE` and
            // `& GRANT == 0`, and once the line above was written out in literals rather than
            // through `direction.rights()` (milestone 211), both became implied by it:
            // `READ = 1<<0`, `WRITE = 1<<1`, `GRANT = 1<<2`, so an equality against one of
            // the first two settles the other two. The `& GRANT == 0` line is the one
            // notes/confinement-claims.md credited as the only thing catching the original
            // defect, and 211's fix turned the rescue assertion into the decoration. That
            // inversion is worth more than the line was.

            assert!(p.caps()[i].0 == held[i].1);
            i += 1;
        }
    }
}

/// **A missing route refuses rather than falling through to a slot.**
///
/// The failure this rules out is the quiet one. A lookup that fell through to zero would endow a
/// child with whatever the supervisor keeps in slot 0, which in every program in this tree is its
/// construction budget. Proved over every arrangement of the two names being routed or not, and
/// over arbitrary slots.
/// Falsification: replayable `crates/capability_plan/falsifications/proofs.a_missing_route_refuses_rather_than_falling_through_to_a_slot.patch`
#[kani::proof]
#[kani::unwind(10)]
fn a_missing_route_refuses_rather_than_falling_through_to_a_slot() {
    // Each entry is present under its own name, or under a name nothing declares.
    let n0: &str = if kani::any() { "a" } else { "z" };
    let n1: &str = if kani::any() { "b" } else { "z" };
    let (s0, s1) = (any_slot(), any_slot());
    let held: [(&str, u64); 2] = [(n0, s0), (n1, s1)];
    for reqs in &SHAPES {
        match plan(reqs, &Provisions { held: &held }) {
            Ok(p) => {
                // Wired at all only if both names were routed, and then to those slots and no
                // others.
                assert!(n0 == "a" && n1 == "b");
                assert!(p.caps()[0].0 == s0 && p.caps()[1].0 == s1);
            }
            Err(r) => {
                assert!(n0 != "a" || n1 != "b");
                assert!(r.code() == Refusal::Unprovided { role: "a" }.code());
            }
        }
    }
}

const SHARED_P: MapNeed = MapNeed {
    role: "p",
    va: 0x1000,
    kind: PageKind::Shared,
};
const DEVICE_P: MapNeed = MapNeed {
    role: "p",
    va: 0x1000,
    kind: PageKind::DeviceRegisters,
};
const SHARED_Q: MapNeed = MapNeed {
    role: "q",
    va: 0x2000,
    kind: PageKind::Shared,
};
const DEVICE_Q: MapNeed = MapNeed {
    role: "q",
    va: 0x2000,
    kind: PageKind::DeviceRegisters,
};

/// Every arrangement of two page needs over the two kinds, which is the whole space for two
/// pages: neither a device, either one, or both.
const PAGE_SHAPES: [Requirements; 4] = [
    Requirements {
        contract: "c",
        caps: &[],
        maps: &[SHARED_P, SHARED_Q],
        pages: 1,
        depends_on: &[],
        state_handoff: None,
    },
    Requirements {
        contract: "c",
        caps: &[],
        maps: &[DEVICE_P, SHARED_Q],
        pages: 1,
        depends_on: &[],
        state_handoff: None,
    },
    Requirements {
        contract: "c",
        caps: &[],
        maps: &[SHARED_P, DEVICE_Q],
        pages: 1,
        depends_on: &[],
        state_handoff: None,
    },
    Requirements {
        contract: "c",
        caps: &[],
        maps: &[DEVICE_P, DEVICE_Q],
        pages: 1,
        depends_on: &[],
        state_handoff: None,
    },
];

/// **The device split is a partition, for every declaration.**
///
/// This is the arithmetic §41's four-syscall down window rests on. The operator endows a
/// replacement with [`Plan::maps_without_devices`] and installs [`Plan::devices`] after the
/// revoke, so an entry falling in both would be mapped twice and one falling in neither would
/// leave a component reading an unmapped page. Proved for every arrangement of kinds, including
/// a device declared first, which is what the sort exists for.
/// Falsification: replayable `crates/capability_plan/falsifications/proofs.the_device_split_partitions_the_mappings.patch`
#[kani::proof]
#[kani::unwind(10)]
fn the_device_split_partitions_the_mappings() {
    let held: [(&str, u64); 2] = [("p", any_slot()), ("q", any_slot())];
    for reqs in &PAGE_SHAPES {
        let p = plan(reqs, &Provisions { held: &held }).expect("both pages are routed");
        let (early, late) = (p.maps_without_devices(), p.devices());
        assert!(p.maps().len() == 2);
        assert!(early.len() + late.len() == p.maps().len());

        let mut declared_devices = 0;
        let mut k = 0;
        while k < reqs.maps.len() {
            if reqs.maps[k].kind == PageKind::DeviceRegisters {
                declared_devices += 1;
            }
            k += 1;
        }
        assert!(late.len() == declared_devices);
        // The late half is all devices and the early half is all ordinary shared pages, so the
        // operator's two-step endowment cannot install one of them at the wrong moment.
        // The two mode words spelled out rather than fetched from `kind.mode()`, which is
        // the call `plan` itself makes to fill them (milestone 211): a `mode()` returning
        // one constant for both kinds would satisfy an assertion phrased through it while
        // the operator mapped a device register page writable.
        for m in late {
            assert!(m.2 == abi::address_space::MAP_RO);
        }
        for m in early {
            assert!(m.2 == abi::address_space::MAP_RW);
        }
        const { assert!(abi::address_space::MAP_RO != abi::address_space::MAP_RW) };
    }
}

// ===========================================================================================
// Dependency-aware orchestration.
// ===========================================================================================

const TARGET_SELF: Requirements = Requirements {
    contract: "a",
    caps: &[],
    maps: &[],
    pages: 1,
    depends_on: &[],
    state_handoff: None,
};
const OTHER_NODEP: Requirements = Requirements {
    contract: "other",
    caps: &[],
    maps: &[],
    pages: 1,
    depends_on: &[],
    state_handoff: None,
};
const OTHER_DEP: Requirements = Requirements {
    contract: "other",
    caps: &[],
    maps: &[],
    pages: 1,
    depends_on: &["a"],
    state_handoff: None,
};

/// Every arrangement a two-instance registry can be in, with respect to one target contract
/// `"a"`: an instance of the target itself, an unrelated instance that does not depend on it,
/// and an unrelated instance that does. Three, so "for every registry of this width" is exact.
const REG_SHAPES: [Requirements; 3] = [TARGET_SELF, OTHER_NODEP, OTHER_DEP];

/// Membership, decided by `core`'s string equality rather than by this crate's `str_eq`.
/// The distinction is the whole point of the helper: see the note in
/// `dependents_finds_exactly_the_non_target_instances_that_declared_it`.
fn declares_by_core_eq(depends_on: &[&str], name: &str) -> bool {
    let mut i = 0;
    while i < depends_on.len() {
        if depends_on[i] == name {
            return true;
        }
        i += 1;
    }
    false
}

/// **`dependents` returns exactly the non-target instances that declared the target, in the
/// order they were given, and nothing else.**
///
/// This is the property the whole mechanism rests on: a supervisor that trusted this function to
/// find every component with a synchronous dependency on the one it is about to swap must not be
/// told fewer than exist (a component left unwarned mid-swap) or more (a component quiesced for
/// no reason). Proved over every arrangement two live instances can be in, at every id a
/// supervisor could have chosen for them.
/// Falsification: replayable `crates/capability_plan/falsifications/proofs.dependents_finds_exactly_the_non_target_instances_that_declared_it.patch`
#[kani::proof]
#[kani::unwind(10)]
fn dependents_finds_exactly_the_non_target_instances_that_declared_it() {
    let ids: [u64; 2] = [any_slot(), any_slot()];
    for r0 in &REG_SHAPES {
        for r1 in &REG_SHAPES {
            let live = [
                LiveInstance {
                    id: ids[0],
                    reqs: r0,
                },
                LiveInstance {
                    id: ids[1],
                    reqs: r1,
                },
            ];
            let d = dependents("a", &live).expect("two instances is under MAX_LIVE");
            // **The expectation is computed with Rust's own `==`, never with `str_eq`**
            // (milestone 211). `dependents` decides membership by calling `str_eq`, so an
            // expectation that called it too would prove the two agree rather than that
            // either is right: a `str_eq` stuck at `true` would make `dependents` return
            // every instance and make this harness expect every instance.
            let expect0 = r0.contract != "a" && declares_by_core_eq(r0.depends_on, "a");
            let expect1 = r1.contract != "a" && declares_by_core_eq(r1.depends_on, "a");
            let out = d.quiesce_order();

            let mut want_len = 0;
            if expect0 {
                want_len += 1;
            }
            if expect1 {
                want_len += 1;
            }
            assert!(out.len() == want_len);

            // Order is the order `live` was given in: index 0's id, if present, precedes
            // index 1's.
            let mut idx = 0;
            if expect0 {
                assert!(out[idx] == ids[0]);
                idx += 1;
            }
            if expect1 {
                assert!(out[idx] == ids[1]);
            }
        }
    }
}

/// **A registry past `MAX_LIVE` is refused rather than silently truncated**, for every length
/// one past the bound could conceivably be constructed at (the proof only needs one, since the
/// check is a single `>` against a fixed constant, but the point of proving it at all is that a
/// truncated answer here is the dangerous failure: a supervisor that got fewer dependents back
/// than exist would swap out from under an unwarned client).
/// Falsification: replayable `crates/capability_plan/falsifications/proofs.too_many_live_instances_never_silently_truncates.patch`
#[kani::proof]
fn too_many_live_instances_never_silently_truncates() {
    let entries: [LiveInstance; MAX_LIVE + 1] = [LiveInstance {
        id: 0,
        reqs: &OTHER_DEP,
    }; MAX_LIVE + 1];
    assert!(matches!(
        dependents("a", &entries),
        Err(OrchestrationRefusal::TooManyLive { asked }) if asked == MAX_LIVE + 1
    ));
}
