/// `invoke(capability, WRITE, ptr, len, _)` -> bytes written.
///
/// `ptr` is a **user** pointer, and the kernel will refuse it unless *the user itself* could
/// read it. See `syscall::user_slice`, and the confused deputy in notes/capabilities.md.
pub const WRITE: u64 = 0;
