// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
//! Platform abstractions and environment.

use crate::{
    error::{Errno, ResultExt},
    io::{self, Read, Write},
    raw_cell::RawCell,
};
use alloc::vec::Vec;
use core::{cell::Cell, fmt, ptr};

pub use self::pal::{Pal, PalSignal};

mod pal;

pub use self::sys::Sys;

// nife's platform layer (milestone 835, §265): the `Pal` implemented on nife's own Rust `std`,
// which already speaks every capability protocol a stage-1 call needs. Replaces relibc's
// `linux/` and `redox/` arms, neither of which is seeded.
#[path = "../../../c_library/src/platform/nife.rs"]
pub mod sys;

pub use self::rlb::{Line, RawLineBuffer};
pub mod rlb;


pub use self::allocator::{alloc, alloc_align, alloc_usable_size, free, realloc};
// nife: `malloc` on `std`'s allocator, nife's code (c_library/src/platform/allocator.rs).
#[path = "../../../c_library/src/platform/allocator.rs"]
mod allocator;

use self::types::{c_char, c_int};
pub mod types;

/// The global `errno` variable used internally in relibc.
///
/// nife: one cell for the whole process, not `#[thread_local]`, because a stage-1 program has one
/// thread (the userspace targets are `singlethread`). Stage 2 (milestone 836) makes it per thread.
pub static ERRNO: ErrnoCell = ErrnoCell(Cell::new(0));

/// A `Cell` the one thread of a stage-1 program may share with itself as a `static`.
pub struct ErrnoCell(Cell<c_int>);
// SAFETY: a stage-1 process has exactly one thread, so no second thread can observe the cell.
unsafe impl Sync for ErrnoCell {}
impl ErrnoCell {
    pub fn get(&self) -> c_int {
        self.0.get()
    }
    pub fn set(&self, v: c_int) {
        self.0.set(v)
    }
    pub fn as_ptr(&self) -> *mut c_int {
        self.0.as_ptr()
    }
}

/// The `argv` argument available to a program's `main` function.
// TODO: change remaining static mut to RawCell
pub static mut argv: *mut *mut c_char = ptr::null_mut();
pub static inner_argv: RawCell<Vec<*mut c_char>> = RawCell::new(Vec::new());
pub static mut program_invocation_name: *mut c_char = ptr::null_mut();
pub static mut program_invocation_short_name: *mut c_char = ptr::null_mut();

#[unsafe(no_mangle)]
pub static mut environ: *mut *mut c_char = ptr::null_mut();

pub static OUR_ENVIRON: RawCell<Vec<*mut c_char>> = RawCell::new(Vec::new());

pub fn environ_iter() -> impl Iterator<Item = *mut c_char> + 'static {
    unsafe {
        let mut ptrs = environ;

        core::iter::from_fn(move || {
            if ptrs.is_null() {
                None
            } else {
                let ptr = ptrs.read();
                if ptr.is_null() {
                    None
                } else {
                    ptrs = ptrs.add(1);
                    Some(ptr)
                }
            }
        })
    }
}

pub trait WriteByte: fmt::Write {
    fn write_u8(&mut self, byte: u8) -> fmt::Result;
}

impl<W: WriteByte> WriteByte for &mut W {
    fn write_u8(&mut self, byte: u8) -> fmt::Result {
        (**self).write_u8(byte)
    }
}

/// An implementation of [`core::fmt::Write`] for a file descriptor.
pub struct FileWriter(pub c_int, Option<Errno>);

impl FileWriter {
    pub fn new(fd: c_int) -> Self {
        Self(fd, None)
    }

    pub fn write(&mut self, buf: &[u8]) -> fmt::Result {
        let _ = Sys::write(self.0, buf).map_err(|err| {
            self.1 = Some(err);
            fmt::Error
        })?;
        Ok(())
    }
}

impl fmt::Write for FileWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if let Ok(()) = self.write(s.as_bytes()) {}; // TODO handle error
        Ok(())
    }
}

impl WriteByte for FileWriter {
    fn write_u8(&mut self, byte: u8) -> fmt::Result {
        if let Ok(()) = self.write(&[byte]) {}; // TODO handle error
        Ok(())
    }
}

/// An implementation of [`Read`] for a file descriptor.
pub struct FileReader(pub c_int);

impl FileReader {
    // TODO: This is a bad interface. Rustify
    pub fn read(&mut self, buf: &mut [u8]) -> isize {
        Sys::read(self.0, buf)
            .map(|u| u as isize)
            .or_minus_one_errno()
    }
}

impl Read for FileReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let i = Sys::read(self.0, buf)
            .map(|u| u as isize)
            .or_minus_one_errno(); // TODO
        if i >= 0 {
            Ok(i.cast_unsigned())
        } else {
            Err(io::Error::from_raw_os_error(-i as i32))
        }
    }
}

/// An implementation of [`Write`]/[`core::fmt::Write`] for a byte array.
pub struct StringWriter(pub *mut c_char, pub usize);
impl Write for StringWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.1 > 1 {
            let copy_size = buf.len().min(self.1 - 1);
            unsafe {
                ptr::copy_nonoverlapping(buf.as_ptr().cast(), self.0, copy_size);
                self.1 -= copy_size;

                self.0 = self.0.add(copy_size);
                *self.0 = 0;
            }
        }

        // Pretend the entire slice was written. This is because many functions
        // (like snprintf) expects a return value that reflects how many bytes
        // *would have* been written. So keeping track of this information is
        // good, and then if we want the *actual* written size we can just go
        // `cmp::min(written, maxlen)`.
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl fmt::Write for StringWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // can't fail
        self.write(s.as_bytes()).unwrap();
        Ok(())
    }
}
impl WriteByte for StringWriter {
    fn write_u8(&mut self, byte: u8) -> fmt::Result {
        // can't fail
        self.write(&[byte]).unwrap();
        Ok(())
    }
}

/// An implementation of [`Write`]/[`core::fmt::Write`] for a byte array,
/// without buffer overflow protection.
pub struct UnsafeStringWriter(pub *mut u8);
impl Write for UnsafeStringWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unsafe {
            ptr::copy_nonoverlapping(buf.as_ptr(), self.0, buf.len());
            self.0 = self.0.add(buf.len());
            *self.0 = b'\0';
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl fmt::Write for UnsafeStringWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // can't fail
        self.write(s.as_bytes()).unwrap();
        Ok(())
    }
}
impl WriteByte for UnsafeStringWriter {
    fn write_u8(&mut self, byte: u8) -> fmt::Result {
        // can't fail
        self.write(&[byte]).unwrap();
        Ok(())
    }
}

/// An implementation of [`Read`] for a byte array, without buffer over-read
/// protection.
pub struct UnsafeStringReader(pub *const u8);
impl Read for UnsafeStringReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        unsafe {
            for (i, inner) in buf.iter_mut().enumerate() {
                if *self.0 == 0 {
                    return Ok(i);
                }

                *inner = *self.0;
                self.0 = self.0.add(1);
            }
            Ok(buf.len())
        }
    }
}

/// A wrapper that keeps track of the number of bytes written with the
/// underlying writer `T`.
pub struct CountingWriter<T> {
    pub inner: T,
    pub written: usize,
}
impl<T> CountingWriter<T> {
    pub fn new(writer: T) -> Self {
        Self {
            inner: writer,
            written: 0,
        }
    }
}
impl<T: fmt::Write> fmt::Write for CountingWriter<T> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.written += s.len();
        self.inner.write_str(s)
    }
}
impl<T: WriteByte> WriteByte for CountingWriter<T> {
    fn write_u8(&mut self, byte: u8) -> fmt::Result {
        self.written += 1;
        self.inner.write_u8(byte)
    }
}
impl<T: Write> Write for CountingWriter<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let res = self.inner.write(buf);
        if let Ok(written) = res {
            self.written += written;
        }
        res
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        match self.inner.write_all(buf) {
            Ok(()) => (),
            Err(ref err) if err.kind() == io::ErrorKind::WriteZero => (),
            Err(err) => return Err(err),
        }
        self.written += buf.len();
        Ok(())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

