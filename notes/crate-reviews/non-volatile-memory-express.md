# Review package: `non_volatile_memory_express`

*Name: provisional, minted by lane maintainer/queue-entry-lint-guards. The `non_volatile_memory_express`
package of the index [crate-reviews.md](../crate-reviews.md), journey 5 step 5.*

Name and version. `non_volatile_memory_express` `0.1.0`, ratified with NVMe spelled out. Every
`Name:` block reads ratified.

API. Ten modules behind a flat public surface: `registers`, `Capabilities`, `Command` with the
opcodes, `Completion`, `SubmissionQueue` and `CompletionQueue`, `Doorbell`, `doorbell`,
`PhysicalRegionPagePair` with `physical_region_page_pair`, `Namespace` and the identify parse. The
caller supplies MMIO and DMA memory. `SubmissionQueue::push` returns `None` on a full ring.
`Completion`, `IdentifyNamespace` and `Namespace` are `#[non_exhaustive]`. The README is the crate
documentation, so its usage example runs as a doctest. The process plan lives in
`non_volatile_memory_express_contract`, not here.

Proofs. Eight harnesses (submission push, completion pop and phase, doorbell collisions, PRP pair,
transfer command, identify parse, capabilities decode, completion decode). All eight prove and all
eight patches fail.

Dependencies. None.

Provenance. Nothing outstanding.

Result of `script/crate-standalone`. No failure. Build, names,
proofs and patches pass, and no citation names this tree.

Outstanding before the move.

| item | owner |
|---|---|
| `design/decisions/0086-el0-nvme-driver/prior-art.md` and milestone 569 (the disk an installer names has no model, only a size) still cite `prp_pair`, `CNS_CONTROLLER` and `CNS_NAMESPACE`. | maintainer |

Recommendation. The architect cleared the move, and has not cleared the tag or publication. The
repository `nifeos/non_volatile_memory_express` exists, and `kernel`, `components` and
`driver_process_contract` pin it by revision.
