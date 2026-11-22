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
fn a_retype_fits_only_inside_the_budget() {
    // (pages, watermark, count, new watermark)
    let cases = [
        (8, 0, 1, Some(1)),
        (8, 0, 8, Some(8)),
        (8, 7, 1, Some(8)),
        (8, 8, 1, None),
        (8, 0, 0, None),
        (8, 4, 5, None),
        (8, u64::MAX, 1, None),
        (0, 0, 1, None),
    ];
    for (pages, watermark, count, expected) in cases {
        assert_eq!(
            retype_new_watermark(pages, watermark, count),
            expected,
            "pages {pages}, watermark {watermark}, count {count}"
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

/// **No short sequence of the table's own calls makes a retype overlap a live child's run, or a
/// child leave its parent's spent part.** Every sequence of up to five calls (a retype of one, two
/// or three pages, a split of one or two pages, or a claim and return of a region) on a root of six
/// pages, over the first four regions made, is replayed from an empty table, and after each call
/// every live region keeps `retyped_to <= watermark <= pages`, every live child lies inside its
/// parent's spent part, and its parent's retyped pages end at or below the child's start or above
/// the child's end. The Kani proof `a_return_keeps_every_run_inside_its_parents_spent_part` starts
/// from any state satisfying that last rule; this test is the evidence that the rule holds in the
/// states the table reaches, which the C Bounded Model Checker (CBMC) cannot build in memory.
#[test]
fn no_short_call_sequence_overlaps_a_childs_run_with_its_parents_retyped_pages() {
    const CALLS: u32 = 4 * (3 + 2) + 3;
    const DEPTH: u32 = 5;
    for code in 0..CALLS.pow(DEPTH) {
        let mut table = RegionTable::<4>::new();
        let root = table.insert_root(0x100, 6).unwrap();
        // (name, parent index, retyped_to) of each region made, in order.
        let mut made: Vec<(RegionName, Option<usize>, u64)> = vec![(root, None, 0)];
        let mut rest = code;
        for _ in 0..DEPTH {
            let call = rest % CALLS;
            rest /= CALLS;
            if call < 20 {
                let (region, kind) = ((call / 5) as usize, call % 5);
                let Some(&(name, _, _)) = made.get(region) else {
                    continue;
                };
                if kind < 3 {
                    if table.retype_run(name, u64::from(kind) + 1).is_some() {
                        made[region].2 = table.usage(name).unwrap().0;
                    }
                } else if let Some(child) = table.split(name, u64::from(kind - 2)) {
                    made.push((child, Some(region), 0));
                }
            } else if let Some(&(name, Some(_), _)) = made.get((call - 19) as usize) {
                table.unpin(name);
                if let Some(claim) = table.claim_for_destroy(name) {
                    table.return_to_parent(claim);
                }
            }
            for &(name, parent, retyped_to) in &made {
                let Some((watermark, pages)) = table.usage(name) else {
                    continue;
                };
                assert!(retyped_to <= watermark && watermark <= pages, "code {code}");
                let Some(parent) = parent else { continue };
                let (parent_name, _, parent_retyped_to) = made[parent];
                let (parent_watermark, _) = table.usage(parent_name).unwrap();
                let (base_page, run) = table.bounds(name).unwrap();
                let (parent_base, _) = table.bounds(parent_name).unwrap();
                let (start, end) = (base_page - parent_base, base_page - parent_base + run);
                assert!(
                    end <= parent_watermark,
                    "outside the spent part, code {code}"
                );
                assert!(
                    parent_retyped_to <= start || parent_retyped_to > end,
                    "a retyped page overlaps a child, code {code}"
                );
            }
        }
    }
}
