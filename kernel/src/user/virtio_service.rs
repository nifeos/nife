use super::*;
use crate::cap::{Rights, irq_cap, notification_cap, rendezvous_cap, timer_cap, virtio_cap};
use crate::sched::RendezvousId;
use crate::user::holding::Holding;

/// Where the driver expects its DMA page. Must match user/src/virtio.rs. The driver holds no
/// device registers: it drives the device through a `Virtio` capability, so it cannot point the
/// device outside this DMA region.
const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000);

const ROLE_VIRTIO_BLK: u64 = 3;
/// The write-path roles (milestone 32 (a real filesystem) phase 1); must match `block_driver.rs`.
const ROLE_VIRTIO_BLK_WRITE: u64 = 30;
const ROLE_VIRTIO_BLK_WRITE_ABANDON: u64 = 31;
/// The virtio-net driver role (milestone 30 (the network stack as a confined component)); must
/// match `block_driver.rs`.
const ROLE_VIRTIO_NET: u64 = 40;

/// Start a driver role against a discovered transport, and return its report endpoint.
///
/// The shared body of [`start`] (mmio), [`start_pci`] (PCIe) and the writer starters; every bus and
/// every role gets the same world. `rid` is the PCIe requester id the IOMMU keys its tables on;
/// mmio devices have no IOMMU in front of them and pass `None`. See notes/virtio.md.
fn wire(
    image: &'static [u8],
    transport: crate::virtio::Transport,
    intid: u32,
    role: u64,
    rid: Option<u32>,
) -> RendezvousId {
    // The driver builds requests from the DMA page's physical address, so it gets it in `arg1`.
    // Zeroed so stale RAM cannot look like a valid descriptor to the device.
    let dma = crate::memory::alloc_zeroed()
        .expect("no DMA frame for the virtio driver")
        .addr();

    let irq_ep = crate::sched::create_rendezvous();
    crate::sched::bind_irq(intid, irq_ep);
    crate::arch::irq::enable(intid);

    let report = crate::sched::create_rendezvous();

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

/// Start the same driver binary against the PCIe disk (§18 (the PCIe transport: one driver, two
/// buses)): enumeration by kernel/src/pci.rs, INTx through the interrupt controller, and the same
/// confinement, backed by the IOMMU when the machine has one (§20 (IOMMU-backed DMA isolation)).
/// The driver cannot tell which bus it is on.
pub fn start_pci(image: &'static [u8]) -> Option<RendezvousId> {
    start_role_pci(image, ROLE_VIRTIO_BLK)
}

/// Start the write-path driver against the mmio disk: it writes a
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

/// Start the virtio-net driver against the mmio NIC. It does a DHCP round trip over QEMU
/// user-mode networking and reports the offered address. `None` if no NIC is attached. The DMA
/// confinement polices both queues through the one `Virtio` capability.
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

/// The same net driver over the PCIe transport, behind the IOMMU (§20): the NIC is confined in
/// hardware to its DMA region and shadow page, `iommu_platform=on`, as the disk is.
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
/// transmit buffers and caches, plus the page tables for the heap and for clients' shared frames.
///
/// `net_stack::HEAP_MAX` caps the heap at 96 pages, which leaves 32 for page tables and clients'
/// frame mappings; if one number moves, the other must. See notes/net/memory-and-reclamation.md.
pub(super) const NET_SERVER_BUDGET_PAGES: u64 = 128;
/// smoltcp builds packets on the stack; one mapped stack page is not enough. Eight extra keeps
/// the poll loop clear (`allocator_exerciser` needed three for `alloc` collections; smoltcp asks more).
pub(super) const NET_SERVER_STACK_PAGES: u64 = 8;

/// Start the net server, the `net_stack` binary, which runs smoltcp over the confined NIC and does
/// DHCP; see [`wire_net_server`]. Returns the endpoint it reports its acquired address on and what
/// ends it, or `None` if no NIC is attached.
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

/// [`wire`] for the net server: the same confined transport, interrupt, DMA page and report
/// endpoint, plus a heap budget, extra stack pages, a retransmit timer and a `Stack` endpoint (slot
/// 4) where clients' socket-contract requests arrive. `listen_grant` goes in `arg2`.
///
/// Returns `(report endpoint, stack endpoint, what ends the service)`. A caller with no client
/// ignores the stack. See notes/net/memory-and-reclamation.md.
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

    // The endpoints come out of a region of their own, or the service cannot be ended: `net_stack`
    // blocks in `receive_cap(STACK)` forever, a `Blocked` thread never reaches the `schedule()` that
    // spends an armed kill (§16 (object revocation)), and only reclaiming this region aborts the
    // waiter. Six pages: three endpoints, the timer and its notification, and one spare.
    let ep_region = crate::memory_region::create(6).expect("no endpoint region for net_stack");
    let irq_ep = crate::sched::create_rendezvous_from(ep_region).expect("no irq endpoint");
    crate::sched::bind_irq(intid, irq_ep);
    crate::arch::irq::enable(intid);

    let report = crate::sched::create_rendezvous_from(ep_region).expect("no report endpoint");
    let stack = crate::sched::create_rendezvous_from(ep_region).expect("no stack endpoint");
    // The retransmit timer's notification is bound by the spawner, because a running thread holds
    // no capability to its own TCB (`notes/timer.md`, "Binding a notification to yourself").
    let wake = crate::sched::create_notification_from(ep_region).expect("no notification");
    let retransmit = crate::sched::create_timer_from(ep_region).expect("no retransmit timer");
    let vid = crate::virtio::register(transport, dma, FRAME_SIZE, rid);
    let budget =
        crate::memory_region::create(NET_SERVER_BUDGET_PAGES).expect("no untyped for net_stack");
    // The extra stack pages get a region of their own, freed only after the server is dead: a
    // `Spawn::maps` page is not a recorded mapping, so revocation cannot pull it, and freeing a
    // running thread's stack is a use-after-free. See `Holding::region_after_death`.
    let stack_region = crate::memory_region::create(NET_SERVER_STACK_PAGES)
        .expect("no stack region for net_stack");

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
    // Bound after the spawn: a signal that lands before the bind is counted in the word, and the
    // bind (or the server's next receive) delivers it.
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

/// A stack client's stack, in pages. Six is margin, not a measurement of `socket_test_client`;
/// lowering it wants a stack-depth reading of that client.
const NET_CLIENT_STACK_PAGES: u64 = 6;

/// The most pages of read-only blob a stack client can be handed (see
/// `NET_CLIENT_CATALOGUE_VA`). One catalog line is about 93 bytes with its `sha256:` label, so one
/// page is some forty packages.
const NET_CLIENT_BLOB_PAGES: u64 = 1;

/// Spawn the net server and a client of its socket contract, both from the `net_stack` binary
/// `image`: the server at entry role 0, the client at role `cli_arg` (UDP DNS or TCP echo). One
/// binary keeps the initial ramdisk under its 15-file directory limit.
///
/// They share a `Stack` endpoint, the server holding `READ` and the client `WRITE`. `pci` picks the
/// transport. `listen_grant` is the port range a `LISTEN` may bind
/// (`socket_contract::listen_grant`), or [`socket_contract::NO_LISTEN_GRANT`]; the spawner decides
/// it, not the client. Returns the client's report endpoint and what ends the service, or `None` if
/// no NIC is attached. See notes/net.md.
pub fn start_net_stack(
    image: &'static [u8],
    cli_arg: u64,
    pci: bool,
    listen_grant: u64,
) -> Option<(RendezvousId, Holding)> {
    start_net_stack_with(image, cli_arg, 0, None, pci, listen_grant)
}

/// Where a stack client finds a read-only blob its spawner handed it, when there is one. Must match
/// `components/src/socket_test_client.rs`'s `CATALOGUE_VA`. The one blob is the image's package
/// catalogue (`package_archive::CATALOG`), handed over as the progenitor's `ChildEndowment::blobs`
/// does for `login`.
const NET_CLIENT_CATALOGUE_VA: u64 = address_space_map::pair_page(0x0000_0000_00C0_0000);

/// Spawn the net server and a client that fetches a package over plain HTTP from the runners'
/// package peer, checks it against `catalogue` (mapped into the client read-only), then fetches the
/// peer's tampered copy and must refuse it. `cli_arg` is the client's selector and `arg1` the
/// catalogue's length; the two verdicts come back as the report's first two words.
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

    // net_stack reports its DHCP lease with a blocking `send`; until it is received, net_stack
    // cannot reach its serve loop and the client's first request waits.
    crate::sched::ipc_receive(net_stack_report);

    Some((cli_report, held))
}

/// Spawn one client of a `Stack` endpoint and return its report endpoint.
///
/// The client holds WRITE on `stack`, its own untyped, a report endpoint and
/// [`NET_CLIENT_STACK_PAGES`] extra stack pages, and no heap; `blob`, if any, is mapped read-only at
/// [`NET_CLIENT_CATALOGUE_VA`]. `arg0` and `arg1` mean whatever the client's `_start` says. Its
/// thread and regions are added to `held`, so one `release` ends the whole service.
pub(super) fn spawn_stack_client(
    image: &'static [u8],
    arg0: u64,
    arg1: u64,
    blob: Option<&'static [u8]>,
    stack: RendezvousId,
    held: &mut Holding,
) -> RendezvousId {
    use crate::cap::memory_region_lending_capability;

    // The report endpoint's region may go while the client exists (that is the wake); the stack
    // region may not.
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

/// Spawn the net server (`net_stack_image`) and a `std::net` client (`std_image`), over mmio.
///
/// The client gets the std slots (heap untyped at 0, stdout at 1) plus the `Stack` endpoint's
/// `WRITE` at 2 and a budget for per-socket shared frames at 3; without 2 and 3 the same binary runs
/// its offline transcript. `listen_grant` decides whether `std::net::TcpListener::bind` may succeed:
/// `socket_contract::NO_LISTEN_GRANT` answers `PermissionDenied` for every port.
///
/// Returns the program's stdout endpoint, its thread (so a test can tell a dead program from a busy
/// one) and what ends the service, or `None` if no NIC is attached. See notes/net.md.
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

    // net_stack's DHCP report is a blocking `send`; until it is received, it cannot serve.
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

/// Spawn a malicious driver that tries the indirect-descriptor escape: it negotiates
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
