# The ledger

An appendix to [the milestone](../1078-boot-ladder-outside-review.md).

| item | state |
|---|---|
| 1, 2 | left: the move's |
| 3 | done in the crate's documentation and manifest comment: no path, milestone, rule number, pull request or person outside a `Name:` block remains in `lib.rs`, `rungs.rs`, `kernel_half.rs`, `self_test_program.rs`, `verdict.rs` or `Cargo.toml`; the earlier-lengths table is keyed by what each list held; the README is the move's |
| 4 | left: the writer shares a `Verdict` type already, but the readers in `serial_console` and `xtask` still parse by hand, and a `parse` with its harness touches both and needs the kernel suite |
| 5 | done: a `const` block holds the check list non-empty, unique and in `[a-z0-9_]`, and the earlier lengths below the current one and ascending; `the_check_names_are_plain_and_distinct` repeats it at run time |
| 6 | done for the shape: `every_head_but_the_shell_banner_ends_on_a_boundary`, `no_head_is_inside_another`; the round trip and the `parse` harness wait for item 4 |
| 7 | done for the three literals that remained, now `boot_ladder::PROMPT` in `xtask/src/swish_check`; the `script/lint` grep is left because a new lint section is a reviewed interface |
| 8, 10 | left: decisions 2 and 3 |
| 9 | done: the crate doc states the matching rule and that the shell banner matches whole; decision 1 keeps the period out of `PROMPT` |
| 11, 12 | left: item 11 is the README's; item 12 waits on milestone 972 (the boot stick's shared names are code) |
| 13 | done: the doc says the banner's tail is free text and a reader takes the architecture from `MACHINE`'s tail |
