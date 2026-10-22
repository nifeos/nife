# Contribute our proofs upstream instead of releasing our own?

Appendix to [857. Crates released on their own](../0857-crates-released-on-their-own.md), at review
items 3 and 4. Both questions are open for calef. Nothing is ruled.

## Item 3: the GPT crate and google/gpt-disk-rs

Should nife contribute `globally_unique_identifier_partition_table`'s Kani proofs, and whatever
`gpt_disk_types` lacks, upstream to google/gpt-disk-rs instead of releasing its own crate, and
switch to `gpt_disk_types` if they accept?

- For: milestone 784 (reuse what the tree wrote)'s survey row reads "yes: `gpt_disk_types`. 857
  says contribute, which fits"
  ([the rest of the tree](../0784-reuse-what-the-tree-wrote/the-rest-of-the-tree.md)).
  `gpt_disk_types` 0.16.1 is MIT or Apache-2.0, `no_std`, and Google's repository takes outside
  work.
- Against, as things stand: 784's recorded reason for keeping ours. The crate is proved (8 Kani
  harnesses, its CRC restructured for the model checker), which a dependency would lose unless the
  proofs go with it, and the installer and the loader must agree byte for byte
  ([recorded reasons](../0784-reuse-what-the-tree-wrote/recorded-reasons.md)).

## Item 4: `elf` and cole14's `elf`

Should nife contribute the `elf` crate's Kani proofs, and whatever cole14's `elf` 0.8.0 lacks,
upstream instead of releasing its own? The `elf` name on crates.io is theirs.

- For: 784's survey row reads "857 says contribute to `elf`, which fits"
  ([the rest of the tree](../0784-reuse-what-the-tree-wrote/the-rest-of-the-tree.md)). `elf` 0.8.0
  is MIT or Apache-2.0, `no_std`, with no dependencies; its last release was 2025-05-14.
- Against, as things stand: the kernel loads program images through ours, so §46 (thin primitives
  or whole subsystems)'s kernel exemption keeps it, and its 8 harnesses prove the loader's front
  half panic-free on hostile input. 784 records `goblin` and `xmas-elf` as losing to our proved
  `elf`, and gives no other reason against `goblin`.

## For both

An offer goes upstream only under 857's fork 8 and its approval cadence: disclosed as agent-written,
and approved by calef before it is sent.

Whether an upstream accepts agent-written patches decides a contribute verdict, under 784's
AI-submissions criterion. Neither google/gpt-disk-rs nor cole14/rust-elf has a written policy (read
2026-10-22 UTC, in
[784's appendix](../0784-reuse-what-the-tree-wrote/upstreams-against-the-fork-test.md)), so neither
question can become an offer. Asking either maintainer is calef's act. google/gpt-disk-rs also
requires Google's CLA, which a person signs.
