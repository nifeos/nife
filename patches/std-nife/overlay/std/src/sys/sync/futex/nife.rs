//! **The futex `std`'s locks are built on, on nife** (milestone 812 (`std::thread::spawn` runs real
//! threads in one address space); §269 (how threads share a process) fork 2): `AddressSpace::WAIT`
//! and `WAKE` on this program's own space, whose capability is `runtimeproto::SPACE_SLOT`. The
//! private 32-bit form, the one the kernel admits. `notes/futex.md` has the contract.
//!
//! **Without the space capability, or with a timeout, a wait yields instead of sleeping.** A
//! futex wait may return spuriously and every caller re-checks its word, so a yield that returns
//! is correct, only slower. That keeps every lock correct in a program given no space capability.

//! # BUGS
//!
//! - **A timed wait spins on `yield` until its deadline** (`notes/futex.md`: the kernel's `WAIT`
//!   has no timeout yet). `Condvar::wait_timeout`, `park_timeout` and timed lock attempts are
//!   correct and burn a core while they wait.

use crate::sync::atomic::Atomic;
use crate::sync::atomic::Ordering::Relaxed;
use crate::sys::pal::nife::{abi, rt, runtimeproto};
use crate::time::Duration;

pub type Futex = Atomic<Primitive>;
pub type Primitive = u32;

pub type SmallFutex = Atomic<SmallPrimitive>;
pub type SmallPrimitive = u32;

const FLAGS: u64 = abi::futex::PRIVATE | abi::futex::SIZE_U32;

/// `WAIT` or `WAKE` on `futex`, through the space capability. A negative answer is the kernel's
/// refusal; the callers below treat every refusal as "no kernel wait here".
fn invoke(method: u64, futex: &Atomic<u32>, word: u64) -> i64 {
    // SAFETY: `invoke` traps to the kernel, which checks the capability names this program's own
    // space and that the word is a 4-aligned, mapped user address, which an `Atomic<u32>` is.
    unsafe { rt::invoke(runtimeproto::SPACE_SLOT, method, futex.as_ptr() as u64, FLAGS, word) }
}

/// **Wait while `futex` holds `expected`.** `false` only when `timeout` ran out first.
pub fn futex_wait(futex: &Atomic<u32>, expected: u32, timeout: Option<Duration>) -> bool {
    let Some(timeout) = timeout else {
        if invoke(abi::address_space::WAIT, futex, expected as u64) < 0 {
            rt::yield_now();
        }
        return true;
    };
    let ticks =
        abi::timer::counter_ticks_for(timeout.as_secs(), timeout.subsec_nanos(), rt::cntfrq());
    let deadline = rt::now().saturating_add(ticks);
    loop {
        if futex.load(Relaxed) != expected {
            return true;
        }
        if rt::now() >= deadline {
            return false;
        }
        rt::yield_now();
    }
}

/// **Wake one waiter on `futex`.** Whether one was woken; `false` too where the kernel cannot be
/// asked, which callers read as "nobody was waiting", correct for a yield-based wait.
#[inline]
pub fn futex_wake(futex: &Atomic<u32>) -> bool {
    invoke(abi::address_space::WAKE, futex, 1) > 0
}

/// **Wake every waiter on `futex`.**
#[inline]
pub fn futex_wake_all(futex: &Atomic<u32>) {
    let _ = invoke(abi::address_space::WAKE, futex, u64::MAX);
}
