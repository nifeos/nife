use super::*;
use crate::cap::{Rights, memory_region_cap, rendezvous_cap};
use crate::sched::RendezvousId;

/// Where the service maps the provisioner's page. Must match components/src/credentialer.rs.
const PROV_VA: u64 = address_space_map::pair_page(0x0000_0000_00e0_0000);
/// Where the service and a client map the verify page. Must match both programs.
const VERIFY_VA: u64 = address_space_map::pair_page(0x0000_0000_00e1_0000);

/// The service's untyped budget, in pages: 6 MiB, for the Argon2id scratch
/// (`credentialer::Cost::DEFAULT.blocks()` KiB, 4 MiB), its page tables and the allocator's slack.
/// Raising the cost means raising this too.
const CRED_BUDGET_PAGES: u64 = 1536;

/// Extra stack pages for the service. Argon2's inner loop copies whole 1 KiB `Block`s through
/// locals, and one page overflowed by 256 bytes on the first derivation. Sixteen, because a KDF
/// that overflows fails every login, and the pages are cheap beside the 4 MiB scratch.
const CRED_STACK_PAGES: u64 = 16;

/// The clients' budgets: none. A `credentialer_test_client` role maps one page the wiring placed
/// and calls one endpoint, so the attacker gets no memory to build with, on purpose.
const _NO_CLIENT_BUDGET: () = ();

/// The `credentialer_test_client` roles; must match `fixtures/src/credentialer_test_client.rs`.
pub const ROLE_HONEST: u64 = 0;
pub const ROLE_ATTACKER: u64 = 1;
pub const ROLE_PROVISIONER: u64 = 2;

/// The flag bits a client packs into its report's third word; must match the same file.
pub const F_CLEAN: u64 = 1 << 0;
// Bits 1 and 2 are retired, not reused: the next flag takes bit 3, so an old transcript cannot be
// misread.

/// The report words `credentialer_test_client` and the service send, likewise.
pub const RPT_DONE: u64 = 0x_c2ed_c11e_0000_0001;
pub const RPT_READY: u64 = 0x_c2ed_0000_0000_0001;

/// A running credential service and the endpoints that reach it.
pub struct Wiring {
    /// The service's readiness endpoint. It reports after the seal, so receiving on this is
    /// also how a caller knows provisioning is over.
    pub ready: RendezvousId,
    /// The verify endpoint. This is what a client is given, with WRITE.
    pub verify: RendezvousId,
    /// The provision endpoint. Held here only so the provisioner can be spawned against it;
    /// after the seal, the service has deleted its receive end and a `CALL` here would block
    /// forever, which is why nothing sends on it afterwards.
    pub provision: RendezvousId,
    /// The physical frame behind the verify page, for this instance: a boot may wire more than
    /// one credential service, so the frame travels with its `Wiring` rather than a global.
    pub verify_page_frame: u64,
    /// The physical frame behind the provision page, for this instance. Same reason as
    /// [`verify_page_frame`](Wiring::verify_page_frame).
    pub provision_page_frame: u64,
}

/// Wire and spawn the credential service. It blocks on its provision endpoint immediately, so
/// nothing happens until [`provisioner`] runs.
///
/// `entropy` is the entropy service's request endpoint (§44 (entropy is a capability)). It is not
/// optional: a credential service that cannot draw a salt refuses to start, and passing it a dead
/// endpoint is how that path gets tested.
pub fn start(image: &'static [u8], entropy: RendezvousId) -> Wiring {
    let provision = crate::sched::create_rendezvous();
    let verify = crate::sched::create_rendezvous();
    let ready = crate::sched::create_rendezvous();
    let budget = crate::memory_region::create(CRED_BUDGET_PAGES)
        .expect("no untyped for the credential store");

    let mut maps = [Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_data(),
    }; CRED_STACK_PAGES as usize + 2];
    maps[0] = Mapping {
        va: PROV_VA,
        phys: page_frame(),
        flags: Flags::user_data(),
    };
    maps[1] = Mapping {
        va: VERIFY_VA,
        phys: page_frame(),
        flags: Flags::user_data(),
    };
    let (provision_page_frame, verify_page_frame) = (maps[0].phys, maps[1].phys);
    for k in 0..CRED_STACK_PAGES as usize {
        // Zeroed, so no one else's bytes sit where key material will go.
        let phys = crate::memory::alloc_zeroed()
            .expect("no frame for the credential service's stack")
            .addr();
        maps[k + 2] = Mapping {
            va: USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE,
            phys,
            flags: Flags::user_data(),
        };
    }
    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: 0,
                arg1: 0, // no physical address: this process touches no device
                arg2: 0,
                grants: &[
                    rendezvous_cap(provision, Rights::READ), // slot 0: write the store, until SEAL
                    rendezvous_cap(verify, Rights::READ),    // slot 1: answer questions, forever
                    rendezvous_cap(entropy, Rights::WRITE),  // slot 2: salts, naming no device
                    memory_region_cap(budget),               // slot 3: the memory-hard scratch
                    rendezvous_cap(ready, Rights::WRITE),    // slot 4: one message, after the seal
                ],
                maps: &maps,
            },
        )
    })
    .expect("could not spawn the credential service");

    Wiring {
        ready,
        verify,
        provision,
        verify_page_frame,
        provision_page_frame,
    }
}

/// Spawn the provisioner and wait for its report. It fills the store and seals it, so when
/// this returns the service is in phase two and the provision endpoint is dead at both ends.
///
/// Its endowment is the provision endpoint, a report endpoint, and the provisioner's page. It
/// holds no verify endpoint, which is the mirror of the client's position: neither party can do
/// the other's job, and neither is prevented from it by a check.
pub fn provisioner(image: &'static [u8], w: &Wiring) -> [u64; 5] {
    spawn_cli(
        image,
        ROLE_PROVISIONER,
        w.provision,
        PROV_VA,
        w.provision_page_frame,
    )
}

/// Spawn a client in `role` against the verify endpoint, and wait for its report.
pub fn client(image: &'static [u8], w: &Wiring, role: u64) -> [u64; 5] {
    spawn_cli(image, role, w.verify, VERIFY_VA, w.verify_page_frame)
}

/// The one spawn site the three `credentialer_test_client` roles share, because the whole claim is that they
/// differ in their endowment and not in their code. Changing `endpoint` and `va` here is the
/// entire difference between a provisioner and an attacker.
///
/// `phys` comes from the caller's own `Wiring`, since a boot may hold more than one service.
fn spawn_cli(
    image: &'static [u8],
    role: u64,
    endpoint: RendezvousId,
    va: u64,
    phys: u64,
) -> [u64; 5] {
    let report = crate::sched::create_rendezvous();
    let maps = [Mapping {
        va,
        phys,
        flags: Flags::user_data(),
    }];
    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: role,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(endpoint, Rights::WRITE), // slot 0: the service
                    rendezvous_cap(report, Rights::WRITE),   // slot 1: say what happened
                ],
                maps: &maps,
            },
        )
    })
    .expect("could not spawn a credential client");
    crate::sched::ipc_receive(report)
}

/// Allocate one fresh, zeroed shared frame. The caller keeps it, in its `Wiring` or `Mapping`,
/// since this module may wire more than one service.
fn page_frame() -> u64 {
    crate::memory::alloc_zeroed()
        .expect("no frame for a credential page")
        .addr()
}

/// Unpack the `k`th reply code from a `credentialer_test_client` report's second word. One byte per code; see
/// `fixtures/src/credentialer_test_client.rs` `Codes`.
pub const fn nth(packed: u64, k: u32) -> u64 {
    (packed >> (8 * k)) & 0xff
}
