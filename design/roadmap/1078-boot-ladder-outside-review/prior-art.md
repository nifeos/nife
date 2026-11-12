# Prior art

An appendix to [the milestone](../1078-boot-ladder-outside-review.md).

How the crate sits against what a stranger knows. Each row says whether this review read it or recalled it.

| system | how the boot or test lines are agreed | shared as | read or recalled | what nife takes, and where it differs |
|---|---|---|---|---|
| Linux KTAP and kselftest | a specified text grammar: `KTAP version 1`, a plan `1..N`, `ok N name`; writer in `kselftest.h` and KUnit, parser in `tools/testing/kunit/kunit_parser.py` | a document, no shared code | recalled | the declared total (`X of Y`) and a name per check; nife shares a crate, not a spec, and prints one verdict line instead of one line per test |
| systemd `sd_notify` | `READY=1`, `STATUS=` messages; `sd_notify` in libsystemd writes them and PID 1 parses them | one library both sides link | recalled | the shape of rule 7: one definition, two binaries |
| U-Boot `bootstage` | stage identifiers in one header; `bootstage report` prints them; tools read the same enum | a header both sides include | recalled | the rung idea; nife's rungs are lines a person reads, not identifiers |
| seL4 `sel4test` | one fixed closing sentence the CI harness matches | a literal on both sides | recalled | the fixed sentence; nife puts it in a crate |
| Zephyr `twister` console harness | per-test regex lists in YAML, plus `PROJECT EXECUTION SUCCESSFUL` | literals on the reader side | recalled | the reader-side literal is the failure item 7 finds in `xtask` |
| QEMU functional tests, LAVA | `wait_for_console_pattern(...)`, `boot_message` per device type | literals on the reader side | recalled | same |
| crates.io | no crate holds one system's boot-line contract; TAP producer and consumer crates exist | n/a | unverified | this crate has no direct peer; its nearest kin is a TAP writer with a frozen grammar |

Where this crate differs from all of them: it ships the printer's and the readers' shared definition as a dependency, so a change is a version bump both sides see. It also carries the list of check names, so a reader can refuse a verdict whose total is not the list's length. What it
lacks today, and item 4 adds, is the one thing KTAP's parser and `sd_notify` both have: the
grammar as code, not only the heads.
