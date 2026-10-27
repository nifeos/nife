//! Tests of the leg-cost verdict.

#[test]
fn a_leg_thirty_times_its_baseline_fails_and_one_at_twice_it_does_not() {
    // Milestone 628's defect under KVM: 7.7 s a line against 0.26 s.
    let slow = vec![7.7; 128];
    let v = super::leg_cost_verdict("x86_64", "kvm", &slow);
    assert!(v.over, "{v:?}");
    let ok = vec![0.52; 128];
    assert!(!super::leg_cost_verdict("x86_64", "kvm", &ok).over);
}

#[test]
fn the_median_ignores_the_one_slow_line_the_mean_would_not() {
    // `interrupt_ignorer` costs 7 s on every leg and is no defect: one outlier in 142.
    let mut seconds = vec![0.2; 141];
    seconds.push(7.0);
    let v = super::leg_cost_verdict("x86_64", "kvm", &seconds);
    assert_eq!(v.median, 0.2);
    assert!(!v.over);
}

#[test]
fn a_boot_of_few_lines_and_a_leg_with_no_row_are_reported_not_judged() {
    assert!(!super::leg_cost_verdict("aarch64", "tcg", &[9.0; 8]).over);
    let v = super::leg_cost_verdict("aarch64", "kvm", &[9.0; 100]);
    assert_eq!((v.baseline, v.over), (None, false));
}

#[test]
fn x86_under_tcg_is_held_to_its_own_row_and_not_to_aarch64s() {
    // 2.85 s a line is fourteen times aarch64's and is this leg's healthy cost under TCG.
    assert!(!super::leg_cost_verdict("x86_64", "tcg", &[2.85; 128]).over);
    assert!(super::leg_cost_verdict("x86_64", "tcg", &[15.0; 128]).over);
}

#[test]
fn an_even_count_takes_the_mean_of_the_middle_two() {
    let v = super::leg_cost_verdict("aarch64", "tcg", &[0.1, 0.1, 0.3, 0.3]);
    assert!((v.median - 0.2).abs() < 1e-9);
}
