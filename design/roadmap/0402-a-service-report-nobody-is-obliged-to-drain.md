---
status: NOT-STARTED
raised: 2026-09-14
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 402. A service report nobody is obliged to drain

Filed 2026-09-14 as an unnumbered proposal by milestone 290, which hit one
instance, fixed that instance, and proposed the general remedy rather than pretending one caller was
the problem. Numbered 2026-09-19 by milestone 433 (drain the proposal pile). Premise re-read
against the tree on 2026-09-19 and still true. `kernel/src/user/entropy_service.rs`'s `ensure`
still returns a `Wiring` whose `ready` is `Some` for the first caller on a bus and `None` for every
later one, the announcement is still a blocking send, and dropping the `Wiring` still compiles.
`entropy_service::ensure` has callers in six kernel test and service modules (`disk_tests`,
`credential_tests`, `entropy_tests`, `identity_provisioning_tests`, `ntp_tests` and `std_service`),
any of which can become the first caller when a filter changes what runs.
*(Number provisional until the merge queue lands it.)*

A lane can close this. It is kernel-side test wiring with no system call surface, no wire
format and no new name.

## The trap

`kernel/src/user/entropy_service.rs`'s `ensure` wires the service once per boot and hands the
first caller a `Wiring` carrying a `ready` endpoint:

```rust
pub fn ensure(image: &'static [u8], bus: Bus) -> Option<Wiring> {
    if WIRED[i].load(Acquire) {
        return Some(Wiring { ready: None, request: ..., ... });   // a later asker
    }
    let w = start(image, bus)?;                                    // this one owns `ready`
    ...
}
```

The service announces itself with a **blocking** send. So the first caller is under an obligation the
type does not express: drain `ready`, or the service parks inside its own startup and never reaches
its request loop. Every later `ensure` gets `ready: None` and cannot rescue it.

**Nothing says so.** `Wiring::wait_for_ready` is a method a caller may ignore, `Wiring` is
`#[must_use]`-free as far as this matters, and dropping it compiles.

## What it cost, measured

`kernel/src/user/ntp_tests.rs`'s `machine_has_no_entropy()` called `ensure` and discarded the result.
Running any NTP exchange test on its own then hung: the client blocked forever in
`call(ENTROPY, ...)` and the failure surfaced two frames away as *"the test server never saw a
request: the client failed before it reached the network"*.

It was invisible in every whole-suite run, because `entropy_tests` sorts before `ntp_tests` and drains
the report first. One test file was correct only because of another test file's name. Reproduced
at `3c156f82`; fixed in `ntp_tests.rs` by milestone 290.

## Why one fix is not the fix

`git grep 'entropy_service::ensure'` finds callers in several test modules, and any of them can become
the first caller when a filter changes what runs. The same hazard exists wherever a kernel-side
service helper hands out a report backed by a blocking send; `entropy_service` is simply the one that
has now bitten.

## Options

**Make `ensure` drain a report its caller did not ask for.** The strongest and smallest: `ensure`
takes the `ready` endpoint, receives on it itself, and returns the report alongside the wiring (or
stores it for `wait_for_ready` to hand back later). Then no caller can get it wrong, which is rung one
of AGENTS.md's ladder: the wrong state stops being representable. The cost is that `ensure` blocks
where today it returns, and a caller that wanted to interleave the bring-up with other work no longer
can. Nothing in the tree wants that today; check before assuming it.

Make the obligation visible in the type. Return the `ready` endpoint as a value that must be
consumed, so dropping it is a compile error rather than a hang. More machinery than the first option
and it only moves the failure from runtime to a lint the author has to satisfy.

Refused: document it. That is rung four, and the comment would sit in `entropy_service.rs` where
the *caller* is not looking. It is how this one survived.

## What would close it

`script/test --arch aarch64 --test <each entropy or ntp test name>`, run one at a time, all green,
plus the full suite unchanged on all three architectures. The one-at-a-time run is the whole point:
the defect is invisible to any run that includes `entropy_tests`.

## Index row

`kernel/src/user/entropy_service.rs`'s `ensure` wires the service once per boot and hands the first
caller a `Wiring` carrying a `ready` endpoint, and the service announces itself with a blocking
send. So the first caller is under an obligation the type does not express: drain `ready`, or the
service parks inside its own startup and never reaches its request loop, and every later `ensure`
gets `ready: None` and cannot rescue it. Nothing says so, `wait_for_ready` is a method a caller may
ignore, and dropping the `Wiring` compiles. What it cost was measured rather than imagined:
`ntp_tests`'s `machine_has_no_entropy()` discarded the result, so running any NTP exchange test on
its own hung, with the failure surfacing two frames away as the test server never seeing a request.
It was invisible in every whole-suite run because `entropy_tests` sorts before `ntp_tests` and
drains the report first, which means one test file was correct only because of another test file's
name. One fix is not the fix, because any of the six modules calling `ensure` can become the first
caller when a filter changes what runs. The strongest option is rung one: `ensure` receives on
`ready` itself and returns the report alongside the wiring, so no caller can get it wrong, at the
cost of blocking where it returns today. Documenting it is refused as rung four, in the file the
caller is not reading, which is how this one survived. What closes it is each entropy and NTP test
run one at a time, which is the only kind of run the defect is visible to.
