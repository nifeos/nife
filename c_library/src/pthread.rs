//! POSIX threads on `std`'s threads (milestone 836 (a C library, stage 2: threads)).
//!
//! A `pthread_create` is a `std` spawn (milestone 812 (`std::thread::spawn` runs real threads in
//! one address space)), and this module is the bookkeeping POSIX asks for on top: joinable or
//! detached, the return value, `pthread_exit`, `pthread_self` and a thread number. relibc's seeded
//! `header/pthread` calls these functions by the names relibc's own module gives them. A thread's
//! record is an `Arc` freed by whichever of the thread and its `pthread_t` ends last, and
//! `pthread_exit` `longjmp`s back to `c/thread_start.c`; `c_library/README.md` ("How a thread
//! starts and ends") and notes/c-library.md ("The platform layer's own constraints") have both.

//! # BUGS
//!
//! - **Cancellation is deferred and checked only at `pthread_testcancel`.** nife has no signals to
//!   interrupt a thread, so POSIX's cancellation points (a blocking `read`, a condition wait) do
//!   not act on a pending cancel.
//! - **A caller-supplied stack (`pthread_attr_setstack`) is refused** with `EINVAL`, and
//!   `pthread_getattr_np` reports no stack address: `std` makes the stack.
//! - **16 threads a process**, the main thread included (milestone 812's `BUGS`); one more
//!   `pthread_create` answers `EAGAIN`.
//! - **Scheduling parameters are accepted and ignored**: one scheduler class (`header/sched`).

//! Name: provisional, lane milestone/836-a-c-library-stage-two-threads (#2009), 2026-10-12 (UTC),
//! for the module and the file. The pull request records no other name considered.

use alloc::sync::Arc;
use core::cell::Cell;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread::JoinHandle;

use crate::error::Errno;
use crate::header::errno::{EAGAIN, EDEADLK, EINVAL, ENOENT, ENOSYS};
use crate::header::pthread as header;
use crate::header::sched::sched_param;
use crate::platform::types::{c_int, c_void, clockid_t, pthread_t};

/// The bits relibc keeps in [`Pthread::flags`]; only `DETACHED` exists.
pub struct PthreadFlags(usize);

impl PthreadFlags {
    pub const DETACHED: Self = Self(1);

    pub const fn bits(&self) -> usize {
        self.0
    }
}

/// A `setjmp` buffer big enough for all three architectures (`include/setjmp.h`: 22 words on
/// aarch64, 26 on riscv64, 16 on x86_64).
type JmpBuf = [u64; 32];

/// One thread, as a `pthread_t` points at it.
pub struct Pthread {
    /// relibc's flags word, which `pthread_getattr_np` reads.
    pub(crate) flags: AtomicUsize,
    /// Always 0: `std` owns the stack and does not say where it is.
    pub(crate) stack_base: usize,
    /// The size asked for at `pthread_create`, or 0 for a thread not made here.
    pub(crate) stack_size: usize,
    /// `std`'s handle, until a join or a detach takes it. `None` for a thread not made here.
    handle: Mutex<Option<JoinHandle<()>>>,
    /// What the start routine returned, or what `pthread_exit` was given. Stored by the thread
    /// before it ends and read by the joiner after `std`'s join, which orders the two.
    retval: AtomicPtr<c_void>,
    /// Whether this thread was made by `pthread_create`, and so has a `setjmp` to jump to.
    created: bool,
    has_queued_cancelation: AtomicBool,
    has_enabled_cancelation: AtomicBool,
}

impl Pthread {
    fn new(created: bool, stack_size: usize) -> Self {
        Self {
            flags: AtomicUsize::new(0),
            stack_base: 0,
            stack_size,
            handle: Mutex::new(None),
            retval: AtomicPtr::new(ptr::null_mut()),
            created,
            has_queued_cancelation: AtomicBool::new(false),
            // POSIX: a new thread starts with cancellation enabled (and deferred).
            has_enabled_cancelation: AtomicBool::new(true),
        }
    }
}

/// A thread's return value, as relibc's module names it.
#[derive(Clone, Copy, Debug)]
pub struct Retval(pub *mut c_void);

std::thread_local! {
    /// The calling thread's record, null until first asked for on a thread not made here. Valid
    /// while the thread runs: a made thread's closure holds a count, and the main thread's record
    /// is never freed.
    static CURRENT: Cell<*const Pthread> = const { Cell::new(ptr::null()) };
    /// Where `pthread_exit` jumps, and where it leaves its value for the trampoline to return: the
    /// `setjmp` buffer and a word on this thread's stack (`run`), null on a thread not made here.
    static EXIT_ENV: Cell<(*mut u64, *mut *mut c_void)> =
        const { Cell::new((ptr::null_mut(), ptr::null_mut())) };
    /// The calling thread's number (`gettid`), 0 until first asked for.
    static TID: Cell<c_int> = const { Cell::new(0) };
}

/// Threads made here that have not yet ended, which a main-thread `pthread_exit` waits for. A
/// futex word: [`thread_ended`] wakes it.
static LIVE: AtomicU32 = AtomicU32::new(0);
static NEXT_TID: AtomicI32 = AtomicI32::new(1);

/// **The calling thread's number**, 1 for the first thread to ask (normally the main thread).
///
/// Name: provisional, lane milestone/836-a-c-library-stage-two-threads (#2009), 2026-10-12 (UTC).
/// The pull request records no other name considered.
pub fn thread_id() -> c_int {
    TID.with(|t| {
        if t.get() == 0 {
            t.set(NEXT_TID.fetch_add(1, Ordering::Relaxed));
        }
        t.get()
    })
}

/// The calling thread's record. A thread not made by `pthread_create` (the main thread) is given
/// one on first use, kept for the rest of the process.
pub fn current_thread() -> Option<&'static Pthread> {
    let p = CURRENT.with(|c| {
        if c.get().is_null() {
            c.set(Arc::into_raw(Arc::new(Pthread::new(false, 0))));
        }
        c.get()
    });
    // SAFETY: `CURRENT`'s comment: the record outlives the calling thread's use of it.
    Some(unsafe { &*p })
}

unsafe extern "C" {
    /// `c_library/c/thread_start.c`: `setjmp` into `env`, then call `start(arg)`; a `longjmp` to
    /// `env` returns `*exit_value` instead.
    ///
    /// Name: provisional, lane milestone/836-a-c-library-stage-two-threads (#2009), 2026-10-12
    /// (UTC). The C side in `c_library/c/thread_start.c` shares it; the pull request records no
    /// other name considered.
    fn nife_pthread_trampoline(
        start: extern "C" fn(*mut c_void) -> *mut c_void,
        arg: *mut c_void,
        env: *mut u64,
        exit_value: *mut *mut c_void,
    ) -> *mut c_void;
}

/// The body of a thread made by [`create`], on its own `std` thread, holding a count of its record.
fn run(this: Arc<Pthread>, start: extern "C" fn(*mut c_void) -> *mut c_void, arg: usize) {
    CURRENT.with(|c| c.set(Arc::as_ptr(&this)));
    let mut env: JmpBuf = [0; 32];
    let mut exited: *mut c_void = ptr::null_mut();
    EXIT_ENV.with(|e| e.set((env.as_mut_ptr(), &raw mut exited)));
    // SAFETY: `env` and `exited` outlive the call; `start` is the program's start routine.
    let ret = unsafe {
        nife_pthread_trampoline(start, arg as *mut c_void, env.as_mut_ptr(), &raw mut exited)
    };
    EXIT_ENV.with(|e| e.set((ptr::null_mut(), ptr::null_mut())));
    // Thread-specific data destructors run on every way out, after `pthread_exit`'s cleanup
    // handlers (which ran before its jump) and before the thread is gone.
    header::tls::run_all_destructors();
    this.retval.store(ret, Ordering::Release);
    thread_ended();
}

fn thread_ended() {
    LIVE.fetch_sub(1, Ordering::Release);
    crate::platform::futex::wake(LIVE.as_ptr(), u32::MAX);
}

/// **`pthread_create`**: a `std` thread running `start_routine(arg)`, joinable or detached as
/// `attrs` says, with the stack size it asks for.
///
/// # Safety
/// `start_routine` is a C function that may be called with `arg` on another thread.
pub(crate) unsafe fn create(
    attrs: Option<&header::RlctAttr>,
    start_routine: extern "C" fn(arg: *mut c_void) -> *mut c_void,
    arg: *mut c_void,
) -> Result<pthread_t, Errno> {
    let attrs = attrs.cloned().unwrap_or_default();
    if attrs.stack != 0 {
        // A caller-supplied stack cannot be given to `std` (BUGS).
        return Err(Errno(EINVAL));
    }
    let detached = c_int::from(attrs.detachstate) == header::PTHREAD_CREATE_DETACHED;
    let record = Arc::new(Pthread::new(true, attrs.stacksize));
    if detached {
        record
            .flags
            .store(PthreadFlags::DETACHED.bits(), Ordering::Relaxed);
    }
    let theirs = Arc::clone(&record);
    // `arg` crosses as an integer: a pointer is not `Send`, and the C program vouches for it.
    let arg = arg as usize;
    LIVE.fetch_add(1, Ordering::Relaxed);
    let spawned = std::thread::Builder::new()
        .stack_size(attrs.stacksize)
        .spawn(move || run(theirs, start_routine, arg));
    let Ok(handle) = spawned else {
        LIVE.fetch_sub(1, Ordering::Relaxed);
        return Err(Errno(EAGAIN));
    };
    if detached {
        // Dropping `std`'s handle is its detach, and the record goes with the thread's count; the
        // `pthread_t` names it for as long as the thread runs, as POSIX allows.
        drop(handle);
        return Ok(Arc::as_ptr(&record).cast_mut().cast());
    }
    *record.handle.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
    Ok(Arc::into_raw(record).cast_mut().cast())
}

/// **`pthread_join`**: wait for `thread` to end, give its `pthread_t` back, and answer what it
/// returned.
///
/// # Safety
/// `thread` came from [`create`] and has been neither joined nor detached.
pub unsafe fn join(thread: &Pthread) -> Result<Retval, Errno> {
    if current_thread().is_some_and(|me| ptr::eq(thread, me)) {
        return Err(Errno(EDEADLK));
    }
    let handle = thread
        .handle
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .ok_or(Errno(EINVAL))?;
    // `std`'s join returns once the thread has left for good. The thread cannot panic: `panic =
    // "abort"`.
    let _ = handle.join();
    let ret = thread.retval.load(Ordering::Acquire);
    // Give back the `pthread_t`'s count. SAFETY: the caller's contract, so `thread` came from
    // `create`'s `Arc::into_raw` and this is that count's one release; POSIX: a joined thread's
    // `pthread_t` is no longer valid.
    unsafe { Arc::decrement_strong_count(thread) };
    Ok(Retval(ret))
}

/// **`pthread_detach`**: `thread` is freed when it ends, or now if it has.
///
/// # Safety
/// `thread` came from [`create`] or [`current_thread`] and has been neither joined nor detached;
/// it is not used again after this returns.
pub unsafe fn detach(thread: &Pthread) -> Result<(), Errno> {
    if !thread.created {
        // The main thread: nothing ever joins it, and its record is kept anyway.
        return Ok(());
    }
    let handle = thread
        .handle
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .ok_or(Errno(EINVAL))?;
    thread
        .flags
        .fetch_or(PthreadFlags::DETACHED.bits(), Ordering::AcqRel);
    drop(handle);
    // Give back the `pthread_t`'s count, as `join` does. SAFETY: as `join`'s.
    unsafe { Arc::decrement_strong_count(thread) };
    Ok(())
}

/// **`pthread_testcancel`**: end the calling thread if a cancel is pending and enabled.
///
/// # Safety
/// As [`exit_current_thread`]: the caller's frames hold nothing that must be dropped.
pub unsafe fn testcancel() {
    let this = current_thread().expect("current thread not present");
    if this.has_queued_cancelation.load(Ordering::Acquire)
        && this.has_enabled_cancelation.load(Ordering::Acquire)
    {
        // SAFETY: the caller's contract.
        unsafe { exit_current_thread(Retval(header::PTHREAD_CANCELED)) };
    }
}

/// **`pthread_exit`**: run the cleanup handlers, then end the calling thread with `retval`.
///
/// # Safety
/// Every frame between here and the thread's start is C or holds nothing with a destructor:
/// `longjmp` skips them all.
pub unsafe fn exit_current_thread(retval: Retval) -> ! {
    // `pthread_cleanup_push` handlers, newest first, then (on the main thread) thread-specific
    // data; a made thread's runs in `run`, after the jump.
    header::run_destructor_stack();
    let (env, exit_value) = EXIT_ENV.with(Cell::get);
    if env.is_null() {
        // The main thread: the process ends once every other thread made here has.
        header::tls::run_all_destructors();
        loop {
            let live = LIVE.load(Ordering::Acquire);
            if live == 0 {
                break;
            }
            let _ = crate::platform::futex::wait_on(&LIVE, live, None);
        }
        // SAFETY: the C library's `exit`, which runs the `atexit` handlers and flushes the C
        // streams, called once.
        unsafe { crate::header::stdlib::exit(0) }
    }
    // SAFETY: `env` was filled by the `setjmp` in `thread_start.c`, on this thread's stack below
    // every frame `longjmp` discards, and `exit_value` is the word beside it that the trampoline
    // then returns. Nothing in this frame has a destructor left to run.
    unsafe {
        exit_value.write(retval.0);
        crate::header::setjmp::longjmp(env.cast(), 1);
        core::hint::unreachable_unchecked()
    }
}

/// **`pthread_cancel`**: mark `thread` cancelled, for its next `pthread_testcancel`.
///
/// # Safety
/// `thread` names a live thread of this process (relibc's signature; nothing here needs more).
pub unsafe fn cancel(thread: &Pthread) -> Result<(), Errno> {
    // Deferred only: the thread acts on it at its next `pthread_testcancel` (BUGS).
    thread.has_queued_cancelation.store(true, Ordering::Release);
    Ok(())
}

pub fn set_sched_param(
    _thread: &Pthread,
    _policy: c_int,
    _param: &sched_param,
) -> Result<(), Errno> {
    // Accepted and ignored: one scheduling class (BUGS).
    Ok(())
}

pub fn set_sched_priority(_thread: &Pthread, _prio: c_int) -> Result<(), Errno> {
    Ok(())
}

pub fn set_cancel_state(state: c_int) -> Result<c_int, Errno> {
    let this = current_thread().expect("current thread not present");
    let was_enabled = match state {
        header::PTHREAD_CANCEL_ENABLE => {
            let old = this.has_enabled_cancelation.swap(true, Ordering::Release);
            if this.has_queued_cancelation.load(Ordering::Acquire) {
                // SAFETY: on the thread being cancelled.
                unsafe { exit_current_thread(Retval(header::PTHREAD_CANCELED)) };
            }
            old
        }
        header::PTHREAD_CANCEL_DISABLE => {
            this.has_enabled_cancelation.swap(false, Ordering::Release)
        }
        _ => return Err(Errno(EINVAL)),
    };
    Ok(if was_enabled {
        header::PTHREAD_CANCEL_ENABLE
    } else {
        header::PTHREAD_CANCEL_DISABLE
    })
}

pub fn set_cancel_type(ty: c_int) -> Result<c_int, Errno> {
    match ty {
        // Asynchronous cancellation is accepted and behaves as deferred (BUGS).
        header::PTHREAD_CANCEL_DEFERRED | header::PTHREAD_CANCEL_ASYNCHRONOUS => {
            Ok(header::PTHREAD_CANCEL_DEFERRED)
        }
        _ => Err(Errno(EINVAL)),
    }
}

pub fn get_cpu_clkid(_thread: &Pthread) -> Result<clockid_t, Errno> {
    // nife accounts no CPU time to a thread.
    Err(Errno(ENOENT))
}

pub fn get_sched_param(_thread: &Pthread) -> Result<(clockid_t, sched_param), Errno> {
    Err(Errno(ENOSYS))
}

/// Whether a lock is private to a process or shared between processes, as relibc's module names
/// it. Taken unchanged from relibc's `pthread` module (MIT).
#[derive(Clone, Copy, Default, Debug)]
pub enum Pshared {
    #[default]
    Private,

    Shared,
}

impl Pshared {
    pub const fn from_raw(raw: c_int) -> Option<Self> {
        Some(match raw {
            header::PTHREAD_PROCESS_PRIVATE => Self::Private,
            header::PTHREAD_PROCESS_SHARED => Self::Shared,

            _ => return None,
        })
    }

    pub const fn raw(self) -> c_int {
        match self {
            Self::Private => header::PTHREAD_PROCESS_PRIVATE,
            Self::Shared => header::PTHREAD_PROCESS_SHARED,
        }
    }
}
