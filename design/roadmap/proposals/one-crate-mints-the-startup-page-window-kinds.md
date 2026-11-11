---
status: PROPOSED
raised: 2026-11-11
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# One crate mints the startup page's window kinds

Raised by lane/maintainer-follow-ups, at the maintainer's request, while reading milestone 1069 (a process reads its window addresses from a startup page).

Name: provisional. The crate is called `window_kinds` here; the name is the lane's.

Reuse: `startup_page_contract::WindowKind` and the uniqueness test that `driver_process_contract`'s `window_kinds::ALL` already carries. No new tool.

## The problem

A window kind is a number that both the spawner and the component link, so the agreement is code §311 (a contract is code, not prose). Today `driver_process_contract` mints eleven kinds, 1 to 11, in one numbering space, and a test holds its eleven distinct from each other.

Milestone 1069 converts the remaining components, about 37 files in `components/src/`, from a literal address to `startup_page::window(KIND)`. Each needs kinds. The startup page contract's `BUGS` says each contract crate mints the kinds of the windows it describes and nothing checks numbers across crates. Two crates that each mint a kind 12 would compile, and would clash the first time one page lists both programs' windows, which is the case the page exists for.

Putting the new kinds in `driver_process_contract` fails on dependencies: a keyboard driver or a credentialer has no reason to link the network and storage handoffs.

## The work

One small crate holds every kind that is not a driver's, in one numbering space that starts after the drivers' eleven.

1. A crate `window_kinds` with one `pub const NAME_KIND: WindowKind` per window and an `ALL` array of them, each documented with the window it names.
2. A test in that crate, with `driver_process_contract` as a dev-dependency, that the kinds of `ALL` and of `driver_process_contract::window_kinds::ALL` are pairwise distinct, so one number never means two windows. A new crate that mints a kind elsewhere does not pass this test, so the crate's header says every kind goes here.
3. A ratchet in `script/lint`, beside the one for literal window addresses, that counts `WindowKind(` constructions outside `startup_page_contract`, `driver_process_contract` and this crate, and only lets the count fall to zero.
4. The `BUGS` entry in `startup_page_contract` names this crate as the one home and drops the sentence that nothing checks across crates.

Sketch of the check in item 2:

```rust
#[test]
fn every_kind_is_distinct() {
    let all: Vec<u32> = window_kinds::ALL
        .iter()
        .chain(driver_process_contract::window_kinds::ALL.iter())
        .map(|kind| kind.0)
        .collect();
    let mut sorted = all.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), all.len());
}
```

## Open questions

- Whether the drivers' eleven kinds move into the new crate too. Leaving them costs nothing and keeps `driver_process_contract` a self-contained handoff contract; moving them gives one list and one home. This proposal leaves them, and the test covers both lists.
- The crate's name.

## Prior art

Linux numbers every auxiliary-vector entry type in one header, `include/uapi/linux/auxvec.h` (`AT_` constants), so no subsystem can reuse a number. Fuchsia does the same for its process-argument handle types, in one header of `PA_` constants (`zircon/processargs.h`). Both keep one list and let any number of programs read it. Where nife differs: the numbers live in a Rust crate, so a duplicate is a failing test and a literal outside the crate is a lint count, not a code review.

## Index row

Each crate that converts to the startup page would mint its own window kinds and nothing checks the numbers across crates. One crate mints the non-driver kinds, and a test holds them distinct from the drivers' eleven.
