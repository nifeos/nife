# The login contract: the reasons its constants link

Name: provisional, for the crate whose comments link it. *Name: provisional, minted 2026-10-24 (UTC)
by the milestone 929 (comments in the Linux style across first-party Rust) lane
comments/929-components-02. The directory follows §212 (a prose budget)'s siting default, `notes/X/`
beside `notes/X.md`. calef names directories.

`crates/login_contract` is the wire contract between a client and the login service, and the start
contract between `login` and whoever spawns it. [login.md](../login.md) has the design. This page
holds the reasons the crate's doc comments point at.

## Why a full logout destroys the budget first

A successful login delegates the client's budget (the third capability) and its logout ticket (the
fourth, the caretaker's construction region). `mint` splits the ticket and then the budget back to
back from login's construction budget. LIFO reclaim returns a child's pages only from the top of the
watermark. Destroying the ticket first would strand its pages under the budget's, so a full logout
destroys the budget and then the ticket.

## Why the start contract lives in a client contract's crate

Three binaries must agree on the addresses `login` finds its start blobs at:
`components/src/login.rs`, `crates/system_initializer`, and the kernel's own test harness in
`kernel/src/user/login_service.rs`. What two binaries agree on is a crate (rule 7 of AGENTS.md),
and rule 7 leaves nowhere else. The siting is the weak point. The crate's first line calls it the
wire contract between a client and the login service, and a spawn contract is neither wire nor
client. The alternative was a crate holding two constants.

## Why `login` is handed one program, not the boot archive

`login` gets the caretaker's ELF bytes mapped at a fixed address, not the initial ramdisk archive.
`crates/system_initializer` spawns it through `supervision_protocol::build_child`, which can map
only pages the spawner holds a `PageFrame` capability for. The archive is reserved RAM that the
frame allocator does not own and no capability names. Milestone 233 (`login` dies on every boot) records the
boot that died at `_start` because of it.

Giving `login` the archive is the refused option. It needs one program's bytes and a manifest to
check them against. A service that can read every file in the boot image to answer a password holds
authority it never exercises.

A blob costs the spawner no capability-table slots at all: `supervision_protocol`'s own
`fill_and_map` holds one frame at a time and deletes it. That is what let it fit against the
21-of-24 peak that `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` records.
