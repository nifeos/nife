# Prior art

An appendix to [the milestone](../1075-abi-outside-review.md).

How the crate sits against what a reader of other kernels' ABI crates already knows. Download
figures are crates.io's at the time of the review, recent (90 days) and all-time.

| system or crate | shape of the ABI | errors | where this crate differs |
|---|---|---|---|
| Linux (`linux-raw-sys` 339M / 1.31G, `syscalls` 486k / 1.56M) | one number per system call, hundreds of them, generated from the kernel's tables per architecture; `Sysno` is an enum with `TryFrom` | positive `errno`, returned negated in the result register, decoded by `Errno::from_raw` | five numbers and a method per kernel object; a negative `i64` enum decoded by `from_ret`; an unknown negative decodes to `None` so a contract above can carry its own refusals |
| seL4 `libsel4` | invocation labels per object (`seL4_TCB_Configure`), generated from XML interface files; capability slots in a CSpace; a badge set once | `seL4_Error`, `seL4_NoError` is 0 and eleven errors follow | one `invoke` with the method in a register; a flat 64-slot table per thread; `Gone` told apart from `NoSuchSlot` on purpose; no `seL4_NoError` since success is a non-negative word |
| Zircon (`zx_status_t`, `ZX_RIGHT_*`) | handles, not slots; syscalls from FIDL; 32-bit rights with a dozen bits | negative `i32` status codes | 64-bit slot index; five rights, with `ENUMERATE` and `BIND` where Zircon has `INSPECT` and `MANAGE_*`; one `ABI_VERSION` compared at spawn where Zircon versions its vDSO |
| Redox (`redox_syscall` 194M / 857M) | the package is `redox_syscall` and the library is `syscall`; numbers in one module, flags in another; `Error::demux` turns a raw word into `Result` | `Error { errno }` with Linux's numbers | the same package-and-library split is the recommendation of decision A; this crate's error is an enum, not an `errno` |
| Hubris `sys/abi` | a workspace crate named `abi`, shared by kernel and tasks, never published; `Sysnum` is an enum via `FromPrimitive` | `FaultInfo` and `TaskState` enums | the same name and role; Hubris never meets the crates.io collision because it never publishes |

The demand figure to read honestly: `redox_syscall`'s downloads come from `std`, `parking_lot` and
`tempfile` dependency chains, not from people writing a second Redox. A kernel's ABI crate has no
outside consumer until a second implementation of either side exists, so the measured demand for
this crate on crates.io is zero today.

The concrete example of where the design differs: `Error::from_ret(-99)` is `None`, so a contract
layered on the syscall (a credential or entropy service) can answer `-99` as its own refusal and a
client can tell "no capability" from "no". On Linux the same word is `EADDRNOTAVAIL`, and every
layer above shares one number space.
