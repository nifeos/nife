//! **The roles**, chosen by `arg0` at `START`, and [`_start`], which dispatches on them.
//!
//! Cut out of `fs_test_client.rs` unchanged on 2026-10-12 (UTC) by lane
//! maintainer/files-over-1500, for §282 (when a file is split, the parent becomes an index). What
//! another role uses is `pub(crate)` here.
//!
//! Name: provisional (lane maintainer/files-over-1500, 2026-10-12 UTC), for the module and the
//! file.

use filesystem_fixture as fixture;

use crate::bench::bench;
use crate::crash::{crash_driver, crash_verify};
use crate::directory_attacker::dir_attacker;
use crate::file_attacker::attacker;
use crate::name_set_attributes::set_attrs;
use crate::proof::proof;
use crate::shared_frame::{share_attacker, share_victim};
use crate::throughput::throughput;
use crate::two_directories::two_dir;

/// Roles, chosen by `arg0` at START (mirrors the kernel side). 0 is the end-to-end proof the test
/// spawns; 1 is the benchmark loop the `--real --smp` bench spawns; 2 is the attacker that a
/// milestone-31 per-file grant is measured against. One binary, six entries.
const ROLE_PROOF: u64 = 0;
const ROLE_BENCH: u64 = 1;
const ROLE_ATTACKER: u64 = 2;
/// Milestone 37: drive the writes the FS server is killed in the middle of.
const ROLE_CRASH_DRIVER: u64 = 3;
/// Milestone 37: read the file back through the FS server that mounted the crashed disk.
const ROLE_CRASH_VERIFY: u64 = 4;
/// Milestone 47: the attacker against a per-*directory* grant. `a1` is a run index, which is the
/// only thing it is told: everything else it has to find out by trying.
const ROLE_DIR_ATTACKER: u64 = 5;
/// Milestone 61: the attribute witness behind a **name-set** grant, the third caretaker. Told
/// nothing; it tries a name the set carries and a name it does not, and reports what got through.
const ROLE_SET_ATTRS: u64 = 6;
/// Milestone 38: sequential and random read/write throughput through the FS server, the four
/// phases `filesystem_fixture::throughput` names. The `--real --smp` bench boot spawns it after
/// [`ROLE_BENCH`], on the service that role already wired. The number lives in `filesystem_contract` because
/// the kernel passes it and this program matches on it, which is rule 7's case exactly.
const ROLE_THROUGHPUT: u64 = fixture::throughput::ROLE;
/// Milestone 154: the confined program that holds **two** directory capabilities at once, one at
/// capability table slot 0 and one at slot 1 (`kernel/src/user/fs_service/two_dirs.rs`'s `start_granted_two_dirs`
/// convention). It
/// proves the roadmap block's own deliverable: `/a/x` and `/b/y` both resolve to the grant their
/// label names, `/a/../b` is refused before it ever reaches a caretaker, and neither caretaker's
/// tree is reachable through the other's endpoint.
const ROLE_TWO_DIR: u64 = 10;
// 11 and 12 were milestone 152 (durable delegation)'s schedule-store seed and verify roles, retired
// 2026-09-27 with `session_reviver`: `login` now reads the store at start-up, and `login_tests`
// writes it.
/// Milestone 599 (a frame per filesystem client channel), provisional: the shared-frame witness's
/// **victim**. Stages its own name, hands off to the attacker over the sync endpoint, and only then
/// calls `OPEN`, so the name the server reads is whatever the attacker left in the page. Reports
/// which file it actually got. See [`share_victim`] and `system_tests/src/user/fs_shared_page_tests.rs`.
const ROLE_SHARE_VICTIM: u64 = 13;
/// Milestone 599 (provisional): the shared-frame witness's **attacker**. Holds the same file frame
/// read-write as the victim, waits for the victim to stage its name, overwrites it with a different
/// (same-length) name, and signals back. It never calls the FS server; sharing the frame is the
/// whole of its power. See [`share_attacker`].
const ROLE_SHARE_ATTACKER: u64 = 14;

user_mode_runtime::program_entry!(start);

fn start(role: u64, a1: u64, _a2: u64) -> ! {
    match role {
        ROLE_BENCH => bench(),
        ROLE_ATTACKER => attacker(a1 != 0),
        ROLE_CRASH_DRIVER => crash_driver(),
        ROLE_CRASH_VERIFY => crash_verify(),
        ROLE_DIR_ATTACKER => dir_attacker(a1),
        ROLE_SET_ATTRS => set_attrs(),
        ROLE_THROUGHPUT => throughput(),
        ROLE_TWO_DIR => two_dir(),
        ROLE_SHARE_VICTIM => share_victim(a1),
        ROLE_SHARE_ATTACKER => share_attacker(),
        ROLE_PROOF => proof(),
        _ => proof(),
    }
}
