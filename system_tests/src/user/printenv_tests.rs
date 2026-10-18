use super::*;
use crate::cap::{Rights, page_frame_cap, rendezvous_cap};
use crate::sched::RendezvousId;

/// Where `printenv` expects the config page, read-only. Must match `components/src/printenv.rs`'s
/// `CONFIG_VA`.
const CONFIG_VA: u64 = address_space_map::pair_page(0x00e0_0000);

/// Spawn `printenv` and return the endpoint its output arrives on.
///
/// `page` is the whole of its config authority, `date_tests::spawn_date`'s own shape: `Some(phys)`
/// grants the frame with **`READ`** and maps it **read-only**, the same rung
/// `crates/system_initializer`'s real wiring grants a child that declares
/// [`grant_plan::Manifest::configuration`]. `None` grants no capability at all, which is the other
/// "nothing to print" cause and a different message.
fn spawn_printenv(endpoints: u64, page: Option<u64>) -> RendezvousId {
    let image = program("printenv").expect("no printenv program in the initial ramdisk");
    let out = crate::sched::create_rendezvous_from(endpoints).expect("no stdout rendezvous");
    crate::sched::spawn(move || match page {
        Some(phys) => run(
            image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(out, Rights::WRITE), // slot 0: stdout
                    page_frame_cap(phys, Rights::READ), // slot 1: a READER, and nothing more
                ],
                maps: &[Mapping {
                    va: CONFIG_VA,
                    phys,
                    flags: Flags::user_rodata(),
                }],
            },
        ),
        None => run(
            image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[rendezvous_cap(out, Rights::WRITE)],
                maps: &[],
            },
        ),
    })
    .expect("could not spawn printenv");
    out
}

/// One line of `printenv`'s output, without its newline. `date_tests::line`'s own framing reader.
fn line(out: RendezvousId, buf: &mut [u8; 128]) -> usize {
    let mut len = 0usize;
    loop {
        let words = crate::sched::ipc_receive(out);
        let count = words[0] as usize;
        assert!(
            (1..=16).contains(&count),
            "printenv: a stdout message with a bad byte count: {count}"
        );
        let mut chunk = [0u8; 16];
        chunk[..8].copy_from_slice(&words[1].to_le_bytes());
        chunk[8..].copy_from_slice(&words[2].to_le_bytes());
        for &b in &chunk[..count] {
            if b == b'\n' {
                return len;
            }
            assert!(
                len < buf.len(),
                "printenv printed a line longer than a line"
            );
            buf[len] = b;
            len += 1;
        }
    }
}

/// Read `printenv`'s stream to its end, so it has nothing left to send when its stdout goes, then
/// reclaim the region the endpoint came from.
fn finish(out: RendezvousId, endpoints: u64) {
    while crate::sched::ipc_receive(out)[0] != byte_sink_contract::eof() {}
    crate::sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// A page nobody assembled: allocated and left zeroed, exactly `date_tests`'s unpublished-clock
/// shape and `environment_contract`'s own `a_zeroed_page_reads_as_no_configuration`.
fn blank_page() -> u64 {
    crate::memory::alloc_zeroed()
        .expect("no frame for a blank config page")
        .addr()
}

/// Assemble a page with `builder`, into a fresh frame, and return its physical address.
fn assembled_page(
    builder: impl FnOnce(
        environment_contract::PageBuilder<'static>,
    ) -> environment_contract::PageBuilder<'static>,
) -> u64 {
    let bytes = builder(environment_contract::PageBuilder::new()).build();
    let phys = crate::memory::alloc_zeroed()
        .expect("no frame for a config page")
        .addr();
    // SAFETY: `phys` is that fresh frame, named through the direct map and owned by nobody else,
    // and `bytes` is a `PageBuilder`'s output, far under `FRAME_SIZE`.
    unsafe {
        core::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            mmu::phys_to_virt(phys) as *mut u8,
            bytes.len(),
        );
    }
    phys
}

/// **`printenv` prints back exactly the page it was granted, real values and all.**
///
/// The values are deliberately not the boot's own defaults (`UTC`/`C`/`dumb`): a program that
/// happened to have those three strings compiled in would pass this test by coincidence. Distinct
/// values from every one of `environment_contract`'s three domains prove the read path carries the
/// page's own bytes rather than a baked-in answer.
#[test_case]
fn printenv_prints_the_page_it_was_granted() {
    let phys = assembled_page(|b| {
        b.tz("America/Los_Angeles")
            .expect("a real KNOWN_TZ member")
            .lang("en_US.UTF-8")
            .expect("a real KNOWN_LANG member")
            .term("xterm-256color")
            .expect("a real KNOWN_TERM member")
    });
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let out = spawn_printenv(endpoints, Some(phys));
    let mut buf = [0u8; 128];

    let n = line(out, &mut buf);
    assert_eq!(
        core::str::from_utf8(&buf[..n]).unwrap(),
        "TZ=America/Los_Angeles"
    );
    let n = line(out, &mut buf);
    assert_eq!(core::str::from_utf8(&buf[..n]).unwrap(), "LANG=en_US.UTF-8");
    let n = line(out, &mut buf);
    assert_eq!(
        core::str::from_utf8(&buf[..n]).unwrap(),
        "TERM=xterm-256color"
    );
    finish(out, endpoints);
}

/// **A key the page never declared reads as `(unset)`, not as an empty value.**
///
/// `LANG` and `TERM` are left off this page entirely; the assertion is that `printenv` says so
/// plainly rather than printing `LANG=` (which would read as "explicitly set to nothing", a
/// different claim `environment_contract`'s `Option<&str>` never makes).
#[test_case]
fn a_key_never_declared_reads_as_unset_not_empty() {
    let phys = assembled_page(|b| b.tz("UTC").expect("UTC is a real KNOWN_TZ member"));
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let out = spawn_printenv(endpoints, Some(phys));
    let mut buf = [0u8; 128];

    let n = line(out, &mut buf);
    assert_eq!(core::str::from_utf8(&buf[..n]).unwrap(), "TZ=UTC");
    let n = line(out, &mut buf);
    assert_eq!(core::str::from_utf8(&buf[..n]).unwrap(), "LANG (unset)");
    let n = line(out, &mut buf);
    assert_eq!(core::str::from_utf8(&buf[..n]).unwrap(), "TERM (unset)");
    finish(out, endpoints);
}

/// **A page nobody assembled reads as no configuration, not as three empty strings.**
///
/// This is `environment_contract`'s own default-honest shape (the same one `boot_clock_page` uses for
/// a machine with no RTC), proven again from the reading side rather than only against the raw
/// bytes: a zeroed frame is indistinguishable from "the page was never carried at all", by design,
/// so a boot that granted the slot but never had anything write to it still tells the truth.
#[test_case]
fn an_unpublished_config_page_reads_as_no_configuration() {
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let out = spawn_printenv(endpoints, Some(blank_page()));
    let mut buf = [0u8; 128];

    let n = line(out, &mut buf);
    assert_eq!(core::str::from_utf8(&buf[..n]).unwrap(), "TZ (unset)");
    let n = line(out, &mut buf);
    assert_eq!(core::str::from_utf8(&buf[..n]).unwrap(), "LANG (unset)");
    let n = line(out, &mut buf);
    assert_eq!(core::str::from_utf8(&buf[..n]).unwrap(), "TERM (unset)");
    finish(out, endpoints);
}

/// **No capability at all is a different, plainer sentence**, and `printenv` has to say it without
/// touching `CONFIG_VA`: a process granted nothing has nothing mapped there, and a read would
/// fault rather than answer. `date_tests`'s `an_unknown_clock_is_said_plainly...` proves the same
/// probe-before-touch property for the clock slot; this is that property's `config` twin.
#[test_case]
fn no_capability_at_all_is_said_plainly() {
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let out = spawn_printenv(endpoints, None);
    let mut buf = [0u8; 128];
    let n = line(out, &mut buf);
    assert_eq!(
        core::str::from_utf8(&buf[..n]).unwrap(),
        "printenv: no configuration was granted",
    );
    finish(out, endpoints);
}
