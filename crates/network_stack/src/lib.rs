#![cfg_attr(not(test), no_std)]
//! **The network stack's host-testable logic.** Two pieces of policy that only the network stack
//! uses, lifted out of the programs so each claim is a host test and a machine-checked proof:
//! [`ephemeral_port`], which local port an outbound socket gets, and [`polled_wait`], whether the stack
//! may sleep while it waits on a network card that raises no interrupt. A third piece,
//! [`receive_trace`], is the network bench build's account of where a received stream's time
//! goes. None of it is what the stack and its clients agree on, which is `socket_contract`'s.
//!
//! No dependency, no `unsafe`. This crate stays in nife and is not published.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope. A crate that holds a
//! component's logic takes the component's name, and the component is `net_stack`; this spells
//! `network` out. Considered `socket_stack`, which names the contract and not the program.
//! Calef names crates.

pub mod ephemeral_port;
pub mod polled_wait;
pub mod receive_trace;
