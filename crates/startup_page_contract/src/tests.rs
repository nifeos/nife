//! The startup page's host tests: a written page reads back, a reader skips unknown kinds, and the
//! writer refuses every list a reader could misread.
//!
//! Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.

use super::*;

fn window(kind: WindowKind, address: u64, pages: u32) -> Window {
    Window {
        kind,
        address,
        pages,
    }
}

/// What the spawner writes is what the program reads, entry for entry and in order.
#[test]
fn a_written_page_reads_back_every_window() {
    let mut page = [0u8; PAGE_BYTES];
    let windows = [
        window(kind::NON_VOLATILE_MEMORY_EXPRESS_DATA_PLANE, 0x98_0000, 50),
        window(
            kind::NON_VOLATILE_MEMORY_EXPRESS_DOORBELL_PAGE,
            0x9c_0000,
            1,
        ),
    ];
    write(&mut page, &windows).unwrap();
    let read = StartupPage::parse(&page).unwrap();
    assert!(read.windows().eq(windows.iter().copied()));
    assert_eq!(read.window(kind::E1000E_DATA_PLANE), None);
}

/// A reader skips a kind it was never told about, so a later spawner may add kinds.
#[test]
fn an_unknown_kind_is_skipped_and_the_known_one_still_found() {
    let mut page = [0u8; PAGE_BYTES];
    let later = window(WindowKind(0xffff), 0x60_0000, 1);
    let known = window(kind::E1000E_DATA_PLANE, 0x90_0000, 64);
    write(&mut page, &[later, known]).unwrap();
    assert_eq!(
        StartupPage::parse(&page)
            .unwrap()
            .window(kind::E1000E_DATA_PLANE),
        Some(known)
    );
}

/// A zeroed frame, a short buffer and a count past the page are not pages.
#[test]
fn bytes_that_are_not_a_page_read_as_none() {
    assert!(StartupPage::parse(&[0u8; PAGE_BYTES]).is_none());
    assert!(StartupPage::parse(&[0u8; 16]).is_none());
    let mut page = [0u8; PAGE_BYTES];
    write(&mut page, &[]).unwrap();
    assert_eq!(StartupPage::parse(&page).unwrap().windows().count(), 0);
    page[OFF_COUNT..OFF_COUNT + 4].copy_from_slice(&(MAXIMUM_WINDOWS as u32 + 1).to_le_bytes());
    assert!(StartupPage::parse(&page).is_none());
}

/// Each malformed list is refused on its own, and nothing is written for it.
#[test]
fn the_writer_refuses_each_malformed_list() {
    let mut page = [0u8; PAGE_BYTES];
    let good = window(kind::E1000E_DATA_PLANE, 0x90_0000, 1);
    assert_eq!(write(&mut [0u8; 64], &[good]), Err(Refusal::PageTooSmall));
    for bad in [
        window(WindowKind(0), 0x90_0000, 1),
        window(kind::E1000E_DATA_PLANE, 0x90_0000, 0),
        window(kind::E1000E_DATA_PLANE, 0x90_0008, 1),
        window(kind::E1000E_DATA_PLANE, u64::MAX & !0xfff, 2),
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

/// Every kind's number is distinct and none is zero, which names no window.
#[test]
fn every_kind_has_its_own_nonzero_number() {
    for (n, k) in kind::ALL.iter().enumerate() {
        assert_ne!(k.0, 0);
        assert!(
            kind::ALL[..n].iter().all(|earlier| earlier != k),
            "{k:?} repeats"
        );
    }
}

/// The page sits in the runtime band and clear of std's runtime pages and the builder scratch.
#[test]
fn the_page_address_sits_clear_of_its_neighbours() {
    assert!(address_space_map::RUNTIME_WINDOWS.contains(PAGE_VIRTUAL_ADDRESS));
    assert!(PAGE_VIRTUAL_ADDRESS >= 0x1700_0000 + address_space_map::PAGE);
}
