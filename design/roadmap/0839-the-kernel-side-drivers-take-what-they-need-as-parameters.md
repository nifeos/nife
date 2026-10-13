---
status: NOT-STARTED
raised: 2026-10-08
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 839. The kernel-side drivers take what they need as parameters

*(The title and slug are provisional.)*

Codebase rule 2 says a driver never reaches into a kernel global: it gets a base address, "later a
DMA allocator, later an interrupt registration". The driver crates underneath already obey it.
`crates/e1000e` takes its hardware through the `Region`, `Tails` and `pch::Hw` traits.
`crates/designware_ethernet` has `Hw`, `Region`, `Tails` and `Mdio`, and
`crates/designware_mobile_storage` has `Registers`. None of them names a kernel path or keeps a
static. The kernel files that wire those crates to a device do not obey it. This milestone moves the
three things they reach for into parameters: the device's base address, a DMA allocator, and an
IOMMU handle.

Reuse: nothing taken; the shape is the tree's own. `crates/paging`'s `Mapper` already takes its frame
allocator as a closure, which milestone 774 (a page-table allocator that fails on its Nth call) cites
as rule 2 paying off. `kernel/src/drivers/pl011.rs` is constructed from a base address and knows
nothing else. A search of `kernel/` and `crates/` for an existing DMA-allocator or IOMMU trait
(`grep -rn -E 'trait [A-Za-z]*(Dma|Alloc|Iommu)'`) found none.

## Today

Measured on `ea7a58d` with `grep -n -E 'crate::(pci|memory|iommu)::|^static '`. Line numbers are
the tree's at that commit.

`kernel/src/e1000e.rs` (379 lines):

- `:97` finds its own device with `crate::pci::find_e1000e_device()`.
- `:91` keeps `static REGION: AtomicU64`, so the DMA region outlives every caller and is found again
  by the next bring-up.
- `:215` allocates that region with `crate::memory::alloc_contiguous_zeroed`.
- `:218` and `:219` call `crate::iommu::is_active` and `crate::iommu::confine` directly.

`kernel/src/virtio.rs` (1,756 lines):

- `:16` imports `VIRTIO_MMIO_BASE` from `arch::mmu`; `:52`, `:78` and `:192` add slot offsets to it,
  and `:62`, `:100` and `:186` read the slot count from `mmu::virtio_slots()`.
- `:175` calls `crate::pci::find_block_device_n`.
- `:735` allocates the shadow ring with `crate::memory::alloc_zeroed`.
- `:743` to `:750` call `crate::iommu::is_active`, `virtio_regions` and `confine` inside
  `register`, which has nine callers under `kernel/src/user/` and in `user.rs`.
- `:707` keeps `static DEVICES`. See the correction below on why this one stays.

Two more files have the same three reaches, and the brief that raised this did not name them:

- `kernel/src/non_volatile_memory_express.rs`: `:507` (`find_nvme_device`), `:510`
  (`alloc_contiguous_zeroed`), `:513`, `:538` and `:539` (`iommu`).
- `kernel/src/extensible_host_controller_interface.rs`: `:82` (`find_xhci_device`), `:110` and
  `:111` (`iommu::scope_of`, `is_active`), `:116` (`alloc_contiguous_zeroed`), `:119` (`confine`).

Leaving those two out would apply the rule to half the identical code, so they are in scope.

## Two corrections to the premise

`virtio.rs` is not a driver, by its own header: it is bus enumeration plus the transport that
mediates a userspace driver's `Object::Virtio` capability. Its `static DEVICES` is the table those
capabilities index. That is kernel object state of the same kind as the scheduler's IPC tables, not
a driver's private global. This milestone leaves it where it is. The enumeration half and
`register` still take the base, the allocator and the IOMMU handle as parameters.

`kernel/src/designware_ethernet.rs` has the same shape too: `:179` (`static REGION`), `:194`
(`crate::memory::jh7110_ethernet`) and `:366` (`alloc_contiguous_zeroed`). It is left out because
nothing proves it under QEMU. Its only machine is the JH7110 on radon, so changing it means
somebody boots radon. It should become a proposal when this lands, with `needs_person: yes`.

## The shape

The composition root is the service under `kernel/src/user/` that calls each bring-up
(`e1000e_service.rs:176` calls `e1000e::bring_up`). It finds the device, owns the region, and hands
both to the driver. So:

1. Bring-up takes the device record the service found (`PciE1000eDevice` and its siblings already
   carry the base and the requester id), instead of calling `crate::pci` itself.
2. Bring-up takes a DMA allocator. Its shape is the lane's call; a closure returning
   `Option<PageFrame>`, as `Mapper` takes, is the precedent and probably enough.
3. Bring-up takes an IOMMU handle that answers `is_active`, `scope_of` and `confine`. Today
   `crate::iommu` is free functions over a global, so the handle is a new type.
4. The once-per-boot region moves from `e1000e.rs`'s `static REGION` to the service, which already
   refuses a second wiring while the first process lives.
5. `virtio`'s probe takes the window (base, stride, slot count and first interrupt) from its caller
   rather than from `arch::mmu`.

Every new type is a name calef has not ruled on, so each ships provisional with a `/// Name:`
marker and the report says so.

Not in scope: `mmu::phys_to_virt` and `arch::timer`. They are arch services every driver reads, and
rule 2's own list puts interrupt registration and the like "later". Threading them through is a
separate argument.

## Done when

1. Outside `#[cfg(test)]` code, this prints nothing:
   `grep -n -E 'crate::(pci|memory|iommu)::' kernel/src/e1000e.rs kernel/src/virtio.rs
   kernel/src/non_volatile_memory_express.rs kernel/src/extensible_host_controller_interface.rs`.
2. `kernel/src/e1000e.rs` has no `static`, and `virtio.rs` imports nothing from `arch::mmu` but
   `phys_to_virt`.
3. The existing gates pass on aarch64, riscv64 and x86_64 with no test changed. That means the
   e1000e gates in `notes/e1000e.md`, the NVMe and xHCI gates, and `virtio`'s two IOMMU tests
   (`the_iommu_faults_a_dma_that_escapes_the_domain`, `a_device_write_to_the_read_only_shadow_is_refused`).
4. Both `virtio` falsification patches under `kernel/falsifications/` still apply and still fail the
   test they name. They patch `kernel/src/iommu.rs` and `crates/paging`, so an IOMMU handle that
   moves code in `iommu.rs` has to carry them along.
5. `script/fastpath-footprint` and `script/stack-frame-check` are unchanged; none of these paths is
   hot, so a change there is a finding.

## What the seam buys

Today `e1000e.rs:216` and `virtio.rs:736` panic on an exhausted allocator with `.expect`. With the
allocator a parameter, a kernel test can hand in one that fails, the way milestone 774 does for page
tables. Turning those panics into refusals is the obvious next step. It changes behavior, so it is
not part of this refactor.

## Index row

The driver crates already take their hardware through traits. The kernel files that wire them
(`e1000e.rs`, `virtio.rs`, the NVMe and xHCI bring-ups) do not: they find their own device on the
PCI bus, allocate DMA from `crate::memory` and confine it through `crate::iommu`, against codebase
rule 2. This passes the device record, a DMA allocator and an IOMMU handle in from the service that
wires each one. It leaves `virtio`'s capability table in place, because that is kernel object state,
and defers the DesignWare Ethernet bring-up to a proposal, because only radon can prove it.
