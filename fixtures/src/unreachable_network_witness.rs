//! **Try to reach what it did not declare, and print what the kernel answered** (milestone 590 (the
//! booted system starts its network stack); widened by milestone 198 (a package manager) rung 3a
//! for milestone 202 (every confinement test is a ritual until somebody breaks the confinement)).
//!
//! The negative control for `network_echo_client`, and the unvouched child of DECISIONS §219 (how
//! the shell names an installed program to the spawner) gate D2. Its manifest
//! (`grant_plan::Program::UnreachableNetworkWitness`) declares nothing, so the spawn service must
//! leave the process domain, entropy and network slots empty; it asks each anyway with a real
//! request, then lists every slot it holds. An endowed authority turns its line into `REACHED` and
//! the census gains that slot; `script/swish-check` fails on either.

//! # EXAMPLES
//!
//! ```text
//! nife> unreachable_network_witness
//! network: refused (no capability at slot 10)
//! entropy: refused (no capability at slot 9)
//! domain: refused (no capability at slot 7)
//! reboot: refused (no capability at slot 13)
//! slots held: 0
//! ```

//! # Two ways to be run, one claim each
//!
//! By name it is a vouched program with an empty manifest, endowed the output alone. As
//! `installed/unvouched` (`xtask/src/disk.rs`), stripped so no digest matches, it is bytes nobody
//! vouched for, run by a session holding the run-unvouched capability, and must be endowed what the
//! line delegated plus the clock and configuration pages (`grant_plan::UNVOUCHED_MANIFEST`):
//! `slots held: 0 1 2`. Only a child the spawn service spawned can report what it was given.

//! # What its note asks
//!
//! All three, deliberately (milestone 597 (a program carries its manifest in an ELF note)):
//! [`NOTE_ASKS`] declares the network, entropy and the process domain. Run by name, the note is
//! never read; run unvouched, §219 says it grants nothing, so the census must still read `0 1 2`
//! while `caps installed/unvouched` prints what the note asks.

//! # The reboot object, and the census
//!
//! The reboot object (milestone 805 (`reboot` at the prompt)) is probed by invoking it: a spawn
//! service that endowed it would reset the machine mid-`script/swish-check`, and the gate fails on
//! the lines that never come back. A note cannot spell it (`capability_manifest_note::encode`). The
//! census asks the kernel: each slot below the reserved fault slot is invoked with a method no
//! object defines (`user_mode_runtime::is_granted`), and an empty slot answers `NoSuchSlot`, so a
//! capability at a slot nobody thought to probe is still counted.

//! # What it holds
//!
//! Slot 0, its output (`crates/byte_sink_contract`), and, only when run unvouched, slots 1 and 2,
//! the clock and configuration pages, which it does not read.

//! # BUGS
//!
//! - **The name says less than the program does**: it witnesses three unreachable authorities, not
//!   one. Renaming is an architect's call.
//!
//! Name: provisional. Introduced 2026-09-24 for milestone 590, after `unwritable_clock_witness`.

#![no_std]
#![allow(missing_docs)]
#![no_main]

use socket_contract::{OPERATION_CLOSE, OPERATION_OPEN_TCP};
use user_mode_runtime::{call, call_receiving, exit, is_granted, send, survey};

/// The output slot: the sink contract.
const OUT: u64 = 0;

/// **What this program's note asks for, which is more than it is ever given.** Its compiled-in
/// manifest (`Program::UnreachableNetworkWitness`), plus the three authorities it probes. See the
/// module documentation for why a witness carries a note that overreaches.
const NOTE_ASKS: grant_plan::Manifest = grant_plan::Manifest {
    network: true,
    entropy: true,
    domain: true,
    ..grant_plan::Program::UnreachableNetworkWitness.manifest()
};

capability_manifest_note::carry!(NOTE_ASKS);

// The lines below name the slots in prose; this keeps the prose honest if a constant moves.
const _: () = assert!(grant_plan::NETWORK_SLOT == 10);
const _: () = assert!(grant_plan::ENTROPY_SLOT == 9);
const _: () = assert!(grant_plan::DOMAIN_SLOT == 7);
const _: () = assert!(grant_plan::REBOOT_SLOT == 13);

user_mode_runtime::program_entry!(start);

fn start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    // A kernel refusal comes back in the first word as a negative `abi::Error`; a service's reply
    // is non-negative. So the sign alone says whether something answered, and anything answering
    // is the failure this program exists to catch.
    let no_slot = abi::Error::NoSuchSlot as i64;
    // The census is taken first and printed last: it reports the slots this program was endowed
    // with, and a stack that answers the network probe files the socket it opened in our table
    // (§255 (each socket is its own capability)), which is the probe's doing and not the spawner's.
    let mut line = [0u8; 96];
    let n = census(&mut line);
    let (r0, _, socket) = call_receiving(grant_plan::NETWORK_SLOT, OPERATION_OPEN_TCP, 0);
    if let Some(socket) = socket {
        let _ = call(socket, OPERATION_CLOSE, 0);
    }
    say(match r0 as i64 {
        r if r == no_slot => b"network: refused (no capability at slot 10)\n",
        r if r < 0 => {
            b"network: refused, but not for want of a capability (slot 10 holds something)\n"
        }
        _ => b"network: REACHED. a program that declared no network was handed one\n",
    });
    let (r0, _) = call(
        grant_plan::ENTROPY_SLOT,
        entropy_contract::req(entropy_contract::GET, entropy_contract::MAX_BYTES),
        0,
    );
    say(match r0 as i64 {
        r if r == no_slot => b"entropy: refused (no capability at slot 9)\n",
        r if r < 0 => {
            b"entropy: refused, but not for want of a capability (slot 9 holds something)\n"
        }
        _ => b"entropy: REACHED. a program that declared no entropy was handed it\n",
    });
    // `SURVEY` from cursor 0: an empty slot is `NoSuchSlot`, a view of a domain answers with a
    // cursor, and a capability without `ENUMERATE` is `NotPermitted`, which still means something
    // is there.
    let (r0, _, _) = survey(grant_plan::DOMAIN_SLOT, 0);
    say(match r0 {
        r if r == no_slot => b"domain: refused (no capability at slot 7)\n",
        r if r < 0 => {
            b"domain: refused, but not for want of a capability (slot 7 holds something)\n"
        }
        _ => b"domain: REACHED. a program that declared no process domain was handed one\n",
    });
    // The reboot object answers only when it refuses, so a held one never gets here (see the
    // module documentation); `DeviceRefused` is a held one whose firmware said no.
    let r0 = user_mode_runtime::reboot(grant_plan::REBOOT_SLOT);
    say(match r0 {
        r if r == no_slot => b"reboot: refused (no capability at slot 13)\n",
        _ => b"reboot: REACHED. a program that declared no reboot was handed the reboot object\n",
    });

    say(&line[..n]);
    send(OUT, byte_sink_contract::eof(), 0, 0);
    exit();
}

/// The census, every slot below the reserved fault slot (which `START` read and cleared), as the
/// `slots held:` line with its newline; returns its length.
fn census(line: &mut [u8; 96]) -> usize {
    let head = b"slots held:";
    line[..head.len()].copy_from_slice(head);
    let mut n = head.len();
    for slot in 0..abi::fault::FAULT_EP_SLOT {
        if is_granted(slot) {
            line[n] = b' ';
            n += 1;
            if slot >= 10 {
                line[n] = b'0' + (slot / 10) as u8;
                n += 1;
            }
            line[n] = b'0' + (slot % 10) as u8;
            n += 1;
        }
    }
    line[n] = b'\n';
    n + 1
}

/// Write `line` to the output, a packed chunk at a time.
fn say(line: &[u8]) {
    let mut rest = line;
    while !rest.is_empty() {
        let (w0, w1, w2, n) = byte_sink_contract::pack(rest);
        send(OUT, w0, w1, w2);
        rest = &rest[n..];
    }
}

user_mode_runtime::panic_handler!();
