//! A file behind a byte sink, and a file behind a byte source. See notes/sink-protocol.md.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// The pages [`start_file_sink`] or [`start_file_source`] retypes from its caller's region: two
/// endpoints each.
///
/// Name: provisional, minted 2026-10-19 (UTC) by lane milestone/608-service-rendezvous-regions.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub const FILE_SINK_PAGES: u64 = 2;

/// What [`start_file_sink`] wired.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct FileSink {
    /// The FS service's two readiness endpoints, if this call is the one that wired it.
    pub readiness: Readiness,
    /// The byte sink. Goes into a program's output slot with `WRITE`.
    pub sink: RendezvousId,
    /// Readiness first, then `DONE` and the byte total at end of stream.
    pub report: RendezvousId,
}

/// Put a file behind a byte sink: wire the FS service (or reuse this boot's) and spawn
/// `fixtures/src/file_sink.rs`, which serves one endpoint whose only request is "append these
/// bytes".
///
/// `sink` is the whole of what a redirected writer holds, with `WRITE`. Both endpoints are retyped
/// from `endpoints`, a region of [`FILE_SINK_PAGES`] the caller reclaims once the sink has reported
/// `DONE` and exited. `None` when no RedoxFS disk is attached.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_file_sink(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    file_sink_image: &'static [u8],
    endpoints: u64,
) -> Option<FileSink> {
    let (file_ep, file_shared, readiness) = ensure(blk_image, fs_server_image)?;
    let sink = crate::sched::create_rendezvous_from(endpoints).expect("no file sink endpoint");
    let report = crate::sched::create_rendezvous_from(endpoints).expect("no file sink report");
    crate::sched::spawn(move || {
        run(
            file_sink_image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(sink, Rights::READ), // slot 0: the byte sink it serves
                    rendezvous_cap(file_ep, Rights::WRITE), // slot 1: CALL the FS server
                    rendezvous_cap(report, Rights::WRITE), // slot 2: readiness and the total
                ],
                maps: &[Mapping {
                    va: FILE_VA_CLIENT,
                    phys: file_shared,
                    flags: Flags::user_data(),
                }],
            },
        )
    })
    .expect("could not spawn the file sink");
    Some(FileSink {
        readiness,
        sink,
        report,
    })
}

/// Read back what the file sink wrote, in a different process with a different FS session.
///
/// Spawn it only after the sink has reported that it closed the file, because the two share the FS
/// server's one file page ([`wait_for_caretaker`]'s reason). It streams the file out over the sink
/// contract, in the framing a `println!` uses. Returns `(out, report)`, both retyped from
/// `endpoints`.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_file_source(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    file_source_image: &'static [u8],
    endpoints: u64,
) -> Option<(RendezvousId, RendezvousId)> {
    let (file_ep, file_shared, _) = ensure(blk_image, fs_server_image)?;
    let out = crate::sched::create_rendezvous_from(endpoints).expect("no file source endpoint");
    let report = crate::sched::create_rendezvous_from(endpoints).expect("no file source report");
    crate::sched::spawn(move || {
        run(
            file_source_image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(out, Rights::WRITE), // slot 0: where the file's bytes go
                    rendezvous_cap(file_ep, Rights::WRITE), // slot 1: CALL the FS server
                    rendezvous_cap(report, Rights::WRITE), // slot 2: the size it found
                ],
                maps: &[Mapping {
                    va: FILE_VA_CLIENT,
                    phys: file_shared,
                    flags: Flags::user_data(),
                }],
            },
        )
    })
    .expect("could not spawn the file source");
    Some((out, report))
}
