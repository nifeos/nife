/// Read the object's contents.
pub const READ: u64 = 1 << 0;
/// Modify the object's contents.
pub const WRITE: u64 = 1 << 1;
/// Delegate a (possibly narrowed) copy of this capability to another capability table.
pub const GRANT: u64 = 1 << 2;

/// **Learn what exists, without acting on it** (milestone 126). The kernel-level twin of
/// `filesystem_contract`'s directory `ENUMERATE`; `capability::Rights::ENUMERATE` carries the full
/// argument and the list of objects expected to grow it.
pub const ENUMERATE: u64 = 1 << 3;

/// **Add a thread to a process** (milestone 812 (`std::thread::spawn` runs real threads in one
/// address space), §269 (how threads share a process) fork 1 as ruled on pull request #1892).
/// `capability::Rights::BIND` carries the argument; see [`super::process`]. A joined thread runs
/// at an entry its builder chose, inside the process's table and space, so `BIND` is full control
/// of the process: delegate it as carefully as `GRANT`.
pub const BIND: u64 = 1 << 4;

/// **Every right defined above.** `capability::Rights::ALL` is the kernel's twin, and
/// `kernel/src/cap.rs` asserts the two equal at compile time. Since milestone 824 (a retype mints no right its budget
/// lacks) a page or object retyped from a budget carries the budget's rights, so a builder that lends
/// what it makes is handed a budget carrying all of them. Name: provisional (milestone 824's lane,
/// 2026-10-12 UTC).
pub const ALL: u64 = READ | WRITE | GRANT | ENUMERATE | BIND;

/// **A spend-only budget: every right but `GRANT`.** What a leaf is handed, so that it may retype,
/// map, split and destroy, and use every object it makes, but lend neither the budget nor anything
/// carved from it (milestone 824). On a memory region `READ` and `BIND` are consulted by no method
/// and `ENUMERATE` only by `USAGE`, the holder's own spending, so `WRITE` is the only right here
/// that acts on the region. Name: provisional (milestone 824's lane, 2026-10-12 UTC).
pub const SPEND_ONLY: u64 = ALL & !GRANT;
