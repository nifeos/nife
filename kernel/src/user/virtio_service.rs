use super::*;
use crate::cap::{Rights, irq_cap, notification_cap, rendezvous_cap, timer_cap, virtio_cap};
use crate::sched::RendezvousId;
use crate::user::holding::Holding;

/// Where the driver expects its DMA page. Must match user/src/virtio.rs. The device registers
/// are NOT mapped to the driver any more: it drives the device through a `Virtio` capability,
/// so it cannot point the device outside this DMA region.
const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000);

const ROLE_VIRTIO_BLK: u64 = 3;
/// The write-path roles (milestone 32 (a real filesystem) phase 1); must match `block_driver.rs`.
const ROLE_VIRTIO_BLK_WRITE: u64 = 30;
const ROLE_VIRTIO_BLK_WRITE_ABANDON: u64 = 31;
/// The virtio-net driver role (milestone 30 (the network stack as a confined component)); must
/// match `block_driver.rs`.
const ROLE_VIRTIO_NET: u64 = 40;

/// Start a driver role against a discovered transport. The shared body of [`start`] (mmio),
/// [`start_pci`] (PCIe), and the writer starters: everything from here on, the DMA region,
/// the Irq routing, the confined `Virtio` capability, the spawn, is bus-agnostic, which is
/// the transport seam doing its job, and role-agnostic, because every role gets the same
/// world and differs only in what it does with it. `rid` is the PCIe requester id the IOMMU
/// keys its tables on; mmio devices have no IOMMU in front of them and pass `None`.
fn wire(
    image: &'static [u8],
    transport: crate::virtio::Transport,
    intid: u32,
    role: u64,
    rid: Option<u32>,
) -> RendezvousId {
    // A DMA page: physical memory the device can reach, mapped into the driver, whose
    // physical address the driver must know (a process sees only virtual addresses). We hand
    // that physical address over in `arg1`.
    // Zeroed so stale RAM cannot look like a valid descriptor to the device before the driver
    // writes the real ones.
    let dma = crate::memory::alloc_zeroed()
        .expect("no DMA frame for the virtio driver")
        .addr();

    // Route the device's interrupt to an endpoint and enable it, so the driver's `WAIT` on
    // its Irq capability will receive it. See milestone 9a.
    let irq_ep = crate::sched::create_rendezvous();
    crate::sched::bind_irq(intid, irq_ep);
    crate::arch::irq::enable(intid);

    // Where the driver reports the bytes it read.
    let report = crate::sched::create_rendezvous();

    // Register the device's transport with the kernel: the kernel owns the registers and the
    // DMA-critical operations, and confines the device to this DMA region. The driver gets a
    // `Virtio` capability, not the registers.
    let vid = crate::virtio::register(transport, dma, FRAME_SIZE, rid);

    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: role,
                arg1: dma, // the DMA region's PHYSICAL address (still needed to build requests)
                arg2: 0,
                grants: &[
                    rendezvous_cap(report, Rights::WRITE), // slot 0: SEND the result
                    irq_cap(intid),                        // slot 1: WAIT / ACK the interrupt
                    virtio_cap(vid),                       // slot 2: drive the device, confined
                ],
                maps: &[Mapping {
                    va: DMA_VA,
                    phys: dma,
                    flags: Flags::user_data(),
                }],
            },
        )
    })
    .expect("could not spawn the virtio driver");

    report
}

/// Start the driver against the virtio-mmio disk. Returns the endpoint it will report its
/// result on, or `None` if there is no disk attached to enumerate.
pub fn start(image: &'static [u8]) -> Option<RendezvousId> {
    start_role(image, ROLE_VIRTIO_BLK)
}

/// Start the SAME driver binary against the PCIe disk (the PCIe transport, DECISIONS §18):
/// enumeration and bring-up by kernel/src/pci.rs, INTx through the interrupt controller, the
/// identical confinement, now backed by the IOMMU when the machine has one (§20). The driver
/// cannot tell which bus it is on, and that is the point.
pub fn start_pci(image: &'static [u8]) -> Option<RendezvousId> {
    start_role_pci(image, ROLE_VIRTIO_BLK)
}

/// Start the write-path driver (milestone 32 phase 1) against the mmio disk: it writes a
/// pattern to the scratch block, reads it back, re-checks the filesystem around it, and
/// reports the read-back bytes.
pub fn start_writer(image: &'static [u8]) -> Option<RendezvousId> {
    start_role(image, ROLE_VIRTIO_BLK_WRITE)
}

/// The same writer over the PCIe transport.
pub fn start_writer_pci(image: &'static [u8]) -> Option<RendezvousId> {
    start_role_pci(image, ROLE_VIRTIO_BLK_WRITE)
}

/// Start the abandoning writer (the kill-mid-write case): it submits a validated write and
/// dies on purpose before collecting the completion. Reports 1 after the submit, so the test
/// knows the request genuinely left before the death.
pub fn start_write_abandoner(image: &'static [u8]) -> Option<RendezvousId> {
    start_role(image, ROLE_VIRTIO_BLK_WRITE_ABANDON)
}

/// Start the virtio-net driver against the mmio NIC (milestone 30). It does a DHCP round trip
/// over QEMU user-mode networking and reports the offered address. `None` if no NIC is attached.
/// The same `wire` machinery as the disk: the DMA confinement now polices two queues, and the
/// driver drives receive and transmit through the one `Virtio` capability.
pub fn start_net(image: &'static [u8]) -> Option<RendezvousId> {
    let dev = crate::virtio::find_net_device()?;
    Some(wire(
        image,
        crate::virtio::Transport::Mmio {
            mmio_phys: dev.mmio_phys,
        },
        dev.intid,
        ROLE_VIRTIO_NET,
        None, // virtio-mmio has no IOMMU in front of it on either board
    ))
}

/// The same net driver over the PCIe transport, behind the IOMMU (milestone 30, §20): the NIC
/// is confined in hardware to its DMA region and shadow page, `iommu_platform=on`, exactly the
/// disk's pattern. The driver cannot tell which bus it is on.
pub fn start_net_pci(image: &'static [u8]) -> Option<RendezvousId> {
    let d = crate::pci::find_net_device()?;
    Some(wire(
        image,
        crate::virtio::Transport::pci(&d),
        d.intid,
        ROLE_VIRTIO_NET,
        Some(d.rid),
    ))
}

/// The heap budget the net server (smoltcp) draws from, in pages: the socket set, per-frame
/// transmit buffers, and caches, plus the page tables for the heap and for clients' shared frames.
///
/// **128, down from 192, because 192 was never measured and the boot could not afford it**
/// (milestone 107). A `net_stack` is spawned ten times over the aarch64 suite and none of those
/// regions is ever reclaimed (the server blocks in its serve loop forever, and nothing reaps it), so
/// this number is multiplied by ten and held until reboot. Adding one more net test took the boot's
/// free frames down to **107**, and the shell-timing test then asked for 128 contiguous pages and
/// could not have them; the failure surfaced as "no swish program in the initial ramdisk",
/// which reads like a packaging bug and is not one.
///
/// The invariant that makes 128 safe rather than lucky: `net_stack::HEAP_MAX` caps the heap at **96**
/// pages, lowered in the same change, so the budget covers the heap's worst case with 32 pages left
/// for page tables and clients' frame mappings. Both numbers are stated on both sides; if one moves,
/// the other must. The suite is what proves 96 is enough for smoltcp's socket set and buffers.
pub(super) const NET_SERVER_BUDGET_PAGES: u64 = 128;
/// smoltcp builds packets on the stack; one mapped stack page is not enough. Eight extra keeps
/// the poll loop clear (`allocator_exerciser` needed three for `alloc` collections; smoltcp asks more).
pub(super) const NET_SERVER_STACK_PAGES: u64 = 8;

/// Start the **net server** (milestone 30, piece 3): the `net_stack` binary, which runs smoltcp over
/// the confined NIC and does DHCP. Like [`wire`] it hands the confined `Virtio` capability, the
/// interrupt, a DMA page, and a report endpoint; unlike it, the server also gets an **untyped
/// budget** (slot 3) for the heap smoltcp allocates against, and extra stack pages. Returns the
/// endpoint the server reports its acquired address on, or `None` if no NIC is attached.
pub fn start_net_server(image: &'static [u8]) -> Option<(RendezvousId, Holding)> {
    let dev = crate::virtio::find_net_device()?;
    let (report, _stack, holding) = wire_net_server(
        image,
        crate::virtio::Transport::Mmio {
            mmio_phys: dev.mmio_phys,
        },
        dev.intid,
        None,
        socket_contract::NO_LISTEN_GRANT, // a DHCP bring-up serves nobody inbound
    );
    Some((report, holding))
}

/// The net server over the PCIe transport, behind the IOMMU (§20).
pub fn start_net_server_pci(image: &'static [u8]) -> Option<(RendezvousId, Holding)> {
    let d = crate::pci::find_net_device()?;
    let (report, _stack, holding) = wire_net_server(
        image,
        crate::virtio::Transport::pci(&d),
        d.intid,
        Some(d.rid),
        socket_contract::NO_LISTEN_GRANT,
    );
    Some((report, holding))
}

/// [`wire`] for the net server: the same confined transport, interrupt, DMA page, and report
/// endpoint, plus an untyped budget for the heap, extra stack pages, and a **`Stack` endpoint**
/// (slot 4) where clients' socket-contract requests arrive. `net_stack` is its own binary (loaded by
/// name), so no role selector is passed. Returns `(report endpoint, stack endpoint)`; a caller
/// that has no client (the phase-A DHCP tests) ignores the stack, and `net_stack` simply blocks on it.
fn wire_net_server(
    image: &'static [u8],
    transport: crate::virtio::Transport,
    intid: u32,
    rid: Option<u32>,
    listen_grant: u64,
) -> (RendezvousId, RendezvousId, Holding) {
    use crate::cap::memory_region_cap;

    // Zeroed so stale RAM cannot look like a valid descriptor to the device before the driver
    // writes the real ones.
    let dma = crate::memory::alloc_zeroed()
        .expect("no DMA frame for the net server")
        .addr();

    // **The three endpoints come out of a region of their own, and that is what makes this service
    // reclaimable at all.** `net_stack` blocks in `receive_cap(STACK)` forever, and DECISIONS §16 (object revocation)'s
    // armed kill is spent by `schedule()`, which a `Blocked` thread never reaches. What wakes it is
    // reclaiming the region its endpoints live in: the reap drains their wait queues and aborts the
    // waiter, and the doomed server is then schedulable enough to die. From the kernel's own
    // endpoint chunks (`create_rendezvous`) there is no such handle, and there was no way to end this
    // process short of rebooting. Six pages: three endpoints, the retransmit timer and its
    // notification (milestone 106), one page each, and one spare.
    let ep_region = crate::memory_region::create(6).expect("no endpoint region for net_stack");
    let irq_ep = crate::sched::create_rendezvous_from(ep_region).expect("no irq endpoint");
    crate::sched::bind_irq(intid, irq_ep);
    crate::arch::irq::enable(intid);

    let report = crate::sched::create_rendezvous_from(ep_region).expect("no report endpoint");
    let stack = crate::sched::create_rendezvous_from(ep_region).expect("no stack endpoint");
    // **The retransmit timer** (milestone 106 (a wait that ends on either the interrupt or the
    // deadline), DECISIONS §147 (a timer a userspace service cannot hold)): a timer and the
    // notification it signals, the notification bound to the server's thread below, so its
    // `Irq::WAIT` ends on a frame or on smoltcp's next deadline, whichever is first. Made and bound
    // here, by the spawner, because a running thread holds no capability to its own TCB and so
    // cannot bind itself (`notes/timer.md`, "Binding a notification to yourself").
    let wake = crate::sched::create_notification_from(ep_region).expect("no notification");
    let retransmit = crate::sched::create_timer_from(ep_region).expect("no retransmit timer");
    let vid = crate::virtio::register(transport, dma, FRAME_SIZE, rid);
    let budget =
        crate::memory_region::create(NET_SERVER_BUDGET_PAGES).expect("no untyped for net_stack");
    // The extra stack pages, in a region of their own so they can be handed back **after** the
    // server is provably dead. They cannot come from `budget`: a `Spawn::maps` page is not a
    // recorded mapping, so revocation cannot pull it, and freeing a running thread's stack is a
    // use-after-free rather than a fault. See `Holding::region_after_death`.
    let stack_region = crate::memory_region::create(NET_SERVER_STACK_PAGES)
        .expect("no stack region for net_stack");

    // The DMA mapping plus the extra stack pages, in one array the spawn closure owns.
    let mut maps = [Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_data(),
    }; NET_SERVER_STACK_PAGES as usize + 1];
    maps[0] = Mapping {
        va: DMA_VA,
        phys: dma,
        flags: Flags::user_data(),
    };
    for k in 0..NET_SERVER_STACK_PAGES as usize {
        // `retype_page` hands the page back zeroed, so the process starts clean without a second
        // scrub here.
        let phys =
            crate::memory_region::retype_page(stack_region).expect("no frame for the net stack");
        maps[k + 1] = Mapping {
            va: USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE,
            phys,
            flags: Flags::user_data(),
        };
    }

    let tid = crate::sched::spawn(move || {
        run_with_own_space(
            image,
            Spawn {
                arg0: 0, // net_stack is its own binary; no role selector
                arg1: dma,
                arg2: listen_grant, // which ports this stack's clients may listen on, if any
                grants: &[
                    rendezvous_cap(report, Rights::WRITE), // slot 0: report the acquired address
                    irq_cap(intid),                        // slot 1: WAIT / ACK the interrupt
                    virtio_cap(vid),                       // slot 2: the confined transport
                    memory_region_cap(budget),             // slot 3: the heap's budget
                    rendezvous_cap(stack, Rights::READ),   // slot 4: serve clients' requests
                    notification_cap(wake, Rights::ALL),   // slot 5: bound; arm it, poll it
                    timer_cap(retransmit, Rights::WRITE),  // slot 6: smoltcp's next deadline
                    // slot 7: what each socket is minted from (§255 (each socket is its own
                    // capability)); slot 8, its own address space, `run_with_own_space` adds.
                    rendezvous_cap(stack, Rights::WRITE.union(Rights::GRANT)),
                ],
                maps: &maps,
            },
            socket_contract::stack_slots::OWN_SPACE,
        )
    })
    .expect("could not spawn the net server");
    // Bound after the spawn rather than before START, which is safe for a reason worth writing
    // down: a signal that lands before the bind is counted in the word, and the bind (or the
    // server's next receive) delivers it. Nothing is lost by the order.
    crate::sched::notification_bind(wake, tid).expect("bind the retransmit notification");

    let mut held = Holding::new();
    held.add_thread(tid);
    held.add_region(ep_region);
    held.add_region_after_death(budget);
    held.add_region_after_death(stack_region);
    (report, stack, held)
}

/// The client's budget, in pages: it mints one shared frame and pays for that frame's own page
/// table plus its mapping; small and fixed.
const NET_CLIENT_BUDGET_PAGES: u64 = 16;

/// A stack client's stack, in pages. **Six, raised from two** by milestone 55's responder lane, and
/// the number came from a call frame rather than from taste: the multicast DNS responder's
/// `respond` held nine 256-byte wire names, the echoed questions and a TXT assembly buffer, about
/// five kilobytes of one frame. **That client was retired on 2026-09-15** (milestone 298,
/// notes/mdns.md) and the number was not re-measured for the socket client that is left, so six is
/// now margin rather than a measurement. Lowering it wants a stack-depth reading of
/// `socket_test_client`, not a guess.
const NET_CLIENT_STACK_PAGES: u64 = 6;

/// The most pages of read-only blob a stack client can be handed (see
/// `NET_CLIENT_CATALOGUE_VA`). One catalog line is about 93 bytes with its `sha256:` label, so one
/// page is some forty packages.
const NET_CLIENT_BLOB_PAGES: u64 = 1;

/// **Spawn the net server and a client of its socket contract** (milestone 30, piece 3 phase B).
/// Both are the `net_stack` binary (`image`): the server is entry role 0, the client is a nonzero
/// role (the client rides in the same binary to keep the initial ramdisk under its 15-file
/// directory limit). They share a `Stack` endpoint: `net_stack` holds `READ` (it serves), the
/// client holds `WRITE` (it requests). The client also gets its own untyped (to mint and delegate
/// the shared frame) and a report endpoint. `cli_arg` selects which exchange the client drives (UDP
/// DNS or TCP echo). Returns the client's report endpoint, or `None` if no NIC is attached.
///
/// `listen_grant` is the **inbound authority** this stack hands its client (milestone 107,
/// `socket_contract::listen_grant`): the port range a `LISTEN` may bind, and
/// [`socket_contract::NO_LISTEN_GRANT`] for every outbound exchange, which is most of them. The grant
/// is decided *here*, by whoever spawns the pair, and not by the client asking: that is the answer
/// to "who binds the port", and it is the same shape as handing a program a directory capability
/// rather than letting it name a path.
pub fn start_net_stack(
    image: &'static [u8],
    cli_arg: u64,
    pci: bool,
    listen_grant: u64,
) -> Option<(RendezvousId, Holding)> {
    start_net_stack_with(image, cli_arg, 0, None, pci, listen_grant)
}

/// Where a stack client finds a read-only blob its spawner handed it, when there is one. Must match
/// `components/src/socket_test_client.rs`'s `CATALOGUE_VA`: the one blob today is the image's
/// package catalogue (`package_archive::CATALOG`), and this is the kernel test harness playing
/// the part the progenitor's `ChildEndowment::blobs` plays for `login` (milestone 233 (`login` dies on every boot)).
const NET_CLIENT_CATALOGUE_VA: u64 = address_space_map::pair_page(0x0000_0000_00C0_0000);

/// **Rung 3a's fetch and verify** (milestone 198 (a package manager)): the net server and a client that fetches a
/// package over plain HTTP from the runners' package peer and checks it against `catalogue`, the
/// image's own package source, mapped into the client read-only, then fetches the peer's tampered
/// copy and must refuse it. `cli_arg` is the client's selector and `arg1` the catalogue's length,
/// in `socket_test_client`'s words; the two verdicts come back as the report's first two words.
pub fn start_package_fetch(
    image: &'static [u8],
    cli_arg: u64,
    arg1: u64,
    catalogue: &'static [u8],
) -> Option<(RendezvousId, Holding)> {
    start_net_stack_with(
        image,
        cli_arg,
        arg1,
        Some(catalogue),
        false,
        socket_contract::NO_LISTEN_GRANT,
    )
}

fn start_net_stack_with(
    image: &'static [u8],
    cli_arg: u64,
    cli_arg1: u64,
    blob: Option<&'static [u8]>,
    pci: bool,
    listen_grant: u64,
) -> Option<(RendezvousId, Holding)> {
    let (transport, intid, rid) = if pci {
        let d = crate::pci::find_net_device()?;
        (crate::virtio::Transport::pci(&d), d.intid, Some(d.rid))
    } else {
        let dev = crate::virtio::find_net_device()?;
        (
            crate::virtio::Transport::Mmio {
                mmio_phys: dev.mmio_phys,
            },
            dev.intid,
            None,
        )
    };

    let (net_stack_report, stack, mut held) =
        wire_net_server(image, transport, intid, rid, listen_grant);
    let cli_report = spawn_stack_client(image, cli_arg, cli_arg1, blob, stack, &mut held);

    // net_stack reports its DHCP lease with a blocking `send`; drain it here so net_stack unblocks and
    // enters its serve loop (the client's first request blocks until it does). This also
    // confirms DHCP completed before the client's exchange runs. The client, spawned above,
    // waits at its first request meanwhile.
    crate::sched::ipc_receive(net_stack_report);

    Some((cli_report, held))
}

/// Spawn one client of a `Stack` endpoint: WRITE on the shared endpoint, its own untyped, a
/// report endpoint, [`NET_CLIENT_STACK_PAGES`] extra stack pages, no heap. The body of
/// [`start_net_stack`]'s socket-contract client; `arg0`/`arg1` are the client's, and mean whatever
/// its `_start` says they mean.
///
/// It used to take a directory capability and a credential endpoint too, for the SMB adapter that
/// was a third client of this spawn; both went with that program on 2026-08-30 (notes/smb.md). Its
/// other caller, `start_shared_net_stack`, spawned the multicast DNS responder as a second client
/// and went with that program on 2026-09-15 (milestone 298, notes/mdns.md). A second client of one
/// `Stack` is a single call to this function away, which is why it stays a function.
///
/// `held` is the caller's [`Holding`]: this client's thread and the three regions behind it are
/// added to whatever the net server already put there, so one `release` at the end of a test ends
/// the whole service rather than the server alone.
pub(super) fn spawn_stack_client(
    image: &'static [u8],
    arg0: u64,
    arg1: u64,
    blob: Option<&'static [u8]>,
    stack: RendezvousId,
    held: &mut Holding,
) -> RendezvousId {
    use crate::cap::memory_region_lending_capability;

    // The client's report endpoint and its stack pages share one region with the budget's
    // lifetime rules: the report endpoint may go while the client still exists (that is the wake),
    // the stack pages may not. A client that reports and exits needs neither, but a client that
    // wedges is exactly the case worth being able to end.
    let cli_eps = crate::memory_region::create(2).expect("no endpoint region for the net client");
    let cli_report =
        crate::sched::create_rendezvous_from(cli_eps).expect("no client report endpoint");
    let cli_budget = crate::memory_region::create(NET_CLIENT_BUDGET_PAGES)
        .expect("no untyped for the net client");
    // The blob's pages ride in the stack region: the same lifetime (they may not go while the
    // client exists) and one fewer region to account for.
    let blob_pages = blob.map_or(0, |b| (b.len() as u64).div_ceil(FRAME_SIZE));
    assert!(
        blob_pages <= NET_CLIENT_BLOB_PAGES,
        "a {}-byte blob is more than a stack client's {NET_CLIENT_BLOB_PAGES} blob pages",
        blob.map_or(0, <[u8]>::len)
    );
    let cli_stack_region = crate::memory_region::create(NET_CLIENT_STACK_PAGES + blob_pages)
        .expect("no stack region for the net client");
    // One slot per stack page. This array carried extra tail slots for the SMB adapter's shared
    // regions until 2026-08-30 (notes/smb.md); no remaining client of this spawn maps anything but
    // its own stack.
    let mut maps = [Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_data(),
    }; (NET_CLIENT_STACK_PAGES + NET_CLIENT_BLOB_PAGES) as usize];
    for (k, m) in maps
        .iter_mut()
        .take(NET_CLIENT_STACK_PAGES as usize)
        .enumerate()
    {
        // Retyped, so the pages come back with the region; `retype_page` zeroes them.
        let phys = crate::memory_region::retype_page(cli_stack_region)
            .expect("no frame for the net client");
        m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
        m.phys = phys;
    }
    let mut n_maps = NET_CLIENT_STACK_PAGES as usize;
    if let Some(bytes) = blob {
        for (k, chunk) in bytes.chunks(FRAME_SIZE as usize).enumerate() {
            let phys = crate::memory_region::retype_page(cli_stack_region)
                .expect("no frame for the net client's blob");
            // SAFETY: the page was just retyped (exclusively ours, zeroed, direct-mapped), and
            // `chunk` is at most one page.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    chunk.as_ptr(),
                    crate::arch::mmu::phys_to_virt(phys) as *mut u8,
                    chunk.len(),
                );
            }
            maps[n_maps] = Mapping {
                va: NET_CLIENT_CATALOGUE_VA + k as u64 * FRAME_SIZE,
                phys,
                flags: Flags::user_rodata(),
            };
            n_maps += 1;
        }
    }

    let tid = crate::sched::spawn(move || {
        let grants = [
            rendezvous_cap(cli_report, Rights::WRITE), // slot 0: report the verdict
            // slot 1: the stack endpoint, WRITE to send requests and to delegate the
            // shared frame onto it (the frame it mints already carries GRANT).
            rendezvous_cap(stack, Rights::WRITE),
            memory_region_lending_capability(cli_budget), // slot 2: mint and map the shared frame
        ];
        run(
            image,
            Spawn {
                arg0,
                arg1,
                arg2: 0,
                grants: &grants,
                maps: &maps[..n_maps],
            },
        )
    })
    .expect("could not spawn the net client");
    held.add_thread(tid);
    held.add_region(cli_eps);
    held.add_region_after_death(cli_budget);
    held.add_region_after_death(cli_stack_region);
    cli_report
}

/// The networked std client's heap budget and extra stack, both larger than the hand-written
/// client's: it is a full std program (formatting, `Vec`, `String`), so it needs the same
/// generous heap and stack the `std_exerciser` demo does.
const STD_NET_HEAP_PAGES: u64 = 256;
const STD_NET_STACK_PAGES: u64 = 32;

/// **Spawn the net server and a `std::net` client** (milestone 27 phase two): the same
/// `std_exerciser` std binary, but now given the network, so its `UdpSocket::bind` probe succeeds
/// and it drives a real UDP DNS query and a TCP echo round trip through `std::net`, whose PAL
/// binds to this same `net_stack` socket contract. `net_stack` is `net_stack_image` (entry role 0, holding the
/// NIC and `READ` on the `Stack` endpoint); `std_image` is the ordinary std ELF given the std
/// slot convention (heap untyped at 0, stdout at 1) plus the two net slots (the `Stack`
/// endpoint `WRITE` at 2, an untyped budget for its per-socket shared frames at 3). Over the
/// mmio transport; the PAL sits above the transport, so proving it on one is proving it. The
/// same binary spawned without slots 2 and 3 runs the offline transcript instead, which is what
/// makes "no ambient network" visible: authority, not the code, decides. Returns the program's
/// stdout endpoint for the test to reassemble and the program's thread, so the test can tell a
/// program that died from one that is still working (milestone 514 (the HVF std listener hangs in
/// some full runs)), or `None` if no NIC is attached.
///
/// `listen_grant` is the **inbound authority** the std program gets, and it is decided here rather
/// than asked for (milestone 64 (enough `std` to run somebody else's crate), `socket_contract::listen_grant`). It is the difference between a std
/// program that may accept connections and one that may not, and it is visible at this call site
/// before the program exists: `socket_contract::NO_LISTEN_GRANT` is a `std::net::TcpListener::bind`
/// that answers `PermissionDenied` for every port there is. This parameter used to be a hardcoded
/// `NO_LISTEN_GRANT` with a comment saying the PAL could not spend a grant; the PAL can now, so the
/// decision moved out to the caller where it belongs.
pub fn start_net_std(
    net_stack_image: &'static [u8],
    std_image: &'static [u8],
    listen_grant: u64,
) -> Option<(RendezvousId, crate::thread::ThreadId, Holding)> {
    use crate::cap::{memory_region_cap, memory_region_lending_capability};

    let dev = crate::virtio::find_net_device()?;
    let transport = crate::virtio::Transport::Mmio {
        mmio_phys: dev.mmio_phys,
    };
    let (net_stack_report, stack, mut held) =
        wire_net_server(net_stack_image, transport, dev.intid, None, listen_grant);

    let std_eps =
        crate::memory_region::create(2).expect("no endpoint region for the std net client");
    let report = crate::sched::create_rendezvous_from(std_eps).expect("no std net report endpoint");
    let heap =
        crate::memory_region::create(STD_NET_HEAP_PAGES).expect("no untyped for the std net heap");
    let frames = crate::memory_region::create(NET_CLIENT_BUDGET_PAGES)
        .expect("no untyped for the std net frames");
    let std_stack_region = crate::memory_region::create(STD_NET_STACK_PAGES)
        .expect("no stack region for the std net client");

    let mut stackmaps = [Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_data(),
    }; STD_NET_STACK_PAGES as usize];
    for (k, m) in stackmaps.iter_mut().enumerate() {
        // Retyped rather than allocated, so the stack comes back with its region once the program
        // is provably gone (`Holding::region_after_death`); `retype_page` zeroes it.
        let phys = crate::memory_region::retype_page(std_stack_region)
            .expect("no frame for the std net stack");
        m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
        m.phys = phys;
    }

    let tid = crate::sched::spawn(move || {
        run(
            std_image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    memory_region_cap(heap),                  // slot 0: the heap's budget
                    rendezvous_cap(report, Rights::WRITE),    // slot 1: stdout/stderr
                    rendezvous_cap(stack, Rights::WRITE),     // slot 2: the Stack endpoint
                    memory_region_lending_capability(frames), // slot 3: mint per-socket shared frames
                ],
                maps: &stackmaps,
            },
        )
    })
    .expect("could not spawn the networked std program");

    // Same discipline as start_net_stack: drain net_stack's blocking DHCP report so it reaches its
    // serve loop before the std program's first request, and confirm DHCP completed.
    crate::sched::ipc_receive(net_stack_report);

    held.add_thread(tid);
    held.add_region(std_eps);
    held.add_region_after_death(heap);
    held.add_region_after_death(frames);
    held.add_region_after_death(std_stack_region);
    Some((report, tid, held))
}

/// [`wire`] against the enumerated mmio disk, at `role`.
fn start_role(image: &'static [u8], role: u64) -> Option<RendezvousId> {
    let dev = crate::virtio::find_block_device()?;
    Some(wire(
        image,
        crate::virtio::Transport::Mmio {
            mmio_phys: dev.mmio_phys,
        },
        dev.intid,
        role,
        None, // virtio-mmio has no IOMMU in front of it on either board
    ))
}

/// [`wire`] against the enumerated PCIe disk, at `role`.
fn start_role_pci(image: &'static [u8], role: u64) -> Option<RendezvousId> {
    let d = crate::pci::find_block_device()?;
    Some(wire(
        image,
        crate::virtio::Transport::pci(&d),
        d.intid,
        role,
        Some(d.rid), // the PCIe requester id, the IOMMU keys its tables on it
    ))
}

const ROLE_VIRTIO_ATTACK: u64 = 8;

/// Spawn a MALICIOUS driver that tries to DMA over kernel memory, for the security test. It
/// holds a real `Virtio` capability and its own DMA region, and points a descriptor at the
/// kernel image. Returns the endpoint on which it reports whether the kernel refused it.
pub fn start_attacker(image: &'static [u8]) -> Option<RendezvousId> {
    let dev = crate::virtio::find_block_device()?;
    let dma = crate::memory::alloc_zeroed().expect("no DMA frame").addr();
    let vid = crate::virtio::register(
        crate::virtio::Transport::Mmio {
            mmio_phys: dev.mmio_phys,
        },
        dma,
        FRAME_SIZE,
        None,
    );
    let report = crate::sched::create_rendezvous();

    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: ROLE_VIRTIO_ATTACK,
                arg1: dma,
                arg2: 0,
                grants: &[
                    rendezvous_cap(report, Rights::WRITE),
                    irq_cap(dev.intid), // slot 1 (unused by the attacker; keeps virtio at slot 2)
                    virtio_cap(vid),    // slot 2
                ],
                maps: &[Mapping {
                    va: DMA_VA,
                    phys: dma,
                    flags: Flags::user_data(),
                }],
            },
        )
    })
    .expect("could not spawn the virtio attacker");

    Some(report)
}

const ROLE_VIRTIO_ATTACK_INDIRECT: u64 = 13;

/// Spawn a malicious driver that tries the **indirect-descriptor** escape: it negotiates
/// `INDIRECT_DESC` and submits one descriptor flagged indirect whose inner table aims the
/// device at the kernel image. Same wiring as [`start_attacker`], different role. The kernel
/// strips the feature and refuses the flag, so the attacker reports `1` (refused). Returns the
/// report endpoint.
pub fn start_attacker_indirect(image: &'static [u8]) -> Option<RendezvousId> {
    let dev = crate::virtio::find_block_device()?;
    let dma = crate::memory::alloc_zeroed().expect("no DMA frame").addr();
    let vid = crate::virtio::register(
        crate::virtio::Transport::Mmio {
            mmio_phys: dev.mmio_phys,
        },
        dma,
        FRAME_SIZE,
        None,
    );
    let report = crate::sched::create_rendezvous();

    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: ROLE_VIRTIO_ATTACK_INDIRECT,
                arg1: dma,
                arg2: 0,
                grants: &[
                    rendezvous_cap(report, Rights::WRITE),
                    irq_cap(dev.intid), // slot 1 (unused; keeps virtio at slot 2)
                    virtio_cap(vid),    // slot 2
                ],
                maps: &[Mapping {
                    va: DMA_VA,
                    phys: dma,
                    flags: Flags::user_data(),
                }],
            },
        )
    })
    .expect("could not spawn the indirect virtio attacker");

    Some(report)
}
