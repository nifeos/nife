use core::sync::atomic::Ordering;

// Shared with the aarch64 module so both ISAs assert a std transcript the same way.
use super::std_tests::{
    assert_a_kill_mid_transaction_recovers, assert_attrs, assert_fs_service_ready,
    assert_std_transcript, assert_std_transcript_from, std_fs_expected,
};
use super::*;
use crate::sched;

/// The `block_driver` program's ELF bytes from the riscv initial ramdisk archive. Absent means the
/// initial ramdisk was built without it (or the aarch64 archive was handed to a riscv boot, the
/// mix-up the xtask riscv test leg exists to prevent); fail loudly rather than skip.
fn blk_image() -> &'static [u8] {
    program("block_driver").expect("no block_driver program in the initial ramdisk")
}

/// The `net_stack` program's ELF bytes (milestone 30, piece 3): the smoltcp net server.
fn net_stack_image() -> &'static [u8] {
    program("net_stack").expect("no net_stack program in the initial ramdisk")
}

/// The net client's test selectors and success word, matching `components/src/socket_test_client.rs`. The client is
/// a nonzero entry role of the `net_stack` binary, so it needs no image of its own.
const NET_TEST_NAME_RESOLUTION: u64 = 1;
const NET_TEST_TCP_ECHO: u64 = 2;
const NET_TEST_TCP_REOPEN: u64 = 3;
const NET_TEST_UDP_TFTP: u64 = 4;
const NET_TEST_TCP_ACCEPT: u64 = 5;
const NET_TEST_HTTP_PACKAGE: u64 = 6;
const NET_TEST_TCP_ACCEPT_UNHEARD: u64 = 12;
/// `TEST_HTTP_PACKAGE`'s refusal word, reported for a package whose digest disagrees.
const NET_CLIENT_DIGEST_REFUSED: u64 = 3;
/// The one port the inbound gate is granted (milestone 107); the runners forward a host port to
/// it. Both ISA legs use the same number and the same host port, because they run one after the
/// other and never hold it at once. From `socket_contract::fixture` since milestone 64 (enough `std` to run somebody else's crate): see the
/// aarch64 twin for why the number has one definition.
const NET_LISTEN_PORT: u16 = socket_contract::fixture::LISTEN_PORT;
/// The fixed UDP port range the accept test's stack is granted (milestone 55's UDP bind grant).
/// `socket_test_client` binds 5354 inside it and is refused 4444 outside it. See the aarch64 twin
/// for why the range still starts at 5353.
const NET_UDP_GRANT_BOTTOM: u16 = 5353;
const NET_UDP_GRANT_TOP: u16 = 5354;
const NET_CLIENT_OK: u64 = 1;
/// The client could not complete for an ENVIRONMENTAL reason (the host resolver never answered),
/// not because of a defect here. Only the non-gating real-DNS half of the name test can report it.
const NET_CLIENT_NO_ANSWER: u64 = 2;

/// Spin the scheduler until `done()`, or give up after a wall-clock deadline. A second copy of
/// the `tests` module's helper, one of the small duplications this module carries; time-based
/// for the same reason (DECISIONS §28: a fixed yield count elapses in no real time on an idle
/// core).
fn wait_for(mut done: impl FnMut() -> bool) -> bool {
    let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if done() {
            return true;
        }
        sched::yield_now();
    }
    done()
}

/// **A faulting riscv user thread dies, and the kernel does not.** DECISIONS §10's promise
/// ("a driver bug is a crashed process, not a dead machine"), proven on the second ISA for
/// the first time.
///
/// This test exists because the promise was NOT kept here: the riscv trap dispatcher stepped
/// over a U-mode `ebreak` (so a panicking driver resumed its own panic loop, alive forever)
/// and panicked the kernel on any other U-mode fault, behind a comment claiming user threads
/// could not run on RISC-V yet. Every riscv userspace binary's panic handler ends in `ebreak`
/// expecting to die, and no test had ever made one fault. The kill-mid-write test (below)
/// needs a driver to genuinely die, which is what flushed this out.
///
/// The blk binary's `_start` panics on an unknown role, so spawning it with one is the
/// smallest honest fault: panic, `ebreak`, killed, reaped.
#[test_case]
fn a_faulting_user_thread_is_killed_and_the_kernel_survives() {
    use crate::arch::exceptions::USER_FAULTS;

    let faults = USER_FAULTS.load(Ordering::Relaxed);
    let threads = sched::thread_count();

    sched::spawn(move || {
        run(
            blk_image(),
            Spawn {
                arg0: 0xDEAD, // no such role: _start panics, and the panic handler ebreaks
                arg1: 0,
                arg2: 0,
                grants: &[],
                maps: &[],
            },
        )
    })
    .expect("spawn failed");

    assert!(
        wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > faults),
        "the faulting user thread was never killed",
    );
    assert!(
        wait_for(|| sched::thread_count() <= threads),
        "the killed thread was never reaped",
    );
}

/// **The confused-deputy question, asked on the ISA where the answer is harder** (the RISC-V
/// twin of `the_hardware_says_el0_cannot_read_the_kernels_memory`, milestone 41).
///
/// aarch64 has `AT S1E0R`, one instruction that makes the silicon answer "could EL0 read
/// this". RISC-V has no such instruction, so `mmu::user_can_read` walks the installed tables
/// in software and reads the `U` bit. That software answer is the thing under test, and it
/// matters more here than on aarch64: RISC-V has one root register, so the **same** page
/// tables that translate a user address also translate the kernel's, and the `U` bit is the
/// only thing between U-mode and the kernel's own memory. On aarch64 the split TTBR0/TTBR1
/// gives a second, structural line of defence. Here there is one line, and this is it.
///
/// The precondition assertion is the same one the aarch64 test carries, for the same reason:
/// "U-mode cannot read the kernel" proves nothing if the kernel is not mapped here at all.
/// On RISC-V it also proves the kernel-half share actually happened.
///
/// This test exists because milestone 41 removed the crate-wide `allow(dead_code)` that riscv64
/// builds carried, and `user_can_read`/`user_can_write` fell out of it as dead on this ISA
/// only: the aarch64 test module proved them, and nothing on RISC-V ever called them.
///
/// Falsification: replayable `system_tests/falsifications/user.riscv_virtio_tests.the_page_tables_say_u_mode_cannot_read_the_kernels_memory.patch`
#[test_case]
fn the_page_tables_say_u_mode_cannot_read_the_kernels_memory() {
    // Inside the direct map, so it is mapped for certain and it is the kernel's own memory.
    let kernel_va = crate::arch::mmu::KERNEL_VA_BASE + 0x8000_0000;

    // Any real compiled user ELF will do here, and every architecture packs this one.
    let image = program("report_child").expect("no report_child program in the initial ramdisk");
    let (space, _) = load(image, 0).expect("the initial ramdisk did not load");

    space.while_installed(|| {
        assert!(
            mmu::translate(kernel_va).is_some(),
            "the kernel's direct map is not mapped, so this test proves nothing",
        );
        assert!(
            !mmu::user_can_read(kernel_va),
            "the page tables say U-mode could read the kernel's own memory",
        );
        assert!(!mmu::user_can_write(kernel_va));

        // And it says yes to the process's own text, or the check is a rubber stamp.
        assert!(
            mmu::user_can_read(address_space_map::IMAGE_BASE),
            "U-mode cannot read its own .text, so the check refuses everything and proves nothing",
        );

        // Not an unmapped address in its own half: the stack band's guard page, which the
        // address-space map promises no loader ever maps.
        assert!(!mmu::user_can_read(address_space_map::STACK.start));
    });
    drop(space);
}

/// The headline, on the second ISA: an unprivileged process drives a real block device over
/// DMA and reads a file off it, with the kernel owning only the confinement. Interrupt
/// delivery is asserted too: the completion reached the driver as a message through its Irq
/// capability, via the PLIC rather than the GIC.
#[test_case]
fn a_userspace_driver_reads_a_file_from_a_virtio_disk() {
    use crate::arch::exceptions::ROUTED_IRQS;

    // Sampled before the device is wired, not after: `ROUTED_IRQS` is bumped ahead of the
    // `irq_notify` that wakes the driver, so a report proves the counter moved, and the only way
    // this assertion can fail is a baseline taken after the completion. See
    // notes/load-sensitive-assertions.md and the aarch64/x86_64 twin.
    let irqs_before = ROUTED_IRQS.load(Ordering::Relaxed);

    let Some(report) = virtio_service::start(blk_image()) else {
        // No disk attached to this run. Nothing to test; do not fail.
        crate::testing::skip!("no virtio disk attached");
    };

    let word = sched::ipc_receive(report)[0];

    assert_eq!(
        &word.to_le_bytes(),
        b"nife: re",
        "the driver reported the wrong file contents",
    );
    let irqs_after = ROUTED_IRQS.load(Ordering::Relaxed);
    assert!(
        irqs_after > irqs_before,
        "the read completed but no device interrupt was delivered as a message \
         ({irqs_before} routed before the device was wired, {irqs_after} after the report)",
    );
}

/// **The RedoxFS filesystem service, end to end, on the second ISA** (milestone 32 phase 2).
/// The aarch64 twin's contract, proven identically on riscv by the same suite (the parity gate):
/// a block server, an FS server over blk IPC, and a client that opens a file through a granted
/// directory capability, reads it, round-trips a write, and reports. The block-server role rides
/// the portable `block_driver` binary here instead of hello.
#[test_case]
fn the_redoxfs_server_serves_redoxfs_over_a_capability_contract() {
    let Some((readiness, report)) = fs_service::start(
        blk_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("fs_test_client").expect("no fs_test_client program in the initial ramdisk"),
        0, // the end-to-end proof role, not the benchmark loop
    ) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };

    assert_fs_service_ready(readiness);
    let [head, status, attrs, ..] = sched::ipc_receive(report);
    assert_eq!(
        status,
        filesystem_fixture::SUCCESS,
        "the client did not report success: a check in the read or write path failed",
    );
    assert_eq!(
        &head.to_le_bytes()[..],
        &filesystem_fixture::MOTD[..8],
        "the client read the wrong motd bytes off the RedoxFS image",
    );
    // Milestone 57: the same attribute witness, the same exact expected set. The layer is the
    // FS server's own logic rather than anything arch-specific, so what this leg adds is the
    // parity gate's own claim (DECISIONS §19): it ships on both ISAs or it does not ship.
    assert_attrs(attrs);
}

/// **`std::fs` over the FS-service contract, on the second ISA** (milestone 27 phase two, the
/// parity gate). The aarch64 twin's proof, same binary, same contract, same transcript: a std
/// program granted one directory capability reads the file the RedoxFS image ships and is
/// refused every path that would leave that directory. See the aarch64 twin for what it proves.
#[test_case]
fn std_fs_reads_a_file_through_a_granted_directory_capability() {
    let Some((readiness, report)) = fs_service::start_std(
        blk_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("std_exerciser").expect("no std_exerciser program in the initial ramdisk"),
    ) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_fs_service_ready(readiness);

    let mut want = [0u8; 768];
    let n = std_fs_expected(&mut want);
    assert_std_transcript(report, &want[..n], "std fs");
}

/// **A read-only per-file grant, attacked, on the second ISA** (the parity gate). The aarch64
/// twin carries the reasoning: same caretaker, same attacker, same verdict.
///
/// Milestone 61's attribute forwarding is inside this zero, in both directions: the attribute
/// reads reached the store (`GRANTED_ATTRS_FAILED` clear) and the attribute write did not
/// (`WROTE_ATTR` clear). Spelled out in the aarch64 twin's second assertion; here the exact
/// verdict carries it, which is what the parity gate wants (§19: the same suite, both ISAs).
#[test_case]
fn a_read_only_per_file_grant_survives_an_attacker() {
    let Some(verdict) = attack_a_grant(filesystem_contract::grant::READ, false) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_eq!(
        verdict, 0,
        "the read-only per-file grant leaked (see the aarch64 twin for what each bit means)",
    );
}

/// **A writable per-file grant, attacked, on the second ISA** (the parity gate, and the read-only
/// test's control here too). See the aarch64 twin.
#[test_case]
fn a_writable_per_file_grant_writes_that_file_and_still_only_that_file() {
    use filesystem_fixture::escape;
    let Some(verdict) = attack_a_grant(
        filesystem_contract::grant::READ | filesystem_contract::grant::WRITE,
        true,
    ) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_eq!(
        verdict,
        escape::WROTE | escape::TRUNCATED | escape::WROTE_ATTR,
        "a writable grant must write, truncate and set an attribute on its own file and do \
         nothing else",
    );
}

/// The riscv half of the aarch64 twin's helper; the only difference is the block-server binary
/// (the portable `block_driver` here, the PL011-tied `hello` there).
fn attack_a_grant(rights: u64, writable: bool) -> Option<u64> {
    let Some(report) = fs_service::start_granted(
        blk_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("fs_file_caretaker").expect("no fs_file_caretaker program in the initial ramdisk"),
        program("fs_test_client").expect("no fs_test_client program in the initial ramdisk"),
        fs_service::Grant {
            name: if writable {
                filesystem_fixture::SCRATCH_NAME
            } else {
                filesystem_fixture::MOTD_NAME
            },
            rights,
            role: 2, // ROLE_ATTACKER
            arg: writable as u64,
        },
    ) else {
        // No print, no skip!() here: this is a helper, and `skip!()` returns from the function it
        // is written in, which would leave the test running. `None` is the fixture's absence
        // travelling to the `#[test_case]`, which is the only place that can honestly skip.
        return None;
    };
    // The two handshakes happened inside `start_granted`, before this attacker existed: they
    // are what makes the caretaker's own staged request safe on a page all three share.
    let [tag, verdict, ..] = sched::ipc_receive(report);
    assert_eq!(
        tag,
        filesystem_fixture::VERDICT,
        "the attacker's report is not a verdict word",
    );
    Some(verdict)
}

/// **The FS server's stack headroom, on the second ISA** (the parity gate). The aarch64 twin
/// carries the reasoning; the number is worth having on both because the two ISAs do not use the
/// same amount of stack for the same recursion, and a size proven on one proves nothing on the
/// other.
/// **A kill mid-transaction, on the real device, on the second ISA** (milestone 37, the §19
/// parity twin of the aarch64 test). Same six steps, same property, same words.
#[test_case]
fn a_kill_mid_transaction_leaves_the_filesystem_consistent() {
    if !fs_service::is_crash_disk_present() {
        crate::testing::skip!("no crash disk attached");
    }
    assert_a_kill_mid_transaction_recovers(
        blk_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("fs_test_client").expect("no fs_test_client program in the initial ramdisk"),
    );
}

#[test_case]
fn the_redoxfs_servers_stack_still_has_headroom() {
    let Some((used, total)) = fs_service::fs_stack_used() else {
        crate::testing::skip!("no FS service wired this boot");
    };
    crate::println!("    (FS server stack high-water: {used} of {total} bytes) ");
    assert!(
        used * 4 <= total * 3,
        "the FS server used {used} of {total} stack bytes: under a quarter left. RedoxFS \
         recurses in 8 KiB frames, so the next verb that deepens a tree walk will overflow and \
         the server will die mid-request. Raise FS_STACK_PAGES.",
    );
}

/// The virtio-net DHCP round trip, on the second ISA (milestone 30): a driver at EL0 brings up
/// both queues, transmits a DHCP DISCOVER, and receives slirp's OFFER, all behind the multi-queue
/// confinement, with the completion delivered via the PLIC. Parity with the aarch64 net test.
#[test_case]
fn a_userspace_driver_completes_a_dhcp_round_trip_over_virtio_net() {
    let Some(report) = virtio_service::start_net(blk_image()) else {
        crate::testing::skip!("no virtio-net device attached");
    };

    let yiaddr = sched::ipc_receive(report)[0] as u32;
    assert_eq!(
        yiaddr & 0xffff_ff00,
        0x0A00_0200,
        "the DHCP OFFER's yiaddr {yiaddr:#010x} is not in QEMU slirp's 10.0.2.0/24",
    );
    // No fresh-interrupt assertion here; see the aarch64 twin. The net completion is the used
    // ring, not one interrupt per operation, and the shared-NIC test suite makes a strict
    // interrupt-delta unreliable. The OFFER round trip is the proof.
}

/// The riscv net round trip over PCIe, behind the RISC-V IOMMU (milestone 30, §20).
#[test_case]
fn a_userspace_driver_completes_a_dhcp_round_trip_over_virtio_net_pci() {
    let Some(report) = virtio_service::start_net_pci(blk_image()) else {
        crate::testing::skip!("no virtio-net-pci device attached");
    };

    let yiaddr = sched::ipc_receive(report)[0] as u32;
    assert_eq!(
        yiaddr & 0xffff_ff00,
        0x0A00_0200,
        "the DHCP OFFER's yiaddr {yiaddr:#010x} over PCIe is not in QEMU slirp's 10.0.2.0/24",
    );
}

/// The net server (smoltcp) acquiring a DHCP lease over the confined NIC, on the second ISA
/// (milestone 30, piece 3). A reused userspace TCP/IP stack, driving a kernel-confined device.
#[test_case]
fn the_net_server_acquires_a_dhcp_lease_over_smoltcp() {
    let Some((report, net)) = virtio_service::start_net_server(net_stack_image()) else {
        crate::testing::skip!("no virtio-net device attached");
    };
    let [addr, nameserver, ..] = sched::ipc_receive(report);
    let addr = addr as u32;
    assert_eq!(
        addr & 0xffff_ff00,
        0x0A00_0200,
        "smoltcp's DHCP lease {addr:#010x} is not in QEMU slirp's 10.0.2.0/24",
    );
    // slirp's DHCP names 10.0.2.3 as the DNS server, and the lease report carries it in its second
    // word (`socket_contract::lease`, provisional) for a resolver's spawner (§248 (the name
    // resolver is its own confined program)).
    assert_eq!(
        socket_contract::lease::word_ipv4(nameserver),
        Some([10, 0, 2, 3]),
        "the lease report did not carry slirp's DNS server (word {nameserver:#x})",
    );
    net.release_or_fail("a net test's net_stack");
}

/// The riscv net server over PCIe, behind the RISC-V IOMMU (milestone 30, §20).
#[test_case]
fn the_net_server_acquires_a_dhcp_lease_over_smoltcp_pci() {
    let Some((report, net)) = virtio_service::start_net_server_pci(net_stack_image()) else {
        crate::testing::skip!("no virtio-net-pci device attached");
    };
    let addr = sched::ipc_receive(report)[0] as u32;
    assert_eq!(
        addr & 0xffff_ff00,
        0x0A00_0200,
        "smoltcp's DHCP lease {addr:#010x} over PCIe is not in QEMU slirp's 10.0.2.0/24",
    );
    net.release_or_fail("a net test's net_stack");
}

/// The socket contract, UDP end to end on the second ISA (milestone 30, piece 3 phase B): a
/// client sends a datagram and reads the reply through the granted `Stack` endpoint and shared
/// frame. The peer is slirp's own TFTP server, so the exchange is deterministic and offline; see
/// the aarch64 twin for why the old DNS-based version was environment-dependent.
#[test_case]
fn a_client_completes_a_udp_round_trip_through_the_socket_contract() {
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_UDP_TFTP,
        false,
        socket_contract::NO_LISTEN_GRANT,
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };
    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "the UDP round trip against slirp's TFTP server failed (client code {verdict:#x})",
    );
    net.release_or_fail("a net test's net_stack");
}

/// The riscv UDP round trip over PCIe, behind the RISC-V IOMMU.
#[test_case]
fn a_client_completes_a_udp_round_trip_through_the_socket_contract_pci() {
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_UDP_TFTP,
        true,
        socket_contract::NO_LISTEN_GRANT,
    ) else {
        crate::testing::skip!("no virtio-net-pci device attached");
    };
    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "the UDP round trip over PCIe failed (client code {verdict:#x})",
    );
    net.release_or_fail("a net test's net_stack");
}

/// A host name resolves through the socket contract on the second ISA, and the lies do not
/// (milestone 384 (in a capability system the resolver is a grant)). The first half gates and the
/// real-DNS half does not, for the aarch64 twin's reasons.
/// Falsification: replayable `system_tests/falsifications/user.riscv_virtio_tests.a_name_resolves_through_the_stack_and_a_real_one_when_the_host_answers.patch`
#[test_case]
fn a_name_resolves_through_the_stack_and_a_real_one_when_the_host_answers() {
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_NAME_RESOLUTION,
        false,
        socket_contract::NO_LISTEN_GRANT,
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };
    let [gating, real, ..] = sched::ipc_receive(report);
    assert_eq!(
        gating, NET_CLIENT_OK,
        "name resolution against the runners' name server failed (client code {gating:#x})",
    );
    net.release_or_fail("a net test's net_stack");
    if real == NET_CLIENT_NO_ANSWER {
        // The partial shape milestone 214 (a test that prints "skipping" and returns is counted as
        // passed) settled on: the gating half ran and was asserted above, and the conditioned half
        // was never put to the test, so the reason says which is which.
        crate::testing::skip!(
            "the name server half passed; the host's resolver did not answer, so no real DNS name \
             was resolved this run"
        );
    }
    assert_eq!(
        real, NET_CLIENT_OK,
        "a DNS response came back from the host's resolver but was not a valid answer to our \
         query (client code {real:#x}): a defect here, not a network problem",
    );
}

/// The socket contract, TCP end to end on the second ISA: connect to slirp's guestfwd echo peer,
/// send, receive the echo, close, the full round trip through the confined NIC.
#[test_case]
fn a_client_echoes_over_tcp_through_the_socket_contract() {
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_TCP_ECHO,
        false,
        socket_contract::NO_LISTEN_GRANT,
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };
    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "the TCP echo round trip through the socket contract failed (client code {verdict:#x})",
    );
    net.release_or_fail("a net test's net_stack");
}

/// The riscv twin of `network_tests::a_package_fetched_over_http_is_accepted_only_by_the_image_digest`
/// (milestone 198 (a package manager) rung 3a): `uptime 0.1.0 riscv64`, genuine then tampered.
#[test_case]
fn a_package_fetched_over_http_is_accepted_only_by_the_image_digest() {
    let catalogue = program(package_archive::CATALOG)
        .expect("no package catalogue in the initial ramdisk: the archive build packs one");
    let Some((report, net)) = virtio_service::start_package_fetch(
        net_stack_image(),
        NET_TEST_HTTP_PACKAGE,
        catalogue.len() as u64,
        catalogue,
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };
    let [genuine, tampered, ..] = sched::ipc_receive(report);
    assert_eq!(
        genuine, NET_CLIENT_OK,
        "the package fetched over HTTP was not accepted (client code {genuine:#x}; \
         {NET_CLIENT_DIGEST_REFUSED} means its digest disagreed with the image's catalogue)",
    );
    assert_eq!(
        tampered, NET_CLIENT_DIGEST_REFUSED,
        "a package with one byte flipped was not refused by digest (client code {tampered:#x}; \
         {NET_CLIENT_OK} would mean it was accepted)",
    );
    net.release_or_fail("the package fetch's net_stack");
}

/// The riscv TCP echo round trip over PCIe, behind the RISC-V IOMMU.
#[test_case]
fn a_client_echoes_over_tcp_through_the_socket_contract_pci() {
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_TCP_ECHO,
        true,
        socket_contract::NO_LISTEN_GRANT,
    ) else {
        crate::testing::skip!("no virtio-net-pci device attached");
    };
    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "the TCP echo round trip over PCIe failed (client code {verdict:#x})",
    );
    net.release_or_fail("a net test's net_stack");
}

/// **An accept nobody answers ends at `net_stack`'s bound** (the riscv-cpu-net-flake lane,
/// 2026-10-13 UTC). The aarch64 twin in `network_tests` carries the reasoning.
///
/// Falsification: replayable `system_tests/falsifications/user.riscv_virtio_tests.an_accept_nobody_answers_ends_at_the_stacks_bound.patch`
#[test_case]
fn an_accept_nobody_answers_ends_at_the_stacks_bound() {
    let started = crate::arch::timer::now();
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_TCP_ACCEPT_UNHEARD,
        true,
        socket_contract::listen_grant(NET_LISTEN_PORT, NET_LISTEN_PORT),
    ) else {
        crate::testing::skip!("no virtio-net-pci device attached");
    };
    let verdict = sched::ipc_receive(report)[0];
    let waited_ms = (crate::arch::timer::now() - started) * 1000 / crate::arch::timer::frequency();
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "an accept nobody could reach did not come back refused (client code {verdict:#x})",
    );
    assert!(
        waited_ms >= 15_000,
        "the accept was refused after {waited_ms} ms, before net_stack's 15 s bound",
    );
    net.release_or_fail("a net test's net_stack");
}

/// Regression on the second ISA: reopening a socket id and connecting again completes (the
/// ephemeral-port fix). See the aarch64 twin for the finding.
#[test_case]
fn a_reopened_socket_id_connects_again_over_tcp() {
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_TCP_REOPEN,
        false,
        socket_contract::NO_LISTEN_GRANT,
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };
    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "reopening a socket id and connecting again failed (client code {verdict:#x})",
    );
    net.release_or_fail("a net test's net_stack");
}

/// **The guest is connected TO, on a granted port, on the second ISA** (milestone 107). A port
/// outside the stack's grant is refused as a matter of authority, the granted one binds and is
/// exclusive, and then a host process opens a TCP connection to it twice through QEMU's `hostfwd`
/// while the guest accepts, reads and answers each. **The UDP bind grant then rides in the same
/// spawn** (milestone 55's stack half): it refuses a port outside the grant and admits one inside
/// it, exclusively. **Milestone 54's SMB adapter and milestone 55's multicast DNS responder rode
/// the same spawn on this ISA too**, until 2026-08-30 and 2026-09-15 respectively; notes/smb.md and
/// notes/mdns.md record them. See the aarch64 twin
/// for the shape, for why all of it shares one exchange (a net server's spawn is frames nothing
/// reclaims), and for what the stage codes mean.
#[test_case]
fn a_host_process_connects_to_the_guest_and_is_answered() {
    // E2's baseline (milestone 134): see the aarch64 twin's comment at the same point. The full
    // suite runs 279 `#[test_case]`s in one continuous boot, so `sched::thread_count()` taken here
    // is what "before this test wired anything" means, and the census below reports the delta
    // against it rather than an absolute reading contaminated by whatever ran first.
    let e2_baseline_threads = sched::thread_count();
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_TCP_ACCEPT,
        false,
        socket_contract::listen_grant(NET_LISTEN_PORT, NET_LISTEN_PORT)
            | socket_contract::udp_bind_grant(NET_UDP_GRANT_BOTTOM, NET_UDP_GRANT_TOP),
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };
    // E2 (milestone 134, design/roadmap/0134-the-measurements-that-decide.md): the thread census on
    // the customer path. Every process this topology needs is already spawned by this point
    // (`net_stack` and the echo client), and neither spawns another kernel
    // thread per connection or per request (each is a single-threaded event loop over its own
    // endpoint), so this count is already the peak. Reported as a delta against
    // `e2_baseline_threads`, not as the absolute reading. See the aarch64 twin, including its note
    // on the wider topology this census covered until 2026-08-30 (notes/smb.md).
    crate::println!(
        "    (E2 thread census: {} threads created by wiring this customer-path topology \
         ({} live now, {e2_baseline_threads} live before this test wired anything): main + \
         net_stack + echo client)",
        sched::thread_count().saturating_sub(e2_baseline_threads),
        sched::thread_count(),
    );
    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "the guest did not serve the inbound exchange (client code {verdict:#x}); 0xE050/0xE080 \
         mean a port outside a grant was bound anyway, 0xE060 or 0xE070 means nobody ever \
         connected (the host side), 0xE082/0xE084 mean the UDP bind grant admitted or refused the \
         wrong port",
    );
    net.release_or_fail("a net test's net_stack");
}

/// The `std_exerciser` std program's ELF bytes from the riscv initial ramdisk. Given the network
/// here, its `UdpSocket::bind` probe succeeds and it runs the net transcript.
fn std_exerciser_image() -> &'static [u8] {
    program("std_exerciser").expect("no std_exerciser program in the initial ramdisk")
}

/// The exact transcript `std_exerciser` prints when it is granted the network **and refused every
/// listening port**. See the aarch64 twin for why `listen refused` is milestone 64's negative
/// control and why it costs this boot nothing.
const STD_NET_EXPECTED: &[u8] = b"std net on nife\nlisten refused\nudp ok\ntcp echo ok\n";

/// The exact transcript the same binary prints when its stack **is** granted the listening port
/// (milestone 64's inbound half). See the aarch64 twin for what each of the four lines claims.
const STD_LISTEN_EXPECTED: &[u8] =
    b"std net on nife\nlisten ok\ndenied refused\nin use refused\nserved 2\n";

/// **`std::net` end to end over the socket contract, on the second ISA** (milestone 27 phase
/// two): the riscv twin of the aarch64 std-net test. The `std_exerciser` std binary, given the
/// network, does a real UDP DNS query and a TCP echo round trip through `std::net`, whose PAL
/// binds to `net_stack`'s contract, proving std's networking runs on the native ABI on both
/// architectures (the §19 parity gate). Its stdout is reassembled and compared byte for byte.
#[test_case]
fn std_net_runs_over_the_socket_contract() {
    let Some((report, writer, net)) = virtio_service::start_net_std(
        net_stack_image(),
        std_exerciser_image(),
        socket_contract::NO_LISTEN_GRANT,
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };

    assert_std_transcript_from(report, writer, STD_NET_EXPECTED, "std net", false);
    net.release_or_fail("a net test's net_stack");
}

/// **A `std::net::TcpListener` serves a port it was granted, on the second ISA** (milestone 64's
/// inbound half; the §19 (architectural parity is a tenet) twin of
/// `network_tests::a_std_program_serves_a_granted_listening_port`, which carries the full
/// reasoning).
///
/// The same `std_exerciser` binary, the same grant, the same host prober, and the same four-line
/// transcript. The PAL is architecture-independent by construction, so what this leg actually
/// guards is the two hand-written `svc`/`ecall` wrappers underneath it and the riscv SMP scatter
/// that has broken this stack's timing twice.
#[test_case]
fn a_std_program_serves_a_granted_listening_port() {
    let Some((report, writer, net)) = virtio_service::start_net_std(
        net_stack_image(),
        std_exerciser_image(),
        socket_contract::listen_grant(NET_LISTEN_PORT, NET_LISTEN_PORT),
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };

    assert_std_transcript_from(report, writer, STD_LISTEN_EXPECTED, "std listen", true);
    net.release_or_fail("a net test's net_stack");
}

/// The DMA confinement holds on riscv: a descriptor aimed at kernel memory is refused and
/// the device is never rung. The attacker reports `1` (refused).
#[test_case]
fn the_kernel_refuses_a_dma_descriptor_that_escapes_the_drivers_region() {
    let Some(report) = virtio_service::start_attacker(blk_image()) else {
        crate::testing::skip!("no virtio disk attached");
    };
    let refused = sched::ipc_receive(report)[0];
    assert_eq!(
        refused, 1,
        "a malicious driver's descriptor pointing at kernel memory was NOT refused: the \
         device could have DMA'd over the kernel",
    );
}

/// The indirect-descriptor escape is refused on riscv too; see the aarch64 twin for why the
/// subtle case needs its own test.
#[test_case]
fn the_kernel_refuses_an_indirect_descriptor_escape() {
    let Some(report) = virtio_service::start_attacker_indirect(blk_image()) else {
        crate::testing::skip!("no virtio disk attached");
    };
    let refused = sched::ipc_receive(report)[0];
    assert_eq!(
        refused, 1,
        "an indirect descriptor whose inner table pointed at kernel memory was NOT refused: \
         the device could have followed it out of the driver's region",
    );
}

/// **The PCIe transport, end to end.** The identical driver binary reads the same file off
/// the disk QEMU attached as `virtio-blk-pci`: found by ECAM enumeration, BARs placed by the
/// kernel, registers reached through the virtio-pci common-config block, the completion
/// interrupt arriving as INTx through the PLIC (the swizzled line, so this is also P3's
/// proof), and the same shadow-ring confinement in the path. The driver cannot tell which
/// bus it is on; this test is the transport seam's contract, held against real device
/// behaviour on both sides.
#[test_case]
fn a_userspace_driver_reads_a_file_over_the_pcie_transport() {
    use crate::arch::exceptions::ROUTED_IRQS;

    // Sampled before the device is wired, not after: `ROUTED_IRQS` is bumped ahead of the
    // `irq_notify` that wakes the driver, so a report proves the counter moved, and the only way
    // this assertion can fail is a baseline taken after the completion. See
    // notes/load-sensitive-assertions.md and the aarch64/x86_64 twin.
    let irqs_before = ROUTED_IRQS.load(Ordering::Relaxed);

    let Some(report) = virtio_service::start_pci(blk_image()) else {
        crate::testing::skip!("no virtio-pci disk on the bus");
    };

    let word = sched::ipc_receive(report)[0];

    assert_eq!(
        &word.to_le_bytes(),
        b"nife: re",
        "the driver reported the wrong file contents over pci",
    );
    let irqs_after = ROUTED_IRQS.load(Ordering::Relaxed);
    assert!(
        irqs_after > irqs_before,
        "the read completed but no INTx interrupt was delivered through the PLIC \
         ({irqs_before} routed before the device was wired, {irqs_after} after the report)",
    );
}

/// The write round trip on the second ISA (milestone 32 phase 1); see the aarch64 twin for
/// what the report certifies.
#[test_case]
fn a_userspace_driver_writes_a_block_and_reads_it_back() {
    let Some(report) = virtio_service::start_writer(blk_image()) else {
        crate::testing::skip!("no virtio disk attached");
    };
    let word = sched::ipc_receive(report)[0];
    assert_eq!(
        &word.to_le_bytes(),
        b"CRKWRIT1",
        "the driver did not read back the pattern it wrote",
    );
}

/// The write round trip over the PCIe transport, on the second ISA.
#[test_case]
fn a_userspace_driver_writes_a_block_over_the_pcie_transport() {
    let Some(report) = virtio_service::start_writer_pci(blk_image()) else {
        crate::testing::skip!("no virtio-pci disk on the bus");
    };
    let word = sched::ipc_receive(report)[0];
    assert_eq!(
        &word.to_le_bytes(),
        b"CRKWRIT1",
        "the driver did not read back the pattern it wrote over pci",
    );
}

/// Kill-mid-write on the second ISA; see the aarch64 twin for the full argument. This is
/// also the test that made the riscv user-fault kill path exist: the abandoner's deliberate
/// death is a U-mode `ebreak`, which used to be silently stepped over.
#[test_case]
fn a_driver_killed_mid_write_leaves_the_device_and_transport_sane() {
    use crate::arch::exceptions::USER_FAULTS;

    let faults = USER_FAULTS.load(Ordering::Relaxed);
    let Some(report) = virtio_service::start_write_abandoner(blk_image()) else {
        crate::testing::skip!("no virtio disk attached");
    };

    assert_eq!(
        sched::ipc_receive(report)[0],
        1,
        "the abandoner never got its write submitted",
    );
    assert!(
        wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > faults),
        "the abandoner never died; nothing was killed mid-write",
    );

    let report = virtio_service::start_writer(blk_image())
        .expect("the disk vanished between the abandoner and the survivor");
    let word = sched::ipc_receive(report)[0];
    assert_eq!(
        &word.to_le_bytes(),
        b"CRKWRIT1",
        "after a mid-write kill, a fresh driver could not use the device",
    );
}
