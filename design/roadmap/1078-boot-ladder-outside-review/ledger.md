# The ledger

An appendix to [the milestone](../1078-boot-ladder-outside-review.md).

| item | state |
|---|---|
| 1 | done: the manifest states its license, minimum Rust version, description, repository, readme and the template's `[lints]` as literals, and no comment points outside the crate |
| 2 | partly done: the README (with `EXAMPLES`, `BUGS`, the matching rule and the version policy) and both license files exist; `CHANGELOG.md`, `_typos.toml` and `docs/stranger-test.md` are the move's scaffold and wait for it |
| 3 | done: no doc comment names a milestone, a nife path, a note, a rule number, a pull request or a person; the earlier list lengths are keyed by the names each list held; the design narrative is in the README |
| 4 | done: `verdict::ParsedVerdict` reads what `verdict::Verdict` prints, and `serial_console` and `xtask` read the verdict through it; a line that is not the grammar but says `FAILED:` is still read as red |
| 5 | done as a test, not a `const` block: `the_check_names_are_plain_and_distinct` holds the names non-empty, unique and in `[a-z0-9_]` and the earlier lengths ascending below the current one; a `const` block drew five mutation survivors that no test can kill |
| 6 | done: the head-shape and check-list tests, the round trip of the verdict, and a Kani harness for `parse` with a replayable falsification patch |
| 7 | partly done: the literals are gone from `xtask` and read the crate's heads; the `script/lint` grep is left because a new lint section is a reviewed interface, and `script/lint` does not gate it yet |
| 8 | done as provisional: `tour` is `core_runs`, `prompt` is `shell_banner` and the earlier-counts list is `EARLIER_SELF_TEST_CHECK_COUNTS`, with `serial_console`'s stages and the `TRUE_RANDOM_NUMBER_GENERATOR_` constants following; the pass over the `self_test_program` names is listed below and not applied, since those names are not decided |
| 9 | done by the heads change: the crate states the matching rule and that the banner matches whole; the architecture is read from the machine line |
| 10 | done by the check-names ruling; the kernel's name column is still ten wide, which the README's `BUGS` records |
| 11 | done: the README states the version policy; no identity constant exists |
| 12 | left: waits on milestone 972 (the boot stick's shared names are code) and the two hoists |
| 13 | done by the heads change: the crate states that the tail of the machine line is free text and the architecture is read from it |

## The `self_test_program` names, proposed and not applied

The module holds provisional names that no ruling covers. Read against the naming rules, each spells
its words out and none is an abbreviation. One name is proposed for a change, and one question waits for the move:

- `DIRECT_MEMORY_ACCESS_PEEK_VIRTUAL_ADDRESS` says `PEEK` where the page it addresses is one the
  program reads the device's view of; `DIRECT_MEMORY_ACCESS_OBSERVER_VIRTUAL_ADDRESS` would say who
  reads it. Left as it is.
- Which of the module's items stay public once the self-test program leaves the kernel's repository
  is a question for that move.
