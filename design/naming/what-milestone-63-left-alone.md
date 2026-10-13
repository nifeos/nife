# What milestone 63 did not rename

*An appendix to [`design/naming.md`](../naming.md), which is the rule. This file holds the names
milestone 63 (directory and package names) left in place on purpose, so the next reader does not
"fix" one by mistake. It exists to verify or challenge the main page, and a reader who only needs to
name, ratify or rename something should not have to open it. The directory `design/naming/` and this
file's stem are provisional names; naming is
calef's.*

## BUGS

What milestone 63 did not rename, each on purpose, so the next reader does not "fix" one of them by
mistake.

- `design/capsicum-and-the-retrofit-question.md`'s honest comparison is a present-tense claim that
  still spells names milestone 175 (split `user/`) retired: the system confines "`worker`,
  `memory_grant_depleter`, `heeder`, `spinner`, a C component, and a filesystem we vendored".
  `worker`, `heeder` and `spinner` are open here, for whoever next has the standing to edit that
  line. A stale record gets fixed when something else brings a writer to the line, not when
  somebody schedules a pass over it, which is why this entry names the line rather than filing a
  task. Every other occurrence of the old names in `design/` is a dated narrative and correctly
  keeps them.

- The boot mode is still called `shell`, and the program is `swish`. `cargo xtask shell` and the
  kernel's `--features shell` name a *configuration* (boot straight to a prompt, milestone tour
  compiled out), not the binary. Renaming them would have been a naming decision nobody made. The
  cost is that a reader who greps `shell` in `xtask` and in `kernel/Cargo.toml` meets a word that
  no longer names a program.
- `caps` is still a shell builtin, and it no longer shares a name with anything. It was the larger
  half of the 285 occurrences of `caps` in the tree before the rename, and it means "print this
  process's endowment". The crate that used to share the spelling is `capability`.
- A rename can reach into the vendored tree, and `script/supply-chain` is what says so. Our one
  divergence comment in `vendor/redoxfs/Cargo.toml` names `redoxfs_host`. So the rename had to touch
  the vendored file and `vendor/redoxfs.divergence.patch` together, or `script/vendor-verify` fails
  with "differs from upstream+patches". Milestone 63 changed the patch first and the vendored file
  not at all, and the gate caught it. Edit both, in the same commit.
- The measured-boot manifest is still `target/init-measure-<arch>.txt`. It is a build artifact name,
  not a crate reference; `kernel/build.rs` reads it and turns it into `TRUST_ROOT`.
- Note filenames did not move, and that is the rule rather than an oversight.
  [fs-server.md](../../notes/fs-server.md), [shell.md](../../notes/shell.md),
  [shell-navigation.md](../../notes/shell-navigation.md) and
  [line-discipline.md](../../notes/line-discipline.md) are markdown. They stay lowercase-hyphenated
  even though the things they describe are now `redoxfs_server`, `swish` and `line_editor`.
