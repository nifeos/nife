/// `invoke(capability, DESTROY, _, _, _)` -> 0. **End the process and every thread in it**: the
/// supervisor's kill. Needs `WRITE`. A caller that is itself a member ends with
/// it and does not return, as [`SYSCALL_EXIT`](super::SYSCALL_EXIT) would. The page stays the region's
/// until that region is destroyed; an ended process answers `Gone` to a join.
pub const DESTROY: u64 = 0;
