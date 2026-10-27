---
status: PROPOSED
raised: 2026-10-27
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The installer's spawn and report words are one definition

Raised by the 2026-10-27 documentation sweep
([its report](../../audit-reports/2026-10-27-the-fat-crate-read-from-the-outside.md), finding 6).
Name: provisional, for this file and for every crate and module name it sketches.

Reuse: the shape `hot_swap_contract` and `confined_syscall_fuzzer_contract` already use, one crate
holding a spawned program's roles and verdicts, which the kernel and the program both depend on.

## The failure

The kernel's install offer (`kernel/src/user/install_service.rs`) spawns `system_installer`
(`components/src/system_installer.rs`) three times a boot at most, and the two sides agree on five
things that each spells for itself:

| agreement | the kernel's spelling | the program's spelling |
|---|---|---|
| the roles in `a0` | `ROLE_INSTALL`, `ROLE_SURVEY`, `ROLE_CONFIRM` (0, 1, 2) | the same three names and values |
| what `a1` carries | `spawn_system_installer`'s comment: the boot file's length, the slot number, nothing | `start`'s comments |
| the capability slots | `SLOT_REPORT` to `SLOT_BLK_PAGE` (0 to 4) | `REPORT`, `BLK`, `ENTROPY`, `BUDGET`, `BLK_PAGE_FRAME` |
| where the boot file is mapped | `BOOT_FILE_VA` | `BOOT_FILE_VA` |
| the report's first word | `R_INSTALLED`, `R_ALREADY`, `R_CONFIRMED`, `R_NO_SLOT` | those four and five more the kernel prints only as hex |

Each side carries a "Must match" comment, which is the whole of what keeps them in step. A changed
role value or slot on one side builds, boots, and fails at the first install or confirm, and the
install gate (`cargo xtask install-boot`) runs on no schedule (milestone 712, the install gates run nowhere). `install_service.rs`
records a reason for the duplication: the words are the kernel's own convention with a program it
spawns. Rule 7 of `AGENTS.md` and ruling 55a (2026-10-26 UTC) both say a word two binaries agree on
is code, and `hot_swap_contract` is the tree's existing answer to the same kind of convention.

`install_service.rs` also spells `mkfs`'s `ROLE_MAKE` and `R_MADE`, which `disk_service` and
`disk_tests` spell again. That is the same failure with a third program, and it belongs in the same
crate or a sibling one.

## The fix

A contract crate, provisionally `system_installer_contract`, that both sides depend on:

```rust
#![no_std]
/// What `a0` asks for.
#[repr(u64)]
pub enum Role { Install = 0, Survey = 1, Confirm = 2 }

/// The capability table the kernel builds and the program reads.
pub mod slot {
    pub const REPORT: u64 = 0;
    pub const BLK: u64 = 1;
    /// Granted to `Role::Install` only.
    pub const ENTROPY: u64 = 2;
    pub const BUDGET: u64 = 3;
    pub const BLK_PAGE: u64 = 4;
}

/// Where the kernel maps the boot file, read-only, for `Role::Install`.
pub const BOOT_FILE_VA: u64 = address_space_map::runtime_window(0x1000_0000);

/// The report's first word, one variant per verdict, with its second word where it has one.
pub enum Verdict {
    Installed { data_first: u64, esp_first: u64 },
    NoEntropy,
    TooSmall { bytes: u64 },
    DiskFailed { step: u64, detail: u64 },
    NoBootFile,
    Already { data_first: u64 },
    Empty,
    Confirmed { slot: u64, written: bool },
    NoSlot { found: u64 },
}

impl Verdict {
    pub fn encode(&self) -> [u64; 3] { /* the words `send` takes */ }
    pub fn decode(words: [u64; 3]) -> Option<Verdict> { /* `None` for a word nobody sends */ }
}
```

`system_installer` sends only through `Verdict::encode`, and `install_service` matches on
`Verdict::decode`, so the kernel prints every refusal by name rather than as hex. A host test
round-trips every variant, and a test asserts that no two first words collide.

## Exit test

`git grep -n "Must match" kernel/src/user/install_service.rs components/src/system_installer.rs`
finds nothing, neither file defines a role, slot or verdict constant, and `cargo xtask
install-boot` passes.

## The class beyond the installer

"Must match", "must agree" and "the two must" mark a contract stated in prose, and on 2026-10-27
(UTC) they appear on 207 lines in 117 Rust files outside `vendor/`. A count held to its baseline in
`script/lint`, the way `helpers/prose_ratchet.py` holds passives, is the gate that keeps the class
from growing. This milestone adds that ratchet and lowers it by the installer's lines; each other
hit is its own owner's work.
