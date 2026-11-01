# The contract crates leave in this order: the prior art

[The proposal](../the-contract-crates-leave-in-this-order.md) holds the order and the policy. This
appendix holds what this lane read on 2026-11-01 (UTC) about how four projects split, version and
change interfaces across repositories, with the source beside each lesson. A claim recalled rather
than read says so. Milestone 979 (interfaces a peer can check) keeps its own appendix on bind-time
checks in Plan 9, Fuchsia, seL4, Mach and Linux; this one does not repeat it.

## Fuchsia: layered repositories, the fold back, and petals

Read: the `//garnet` README at https://fuchsia.googlesource.com/fuchsia/+/bbc9ce36f629/garnet and
the project page at
https://fuchsia.dev/fuchsia-src/contribute/open_projects/graduated/layer_cake_deprecation.

Fuchsia once split its tree into layers (Zircon, Garnet, Peridot, Topaz), each a repository. The
README says the design "was deprecated in 2019 in favor of consolidating all of Fuchsia's open
source code in a single repository". The project page gives the reasons: "The distinctions between
layers weren't crystal clear", and "the cost/benefit of this arrangement wasn't compelling".

Read: https://fuchsia.dev/fuchsia-src/development/source_code/working_across_petals.

Fuchsia kept separate repositories, called petals, for code outside the platform, and an
integration repository rolls each to a pinned revision. A change across petals takes a soft
transition in five steps. Land the new interface in the dependency beside the old one. Wait for it
to roll into integration. Migrate the dependent. Wait for that to roll. Remove the old interface. A
hard transition, a break plus a manual update of the dependent, is allowed where a soft one is
impossible. The page calls it "more stressful" and limits it to Google developers.

Lesson for nife. The fold back came from boundaries that were not real: layers whose edges nobody
could state. A contract crate's edge is a dependency graph a script can print, which is why the
order here follows that graph. Fuchsia's soft transition is milestone 979's N-1 dual path for
message contracts. Its hard transition is the shared-page case, which nife lands as one coherent
basalt bump rather than limiting it to a group of people. Fuchsia's integration repository is
basalt.

## Android: stable AIDL

Read: https://source.android.com/docs/core/architecture/aidl/stable-aidl.

An owner freezes an AIDL interface into a numbered version under `aidl_api/`, with a `.hash` file,
and the build checks each frozen version against the next. Only additive changes are allowed: new
methods at the end, new parcelable fields with defaults, new constants and enumerators. The page
states: "No other actions are permitted, and no one else can modify an interface." At run time a
client may ask a server for `getInterfaceVersion()` and `getInterfaceHash()`. A call to a method an
older server lacks fails with `UNKNOWN_TRANSACTION`.

Lesson for nife. The frozen dump and the build-time comparison are what this proposal asks of the
crate template's semver rule, with the baseline at the last version-bumping commit. Android forbids
breaks outright; milestone 979's ruling of 2026-10-29 (UTC) permits them with a version bump and,
for message contracts, a one-generation dual path. The `UNKNOWN_TRANSACTION` refusal matches 979's
reading of an unknown-operation reply. nife differs in allowing the break; it keeps the refusal.

## Rust: Cargo's semver rules and `cargo-semver-checks`

Read: https://doc.rust-lang.org/cargo/reference/semver.html.

Cargo treats the left-most nonzero number as the incompatible one: "Initial development releases
starting with '0.y.z' can treat changes in 'y' as a major release, and 'z' as a minor release." So
`0.1` to `0.2` is a break and `0.1.0` to `0.1.1` is not. Removing or renaming a public item is
major; adding one is minor. The guide does not address a change to a public constant's value.

Read: https://github.com/obi1kenobi/cargo-semver-checks.

The tool lints a crate's API from rustdoc's JSON. Its baseline defaults to the last version on
crates.io, and `--baseline-rev`, `--baseline-root`, `--baseline-version` and `--baseline-rustdoc`
pick another. Its README says it does not yet catch type changes in fields or parameters, generic or
lifetime changes, or breakage under some feature combinations.

Lesson for nife. The Cargo minor carries a Rust-API break and nothing more. A constant's value is
outside both the guide and the tool, which is why milestone 956's diff check, failing a moved magic
without its version constant, has to travel with each crate. `--baseline-rev` exists, so the
template's rule can compare against a commit while a crate stays unpublished.

## Linux: no stable in-kernel API

Read: https://www.kernel.org/doc/html/latest/process/stable-api-nonsense.html.

Linux "does not have a binary kernel interface, nor does it have a stable kernel interface", for
code inside the kernel. It keeps the system call interface stable for programs. The remedy it gives
a driver author is to get the driver into the main tree, where a change to an internal interface
"will be fixed up by the person who did the kernel change in the first place".

Lesson for nife. Linux answers churn by keeping every consumer of an internal interface in one tree,
so a break is one commit. That is the monorepo answer the 2026-11-01 direction turns down for
contract crates, and it names the price nife accepts: a break becomes two landings. Linux draws the
stable line at the boundary a separate program sees. Of the 22, `abi` and the kernel's shared pages
(`block_roster`, `current_cpu_contract`, `machine_statistics_contract`) are that boundary; the rest
are agreements between programs. So the argument for keeping a crate in nife is strongest for
code internal to the kernel, and no crate of the 22 is that.

## Recalled, not read

- Rust's crater builds every public crate against a compiler change before it lands, to size the
  breakage. The trial run in the proposal's gate is that idea at the size of one pin set.
- Debian's transitions move a library and its reverse dependencies to a new soname together, through
  a release team. That is the coherent basalt bump, recalled from general knowledge.
