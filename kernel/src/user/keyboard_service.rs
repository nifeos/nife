use super::*;
use crate::cap::{Rights, irq_cap, rendezvous_cap, virtio_cap};
use crate::sched::RendezvousId;

/// Where the driver maps its DMA page and the input ring. Must match `components/src/keyboard_driver.rs`.
const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000);
const RING_VA: u64 = address_space_map::pair_page(0x0000_0000_0082_0000);

/// One page: a keyboard's event queue is eight eight-byte records, and a device gets the grant it
/// needs and no more.
const DMA_PAGE_FRAMES: u64 = 1;

pub struct Wiring {
    /// The driver's status endpoint.
    pub report: RendezvousId,
    /// The doorbell the driver rings. The kernel holds READ here, playing the compositor.
    pub doorbell: RendezvousId,
    /// The input ring's frame, so the kernel can read what was typed.
    pub ring: u64,
    /// The rendezvous the keyboard's interrupt is bound to.
    pub irq: RendezvousId,
    head: u32,
}

/// A virtio keyboard, wired and confined but driven by nobody yet: what [`wire_device`] returns,
/// [`display_service::GpuDevice`]'s twin. [`start`] drives it, and `kernel::user::boot_progenitor`
/// grants it to the progenitor. Name: provisional.
pub(crate) struct KeyboardDevice {
    /// The `Virtio` capability's id.
    pub vid: usize,
    /// The event interrupt, routed and enabled.
    pub intid: u32,
    /// The one DMA page's physical base, which the page also carries at
    /// `abi::virtio::DMA_PHYS_OFFSET`.
    pub dma: u64,
    /// The rendezvous the interrupt is bound to, which a bench row watches.
    pub irq: RendezvousId,
}

/// Find the keyboard, build its DMA page, route its interrupt and register its confined
/// transport, and spawn nothing. `None` if no virtio-input function is on the bus.
pub(crate) fn wire_device() -> Option<KeyboardDevice> {
    let d = crate::pci::find_input_device()?;

    // Zeroed so no stale descriptor and no stale event is ever visible to the device or to us.
    let dma = crate::memory::alloc_contiguous_zeroed(DMA_PAGE_FRAMES as usize)
        .expect("no DMA region for the keyboard driver")
        .addr();
    // Where the page is, in the page (`abi::virtio::DMA_PHYS_OFFSET`): the driver reads it there
    // rather than from `arg1`, so a supervisor that holds only the frame capability can build it.
    super::write_dma_phys(dma);

    let irq_ep = crate::sched::create_rendezvous();
    crate::sched::bind_irq(d.intid, irq_ep);
    crate::arch::irq::enable(d.intid);

    let vid = crate::virtio::register(
        crate::virtio::Transport::pci(&d),
        dma,
        DMA_PAGE_FRAMES * FRAME_SIZE,
        Some(d.rid),
    );
    Some(KeyboardDevice {
        vid,
        intid: d.intid,
        dma,
        irq: irq_ep,
    })
}

/// Wire and spawn the keyboard driver. `None` if no virtio-input function is on the bus.
///
/// The kernel keeps the doorbell's receiving half and the ring, standing in for the compositor; a
/// real system hands both to `compositor` and the driver does not change.
pub fn start(image: &'static [u8]) -> Option<Wiring> {
    let KeyboardDevice {
        vid,
        intid,
        dma,
        irq,
    } = wire_device()?;
    let ring = crate::memory::alloc_zeroed()
        .expect("no frame for the input ring")
        .addr();

    let report = crate::sched::create_rendezvous();
    let doorbell = crate::sched::create_rendezvous();

    let maps = [
        Mapping {
            va: DMA_VA,
            phys: dma,
            flags: Flags::user_data(),
        },
        Mapping {
            va: RING_VA,
            phys: ring,
            flags: Flags::user_data(),
        },
    ];
    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: 0,
                arg1: 0, // no physical address: the DMA page carries its own
                arg2: 0,
                grants: &[
                    rendezvous_cap(report, Rights::WRITE),   // slot 0: status
                    irq_cap(intid),                          // slot 1: the event interrupt
                    virtio_cap(vid),                         // slot 2: the confined transport
                    rendezvous_cap(doorbell, Rights::WRITE), // slot 3: ring the compositor
                ],
                maps: &maps,
            },
        )
    })
    .expect("could not spawn the keyboard driver");

    Some(Wiring {
        report,
        doorbell,
        ring,
        irq,
        head: 0,
    })
}

impl Wiring {
    /// Take what the driver has typed into the ring, advancing the head the way a compositor
    /// does. Returns how many bytes landed in `out`.
    pub fn take_typed(&mut self, out: &mut [u8]) -> usize {
        use compositor::proto::ring;
        let base = mmu::phys_to_virt(self.ring);
        // SAFETY: inside the ring frame this kernel allocated and shares with the driver.
        let tail = unsafe { core::ptr::read_volatile((base + ring::TAIL) as *const u32) };
        // The tail is published after the bytes it advertises; read it before them.
        //
        // PAIR: `ring_publish`'s `fence(SeqCst)` in components/src/keyboard_driver.rs.
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        let mut n = 0;
        while self.head != tail && n < out.len() {
            let at = base + ring::BYTES + (self.head % ring::CAPACITY) as u64;
            // SAFETY: inside the ring frame.
            out[n] = unsafe { core::ptr::read_volatile(at as *const u8) };
            self.head = self.head.wrapping_add(1);
            n += 1;
        }
        // SAFETY: inside the ring frame; the head is ours to advance.
        unsafe { core::ptr::write_volatile((base + ring::HEAD) as *mut u32, self.head) };
        n
    }

    /// Answer the driver's `COMMIT`, the way a compositor would after compositing. Returns the
    /// counter reading taken as the `COMMIT` arrived, before the reply, for `bench::keyboard`.
    pub fn answer_doorbell(&self) -> u64 {
        let m = crate::sched::ipc_receive_cap(self.doorbell);
        let arrived = crate::arch::timer::now();
        let crate::cap::Object::Reply(caller) = crate::sched::current_cap(m[1])
            .expect("the keyboard driver's ring was not a CALL")
            .object
        else {
            panic!("the keyboard driver rang without a reply capability");
        };
        assert_eq!(
            compositor::proto::operation(m[0]),
            compositor::proto::COMMIT,
            "the keyboard driver rang with something other than COMMIT",
        );
        crate::sched::ipc_reply(caller, [0, 0]);
        crate::sched::delete_current_cap(m[1]).expect("consume the one-shot reply");
        arrived
    }
}
