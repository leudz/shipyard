use alloc::vec::Vec;

#[test]
fn candidate_bitmaps_preserve_single_and_disjoint_runs() {
    use crate::sparse_set::tracking_plan::TrackingPlan;

    let len = 130 * 64 - 7;
    for runs in [
        [0..4, 0..0],
        [62..66, 0..0],
        [127..130, 0..0],
        [2..5, 63..67],
    ] {
        let selected: Vec<_> = (0..len)
            .filter(|index| runs.iter().any(|run| run.contains(&(index / 64))))
            .collect();
        let plan = TrackingPlan::build(len, |chunk| runs.iter().any(|run| run.contains(&chunk)));
        for start in (0..len).step_by(31) {
            let before = selected.partition_point(|&index| index < start);
            assert_eq!(plan.count(0, start), before);
            assert_eq!(
                plan.next_in(start, len),
                selected.get(before).copied().unwrap_or(len)
            );
        }
        for count in (0..=selected.len()).step_by(17) {
            let cut = plan.split_at(0, len, count);
            assert_eq!(plan.count(0, cut), count);
            assert_eq!(plan.count(cut, len), selected.len() - count);
        }
    }
}
