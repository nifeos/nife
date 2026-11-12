# P1: before the tag

An appendix to [the milestone](../1113-virtio-outside-review.md).

## P1

3. The ring layout is a copy of the proved layout, and nothing checks the copy. `QSIZE`
(`:72`), `OFF_DESC`, `OFF_AVAIL`, `OFF_USED` (`:99-101`) and the net driver's `0x200` stride
(`:564-569`) restate `direct_memory_access_validator::{LAYOUT_QSIZE, DESC_OFF, AVAIL_OFF,
USED_OFF, RING_BLOCK}` as literals. The kernel aliases those constants, so the layout it runs
matches the layout Kani proves (`kernel/src/virtio.rs` lines 422 to 442). The driver does not alias them.
A change to the proved layout would leave the driver reading the wrong offsets, and nothing would
fail until a QEMU test did. `components/src/virtio_net_transport.rs` lines 28 to 31 records the same copy as
a bug, and this crate records nothing. `notes/net/the-driver-and-dhcp.md` lines 130 to 137 says §46 (thin primitives or whole subsystems; we write everything in between) priced
the dependency edge and the tree did not take it. The validator has no dependencies of its own
(`crates/direct_memory_access_validator/Cargo.toml`). The fix: see Decision 2. Until the architect
rules, add a `BUGS` entry at the constants, as `virtio_net_transport` has. Effort: S. One-way in
the sense the brief uses: the layout is a wire contract between kernel and driver, and where it
lives is a dependency decision.

4. The block server dies on a device error that a client can cause. `complete_blk` panics on a
non-OK status (`:1004-1007`) and `submit_blk` panics on a refused notify (`:1153-1155`). Meanwhile
`blk_flush` deliberately returns the verdict (`:1037-1047`) because, as its own comment says, the
long-lived block server's death would block every filesystem client. A `blk::READ` or `WRITE`
whose `block` lies past the device's capacity completes with `VIRTIO_BLK_S_IOERR`, and the server
panics. The sector arithmetic `block * blk::SECTORS_PER_BLOCK` (`:1017`, `:1031`) can also
overflow, which panics under the workspace's release overflow checks. The `block` word arrives
from the FS server (`:949`), and nothing checks it against `disk_size_bytes` (`:1161`). Why it
matters: the only production path in this crate (RedoxFS behind the FS server) has an
availability failure that a bad request reaches, and the crate's own flush path shows the intended
shape. The fix: check `block + count` against the capacity read once at start. Answer `-22`
(EINVAL) for an out-of-range request and `-5` (EIO) for a non-OK status, as the flush verb does.
Let the panic stay only for a refused notify (a server bug). Effort: M. Two-way.

5. The two `unsafe` blocks state a precondition nothing checks, and omit the one that matters.
`direct_memory_access_write<T>` and `direct_memory_access_read<T>` (`:118-125`) are safe functions
whose SAFETY comments say the offset lies within the DMA page and `T` fits. Neither claim has a
check, and the block server's region spans seventeen pages and not one (`:912-919`). The comments
never mention alignment, which `write_volatile` and `read_volatile` require. Every call site today
has alignment by construction (`u16` at even offsets, `u32` at multiples of 4, `u64` at multiples
of 8). Why it matters: the invariant lives at twenty call sites instead of at the one function that
owes it, and the next `direct_memory_access_write::<u32>(OFF_HEADER + 2, ...)` has undefined
behavior with no diagnostic. The fix: a `debug_assert!` on bounds and `off % size_of::<T>() == 0`,
or `MappedWindow` (item 2), whose constructor asserts once. Effort: S. Two-way.

6. The handshake skips two steps the specification requires of a driver. After writing 0 to
`Status` (`:143`, `:624`) the driver proceeds without reading `Status` back as 0, which the
specification requires before reinitialization (virtio 1.2, the device initialization
requirements of the PCI and MMIO transports; section numbers unverified). It checks the magic
value (`:141`, `:622`) and never the `Version` register (`0x004 == 2`) or `DeviceID`. Today the
kernel enumerates the device and QEMU resets synchronously, so neither gap can bite. Under the
ruled direct grant (milestone 1062 (the device control planes leave the kernel) step 7) a driver will own its own bring-up against a real
device, and these are the two checks that catch a wrong or half-reset device. The fix: a bounded
wait for `Status == 0`, and a `Version` check beside the magic check. Effort: S. Two-way.
Inferred from reading.

7. Four copies of one completion loop and three copies of one submit. `complete_block`
(`:487-510`), `complete_blk` (`:990-1008`), `complete_flush` (`:1091-1103`) and `wait_receive`
(`:802-816`) are one loop with three endings (panic, report, return). `submit_block` (`:427-480`),
`submit_blk` (`:1110-1157`) and `submit_flush` (`:1052-1087`) are one publish with three chains.
The ring publish sequence (`:467-472`) appears six times in all, the attackers included. Why it
matters: item 4's fix lands in one place or in three, and a mutation census, once one can run,
scores each copy separately. The fix: `submit(chain: &[Descriptor]) -> u16` and
`complete(used_before: u16) -> u8` returning the status byte. The caller decides what a non-zero
status means. Effort: M. Two-way.

8. The receive parser trusts the buffer, not the device's length. `parse_dhcp_offer` (`:821-865`)
never reads the used ring element's `len` (the bytes the device wrote), so it parses whatever the
buffer held before, up to `NET_RECEIVE_BUF_LEN`. The option walk's guard `o + 1 < end` (`:848`)
admits a read at `o + 2` (`:858`) when `o + 1 == end - 1`. That read lands one byte past the
receive buffer and into the transmit buffer at `0xA00`. It stays inside the DMA page, so it
causes no memory-safety problem, but it parses wrongly. `ihl` has a lower bound (`:828`) and no
upper bound within the frame. The parser does not read the IPv4 total length or the UDP length.
`ip_checksum` (`:606-617`) drops a trailing odd byte while its doc comment says it sums `len`
bytes. Why it matters: this is the only frame parser in the crate, and the stale-buffer case is
exactly what a second broadcast in the loop at `:883-897` produces. The fix: read
`used.ring[i].len`, bound every read by it, and make the option loop's guard `o + 2 < end`.
Effort: S. Two-way.

9. Names, against `design/naming.md`. The rule covers parameters and private items as well as
public ones, and `script/crate-standalone` requires every name a stranger meets to carry a
ratification. Found, with the rule each breaks:
- `DMA_VA` (`:39`): `dma` and `va` both appear on the spelled-out list, and the appendix wants
  `_VIRTUAL_ADDRESS`. `direct_memory_access_phys`, the parameter of all seven public functions:
  `phys` appears on the appendix's list (`physical_address`).
- `CONFIG` (`:927`): `config` appears on the list. `INTERRUPT_ACK` (`:54`) abbreviates, and a
  register offset ends `_REGISTER`. The same suffix applies to `MAGIC`, `DEVICE_FEATURES`,
  `DEVICE_FEATURES_SEL` (`sel`), `DRIVER_FEATURES`, `DRIVER_FEATURES_SEL`, `INTERRUPT_STATUS` and
  `STATUS` (`:48-55`). Each keeps the specification's spelling as `#[doc(alias)]` and in the doc
  comment's opening.
- `BLOCK` (`:106`), `NET_HDR_LEN` (`:558`), `NET_RECEIVE_BUF_LEN` (`:573`): a length carries its
  unit (`_BYTES`), and `hdr` and `buf` are abbreviations.
- `blk` in `run_blk_server`, `blk_read`, `blk_write`, `blk_flush`, `submit_blk`, `complete_blk`,
  `BLK_REQ`, `BLK_READY`, `BLK_OFF_DATA`: the acronyms appendix names `blk` as a failing
  abbreviation, and the maintainers renamed the binary that carried it to `block_driver` on that
  ground. (The `filesystem_contract::blk` module it mirrors sits outside this crate.)
- `mr`, `mw` (`:109`, `:114`), `wr_be16`, `wr_be32`, `rd_be16`, `rd_be32` (`:583-602`),
  `write_desc`, `desc_base`, `avail_idx`, `QSIZE`, `OFF_*`, `csum`, `istatus`, `dev_lo`,
  `dev_hi`, `ack_hi`, `req_type`, `req`, `w0`, `st`, `ihl`: abbreviations a reader decodes.
- `S_ACKNOWLEDGE`, `S_DRIVER`, `S_DRIVER_OK`, `S_FEATURES_OK`, `F_VERSION_1_HI`,
  `F_ACCESS_PLATFORM_HI`, `F_INDIRECT_DESC_LO`, `F_FLUSH_LO`, `VIRTQ_DESC_F_*`,
  `VIRTIO_BLK_T_*`: specification terms a reader searches for. The rule lets a name keep them with
  an expansion and an alias, and today none carries either.
- The seven public functions have no `Name:` block.

Why it matters: if the crate leaves, the move check fails on the first bullet. If it folds into
the program (Decision 1), the names become private and the same list becomes the program's
worklist. The fix: one rename lane after Decision 1, bringing the public names to the architect.
Effort: M. Two-way today (nothing has a release); one-way the day the crate publishes a name.

10. The documentation describes a different crate. The crate doc (`:2`) opens with "A virtio
driver, block and net, at EL0". A stranger expects a library and finds seven `-> !` entry points.
There is no README. Shipped text cites 17 paths into this tree, 9 milestones, 3 sections and the
architect by name (`script/crate-standalone`), and a reader outside the tree can follow none of
them. The report codes (`0xE4` through `0xEA`, `0xE200 | status`, `0xDEAD_0000_0000_0000 | code`)
form a protocol with the kernel test, and no document tabulates them. The `run` example
(`:192-197`) imports `exit` and never uses it. `kernel/src/virtio.rs` line 419 and
`kernel/src/user/virtio_service.rs` line 9 say the layout and `DMA_VA` must match `user/src/virtio.rs`,
a file that no longer exists. The docs-versus-reality audit report in `design/audit-reports/`
recorded the rename to `crates/virtio`, and the two kernel comments slipped through. The fix: a
crate doc that says what the crate is (one program's roles, or the library of Decision 1). Add a
table of the report codes beside `report_code`, correct the stale paths, and, if the crate leaves,
add a README and resolve the citations. Effort: S to M. Two-way.

11. The manifest does not stand alone. It has no `rust-version`. It sets `license.workspace = true`
    and `[lints] workspace = true`. It has no license files, and a `publish = false` comment that points at this
tree's allowlist (`Cargo.toml` line 5). Every one fails `script/crate-standalone`, and every one stops
mattering if Decision 1 folds the crate into its program. Effort: S. Two-way.
