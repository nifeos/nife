//! The startup page's host tests: a written page reads back, a reader skips unknown kinds, and the
//! writer refuses every list a reader could misread.
//!
//! Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.

use super::*;

const DATA_PLANE: WindowKind = WindowKind(1);
const DOORBELL_PAGE: WindowKind = WindowKind(2);
const OTHER_DATA_PLANE: WindowKind = WindowKind(3);

fn window(kind: WindowKind, virtual_address: u64, pages: u32) -> Window {
    Window {
        kind,
        virtual_address,
        pages,
    }
}

/// What the spawner writes is what the program reads, entry for entry and in order.
#[test]
fn a_written_page_reads_back_every_window() {
    let mut page = [0u8; PAGE_BYTES];
    let windows = [
        window(DATA_PLANE, 0x98_0000, 50),
        window(DOORBELL_PAGE, 0x9c_0000, 1),
    ];
    write(&mut page, &windows).unwrap();
    let read = StartupPage::parse(&page).unwrap();
    assert!(read.windows().eq(windows.iter().copied()));
    assert_eq!(read.window(OTHER_DATA_PLANE), None);
}

/// A reader skips a kind it was never told about, so a later spawner may add kinds.
#[test]
fn an_unknown_kind_is_skipped_and_the_known_one_still_found() {
    let mut page = [0u8; PAGE_BYTES];
    let later = window(WindowKind(0xffff), 0x60_0000, 1);
    let known = window(OTHER_DATA_PLANE, 0x90_0000, 64);
    write(&mut page, &[later, known]).unwrap();
    assert_eq!(
        StartupPage::parse(&page).unwrap().window(OTHER_DATA_PLANE),
        Some(known)
    );
}

/// A zeroed frame, a short buffer and a count past the page are not pages.
#[test]
fn bytes_that_are_not_a_page_read_as_none() {
    assert!(StartupPage::parse(&[0u8; PAGE_BYTES]).is_none());
    let mut page = [0u8; PAGE_BYTES];
    write(&mut page, &[]).unwrap();
    assert!(
        StartupPage::parse(&page[..OFF_ENTRIES]).is_none(),
        "a header alone, magic and all, is not a page"
    );
    assert_eq!(StartupPage::parse(&page).unwrap().windows().count(), 0);
    page[OFF_COUNT..OFF_COUNT + 4].copy_from_slice(&(MAXIMUM_WINDOWS as u32 + 1).to_le_bytes());
    assert!(StartupPage::parse(&page).is_none());
}

/// Each malformed list is refused on its own, and nothing is written for it.
#[test]
fn the_writer_refuses_each_malformed_list() {
    let mut page = [0u8; PAGE_BYTES];
    let good = window(OTHER_DATA_PLANE, 0x90_0000, 1);
    assert_eq!(write(&mut [0u8; 64], &[good]), Err(Refusal::PageTooSmall));
    for bad in [
        window(WindowKind(0), 0x90_0000, 1),
        window(OTHER_DATA_PLANE, 0x90_0000, 0),
        window(OTHER_DATA_PLANE, 0x90_0008, 1),
        window(OTHER_DATA_PLANE, 0xffff_ffff_ffff_f000, 2),
        window(OTHER_DATA_PLANE, 0xffff_ffff_ffff_0000, 32),
    ] {
        assert_eq!(
            write(&mut page, &[bad]),
            Err(Refusal::MalformedWindow(bad.kind))
        );
    }
    assert_eq!(
        write(&mut page, &[good, good]),
        Err(Refusal::DuplicateKind(good.kind))
    );
    let many = [good; MAXIMUM_WINDOWS + 1];
    assert_eq!(write(&mut page, &many), Err(Refusal::TooManyWindows));
    assert!(
        StartupPage::parse(&page).is_none(),
        "a refusal writes nothing"
    );
}

/// The bytes sit where the README says: magic, count, four zero bytes, then entries from byte 16
/// as kind, pages and virtual address, little-endian.
#[test]
fn the_layout_is_the_documented_one() {
    let mut page = [0xaau8; PAGE_BYTES];
    write(&mut page, &[window(DOORBELL_PAGE, 0x9c_0000, 3)]).unwrap();
    assert_eq!(&page[..8], b"STARTPG1");
    assert_eq!(page[8..16], [1, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(page[16..20], 2u32.to_le_bytes());
    assert_eq!(page[20..24], 3u32.to_le_bytes());
    assert_eq!(page[24..32], 0x9c_0000u64.to_le_bytes());
    assert_eq!(page[32], 0xaa, "past the last entry the page is as given");
}

/// A full page holds exactly `MAXIMUM_WINDOWS` entries, the last ending at the page's last byte.
#[test]
fn a_full_page_fits_its_maximum() {
    let mut page = [0u8; PAGE_BYTES];
    let windows: Vec<Window> = (1..=MAXIMUM_WINDOWS as u32)
        .map(|k| window(WindowKind(k), k as u64 * 0x1000, 1))
        .collect();
    write(&mut page, &windows).unwrap();
    assert_eq!(OFF_ENTRIES + MAXIMUM_WINDOWS * ENTRY_BYTES, PAGE_BYTES);
    assert!(StartupPage::parse(&page).unwrap().windows().eq(windows));
}

/// The page sits in the runtime band and clear of std's runtime pages and the builder scratch.
#[test]
#[allow(clippy::assertions_on_constants)]
fn the_page_sits_clear_of_its_neighbors() {
    assert!(address_space_map::RUNTIME_WINDOWS.contains(PAGE_VIRTUAL_ADDRESS));
    assert!(PAGE_VIRTUAL_ADDRESS >= 0x1700_0000 + address_space_map::PAGE);
}

/// A `StartupWindow` finds its kind's address on the page, then answers from its cache without
/// reading the page again.
#[test]
fn a_startup_window_reads_the_page_once() {
    let mut page = [0u8; PAGE_BYTES];
    write(&mut page, &[window(DATA_PLANE, 0x98_0000, 2)]).unwrap();
    // SAFETY: the test hands the page in a buffer and never calls `virtual_address`.
    let plane = unsafe { StartupWindow::new(DATA_PLANE, 2 * address_space_map::PAGE) };
    assert_eq!(plane.length_bytes(), 2 * address_space_map::PAGE);
    assert_eq!(
        plane.virtual_address_in(|| StartupPage::parse(&page)),
        0x98_0000
    );
    assert_eq!(
        plane.virtual_address_in(|| panic!("the page was read twice")),
        0x98_0000
    );
}

/// A missing page, a missing kind and a window shorter than the program uses each panic.
#[test]
fn a_startup_window_refuses_a_page_that_does_not_match() {
    let mut page = [0u8; PAGE_BYTES];
    write(&mut page, &[window(DATA_PLANE, 0x98_0000, 1)]).unwrap();
    let run = |kind: WindowKind, length_bytes: u64, present: bool| {
        std::panic::catch_unwind(|| {
            // SAFETY: the test hands the page in a buffer and never calls `virtual_address`.
            let w = unsafe { StartupWindow::new(kind, length_bytes) };
            w.virtual_address_in(|| present.then(|| StartupPage::parse(&page).unwrap()))
        })
    };
    assert_eq!(
        run(DATA_PLANE, address_space_map::PAGE, true).ok(),
        Some(0x98_0000)
    );
    assert!(run(DATA_PLANE, address_space_map::PAGE, false).is_err());
    assert!(run(DOORBELL_PAGE, address_space_map::PAGE, true).is_err());
    assert!(run(DATA_PLANE, address_space_map::PAGE + 1, true).is_err());
}
