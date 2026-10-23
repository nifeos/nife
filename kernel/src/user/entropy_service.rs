use super::*;
use crate::cap::{Rights, irq_cap, rendezvous_cap, virtio_cap};
use crate::sched::RendezvousId;

/// Where the service maps its DMA page. Must match components/src/entropy.rs.
const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000);

/// One page, and no more: the rings take 0x16e of it and the buffer 0x100, and a device gets the
/// grant it needs and no more.
const DMA_PAGE_FRAMES: u64 = 1;

/// What `Spawn::arg0` means. Must match `components/src/entropy.rs`; the kernel's own spawn-argument
/// convention with the one program it spawns (not a wire contract between two user programs, so
/// rule 7 does not apply), the same footing `DMA_VA` above is already on.
const MODE_VIRTIO: u64 = 0;
const MODE_INSTRUCTION: u64 = 1;

/// Which source to take randomness from: a virtio-rng on mmio or PCIe (one binary over both,
/// §18 (the PCIe transport: one driver, two buses)), an instruction (`RDSEED`, `RNDRRS`), or the
/// JH7110's TRNG.
///
/// `Instruction` and `Jh7110` are not buses. They are variants so that one type picks what
/// `ensure` wires, indexes the once-per-boot state and names the source a failure came from.
/// Provisional; flagged for an architect.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bus {
    Mmio,
    Pci,
    Instruction,
    Jh7110,
}

pub struct Wiring {
    /// The service's readiness endpoint, and **only on the call that did the wiring**: the
    /// report is sent once and whoever asked first has taken it.
    pub ready: Option<RendezvousId>,
    /// The request endpoint. **This is the capability a client is given**, with WRITE; the
    /// service holds READ. Nothing about it names the device.
    pub request: RendezvousId,
    /// Which bus this instance took its device from, so a failure names it.
    pub bus: Bus,
    /// True when the device sat behind an IOMMU, which on this machine means the PCIe wiring.
    pub confined_by_iommu: bool,
    /// What the clock and reset controller said, only on the call that did the wiring, as with
    /// [`Wiring::ready`]. `None` on every backend but [`Bus::Jh7110`], and on that one when the
    /// tree named no controller. Read only by the riscv64 boot tour; not `cfg`-gated, so `Wiring`
    /// has one shape everywhere.
    #[cfg_attr(not(target_arch = "riscv64"), allow(dead_code))]
    pub clock: Option<jh7110_clock_and_reset::Report>,
}

/// One entropy service per device per boot: a second would reset the device and reprogram its
/// queue under the first, which would then wait forever. The first caller gets the readiness
/// endpoint; later callers get the same request endpoint and `None` for it.
///
/// Plain atomics rather than a lock: the only writer is the boot/test thread that calls this.
static WIRED: [core::sync::atomic::AtomicBool; 4] = [
    core::sync::atomic::AtomicBool::new(false),
    core::sync::atomic::AtomicBool::new(false),
    core::sync::atomic::AtomicBool::new(false),
    core::sync::atomic::AtomicBool::new(false),
];
static REQUEST: [core::sync::atomic::AtomicU64; 4] = [
    core::sync::atomic::AtomicU64::new(0),
    core::sync::atomic::AtomicU64::new(0),
    core::sync::atomic::AtomicU64::new(0),
    core::sync::atomic::AtomicU64::new(0),
];
static CONFINED: [core::sync::atomic::AtomicBool; 4] = [
    core::sync::atomic::AtomicBool::new(false),
    core::sync::atomic::AtomicBool::new(false),
    core::sync::atomic::AtomicBool::new(false),
    core::sync::atomic::AtomicBool::new(false),
];

impl Bus {
    const fn index(self) -> usize {
        match self {
            Bus::Mmio => 0,
            Bus::Pci => 1,
            Bus::Instruction => 2,
            Bus::Jh7110 => 3,
        }
    }
}

/// Wire the entropy service on `bus` if this boot has not already, else hand back what is
/// already running. `None` means there is no virtio-rng function on that bus.
pub fn ensure(image: &'static [u8], bus: Bus) -> Option<Wiring> {
    use core::sync::atomic::Ordering;

    let i = bus.index();
    if WIRED[i].load(Ordering::Acquire) {
        return Some(Wiring {
            ready: None,
            request: REQUEST[i].load(Ordering::Relaxed),
            bus,
            confined_by_iommu: CONFINED[i].load(Ordering::Relaxed),
            clock: None,
        });
    }
    let w = start(image, bus)?;
    REQUEST[i].store(w.request, Ordering::Relaxed);
    CONFINED[i].store(w.confined_by_iommu, Ordering::Relaxed);
    WIRED[i].store(true, Ordering::Release);
    Some(w)
}

/// Wire and spawn the entropy service. `None` if `bus` has no virtio-rng function on it (or,
/// for [`Bus::Instruction`], if this machine does not implement the instruction).
fn start(image: &'static [u8], bus: Bus) -> Option<Wiring> {
    if bus == Bus::Instruction {
        return start_instruction(image);
    }
    if bus == Bus::Jh7110 {
        return start_jh7110(image);
    }
    let (transport, intid, rid) = match bus {
        Bus::Instruction | Bus::Jh7110 => {
            unreachable!("the two early returns above handle these; see start()'s first lines")
        }
        Bus::Mmio => {
            let d = crate::virtio::find_entropy_device()?;
            (
                crate::virtio::Transport::Mmio {
                    mmio_phys: d.mmio_phys,
                },
                d.intid,
                None,
            )
        }
        Bus::Pci => {
            let d = crate::pci::find_rng_device()?;
            (crate::virtio::Transport::pci(&d), d.intid, Some(d.rid))
        }
    };

    // Zeroed so no stale descriptor reaches the device, and an unfilled buffer is not somebody's
    // old page posing as entropy.
    let dma = crate::memory::alloc_contiguous_zeroed(DMA_PAGE_FRAMES as usize)
        .expect("no DMA region for the entropy service")
        .addr();

    let irq_ep = crate::sched::create_rendezvous();
    crate::sched::bind_irq(intid, irq_ep);
    crate::arch::irq::enable(intid);

    // Asked of the unit that owns the device, not of "is any IOMMU up": on a VT-d machine with
    // more than one unit those differ (`iommu::scope_of`).
    let confined_by_iommu = rid.is_some_and(|rid| crate::iommu::scope_of(rid).is_confining());
    let vid = crate::virtio::register(transport, dma, DMA_PAGE_FRAMES * FRAME_SIZE, rid);

    let ready = crate::sched::create_rendezvous();
    let request = crate::sched::create_rendezvous();

    let maps = [Mapping {
        va: DMA_VA,
        phys: dma,
        flags: Flags::user_data(),
    }];
    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: MODE_VIRTIO,
                arg1: dma, // the DMA region's PHYSICAL base: descriptors speak physical
                arg2: 0,
                grants: &[
                    rendezvous_cap(request, Rights::READ), // slot 0: RECEIVE client requests
                    irq_cap(intid),                        // slot 1: the completion interrupt
                    virtio_cap(vid),                       // slot 2: the confined transport
                    rendezvous_cap(ready, Rights::WRITE),  // slot 3: signal readiness once
                ],
                maps: &maps,
            },
        )
    })
    .expect("could not spawn the entropy service");

    Some(Wiring {
        ready: Some(ready),
        request,
        bus,
        confined_by_iommu,
        clock: None,
    })
}

/// Does this machine implement an instruction source `entropy` can use? The kernel's call to
/// make, not the process's: `ID_AA64ISAR0_EL1` (aarch64) is not readable from EL0, so `entropy.rs`
/// cannot check `FEAT_RNG` itself, the same way it cannot check whether a `Virtio` capability really
/// names a device; it trusts what it was spawned with.
#[cfg(target_arch = "aarch64")]
fn is_instruction_backend_available() -> bool {
    crate::arch::isa::get().rndr
}

/// `x86_64`: `CPUID` leaf 7's `EBX` bit 18, read once at boot (`kernel/src/arch/x86_64/isa.rs`)
/// and cached in `Isa`. `entropy.rs` trusts what it was spawned with, as on aarch64.
#[cfg(target_arch = "x86_64")]
fn is_instruction_backend_available() -> bool {
    crate::arch::isa::get().has_random_seed_instruction()
}

/// riscv64: neither `RDSEED` nor `RNDR`/`RNDRRS` exists on this ISA. Milestone 159's JH7110 TRNG is
/// the real hardware source there, wired through the JH7110 driver rather than through this file.
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
fn is_instruction_backend_available() -> bool {
    false
}

/// Wire and spawn the entropy service in instruction mode: no virtio device, no DMA page, no
/// IRQ. `None` if this machine does not implement the instruction its architecture would use (see
/// [`is_instruction_backend_available`]), the same "no source, no service" answer the virtio path
/// gives for a missing device.
fn start_instruction(image: &'static [u8]) -> Option<Wiring> {
    if !is_instruction_backend_available() {
        return None;
    }

    let ready = crate::sched::create_rendezvous();
    let request = crate::sched::create_rendezvous();

    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: MODE_INSTRUCTION,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(request, Rights::READ), // slot 0: RECEIVE client requests
                    rendezvous_cap(ready, Rights::WRITE),  // slot 1: signal readiness once
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the entropy service");

    Some(Wiring {
        ready: Some(ready),
        request,
        bus: Bus::Instruction,
        // Not a device, so nothing for an IOMMU to confine.
        confined_by_iommu: false,
        clock: None,
    })
}

/// Where the service maps the TRNG's register page. Must match `components/src/jh7110_entropy.rs`'s
/// `TRNG_VA`; distinct from [`DMA_VA`] so neither constant means two things.
const TRNG_VA: u64 = address_space_map::pair_page(0x0000_0000_0094_0000);

/// Does this machine have a JH7110 TRNG? The device tree's answer, decoded by
/// `jh7110_entropy::discover`, which owns the `compatible` string and the `reg` decode.
///
/// riscv64 only, since the JH7110 is a RISC-V `SoC`. QEMU's `virt` board has no such node, so CI
/// takes the `None` path (`discover_finds_nothing_on_qemus_virt_board` pins it).
///
/// Name: recorded (design/roadmap/0388-an-acronym-sweep-the-tree-can-do-at-once.md). The block
/// argues the abbreviation is the hardware block's own, spelled `starfive,jh7110-trng` by the
/// vendor's tree and `jh7110-trng.c` by mainline, a name received rather than authored. It does
/// not settle that: AGENTS.md puts public function names in an architect's hands, and calef has
/// not ruled on this one.
#[cfg(target_arch = "riscv64")]
pub fn jh7110_trng_device() -> Option<jh7110_entropy::Discovered> {
    jh7110_entropy::discover(&crate::device_tree().ok()?)
        .ok()
        .flatten()
}

/// See the riscv64 arm: no JH7110 anywhere but a JH7110.
#[cfg(not(target_arch = "riscv64"))]
pub fn jh7110_trng_device() -> Option<jh7110_entropy::Discovered> {
    None
}

/// Wire and spawn the entropy service against the JH7110's TRNG. `None` when this machine's device
/// tree does not describe the device, which is every machine CI boots.
///
/// The driver holds a request endpoint, a readiness endpoint, and one page of device memory: the
/// first `0x1000` of the TRNG's `0x4000` register window at [`TRNG_VA`], which covers every
/// register `jh7110_entropy::regs` names. No DMA page, interrupt or transport. See
/// notes/entropy/jh7110-trng.md.
fn start_jh7110(image: &'static [u8]) -> Option<Wiring> {
    let device = jh7110_trng_device()?;

    // Ungate the device before the spawn: the driver reads `STAT` first, and a gated block reads
    // as zeros. The clock admin plane stays in the kernel
    // (`kernel/src/drivers/jh7110_clock_and_reset.rs`).
    let clock = jh7110_clock_bring_up();

    let ready = crate::sched::create_rendezvous();
    let request = crate::sched::create_rendezvous();

    let maps = [Mapping {
        va: TRNG_VA,
        phys: device.reg_base,
        flags: Flags::user_device(),
    }];
    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(request, Rights::READ), // slot 0: RECEIVE client requests
                    rendezvous_cap(ready, Rights::WRITE),  // slot 1: signal readiness once
                ],
                maps: &maps,
            },
        )
    })
    .expect("could not spawn the JH7110 TRNG entropy service");

    Some(Wiring {
        ready: Some(ready),
        request,
        bus: Bus::Jh7110,
        // This device never touches memory, so there is no DMA for an IOMMU to confine.
        confined_by_iommu: false,
        clock,
    })
}

/// Enable the TRNG's two clocks and release its reset, or `None` when this machine's tree named no
/// clock-and-reset window, so the mapping does not exist and nothing may be stored to. riscv64
/// only.
#[cfg(target_arch = "riscv64")]
fn jh7110_clock_bring_up() -> Option<jh7110_clock_and_reset::Report> {
    let crg = crate::memory::jh7110_clock_and_reset()?;
    // SAFETY: `memory::init` recorded this region only for a machine whose tree names a JH7110,
    // and `mmu::map_everything` mapped exactly it, device-typed, in the direct map. The domain
    // and the plan are the same crate's, so every identifier in the plan is in range for it.
    Some(unsafe {
        crate::drivers::jh7110_clock_and_reset::bring_up(
            crate::arch::mmu::phys_to_virt(crg.base) as usize,
            &jh7110_clock_and_reset::STG,
            jh7110_clock_and_reset::TRNG_BRING_UP,
        )
    })
}

/// See the riscv64 arm: no JH7110 anywhere but a JH7110.
#[cfg(not(target_arch = "riscv64"))]
fn jh7110_clock_bring_up() -> Option<jh7110_clock_and_reset::Report> {
    None
}

/// Where this machine's clock-and-reset window is, and who said so, for the boot tour to print
/// beside the bring-up report: [`Wiring::clock`] records what the hardware answered, not where it
/// was asked. `None` on a machine that is not a JH7110.
///
/// Name: recorded (design/roadmap/0388-an-acronym-sweep-the-tree-can-do-at-once.md). The same
/// received-spelling question as `jh7110_trng_device`, sharpened by calef's 2026-09-13 ruling
/// that the crate spelling this block's clocks expands to `jh7110_clock_and_reset`. The function
/// waits on the name ratification queue beside its sibling; it does not rule for itself.
pub fn jh7110_crg_window() -> Option<jh7110_clock_and_reset::Found> {
    crate::memory::jh7110_clock_and_reset()
}

impl Wiring {
    /// Take the startup report, or `None` when this caller was not the one that wired the service,
    /// since the report is sent once. Three words, as the JH7110 backend fills them:
    /// - `[0]` the readiness verdict: [`entropy_contract::READY`], or a `0xDEAD_..` word whose low
    ///   byte names the bring-up step that failed.
    /// - `[1]` `(STAT-after-init << 32) | bytes_in_hand`: the status register once programmed
    ///   (the only place `STAT.R256` can be checked), and the bytes the driver had in hand.
    /// - `[2]` `(STAT << 32) | ISTAT` at report time, whatever `[0]` says.
    ///
    /// The virtio-rng and instruction backends fill `[1]` and `[2]` with their own equivalents.
    pub fn wait_for_ready(&self) -> Option<[u64; 5]> {
        self.ready.map(crate::sched::ipc_receive)
    }

    /// Play a client: ask for `n` random bytes over the request endpoint, the one a userspace
    /// client holds, and copy what arrives into `out`. Returns how many landed.
    pub fn get(&self, n: u64, out: &mut [u8]) -> usize {
        let r = crate::sched::ipc_call(
            self.request,
            [entropy_contract::req(entropy_contract::GET, n), 0],
        );
        match entropy_contract::delivered(r[0]) {
            Some(count) => entropy_contract::take(count, r[1], out),
            None => 0,
        }
    }

    /// Fill `out` completely, however many round trips that takes, and return how many bytes
    /// landed. Stops at the first short reply, since a service short of bytes is out.
    ///
    /// [`get`](Wiring::get) is one exchange of at most
    /// [`MAX_BYTES`](entropy_contract::MAX_BYTES) bytes, so `get(32, ..)` returns 8; a caller that
    /// wants a bufferful uses this rather than comparing one `get` against its request.
    pub fn fill(&self, out: &mut [u8]) -> usize {
        let mut got = 0;
        while got < out.len() {
            let n = self.get(entropy_contract::MAX_BYTES, &mut out[got..]);
            got += n;
            if n == 0 || (n as u64) < entropy_contract::MAX_BYTES {
                break;
            }
        }
        got
    }
}
