use super::*;

#[test]
fn split_refuses_zero_overflow_and_over_budget() {
    assert_eq!(split_new_watermark(8, 0, 0), None); // zero carve
    assert_eq!(split_new_watermark(8, 4, 5), None); // 4+5 > 8, over budget
    assert_eq!(split_new_watermark(8, u64::MAX, 1), None); // overflow
    assert_eq!(split_new_watermark(8, 4, 4), Some(8)); // exact fit
    assert_eq!(split_new_watermark(8, 0, 4), Some(4)); // first carve
}

#[test]
fn destroy_decides_by_ownership() {
    assert_eq!(
        destroy_outcome(true, 0, true, false, 0),
        DestroyOutcome::Refused
    );
    assert_eq!(
        destroy_outcome(false, 1, true, false, 0),
        DestroyOutcome::Refused
    );
    assert_eq!(
        destroy_outcome(false, 0, true, false, 0),
        DestroyOutcome::FreeToGlobal
    );
    assert_eq!(
        destroy_outcome(false, 0, false, true, 4),
        DestroyOutcome::ReturnToParent { unbump: 4 }
    );
    assert_eq!(
        destroy_outcome(false, 0, false, false, 4),
        DestroyOutcome::ReturnToParent { unbump: 0 }
    );
}

#[test]
fn a_retype_count_of_zero_means_one_page() {
    // (requested, pages)
    for (requested, pages) in [(0, 1), (1, 1), (2, 2), (u64::MAX, u64::MAX)] {
        assert_eq!(retype_pages(requested), pages, "requested {requested}");
    }
}

#[test]
fn a_retype_fits_only_inside_the_budget() {
    // (pages, watermark, requested, new watermark)
    let cases = [
        (8, 0, 0, Some(1)),
        (8, 0, 8, Some(8)),
        (8, 7, 0, Some(8)),
        (8, 8, 0, None),
        (8, 4, 5, None),
        (8, u64::MAX, 1, None),
        (0, 0, 0, None),
    ];
    for (pages, watermark, requested, expected) in cases {
        assert_eq!(
            retype_new_watermark(pages, watermark, requested),
            expected,
            "pages {pages}, watermark {watermark}, requested {requested}"
        );
    }
}

#[test]
fn the_watermark_falls_only_when_quiescent_and_never_rises() {
    // (watermark, retyped_to, live_to, quiescent, result)
    let cases = [
        (10, 2, 4, true, 4),
        (10, 6, 4, true, 6),
        (10, 0, 0, true, 0),
        (10, 2, 4, false, 10),
        (3, 9, 9, true, 3),
    ];
    for (watermark, retyped_to, live_to, quiescent, expected) in cases {
        assert_eq!(
            coalesced_watermark(watermark, retyped_to, live_to, quiescent),
            expected,
            "({watermark}, {retyped_to}, {live_to}, {quiescent})"
        );
    }
}
