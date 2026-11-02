/// `invoke(capability, WRITE, ptr, len, _)` -> bytes written.
///
/// `ptr` is a **user** pointer, and the kernel will refuse it unless *the user itself* could
/// read it. This is the confused-deputy refusal: a kernel that followed a user pointer with its own
/// authority would be the deputy.
pub const WRITE: u64 = 0;
