/// `invoke(capability, DESTROY, _, _, _)` -> 0. **End the process and every thread in it**: the
/// supervisor's kill of §269 fork 5. Needs `WRITE`. A caller that is itself a member ends with
/// it and does not return, as [`SYS_EXIT`](super::SYS_EXIT) would. The page stays the region's
/// until that region is destroyed; an ended process answers `Gone` to a join.
pub const DESTROY: u64 = 0;
