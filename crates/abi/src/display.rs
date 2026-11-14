//! `Display` and `core::error::Error` for [`Error`], so it sits behind `?` and in a boxed error.
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane maintainer/outside-review-fixes-1075-1088. A private module; it names no public item.

use super::Error;

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Error::NoSuchSlot => "the capability slot is empty",
            Error::WrongObject => "the capability is not that kind of object",
            Error::NotPermitted => {
                "the capability lacks the rights, or the object refuses the operation"
            }
            Error::BadPointer => "the pointer is not memory the caller could touch itself",
            Error::BadMethod => "no such method on that object",
            Error::BadSyscall => "no such syscall",
            Error::OutOfMemory => "the untyped region is exhausted",
            Error::DeviceRefused => "a device operation was refused",
            Error::StillAlive => "the thread is still running",
            Error::NotSupervised => "this rendezvous does not supervise a thread by that id",
            Error::Gone => "the capability names an object that no longer exists",
            Error::NoResetMechanism => "no route exists to ask for a reset",
            Error::ResetNotSupported => "the firmware does not offer a reset",
            Error::ResetDenied => "the firmware refused this caller a reset",
            Error::ResetDidNotHappen => "the reset was asked for and the machine is still running",
        })
    }
}

impl core::error::Error for Error {}
