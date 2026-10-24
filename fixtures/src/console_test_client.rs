//! An ordinary program that wants to print (milestone 8 (the console driver leaves the kernel)).
//! It owns no UART and cannot reach one: it writes its text into a page it shares with the console
//! server (`components/src/console.rs`) and sends the length over an endpoint.
//!
//! IPC carries control and shared memory carries data (DECISIONS §10 (process model)), so the
//! kernel never sees, copies or validates the bytes, and the confused deputy that milestone 7d
//! (user mode: EL0, capabilities, the ELF loader, and IPC) defended against cannot arise. It checks
//! its own image first ([`loaded_image_check`]) and kills itself if any part is wrong, which lets
//! `a_user_client_moves_data_through_shared_memory` assert on the absence of a fault as well as on
//! the bytes.

//! Name: provisional (milestone 291). This was `hello`'s `PRINTING` role, number 2. Follows the
//! `*_test_client` shape the tree already uses for a fixture that drives a real service
//! (`fs_test_client`, `login_test_client`, `credentialer_test_client`, `socket_test_client`).
//! Refused `printing_client`, which is what the role was called: a verb-participle where every
//! other client here is named for the service it drives, and the `_test_` is what says this is a
//! fixture rather than the console's real client (the shell is that).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use abi::Error;
use user_mode_runtime::{exit, receive, require, send};

/// The page shared with the console server. We write text here; the server reads it. Mapped
/// read/write here, read-only there. Must match `components/src/console.rs`'s `SHARED_VA`.
const SHARED_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);

/// Slot 0 sends the print request; the server receives it there.
const REQUEST: u64 = 0;
/// Slot 1 receives the ack; the server sends it there.
const REPLY: u64 = 1;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    loaded_image_check::verify(user_mode_runtime::trap);

    // These cannot fail: this program is only ever spawned WITH the console, so `print` holds its
    // capabilities. A failure traps, which is what we want if the wiring is wrong.
    require(print(b"      hello from EL0, printed by a driver that also runs at EL0.\n").is_ok());
    require(print(b"      the kernel never saw these bytes.\n").is_ok());

    // Done, so exit. Preemption is `interrupt_ignorer`'s to prove, and spinning here would leave a
    // CPU-bound thread behind for the rest of the run.
    exit();
}

/// Print `bytes` by handing them to the console server through shared memory.
///
/// Returns `Ok` if we hold the endpoints to reach the server, `Err(NoSuchSlot)` if we were not
/// given them. The bytes go in the shared page; only the length crosses the endpoint.
fn print(bytes: &[u8]) -> Result<(), Error> {
    let n = bytes.len().min(4096);

    // SAFETY: the shared page is mapped read/write in our address space. We own it between an ack
    // and the next send, which the reply below is what guarantees.
    let shared = SHARED_VA as *mut u8;
    for (i, &b) in bytes[..n].iter().enumerate() {
        // SAFETY: as above; `i < n <= 4096`, one page.
        unsafe { core::ptr::write_volatile(shared.add(i), b) };
    }

    // The length is the message. The data is already in place, shared, uncopied.
    let r = send(REQUEST, n as u64, 0, 0);
    if let Some(e) = Error::from_ret(r) {
        return Err(e); // e.g. NoSuchSlot: we were not handed a console
    }

    // Wait for the server to finish reading the buffer before we touch it again.
    let (_ack, _, _) = receive(REPLY);
    Ok(())
}

user_mode_runtime::panic_handler!();
