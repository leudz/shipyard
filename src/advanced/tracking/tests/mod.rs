use super::*;

#[test]
fn is_within_bounds() {
    let tests = [
        (5, 0, 10, true),
        (11, 0, 10, false),
        (1, 2, 0, false),
        // timestamp is equal to last
        (1, 1, 0, false),
        // timestamp is equal to now
        (1, 0, 1, true),
    ];

    for (timestamp, last, current, expected) in tests {
        assert_eq!(
            TrackingTimestamp::new(timestamp).is_within(
                TrackingTimestamp::new(last),
                TrackingTimestamp::new(current)
            ),
            expected,
            "t: {timestamp}, l: {last}, c: {current}"
        );
    }
}

#[test]
fn is_older() {
    let tests = [
        (0, 0, false),
        (5, 10, true),
        (11, 10, false),
        // timestamp is equal to other
        (1, 1, false),
    ];

    for (timestamp, other, expected) in tests {
        assert_eq!(
            TrackingTimestamp::new(timestamp).is_older_than(TrackingTimestamp::new(other)),
            expected,
            "t: {timestamp}, o: {other}"
        );
    }
}
