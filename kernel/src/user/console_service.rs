use super::*;
use crate::cap::{Rights, rendezvous_cap};
use crate::sched::RendezvousId;

/// The PL011's physical address on QEMU `virt`. The kernel maps it for its own debug output;
/// here we hand a *second* mapping of the same registers to the userspace server. On real
/// hardware you would give the server exclusive ownership; in QEMU both mappings are fine,
/// and the kernel's is now used only for panics and boot, not for anyone's `print`.
const PL011_PHYS: u64 = 0x0900_0000;

/// What a client needs to talk to the console server: two endpoints and the shared page.
///
/// **Nothing reads these fields today, and that is the honest state of this module rather than an
/// oversight.** `spawn_client` was the only reader and went with the narrator on 2026-09-13
/// (milestone 267). [`start`] still fills the handle in because the wiring it describes is real:
/// the server is listening on `request` and will ack on `reply`. What is missing is a client, and
/// whether this boot should still have one, or should stop starting the server at all, is the
/// question the block of milestone 267 (the milestone tour is three things wearing one name) leaves
/// open for an architect. The `expect` is that question made visible; delete it, not the fields,
/// when the question is answered.
// Not in the system-test image, where `system_test_access` re-exports `user` and so reaches these
// fields: there the lint cannot fire, and an `expect` that cannot be met is its own warning.
#[cfg_attr(
    not(feature = "system_tests"),
    expect(
        dead_code,
        reason = "no client since the narrator went; see milestone 267's block"
    )
)]
#[derive(Clone, Copy)]
pub struct Console {
    pub request: RendezvousId,
    pub reply: RendezvousId,
    pub shared_phys: u64,
}

/// Spawn the console server as a user process and return a handle for wiring up clients.
///
/// The server holds: `RECEIVE` on `request` (slot 0), `SEND` on `reply` (slot 1), the shared
/// page mapped **read-only** (it only reads what clients wrote), and the **UART's registers**
/// mapped as user device memory. That last mapping is the whole milestone: a driver, at EL0,
/// holding its hardware.
pub fn start() -> Console {
    // The console server is its own binary now (19f.3), loaded from the archive by name rather
    // than entered as a role of hello.
    let image = program("console").expect("no console program in the initial ramdisk");
    let request = crate::sched::create_rendezvous();
    let reply = crate::sched::create_rendezvous();
    // Zeroed so a client's first print cannot leak stale RAM.
    let shared_phys = crate::memory::alloc_zeroed()
        .expect("no frame for the shared console buffer")
        .addr();

    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: 0, // no role selector: the console is its own binary
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(request, Rights::READ), // slot 0: RECEIVE requests
                    rendezvous_cap(reply, Rights::WRITE),  // slot 1: SEND acks
                ],
                maps: &[
                    Mapping {
                        va: SHARED_VA,
                        phys: shared_phys,
                        flags: Flags::user_rodata(),
                    },
                    Mapping {
                        va: UART_VA,
                        phys: PL011_PHYS,
                        flags: Flags::user_device(),
                    },
                ],
            },
        )
    })
    .expect("could not spawn the console server");

    Console {
        request,
        reply,
        shared_phys,
    }
}

/// The user VAs the client and server agree on. Kept here so the kernel and the binary have
/// one source of truth; they must match `components/src/console.rs` and
/// `fixtures/src/console_server_builder.rs`.
const SHARED_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);
const UART_VA: u64 = address_space_map::pair_page(0x0000_0000_0070_0000);
