use core::sync::atomic::Ordering::Relaxed;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};

use super::*;

/// The modelled `satp`.
static SATP: AtomicU64 = AtomicU64::new(0);
/// Which of the 16 ASID bits the modelled hart implements, as a mask in ASID position 0.
static IMPLEMENTED_ASID: AtomicU64 = AtomicU64::new(0);
/// How many `csrw satp` the code under proof executed.
static WRITES: AtomicU32 = AtomicU32::new(0);
/// How many full `sfence.vma` sweeps it executed.
static SWEEPS: AtomicU32 = AtomicU32::new(0);
/// Whether a `csrw satp` has happened that no sweep has followed yet.
static UNSWEPT: AtomicBool = AtomicBool::new(false);

/// `satp[59:44]`.
const ASID_FIELD: u64 = 0xffff << SATP_ASID_SHIFT;

/// Model of `csrr satp`.
#[allow(dead_code)] // named only by `#[kani::stub]`, which rustc cannot see
fn read_satp_model() -> u64 {
    SATP.load(Relaxed)
}

/// Model of `csrw satp`: WARL on the ASID field, exact everywhere else.
///
/// # Safety
/// None of its own: `unsafe` only so its signature matches the `unsafe fn` it stands in for.
/// It touches nothing but the model's statics.
#[allow(dead_code)] // named only by `#[kani::stub]`, which rustc cannot see
unsafe fn write_satp_model(satp: u64) {
    let kept = IMPLEMENTED_ASID.load(Relaxed) << SATP_ASID_SHIFT;
    SATP.store((satp & !ASID_FIELD) | (satp & kept), Relaxed);
    WRITES.fetch_add(1, Relaxed);
    UNSWEPT.store(true, Relaxed);
}

/// Model of `sfence.vma` with no operands: counted, nothing else.
#[allow(dead_code)] // named only by `#[kani::stub]`, which rustc cannot see
fn sfence_vma_all_model() {
    SWEEPS.fetch_add(1, Relaxed);
    UNSWEPT.store(false, Relaxed);
}

/// A physical address Sv39 can name as a table root: a page frame below 2^56.
fn any_root() -> u64 {
    let root: u64 = kani::any();
    kani::assume(root % PAGE_SIZE == 0);
    kani::assume(root < 1 << 56);
    root
}

/// The root and tag the MMU walks are the ones installed, as a round trip through
/// [`ttbr0_value`], so a composer and decoder that disagree about a field fail it.
///
/// Falsification: attested 2026-09-25. On patagonia with the patched Kani (pull request #1287, not yet merged).
/// The shift in `current_root_pa` changed from 12 to 10: this goes red and its two siblings stay
/// green. `attested` rather than `replayable` for the reason `script/falsifications` gives for
/// every architecture-specific harness: the sweep compiles for its own host, which never
/// compiles this file.
#[kani::proof]
#[kani::stub(super::super::instructions::read_satp, read_satp_model)]
fn the_live_root_is_the_root_that_was_installed() {
    let root = any_root();
    let asid: u16 = kani::any();
    SATP.store(ttbr0_value(root, asid), Relaxed);

    assert!(
        current_root_pa() == root,
        "the walk starts at a different root"
    );
    assert!(
        asid_of(read_satp()) == asid,
        "the live tag is not the one installed"
    );
    assert!(is_enabled(), "a composed satp reads as Bare");
}

/// The ASID probe counts exactly the bits the hardware kept, for every pattern including zero
/// bits, and leaves `satp` as it found it, swept. Overcounting would share a tag unflushed.
///
/// Falsification: attested 2026-09-25. On patagonia with the patched Kani, twice and one claim
/// at a time: the readback shifted by `SATP_ASID_SHIFT - 1` (a miscount), and the restoring
/// `csrw satp` and its sweep deleted. Each turns this red and leaves both siblings green.
/// `attested` for the same reason as the harness above.
#[kani::proof]
#[kani::stub(super::super::instructions::read_satp, read_satp_model)]
#[kani::stub(super::super::instructions::write_satp, write_satp_model)]
#[kani::stub(super::super::instructions::sfence_vma_all, sfence_vma_all_model)]
fn the_asid_probe_counts_the_implemented_bits_and_puts_satp_back() {
    let implemented = u64::from(kani::any::<u16>());
    IMPLEMENTED_ASID.store(implemented, Relaxed);
    // A live `satp` holds only ASID bits the hart implements.
    let live: u64 = kani::any();
    kani::assume(live & ASID_FIELD & !(implemented << SATP_ASID_SHIFT) == 0);
    SATP.store(live, Relaxed);

    let counted = probe_asid_bits();

    assert!(
        counted == implemented.count_ones() as usize,
        "the probe miscounted"
    );
    assert!(
        SATP.load(Relaxed) == live,
        "the probe did not put satp back"
    );
    assert!(
        WRITES.load(Relaxed) == 2,
        "the probe wrote satp other than twice"
    );
    assert!(
        !UNSWEPT.load(Relaxed),
        "the probe's last satp write was never swept"
    );
}

/// A switch to the kernel-only root, through [`deactivate_user`], sweeps the TLB exactly when
/// the ASID cannot be trusted and writes nothing when that root is already live.
///
/// Falsification: attested 2026-09-25. On patagonia with the patched Kani, twice: `write_satp`'s
/// condition inverted (sweep only when trusted), and `switch_user_root`'s early return deleted.
/// Each turns this red and leaves both siblings green. `attested` as above.
#[kani::proof]
#[kani::stub(super::super::instructions::read_satp, read_satp_model)]
#[kani::stub(super::super::instructions::write_satp, write_satp_model)]
#[kani::stub(super::super::instructions::sfence_vma_all, sfence_vma_all_model)]
fn a_switch_sweeps_the_tlb_exactly_when_the_asid_cannot_be_trusted() {
    KERNEL_ROOT.store(any_root(), Relaxed);
    let trusted: bool = kani::any();
    ASID_TAGGING_TRUSTED.store(trusted, Relaxed);
    IMPLEMENTED_ASID.store(0xffff, Relaxed);
    let live: u64 = kani::any();
    SATP.store(live, Relaxed);

    deactivate_user();

    let target = reserved_root();
    assert!(
        SATP.load(Relaxed) == target,
        "the kernel-only root is not installed"
    );
    if live == target {
        assert!(
            WRITES.load(Relaxed) == 0,
            "rewrote a satp that was already live"
        );
        assert!(
            SWEEPS.load(Relaxed) == 0,
            "swept for a switch that changed nothing"
        );
    } else {
        assert!(
            WRITES.load(Relaxed) == 1,
            "installed the root other than once"
        );
        assert!(
            UNSWEPT.load(Relaxed) == trusted,
            "swept a trusted switch, or left an untrusted one unswept"
        );
    }
}
