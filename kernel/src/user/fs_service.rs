use super::*;
use crate::cap::{
    Rights, irq_cap, memory_region_cap, page_frame_cap, rendezvous_cap, rendezvous_cap_badged,
    virtio_cap,
};
use crate::sched::RendezvousId;

// Every type and function of `fs_service` lives in a child named for it, and this file is the
// index (§282 (when a file is split, the parent becomes an index)). Each child takes `use super::*`
// and is re-exported whole, so every `fs_service::` path resolves and siblings reach each other.
// A child whose only callers are the system tests may leave its re-export unused without them.
mod channel;
pub use channel::*;
mod stack_watermark;
pub use stack_watermark::*;
mod wiring;
pub use wiring::*;
mod client_windows;
pub use client_windows::*;
mod crash;
#[cfg_attr(not(feature = "system_tests"), allow(unused_imports))]
pub use crash::*;
mod client;
pub use client::*;
mod file_grant;
#[cfg_attr(not(feature = "system_tests"), allow(unused_imports))]
pub use file_grant::*;
mod images;
pub use images::*;
mod directory_grant;
pub use directory_grant::*;
mod set_grant;
#[cfg_attr(not(feature = "system_tests"), allow(unused_imports))]
pub use set_grant::*;
mod shared_frame_witness;
#[cfg_attr(not(feature = "system_tests"), allow(unused_imports))]
pub use shared_frame_witness::*;
mod file_sink;
#[cfg_attr(not(feature = "system_tests"), allow(unused_imports))]
pub use file_sink::*;
mod std_start;
pub use std_start::*;
mod two_dirs;
#[cfg_attr(not(feature = "system_tests"), allow(unused_imports))]
pub use two_dirs::*;

mod std_spawn;
use std_spawn::spawn_std;

// `install_for_test`, the test harness's stand-in for an installer.
#[cfg(feature = "system_tests")]
mod test_install;
#[cfg(feature = "system_tests")]
pub use test_install::install_for_test;
#[cfg(any(test, feature = "system_tests"))]
mod test_files;
#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(unused_imports))]
pub use test_files::*;
