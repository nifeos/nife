#!/bin/sh
# The probe behind virtio-drivers-probe.md, beside this script. Name provisional.
#
#   virtio-drivers-probe.sh [aarch64|riscv64|x86_64 ...]   # default: all three
#
# Builds a throwaway system-test image that drives the virtio keyboard through vendor/virtio-drivers
# (0.13.0 plus rcore-os/virtio-drivers#232) behind the machine's IOMMU, and runs it with
# script/test's referee pressing the key. The image is built in a detached worktree under $WORK, so
# this tree is never modified and nothing in it depends on the crate. The probe's source is the
# patch `probe_patch` prints.
#
# BUGS: the first run builds the whole tree in the new worktree, about 25 minutes on an Apple M3.
# Needs the network for the crate's dependencies. The kernel module holds the DMA region in
# globals because `Hal`'s methods are static, and allocates from a bump heap that never frees;
# neither is a design for (c1). The worktree stays for a rerun; `git worktree remove --force
# "$WORK/tree"` drops it. The patch was written against 79941d883; on a later base its context may
# need refreshing.
set -eu
HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH='' cd -- "$HERE/../../.." && pwd)
WORK="${WORK:-${TMPDIR:-/tmp}/nife-784-virtio-drivers}"
ARCHES=${*:-aarch64 riscv64 x86_64}
TREE="$WORK/tree"

probe_patch() {
    cat <<'PROBE'
diff --git a/kernel/Cargo.toml b/kernel/Cargo.toml
index 3153e0544..effa9de57 100644
--- a/kernel/Cargo.toml
+++ b/kernel/Cargo.toml
@@ -28,2 +28,3 @@ workspace = true
 [dependencies]
+virtio-drivers = { path = "@VIRTIO_DRIVERS@", default-features = false, features = ["alloc"] }
 # NOTE: `aarch64-cpu` is NOT here. It is target-gated at the bottom of this file, and the reason is
diff --git a/kernel/src/lib.rs b/kernel/src/lib.rs
index aa1b6d06c..c38aa0f27 100644
--- a/kernel/src/lib.rs
+++ b/kernel/src/lib.rs
@@ -171,2 +171,4 @@ mod user;
 mod virtio;
+#[cfg(feature = "system_tests")]
+mod virtio_drivers_probe;
 
@@ -276,2 +278,5 @@ pub mod system_test_access {
     }
+    pub mod virtio_drivers_probe {
+        pub use crate::virtio_drivers_probe::*;
+    }
 }
diff --git a/kernel/src/pci.rs b/kernel/src/pci.rs
index c5490ad07..ad983e371 100644
--- a/kernel/src/pci.rs
+++ b/kernel/src/pci.rs
@@ -1442 +1442,11 @@ mod tests {
 }
+
+// THROWAWAY (784 c0 probe).
+#[cfg(feature = "system_tests")]
+pub(crate) fn probe_cfg_read32(bdf: Bdf, off: u64) -> u32 {
+    cfg_read32(bdf, off)
+}
+#[cfg(feature = "system_tests")]
+pub(crate) fn probe_cfg_write32(bdf: Bdf, off: u64, v: u32) {
+    cfg_write32(bdf, off, v)
+}
diff --git a/kernel/src/virtio_drivers_probe.rs b/kernel/src/virtio_drivers_probe.rs
new file mode 100644
index 000000000..4f9800781
--- /dev/null
+++ b/kernel/src/virtio_drivers_probe.rs
@@ -0,0 +1,248 @@
+//! THROWAWAY: milestone 784 (c0)'s probe. Does `virtio-drivers` 0.13.0 plus rcore-os#232 drive the
+//! virtio keyboard behind this machine's IOMMU, with `ACCESS_PLATFORM` negotiated, and does a key
+//! event arrive with the device's DMA translated? Never merged.
+
+extern crate alloc;
+
+use core::alloc::{GlobalAlloc, Layout};
+use core::ptr::NonNull;
+use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
+
+use paging::domain::DmaRegion;
+use virtio_drivers::transport::pci::PciTransport;
+use virtio_drivers::transport::pci::bus::{ConfigurationAccess, DeviceFunction, PciRoot};
+use virtio_drivers::{BufferDirection, Hal, PhysAddr};
+
+use crate::arch::mmu::phys_to_virt;
+
+// A bump heap for the crate's two `Box`es. Never frees; the probe allocates a few hundred bytes.
+const HEAP_SIZE: usize = 256 * 1024;
+#[repr(align(4096))]
+struct Heap([u8; HEAP_SIZE]);
+static mut HEAP: Heap = Heap([0; HEAP_SIZE]);
+static HEAP_NEXT: AtomicUsize = AtomicUsize::new(0);
+struct Bump;
+// SAFETY: hands out disjoint, aligned ranges of a static array and never reuses one.
+unsafe impl GlobalAlloc for Bump {
+    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
+        let base = &raw mut HEAP as usize;
+        let mut cur = HEAP_NEXT.load(Ordering::Relaxed);
+        loop {
+            let start = (base + cur).next_multiple_of(l.align()) - base;
+            let end = start + l.size();
+            if end > HEAP_SIZE {
+                return core::ptr::null_mut();
+            }
+            match HEAP_NEXT.compare_exchange(cur, end, Ordering::Relaxed, Ordering::Relaxed) {
+                Ok(_) => return (base + start) as *mut u8,
+                Err(c) => cur = c,
+            }
+        }
+    }
+    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
+}
+#[global_allocator]
+static BUMP: Bump = Bump;
+
+// The DMA capability sits in a global because `Hal`'s methods are static (784's item 3 names this).
+// Pages [0, QUEUE_PAGES) back `dma_alloc` (the virtqueues); the rest is the bounce pool `share`
+// copies through, so every address the device is handed lies in a region `confine` mapped.
+const REGION_PAGES: u64 = 16;
+const QUEUE_PAGES: u64 = 8;
+static REGION: AtomicU64 = AtomicU64::new(0);
+static QUEUE_NEXT: AtomicU64 = AtomicU64::new(0);
+static BOUNCE_NEXT: AtomicU64 = AtomicU64::new(0);
+static SAW_FALSE: AtomicBool = AtomicBool::new(false);
+static SAW_TRUE: AtomicBool = AtomicBool::new(false);
+
+fn note(access_platform: bool) {
+    if access_platform {
+        SAW_TRUE.store(true, Ordering::Relaxed);
+    } else {
+        SAW_FALSE.store(true, Ordering::Relaxed);
+    }
+}
+
+pub struct NifeHal;
+// SAFETY: `dma_alloc` returns zeroed, page-aligned, unaliased pages of the confined region;
+// `share` bounces through a disjoint part of the same region.
+unsafe impl Hal for NifeHal {
+    fn dma_alloc(
+        pages: usize,
+        _direction: BufferDirection,
+        access_platform: bool,
+    ) -> (PhysAddr, NonNull<u8>) {
+        note(access_platform);
+        let at = QUEUE_NEXT.fetch_add(pages as u64, Ordering::Relaxed);
+        assert!(at + pages as u64 <= QUEUE_PAGES, "probe: queue pages exhausted");
+        let pa = REGION.load(Ordering::Relaxed) + at * 4096;
+        let va = phys_to_virt(pa) as *mut u8;
+        // SAFETY: `pa` is inside the region this probe allocated and nothing else names.
+        unsafe { core::ptr::write_bytes(va, 0, pages * 4096) };
+        (pa as PhysAddr, NonNull::new(va).unwrap())
+    }
+    unsafe fn dma_dealloc(_: PhysAddr, _: NonNull<u8>, _: usize, access_platform: bool) -> i32 {
+        note(access_platform);
+        0
+    }
+    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _size: usize) -> NonNull<u8> {
+        NonNull::new(phys_to_virt(paddr as u64) as *mut u8).unwrap()
+    }
+    unsafe fn share(
+        buffer: NonNull<[u8]>,
+        direction: BufferDirection,
+        access_platform: bool,
+    ) -> PhysAddr {
+        note(access_platform);
+        let len = buffer.len() as u64;
+        let off = BOUNCE_NEXT.fetch_add(len.next_multiple_of(16), Ordering::Relaxed);
+        assert!(
+            off + len <= (REGION_PAGES - QUEUE_PAGES) * 4096,
+            "probe: bounce pool exhausted"
+        );
+        let pa = REGION.load(Ordering::Relaxed) + QUEUE_PAGES * 4096 + off;
+        if direction != BufferDirection::DeviceToDriver {
+            // SAFETY: the bounce slot is in the probe's region and disjoint from every other slot.
+            unsafe {
+                core::ptr::copy_nonoverlapping(
+                    buffer.as_ptr() as *const u8,
+                    phys_to_virt(pa) as *mut u8,
+                    len as usize,
+                )
+            };
+        }
+        pa as PhysAddr
+    }
+    unsafe fn unshare(
+        paddr: PhysAddr,
+        buffer: NonNull<[u8]>,
+        direction: BufferDirection,
+        access_platform: bool,
+    ) {
+        note(access_platform);
+        if direction != BufferDirection::DriverToDevice {
+            // SAFETY: `paddr` is the bounce slot `share` returned for this buffer.
+            unsafe {
+                core::ptr::copy_nonoverlapping(
+                    phys_to_virt(paddr as u64) as *const u8,
+                    buffer.as_ptr() as *mut u8,
+                    buffer.len(),
+                )
+            };
+        }
+    }
+}
+
+#[derive(Clone, Copy)]
+struct Cam;
+impl ConfigurationAccess for Cam {
+    fn read_word(&self, df: DeviceFunction, off: u8) -> u32 {
+        crate::pci::probe_cfg_read32(bdf(df), off as u64)
+    }
+    fn write_word(&mut self, df: DeviceFunction, off: u8, data: u32) {
+        crate::pci::probe_cfg_write32(bdf(df), off as u64, data)
+    }
+    unsafe fn unsafe_clone(&self) -> Self {
+        Cam
+    }
+}
+fn bdf(df: DeviceFunction) -> pci::Bdf {
+    pci::Bdf {
+        bus: df.bus,
+        dev: df.device,
+        func: df.function,
+    }
+}
+
+/// What one probe run saw.
+#[derive(Debug, Default)]
+pub struct Outcome {
+    pub rid: u32,
+    pub iommu_confining: bool,
+    pub status_after_init: u32,
+    pub hal_saw_access_platform_true: bool,
+    pub hal_saw_access_platform_false: bool,
+    pub key_press: Option<(u16, u16, u32)>,
+    pub events: usize,
+    pub fault: Option<(u32, Option<u64>)>,
+    pub bounce_pool: (u64, u64),
+}
+
+/// Bring the keyboard up through `virtio-drivers`, confined to `confined` of the probe's region
+/// (in pages from its base), and poll for `secs` for a key press.
+pub fn run(confine_pages: core::ops::Range<u64>, secs: u64) -> Option<Outcome> {
+    let d = crate::pci::find_input_device()?;
+    let mut out = Outcome {
+        rid: d.rid,
+        iommu_confining: crate::iommu::scope_of(d.rid).is_confining(),
+        ..Outcome::default()
+    };
+    if REGION.load(Ordering::Relaxed) == 0 {
+        let r = crate::memory::alloc_contiguous_zeroed(REGION_PAGES as usize)
+            .expect("probe: no region")
+            .addr();
+        REGION.store(r, Ordering::Relaxed);
+    }
+    QUEUE_NEXT.store(0, Ordering::Relaxed);
+    BOUNCE_NEXT.store(0, Ordering::Relaxed);
+    SAW_TRUE.store(false, Ordering::Relaxed);
+    SAW_FALSE.store(false, Ordering::Relaxed);
+    while crate::iommu::take_fault().is_some() {}
+    let base = REGION.load(Ordering::Relaxed);
+    // Poison the bounce pool so a buffer the device never wrote reads back as 0xff, not as the
+    // previous run's events.
+    let pool = base + QUEUE_PAGES * 4096;
+    let pool_len = (REGION_PAGES - QUEUE_PAGES) * 4096;
+    // SAFETY: the pool is the probe's own region, and no device is live on it between runs.
+    unsafe { core::ptr::write_bytes(phys_to_virt(pool) as *mut u8, 0xff, pool_len as usize) };
+    out.bounce_pool = (pool, pool + pool_len);
+    crate::iommu::confine(
+        d.rid,
+        &[DmaRegion {
+            base: base + confine_pages.start * 4096,
+            size: (confine_pages.end - confine_pages.start) * 4096,
+            writable: true,
+        }],
+    );
+
+    let df = DeviceFunction {
+        bus: d.bdf.bus,
+        device: d.bdf.dev,
+        function: d.bdf.func,
+    };
+    let mut root = PciRoot::new(Cam);
+    let transport = PciTransport::new::<NifeHal, Cam>(&mut root, df).expect("probe: transport");
+    let mut input = virtio_drivers::device::input::VirtIOInput::<NifeHal, PciTransport>::new(
+        transport,
+    )
+    .expect("probe: VirtIOInput::new");
+    out.status_after_init = device_status(d.common);
+    out.hal_saw_access_platform_true = SAW_TRUE.load(Ordering::Relaxed);
+    out.hal_saw_access_platform_false = SAW_FALSE.load(Ordering::Relaxed);
+
+    let deadline = crate::arch::timer::now() + secs * crate::arch::timer::frequency();
+    while crate::arch::timer::now() < deadline {
+        let _ = input.ack_interrupt();
+        while let Some(e) = input.pop_pending_event() {
+            out.events += 1;
+            if e.event_type == 1 && e.value == 1 && out.key_press.is_none() {
+                out.key_press = Some((e.event_type, e.code, e.value));
+            }
+        }
+        if out.key_press.is_some() {
+            break;
+        }
+        crate::sched::yield_now();
+    }
+    if let Some(f) = crate::iommu::take_fault() {
+        out.fault = Some((f.code, f.addr));
+    }
+    drop(input);
+    Some(out)
+}
+
+// device_status is byte 20 of virtio_pci_common_cfg (virtio 1.2, 4.1.4.3).
+fn device_status(common: u64) -> u32 {
+    // SAFETY: the common-config block is in a BAR `bring_up` placed and the direct map covers.
+    unsafe { core::ptr::read_volatile(phys_to_virt(common + 20) as *const u8) as u32 }
+}
diff --git a/system_tests/src/user.rs b/system_tests/src/user.rs
index 90acd013e..964a66f9b 100644
--- a/system_tests/src/user.rs
+++ b/system_tests/src/user.rs
@@ -361,2 +361,3 @@ mod compositor_tests;
 mod display_tests;
+mod virtio_drivers_probe_tests;
 
diff --git a/system_tests/src/user/virtio_drivers_probe_tests.rs b/system_tests/src/user/virtio_drivers_probe_tests.rs
new file mode 100644
index 000000000..2427b1cd5
--- /dev/null
+++ b/system_tests/src/user/virtio_drivers_probe_tests.rs
@@ -0,0 +1,28 @@
+//! THROWAWAY: milestone 784 (c0)'s probe. Never merged.
+
+use crate::virtio_drivers_probe::run;
+
+#[test_case]
+fn probe_784_virtio_drivers_input_behind_the_iommu() {
+    // Positive: the whole region confined. A press of the host's key must arrive.
+    let Some(ok) = run(0..16, 20) else {
+        crate::testing::skip!("no virtio-input function on the bus");
+    };
+    crate::println!("PROBE784 positive {:?}", ok);
+    // Control: only the queue pages confined, so the bounce pool the event buffers live in is
+    // outside the domain. If the device's writes are translated, nothing arrives and the IOMMU
+    // records a fault in the bounce pool.
+    let bad = run(0..8, 8).unwrap();
+    crate::println!("PROBE784 control {:?}", bad);
+    assert!(ok.iommu_confining, "IOMMU does not confine the keyboard");
+    assert!(ok.hal_saw_access_platform_true && !ok.hal_saw_access_platform_false, "ACCESS_PLATFORM not negotiated");
+    assert_eq!(ok.status_after_init & 0x0f, 0x0f, "status is not ACK|DRIVER|DRIVER_OK|FEATURES_OK");
+    assert_eq!(ok.key_press.map(|k| k.1), Some(30), "no KEY_A press arrived");
+    assert!(ok.fault.is_none(), "positive run faulted");
+    assert!(bad.key_press.is_none(), "control: an event arrived through an unmapped buffer");
+    let (lo, hi) = bad.bounce_pool;
+    if let Some((_, Some(a))) = bad.fault {
+        assert!(a >= lo && a < hi, "control: the fault is not in the bounce pool");
+    }
+    assert!(bad.fault.is_some(), "control: the IOMMU recorded no fault");
+}
PROBE
}

if [ ! -d "$TREE" ]; then
    mkdir -p "$WORK"
    git -C "$ROOT" worktree add -q --detach "$TREE" HEAD
    probe_patch | sed "s#@VIRTIO_DRIVERS@#$ROOT/vendor/virtio-drivers#" | git -C "$TREE" apply -
fi
failed=0
for arch in $ARCHES; do
    log="$WORK/probe-$arch.log"
    if (cd "$TREE" && script/test --arch "$arch" --test probe_784) > "$log" 2>&1; then
        verdict=pass
    else
        verdict=FAIL
        failed=1
    fi
    echo "== $arch: $verdict ($log)"
    grep -E 'PROBE784|panicked at' "$log" || true
done
exit "$failed"
