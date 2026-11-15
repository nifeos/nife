/// **Reserved, retired.** `invoke(capability, WRITE, ptr, len, _)` was the write method of the
/// kernel's `Console` object. That object is gone, so no object type in [`crate::object_type`]
/// names a console, no capability reaches one and no kernel dispatches this number. It is never
/// reused for another method of a console.
///
/// A program that invokes method 0 on a capability reaches that object's own method 0, not a
/// console write. A method that a kernel object does not serve answers `Error::BadMethod`.
pub const WRITE: u64 = 0;
