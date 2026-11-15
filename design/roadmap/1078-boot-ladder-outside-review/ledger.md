# The ledger

An appendix to [the milestone](../1078-boot-ladder-outside-review.md).

| item | state |
|---|---|
| 1, 2 | left: the move's |
| 3 | partly done: `kernel_half`, `self_test_program`, `verdict` and the manifest comment carry no milestone, note path or rule number; `lib.rs` and `rungs.rs` read as the heads change left them and still cite a note and a milestone, which the move's rewrite takes; the README is the move's |
| 4 | left: the writer shares a `Verdict` type already, but the readers in `serial_console` and `xtask` still parse by hand, and a `parse` with its harness touches both and needs the kernel suite |
| 5 | done as a test, not a `const` block: `the_check_names_are_plain_and_distinct` holds the names non-empty, unique and in `[a-z0-9_]` and the earlier lengths ascending below the current one; a `const` block drew five mutation survivors that no test can kill |
| 6 | partly done: `the_check_names_are_plain_and_distinct`; the head-shape tests went with the heads into `Heads`, and the round trip and the `parse` harness wait for item 4 |
| 7 | done for the three literals that remained, now `boot_ladder::PROMPT` in `xtask/src/swish_check`; the `script/lint` grep is left because a new lint section is a reviewed interface |
| 8, 10 | left: decisions 2 and 3 |
| 9 | done by the heads change: the crate states the matching rule and that the banner matches whole; the architecture is read from the machine line |
| 11, 12 | left: item 11 is the README's; item 12 waits on milestone 972 (the boot stick's shared names are code) |
| 13 | done by the heads change: the crate states the matching rule and that the banner matches whole; the architecture is read from the machine line |
