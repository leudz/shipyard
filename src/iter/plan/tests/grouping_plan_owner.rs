use super::super::*;
use alloc::vec;
use core::ptr;

fn entities(len: u64) -> Vec<EntityId> {
    (0..len).map(EntityId::new).collect()
}

fn segment(entities: &[EntityId], row: usize) -> SegmentDescriptor {
    // SAFE: Every test keeps `entities` alive and unmodified until its plans and cursors are gone.
    unsafe { SegmentDescriptor::new(NonNull::from(entities).cast(), entities.len(), row) }
}

fn empty_segment(row: usize) -> SegmentDescriptor {
    // SAFE: `len` is 0 so the pointer is never read.
    unsafe { SegmentDescriptor::new(NonNull::dangling(), 0, row) }
}

fn window(segment_start: usize) -> NodeState {
    NodeState::Window { segment_start }
}

fn mixed(captain_mask: u32, start: usize, len: usize) -> NodeState {
    NodeState::Mixed {
        captain_mask,
        coverage: CoverageSpan { start, len },
    }
}

/// A single window plan with one row per segment.
fn window_plan(segments: &[&[EntityId]]) -> Plan {
    let descriptors = segments
        .iter()
        .enumerate()
        .map(|(row, entities)| segment(entities, row))
        .collect();
    let rows = (0..segments.len()).map(|row| window(row * 10)).collect();

    Plan::new(&[NodeKind::Window], descriptors, rows, Vec::new()).unwrap()
}

const MIXED_LAYOUT: [NodeKind; 3] = [
    NodeKind::Mixed { children: 2 },
    NodeKind::Window,
    NodeKind::Window,
];

/// Builds a `Mixed` plan with `rows` and `coverage`, using row 0 for a single segment.
fn mixed_plan(rows: Vec<NodeState>, coverage: Vec<Coverage>) -> Result<Plan, PlanError> {
    let entities = entities(1);

    Plan::new(&MIXED_LAYOUT, vec![segment(&entities, 0)], rows, coverage)
}

fn collect(mut cursor: EntityCursor) -> Vec<EntityId> {
    let mut result = Vec::new();
    while !cursor.is_empty() {
        // SAFE: The cursor isn't empty.
        result.push(unsafe { cursor.current() });
        cursor.advance();
    }

    result
}

fn assert_send_sync<T: Send + Sync>() {}

/// Asserts that schedules and their plans can be sent and shared across threads.
#[test]
fn send_sync() {
    assert_send_sync::<Plan>();
    assert_send_sync::<SegmentSchedule>();
}

/// Asserts that a plan without any segment is rejected, an empty traversal has no plan.
#[test]
fn no_segment() {
    let result = Plan::new(&[NodeKind::Window], Vec::new(), vec![window(0)], Vec::new());

    assert_eq!(result.err(), Some(PlanError::NoSegment));
}

/// Asserts that a one segment plan yields its segment once, then keeps the plan
/// with no follow-up left.
#[test]
fn one_segment() {
    let entities = entities(3);
    let mut schedule = SegmentSchedule::new(window_plan(&[&entities]));

    assert_eq!(
        (schedule.follow_ups().len(), schedule.remaining_len()),
        (1, 3)
    );

    let segment = schedule.take_next().unwrap();

    assert_eq!(collect(segment.cursor()), entities);
    assert_eq!(schedule.plan().row(&segment), [window(0)]);
    assert_eq!(
        (schedule.follow_ups().len(), schedule.remaining_len()),
        (0, 0)
    );
    assert!(!schedule.has_follow_ups());
}

/// Asserts that follow-ups come out in order with their own rows
/// and that the remaining candidate count shrinks with each of them.
#[test]
fn follow_ups() {
    let first = entities(2);
    let second = entities(5);
    let third = entities(1);
    let mut schedule = SegmentSchedule::new(window_plan(&[&first, &second, &third]));

    assert_eq!(schedule.remaining_len(), 8);

    for (entities, remaining_len) in [(&first, 6), (&second, 1), (&third, 0)] {
        let peeked = schedule.follow_ups().next().unwrap();
        let segment = schedule.take_next().unwrap();

        assert_eq!(peeked.entities, segment.entities);
        assert_eq!(collect(segment.cursor()), *entities);
        assert_eq!(schedule.remaining_len(), remaining_len);
    }

    let rows: Vec<_> = schedule
        .plan
        .segments
        .iter()
        .map(|s| schedule.plan().row(s)[0])
        .collect();
    assert_eq!(rows, [window(0), window(10), window(20)]);
}

/// Asserts that an exhausted schedule keeps returning no segment without changing its counts.
#[test]
fn exhausted() {
    let entities = entities(2);
    let mut schedule = SegmentSchedule::new(window_plan(&[&entities, &entities]));
    schedule.take_next();
    schedule.take_next();

    for _ in 0..3 {
        assert!(schedule.take_next().is_none());
        assert_eq!(
            (schedule.follow_ups().len(), schedule.remaining_len()),
            (0, 0)
        );
    }
}

/// Asserts that rows can be shared by several segments.
#[test]
fn shared_row() {
    let entities = entities(2);
    let plan = Plan::new(
        &[NodeKind::Window],
        vec![segment(&entities, 0), segment(&entities, 0)],
        vec![window(4)],
        Vec::new(),
    )
    .unwrap();
    let mut schedule = SegmentSchedule::new(plan);

    while let Some(segment) = schedule.take_next() {
        assert_eq!(schedule.plan().row(&segment), [window(4)]);
    }
}

/// Asserts that advancing a clone doesn't move the original and that both share one plan.
#[test]
fn clone_positions() {
    let first = entities(2);
    let second = entities(3);
    let mut original = SegmentSchedule::new(window_plan(&[&first, &second]));
    original.take_next();

    let mut clone = original.clone();
    clone.take_next();

    assert!(ptr::eq(original.plan(), clone.plan()));
    assert_eq!(
        (original.follow_ups().len(), original.remaining_len()),
        (1, 3)
    );
    assert_eq!((clone.follow_ups().len(), clone.remaining_len()), (0, 0));
    assert_eq!(collect(original.take_next().unwrap().cursor()), second);
}

/// Asserts that splitting gives every follow-up to the new schedule
/// and leaves the original with the plan but no follow-up.
#[test]
fn split_follow_ups() {
    let first = entities(2);
    let second = entities(3);
    let mut left = SegmentSchedule::new(window_plan(&[&first, &second]));
    left.take_next();

    let mut right = left.split_follow_ups();

    assert!(ptr::eq(left.plan(), right.plan()));
    assert_eq!((left.follow_ups().len(), left.remaining_len()), (0, 0));
    assert!(left.take_next().is_none());
    assert_eq!((right.follow_ups().len(), right.remaining_len()), (1, 3));
    assert_eq!(collect(right.take_next().unwrap().cursor()), second);
}

/// Asserts that the plan is destroyed exactly once, when its last owner is dropped.
#[test]
fn destroy_once() {
    let entities = entities(2);
    let mut schedule = SegmentSchedule::new(window_plan(&[&entities]));
    let weak = Arc::downgrade(&schedule.plan);
    let clone = schedule.clone();
    let split = schedule.split_follow_ups();

    assert_eq!(weak.strong_count(), 3);

    drop(split);
    assert_eq!(weak.strong_count(), 2);

    drop(schedule);
    assert_eq!(weak.strong_count(), 1);

    drop(clone);
    assert!(weak.upgrade().is_none());
}

const EXPECTED_COVERAGE: [Coverage; 1] = [Coverage {
    child: 0,
    start: 0,
    end: 4,
}];

/// Builds a schedule over three segments, the last two reading one coverage record.
///
/// The schedule has entered the second segment and has one follow-up left.\
/// Returns it with the coverage of its current segment, as the raw slice a `Mixed`
/// caches when entering a segment.
fn coverage_schedule(
    first: &[EntityId],
    second: &[EntityId],
) -> (SegmentSchedule, *const [Coverage]) {
    let rows = vec![
        mixed(0b01, 0, 0),
        window(0),
        NodeState::Sailor,
        mixed(0b10, 0, 1),
        NodeState::Sailor,
        window(7),
    ];
    let segments = vec![segment(first, 0), segment(second, 1), segment(second, 1)];
    let plan = Plan::new(&MIXED_LAYOUT, segments, rows, EXPECTED_COVERAGE.to_vec()).unwrap();

    let mut schedule = SegmentSchedule::new(plan);
    schedule.take_next();
    let current = schedule.take_next().unwrap();

    let NodeState::Mixed { coverage, .. } = schedule.plan().row(&current)[0] else {
        unreachable!()
    };
    let coverage: *const [Coverage] = schedule.plan().coverage(coverage);

    (schedule, coverage)
}

/// Asserts that after a split, the half without follow-ups keeps its current coverage
/// readable once the half holding the follow-ups is dropped.
#[test]
fn coverage_right_dropped() {
    let first = entities(1);
    let second = entities(2);
    let (mut left, coverage) = coverage_schedule(&first, &second);
    let right = left.split_follow_ups();
    drop(right);

    assert_eq!(left.follow_ups().len(), 0);
    // SAFE: `left` still owns the plan holding these records.
    assert_eq!(unsafe { &*coverage }, EXPECTED_COVERAGE);
}

/// Asserts that after a split, the half holding the follow-ups keeps the shared current
/// coverage readable once the half without follow-ups is dropped.
#[test]
fn coverage_left_dropped() {
    let first = entities(1);
    let second = entities(2);
    let (mut left, coverage) = coverage_schedule(&first, &second);
    let right = left.split_follow_ups();
    drop(left);

    assert_eq!(right.follow_ups().len(), 1);
    // SAFE: `right` still owns the plan holding these records.
    assert_eq!(unsafe { &*coverage }, EXPECTED_COVERAGE);
}

/// Asserts that schedules dropped while unwinding release the plan exactly once.
#[cfg(feature = "std")]
#[test]
fn unwind() {
    let entities = entities(2);
    let schedule = SegmentSchedule::new(window_plan(&[&entities]));
    let weak = Arc::downgrade(&schedule.plan);

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(move || {
        let mut schedule = schedule;
        let _clone = schedule.clone();
        let _split = schedule.split_follow_ups();

        panic!("unwinding with three schedules alive");
    }));

    assert!(result.is_err());
    assert!(weak.upgrade().is_none());
}

/// Asserts that validation doesn't promise entity matching or unique outputs:
/// two segments walking the same entities with the same window start are accepted.
#[test]
fn no_uniqueness_promise() {
    let entities = entities(2);
    let plan = Plan::new(
        &[NodeKind::Window],
        vec![segment(&entities, 0), segment(&entities, 0)],
        vec![window(0)],
        Vec::new(),
    );

    assert!(plan.is_ok());
}

/// Asserts that a segment without candidates is rejected.
#[test]
fn zero_length() {
    let entities = entities(2);
    let result = Plan::new(
        &[NodeKind::Window],
        vec![segment(&entities, 0), empty_segment(0)],
        vec![window(0)],
        Vec::new(),
    );

    assert_eq!(result.err(), Some(PlanError::EmptySegment));
}

/// Asserts that a segment referring to a row past the pool is rejected,
/// including a row index whose offset would overflow.
#[test]
fn row_bounds() {
    let entities = entities(2);

    for row in [1, usize::MAX] {
        let result = Plan::new(
            &[NodeKind::Window],
            vec![segment(&entities, row)],
            vec![window(0)],
            Vec::new(),
        );

        assert_eq!(result.err(), Some(PlanError::RowOutOfBounds));
    }
}

/// Asserts that a row pool holding a partial row is rejected.
#[test]
fn partial_row() {
    let rows = vec![mixed(0b01, 0, 0), window(0), NodeState::Sailor, window(0)];

    assert_eq!(
        mixed_plan(rows, Vec::new()).err(),
        Some(PlanError::MalformedRows)
    );
}

/// Asserts that coverage spans past the pool are rejected, including one whose end overflows.
#[test]
fn coverage_bounds() {
    let record = Coverage {
        child: 0,
        start: 0,
        end: 1,
    };

    for (start, len) in [(0, 2), (1, 1), (usize::MAX, 2)] {
        let rows = vec![mixed(0b01, start, len), window(0), NodeState::Sailor];

        assert_eq!(
            mixed_plan(rows, vec![record]).err(),
            Some(PlanError::CoverageOutOfBounds)
        );
    }
}

/// Asserts that coverage records with an empty region or naming a missing child are rejected.
#[test]
fn coverage_records() {
    let records = [
        Coverage {
            child: 0,
            start: 3,
            end: 3,
        },
        Coverage {
            child: 2,
            start: 0,
            end: 1,
        },
    ];

    for record in records {
        let rows = vec![mixed(0b01, 0, 1), window(0), NodeState::Sailor];

        assert_eq!(
            mixed_plan(rows, vec![record]).err(),
            Some(PlanError::InvalidCoverage)
        );
    }
}

/// Asserts that a coverage record naming a nested `Mixed` is rejected,
/// only window children have a storage index to match.
#[test]
fn coverage_child_kind() {
    // Mixed(Window, Mixed(Window)), the outer Mixed has a record on its second child.
    let layout = [
        NodeKind::Mixed { children: 2 },
        NodeKind::Window,
        NodeKind::Mixed { children: 1 },
        NodeKind::Window,
    ];
    let entities = entities(1);
    let row = vec![
        mixed(0b01, 0, 1),
        window(0),
        NodeState::Sailor,
        NodeState::Sailor,
    ];
    let record = Coverage {
        child: 1,
        start: 0,
        end: 1,
    };

    let result = Plan::new(&layout, vec![segment(&entities, 0)], row, vec![record]);

    assert_eq!(result.err(), Some(PlanError::InvalidCoverage));
}

/// Asserts that the total candidate count overflowing a `usize` is rejected.
#[test]
fn length_overflow() {
    assert_eq!(
        checked_total_len([usize::MAX, 1]),
        Err(PlanError::LengthOverflow)
    );
    assert_eq!(checked_total_len([usize::MAX - 1, 1]), Ok(usize::MAX));
}

/// Asserts that a state of the wrong kind is rejected, including a sailor root.
#[test]
fn state_tag() {
    let rows = [
        [window(0), window(0), NodeState::Sailor],
        [NodeState::Sailor, NodeState::Sailor, NodeState::Sailor],
        [mixed(0b01, 0, 0), mixed(0b01, 0, 0), NodeState::Sailor],
    ];

    for row in rows {
        assert_eq!(
            mixed_plan(row.to_vec(), Vec::new()).err(),
            Some(PlanError::StateMismatch)
        );
    }
}

/// Asserts that a sailor child with an active state is rejected,
/// as are the descendants of a sailor `Or` branch.
#[test]
fn sailor_subtree() {
    let rows = vec![mixed(0b01, 0, 0), window(0), window(0)];
    assert_eq!(
        mixed_plan(rows, Vec::new()).err(),
        Some(PlanError::StateMismatch)
    );

    // Or(A: Mixed(Window, Window), B: Window) with A as a sailor.
    let layout = [
        NodeKind::Or,
        NodeKind::Mixed { children: 2 },
        NodeKind::Window,
        NodeKind::Window,
        NodeKind::Window,
    ];
    let entities = entities(1);
    let row = vec![
        NodeState::Or {
            branch: OrBranch::B,
        },
        NodeState::Sailor,
        window(0),
        NodeState::Sailor,
        window(0),
    ];
    let result = Plan::new(&layout, vec![segment(&entities, 0)], row, Vec::new());

    assert_eq!(result.err(), Some(PlanError::StateMismatch));
}

/// Asserts that a nested `Mixed` keeps its own captain mask:
/// the outer mask flags its immediate children, the inner one flags its own.
#[test]
fn nested_masks() {
    // Mixed(Window, Mixed(Window, Window)), the inner Mixed drives with its second child.
    let layout = [
        NodeKind::Mixed { children: 2 },
        NodeKind::Window,
        NodeKind::Mixed { children: 2 },
        NodeKind::Window,
        NodeKind::Window,
    ];
    let entities = entities(1);
    let row = vec![
        mixed(0b10, 0, 0),
        NodeState::Sailor,
        mixed(0b10, 0, 0),
        NodeState::Sailor,
        window(0),
    ];

    assert!(Plan::new(&layout, vec![segment(&entities, 0)], row, Vec::new()).is_ok());
}

/// Asserts that empty captain masks and masks flagging missing children are rejected.
#[test]
fn captain_mask() {
    for captain_mask in [0, 0b100, 1 << 31] {
        let rows = vec![mixed(captain_mask, 0, 0), window(0), window(0)];

        assert_eq!(
            mixed_plan(rows, Vec::new()).err(),
            Some(PlanError::InvalidCaptainMask)
        );
    }
}

/// Asserts that a `Mixed` with 32 children accepts its last child as captain.
#[test]
fn mask_width() {
    let mut layout = vec![NodeKind::Mixed { children: 32 }];
    layout.extend([NodeKind::Window; 32]);
    let mut row = vec![mixed(1 << 31, 0, 0)];
    row.extend([NodeState::Sailor; 31]);
    row.push(window(0));
    let entities = entities(1);

    assert!(Plan::new(&layout, vec![segment(&entities, 0)], row, Vec::new()).is_ok());
}

/// Asserts that layouts that aren't a single preorder tree are rejected:
/// an empty layout, missing children, extra nodes, and `Mixed`s with 0 or 33 children.
#[test]
fn layout() {
    let entities = entities(1);
    let or_a = NodeState::Or {
        branch: OrBranch::A,
    };
    let cases: [(&[NodeKind], Vec<NodeState>); 5] = [
        (&[], Vec::new()),
        (&[NodeKind::Or, NodeKind::Window], vec![or_a, window(0)]),
        (
            &[NodeKind::Window, NodeKind::Window],
            vec![window(0), NodeState::Sailor],
        ),
        (&[NodeKind::Mixed { children: 0 }], vec![mixed(0b1, 0, 0)]),
        (&[NodeKind::Mixed { children: 33 }], vec![mixed(0b1, 0, 0)]),
    ];

    for (layout, row) in cases {
        let result = Plan::new(layout, vec![segment(&entities, 0)], row, Vec::new());

        assert_eq!(result.err(), Some(PlanError::MalformedLayout));
    }
}

/// Builds a schedule over entities `0..10` in four segments of 3, 1, 4 and 2 candidates,
/// with rows `window(0)` to `window(30)`.
fn four_segments(all: &[EntityId]) -> SegmentSchedule {
    SegmentSchedule::new(window_plan(&[&all[..3], &all[3..4], &all[4..8], &all[8..]]))
}

/// Returns the number of candidates of each follow-up left.
fn lens(schedule: &SegmentSchedule) -> Vec<usize> {
    schedule.follow_ups().map(|segment| segment.len()).collect()
}

/// Returns the entities of every follow-up left, in order, without consuming them.
fn stream(schedule: &SegmentSchedule) -> Vec<EntityId> {
    schedule
        .follow_ups()
        .flat_map(|segment| collect(segment.cursor()))
        .collect()
}

/// Returns the entities of `cut`'s segment from the cut on, checking they keep their local indices.
fn suffix(cut: &FollowUpCut) -> Vec<EntityId> {
    let (_, suffix) = cut.segment.cursor().split_at(cut.local_index);
    assert_eq!(suffix.local_index(), cut.local_index);

    collect(suffix)
}

/// Asserts that a cut inside a later segment leaves a prefix of it, with the segment's own row,
/// and hands the suffix and the following segments to the rest.
#[test]
fn split_at_inside() {
    let all = entities(10);
    let mut left = four_segments(&all);

    let cut = left.split_follow_ups_at(6);

    assert_eq!(lens(&left), [3, 1, 2]);
    assert_eq!(left.remaining_len(), 6);
    assert_eq!(stream(&left), all[..6]);
    let prefix = left.follow_ups().last().unwrap();
    assert_eq!(left.plan().row(&prefix), [window(20)]);

    assert_eq!((cut.segment.len(), cut.local_index), (4, 2));
    assert_eq!(cut.rest.plan().row(&cut.segment), [window(20)]);
    assert_eq!(suffix(&cut), all[6..8]);
    assert_eq!(lens(&cut.rest), [2]);
    assert_eq!(cut.rest.remaining_len(), 2);
}

/// Asserts that a cut at a segment boundary leaves whole segments on both sides,
/// the rest starting with the segment after the cut.
#[test]
fn split_at_boundary() {
    let all = entities(10);
    let mut left = four_segments(&all);

    let cut = left.split_follow_ups_at(4);

    assert_eq!(lens(&left), [3, 1]);
    assert_eq!(left.remaining_len(), 4);
    assert_eq!((cut.segment.len(), cut.local_index), (4, 0));
    assert_eq!(suffix(&cut), all[4..8]);
    assert_eq!(lens(&cut.rest), [2]);
}

/// Asserts that a cut before the first candidate leaves no follow-up but keeps the plan,
/// everything goes to the rest.
#[test]
fn split_at_start() {
    let all = entities(10);
    let mut left = four_segments(&all);
    let weak = Arc::downgrade(&left.plan);

    let cut = left.split_follow_ups_at(0);

    assert!(!left.has_follow_ups());
    assert_eq!(left.remaining_len(), 0);
    assert!(left.take_next().is_none());
    assert_eq!((cut.segment.len(), cut.local_index), (3, 0));
    assert_eq!(lens(&cut.rest), [1, 4, 2]);

    drop(cut);
    assert_eq!(weak.strong_count(), 1);
}

/// Asserts that a cut inside an already cut last segment splits the prefix we own,
/// not the plan's whole segment, and leaves the earlier rest untouched.
#[test]
fn split_at_prefix() {
    let all = entities(10);
    let mut left = four_segments(&all);
    let first_cut = left.split_follow_ups_at(6);

    let cut = left.split_follow_ups_at(5);

    assert_eq!(lens(&left), [3, 1, 1]);
    assert_eq!(left.remaining_len(), 5);
    assert_eq!((cut.segment.len(), cut.local_index), (2, 1));
    assert_eq!(suffix(&cut), all[5..6]);
    assert!(!cut.rest.has_follow_ups());
    assert_eq!(cut.rest.remaining_len(), 0);
    assert_eq!(suffix(&first_cut), all[6..8]);
    assert_eq!(lens(&first_cut.rest), [2]);
}

/// Asserts that every cut, before or after taking a follow-up, shares the candidates without
/// losing or repeating one: our follow-ups, the cut segment's suffix and the rest's follow-ups
/// give back the original stream, and the counts add up.
#[test]
fn split_at_every_count() {
    let all = entities(10);

    for taken in 0..2 {
        let mut original = four_segments(&all);
        for _ in 0..taken {
            original.take_next();
        }
        let total = original.remaining_len();

        for count in 0..total {
            let mut left = original.clone();
            let before = stream(&left);

            let cut = left.split_follow_ups_at(count);

            let mut after = stream(&left);
            after.extend(suffix(&cut));
            after.extend(stream(&cut.rest));
            assert_eq!(after, before, "{taken} {count}");

            assert_eq!(left.remaining_len(), count, "{taken} {count}");
            assert_eq!(lens(&left).iter().sum::<usize>(), count, "{taken} {count}");
            let rest_len: usize = lens(&cut.rest).iter().sum();
            assert_eq!(cut.rest.remaining_len(), rest_len, "{taken} {count}");
            assert_eq!(
                cut.segment.len() - cut.local_index + rest_len,
                total - count,
                "{taken} {count}"
            );
        }
    }
}

/// Asserts that a cut has to leave a candidate to the rest.
#[test]
#[should_panic(expected = "the rest of the follow-ups keeps at least one candidate")]
fn split_at_end() {
    let all = entities(10);
    let mut schedule = four_segments(&all);

    schedule.split_follow_ups_at(10);
}

/// Asserts that advancing the clone of a cut schedule doesn't move the original, and that both
/// stop at the prefix we own and stay exhausted afterwards.
#[test]
fn split_at_clone() {
    let all = entities(10);
    let mut left = four_segments(&all);
    let _cut = left.split_follow_ups_at(6);

    let mut clone = left.clone();
    let taken: Vec<usize> = core::iter::from_fn(|| clone.take_next())
        .map(|segment| segment.len())
        .collect();

    assert_eq!(taken, [3, 1, 2]);
    assert!(ptr::eq(left.plan(), clone.plan()));
    for _ in 0..2 {
        assert!(clone.take_next().is_none());
        assert_eq!(clone.remaining_len(), 0);
    }
    assert_eq!(lens(&left), [3, 1, 2]);
    assert_eq!(left.remaining_len(), 6);
}

/// Asserts that the plan is released once the last of the cut schedules is dropped,
/// whichever is dropped first.
#[test]
fn split_at_release() {
    let all = entities(10);

    for drop_left_first in [true, false] {
        let mut left = four_segments(&all);
        let weak = Arc::downgrade(&left.plan);
        let FollowUpCut { rest, .. } = left.split_follow_ups_at(6);
        assert_eq!(weak.strong_count(), 2);

        if drop_left_first {
            drop(left);
            assert_eq!(stream(&rest), all[8..]);
            drop(rest);
        } else {
            drop(rest);
            assert_eq!(stream(&left), all[..6]);
            drop(left);
        }
        assert!(weak.upgrade().is_none());
    }
}

/// Asserts that embedding a cut schedule in a parent plan copies the parts it owns,
/// ending with its prefix, each with its own row.
#[test]
fn split_at_embed() {
    let all = entities(10);
    let mut child = four_segments(&all);
    let _cut = child.split_follow_ups_at(6);

    let mut builder = PlanBuilder::new(&[NodeKind::Window]);
    builder.push_child_follow_ups(&child, &[window(0)], 0);
    let parent = builder.finish().unwrap();

    assert_eq!(lens(&parent), [3, 1, 2]);
    assert_eq!(stream(&parent), all[..6]);
    let rows: Vec<NodeState> = parent
        .follow_ups()
        .map(|segment| parent.plan().row(&segment)[0])
        .collect();
    assert_eq!(rows, [window(0), window(10), window(20)]);
}
