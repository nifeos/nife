#![no_std]
#![doc = include_str!("../README.md")]
//!
//! Name: ratified 2026-09-17 (#1806). NVMe spelled out, as the acronym test asks; permanent once
//! published.

mod capabilities;
mod command;
mod completion;
mod configuration;
mod doorbell;
mod identify;
mod namespace;
mod physical_region_page;
mod queues;
pub mod registers;

#[cfg(kani)]
mod verification;

pub use capabilities::Capabilities;
pub use command::{
    ADMIN_CREATE_IO_COMPLETION_QUEUE, ADMIN_CREATE_IO_SUBMISSION_QUEUE, ADMIN_IDENTIFY,
    CONTROLLER_OR_NAMESPACE_STRUCTURE_CONTROLLER, CONTROLLER_OR_NAMESPACE_STRUCTURE_NAMESPACE,
    Command, NON_VOLATILE_MEMORY_FLUSH, NON_VOLATILE_MEMORY_READ, NON_VOLATILE_MEMORY_WRITE,
};
pub use completion::Completion;
pub use configuration::enabled_controller_configuration;
pub use doorbell::{Doorbell, doorbell};
pub use identify::{
    IdentifyNamespace, logical_block_address_format_shift, parse_identify_namespace,
};
pub use namespace::Namespace;
pub use physical_region_page::{PhysicalRegionPagePair, physical_region_page_pair};
pub use queues::{CompletionQueue, SubmissionQueue};
pub use registers::{CONTROLLER_STATUS_FATAL, CONTROLLER_STATUS_READY};
