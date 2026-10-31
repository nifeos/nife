---
status: NOT-STARTED
promoted_from: the-installers-spawn-and-report-words-are-one-definition
raised: 2026-10-27
milestone_dependencies: 407
decision_dependencies: 187
machine_requirements: none
specific_machine: none
needs_person: no
---
# 973. The installer's spawn and report words are one definition

Raised by the 2026-10-27 documentation sweep
([its report](../audit-reports/2026-10-27-the-fat-crate-read-from-the-outside.md), finding 6).
Name: provisional, for this file and for every module name it sketches.

This proposal adds rows to a class the tree already tracks. Milestone 407 (one definition of the
numbers a kernel test and its program agree on) holds every pair of a kernel `*_service.rs` and a
program it spawns that keep their roles and report words in step with "must match" comments. Its
measurement does not list `install_service.rs` and `system_installer.rs`. §187 (one crate per
kernel-test pair, or one crate for all of them?) decides where the shared definitions live, so this
proposal sketches the contents and leaves the crate to that ruling. It can be promoted as a row of
407, or folded into it.

Reuse: the shape `hot_swap_contract` and `confined_syscall_fuzzer_contract` already use, roles and
verdicts in a crate that the kernel and the program both depend on.

## The failure

The kernel's install offer (`kernel/src/user/install_service.rs`) spawns `system_installer`
(`components/src/system_installer.rs`) at most three times a boot, and the two sides agree on five
things that each spells for itself:

| agreement | the kernel's spelling | the program's spelling |
|---|---|---|
| the roles in `a0` | `ROLE_INSTALL`, `ROLE_SURVEY`, `ROLE_CONFIRM` (0, 1, 2) | the same three names and values |
| what `a1` carries | `spawn_system_installer`'s comment: the boot file's length, the slot number, nothing | `start`'s comments |
| the capability slots | `SLOT_REPORT` to `SLOT_BLK_PAGE` (0 to 4) | `REPORT`, `BLK`, `ENTROPY`, `BUDGET`, `BLK_PAGE_FRAME` |
| where the boot file is mapped | `BOOT_FILE_VA` | `BOOT_FILE_VA` |
| the report's first word | `R_INSTALLED`, `R_ALREADY`, `R_CONFIRMED`, `R_NO_SLOT` | those four and five more the kernel prints only as hex |

Each side carries a "Must match" comment, and nothing else keeps them in step. A changed role
value or slot on one side builds, boots, and fails at the first install or confirm, and the
install gate (`cargo xtask install-boot`) runs on no schedule (milestone 712 (the install gates
run nowhere)). `install_service.rs` records a reason for the duplication: the words are the
kernel's own convention with a program it spawns. Rule 7 of `AGENTS.md`, and the method ruled
2026-10-26 UTC that a contract is code and not prose, both say otherwise.

`install_service.rs` also spells `mkfs`'s `ROLE_MAKE` and `R_MADE`, which milestone 407's table
already lists for `disk_tests`.

## The fix

The definitions, in whichever crate §187 chooses:

```rust
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

/// The report's first word, one variant per verdict, with the words after it.
pub enum Verdict {
    Installed { data_first: u64, esp_first: u64 },
    NoEntropy,
    TooSmall { bytes: u64 },
    DiskFailed { step: u64, detail: u64 },
    NoBootFile,
    Already { data_first: u64 },
    Empty,
    Confirmed { slot: u64, written: bool },
    /// Word 1 is how many boot slots the disk holds; word 2 is the slot asked for, or 0 when the
    /// table was refused before the count finished.
    NoSlot { found: u64, asked: u64 },
}

impl Verdict {
    pub fn encode(&self) -> [u64; 3] { /* the words `send` takes */ }
    pub fn decode(words: [u64; 3]) -> Option<Verdict> { /* `None` for a word nobody sends */ }
}
```

`system_installer` sends only through `Verdict::encode`, and `install_service` matches on
`Verdict::decode`, so the kernel prints every refusal by name rather than as hex. A host test
round-trips every variant, and a test asserts that no two first words collide.

## How big the class is

The phrases "Must match", "must agree" and "the two must" appear on 207 lines in 117 Rust files
outside `vendor/` when the search ignores case, and on 173 lines in 95 files when it does not
(2026-10-27 UTC). Many hits are not contracts between programs: test oracles, hardware
specifications, invariants inside one crate, and quotations of rule 7. The count measures the
class and is not a gate, since rewording a comment would satisfy a ratchet on it. Milestone 407's
own exit is the gate.

## Exit test

Neither `install_service.rs` nor `system_installer.rs` defines a role, slot or verdict constant,
both take them from the shared definitions, and `cargo xtask install-boot` passes.

## Index row

The kernel's install offer spawns `system_installer` at most three times a boot. The two sides
agree on five things that each spells for itself. The tree already tracks that class as milestone
407 (one definition of the numbers a kernel test and its program agree on). This block sketches one
definition both sides depend on and leaves the crate to §187 (one crate per kernel-test pair, or one
crate for all of them?). It can also become a row of 407.
