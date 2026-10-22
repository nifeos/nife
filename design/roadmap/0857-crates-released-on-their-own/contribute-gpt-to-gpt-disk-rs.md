# Contribute the GPT proofs to gpt-disk-rs instead of releasing ours?

Appendix to [857. Crates released on their own](../0857-crates-released-on-their-own.md), at review
item 3.

This question is open for calef. Nothing is ruled.

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

An offer goes upstream only under 857's fork 8 and its approval cadence: disclosed as agent-written,
and approved by calef before it is sent.
