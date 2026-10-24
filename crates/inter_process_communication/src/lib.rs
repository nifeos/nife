//! The decision core of the kernel's IPC: the synchronous rendezvous, the notification, the timer
//! and the futex wait queues, each a state machine the kernel wraps with what it cannot see.
//!
//! A rendezvous holds two intrusive wait queues and a pending-signal count, and at most one of the
//! queues is ever non-empty. See `notes/inter-process-communication.md` for the design and the
//! proofs.
//!
//! Name: ratified 2026-09-18 (calef, §154 (the acronym test is whether the phrase is spoken)),
//! deratifying the 2026-08-01 ratification: "inter-process communication" is a phrase people say,
//! where "peripheral component interconnect" is not. IPC the concept keeps its name everywhere
//! else. Refused `ipc`.

#![cfg_attr(not(test), no_std)]

use core::ptr::NonNull;

use intrusive_fifo::{Fifo, Node, Unqueued};

pub mod futex;
pub mod notification;
pub mod timer;

/// Machine-checked proofs of the rendezvous state machine: every operation preserves the
/// one-queue invariant, and each decision matches its rule.
///
/// The harnesses are inductive steps over one waiter per queue, and each discharges the token
/// obligations `notes/inter-process-communication.md` ("How it is proved") states: every node is
/// declared before the rendezvous, on no queue, and minted once.
#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;

mod rendezvous;

pub use rendezvous::{Receive, Rendezvous, Send};
