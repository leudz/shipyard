//! Query-local upper bounds from the existing 64-slot tracking timestamps.
//! A candidate slot still needs its normal per-component timestamp/join checks.

use super::TRACKING_CHUNK_SIZE;
use alloc::{boxed::Box, sync::Arc, vec::Vec};

const MIN_DENSE_FALLBACK_CHUNKS: usize = 32;

#[derive(Clone)]
pub(crate) enum TrackingPlan {
    Empty,
    Dense { len: usize },
    Sparse(Arc<SparsePlan>),
}

pub(crate) struct SparsePlan {
    ranges: Box<[CandidateRange]>,
    chunk_bits: Box<[usize]>,
    len: usize,
    slots: usize,
    chunks: usize,
}

struct CandidateRange {
    start: usize,
    end: usize,
    /// Number of candidate slots in all preceding ranges.
    prefix: usize,
}

impl TrackingPlan {
    #[cfg(test)]
    pub(crate) fn build(len: usize, candidate: impl FnMut(usize) -> bool) -> Self {
        Self::build_with_budget(len, usize::MAX, candidate)
    }

    /// `candidate` must return true for missing/uncertain chunk metadata.
    pub(crate) fn build_with_budget(
        len: usize,
        max_chunks: usize,
        mut candidate: impl FnMut(usize) -> bool,
    ) -> Self {
        let chunk_count = len.div_ceil(TRACKING_CHUNK_SIZE);
        if chunk_count == 0 {
            return Self::Empty;
        }
        // This guard precedes all metadata reads and bitmap/range allocations.
        // An uninspected input stays on the exact dense traversal path.
        if chunk_count > max_chunks {
            return Self::Dense { len };
        }
        if chunk_count == 1 {
            return if candidate(0) {
                Self::Dense { len }
            } else {
                Self::Empty
            };
        }
        let bits = usize::BITS as usize;
        let dense_threshold = if chunk_count >= MIN_DENSE_FALLBACK_CHUNKS {
            chunk_count.div_ceil(2)
        } else {
            chunk_count
        };
        // Dense prefixes need neither a bitmap nor run bookkeeping.
        let mut leading = 0;
        while leading < dense_threshold && candidate(leading) {
            leading += 1;
        }
        if leading == dense_threshold {
            return Self::Dense { len };
        }
        let mut chunk_bits = Vec::new();
        let mut chunks = leading;
        let mut runs = usize::from(leading != 0);
        let mut previous_candidate = false;
        if leading != 0 {
            chunk_bits.resize(chunk_count.div_ceil(bits), 0usize);
            for chunk in 0..leading {
                chunk_bits[chunk / bits] |= 1usize << (chunk % bits);
            }
        }
        // The prefix scan already checked the first non-candidate chunk.
        for chunk in leading + 1..chunk_count {
            let is_candidate = candidate(chunk);
            if is_candidate {
                chunks += 1;
                // The remaining chunks cannot change this fallback decision.
                // Dense iteration still performs every exact tracking check.
                if chunks >= dense_threshold {
                    return Self::Dense { len };
                }
                if chunk_bits.is_empty() {
                    chunk_bits.resize(chunk_count.div_ceil(bits), 0usize);
                }
                chunk_bits[chunk / bits] |= 1usize << (chunk % bits);
                runs += usize::from(!previous_candidate);
            }
            previous_candidate = is_candidate;
        }
        if chunks == 0 {
            return Self::Empty;
        }
        let mut ranges = Vec::with_capacity(runs);
        let mut run_start = None;
        let mut run_end = 0;
        let mut slots = 0;
        // Build ranges from the captured bitmap, without re-reading timestamps.
        for (word_index, &word) in chunk_bits.iter().enumerate() {
            let mut remaining = word;
            while remaining != 0 {
                let chunk = word_index * bits + remaining.trailing_zeros() as usize;
                remaining &= remaining - 1;
                let start = chunk * TRACKING_CHUNK_SIZE;
                if start != run_end
                    && let Some(start_of_run) = run_start.take()
                {
                    ranges.push(CandidateRange {
                        start: start_of_run,
                        end: run_end,
                        prefix: slots,
                    });
                    slots += run_end - start_of_run;
                }
                run_start.get_or_insert(start);
                run_end = (start + TRACKING_CHUNK_SIZE).min(len);
            }
        }
        if let Some(start) = run_start {
            ranges.push(CandidateRange {
                start,
                end: run_end,
                prefix: slots,
            });
            slots += run_end - start;
        }
        Self::Sparse(Arc::new(SparsePlan {
            ranges: ranges.into_boxed_slice(),
            chunk_bits: chunk_bits.into_boxed_slice(),
            len,
            slots,
            chunks,
        }))
    }

    pub(crate) fn next_range(&self, start: usize, end: usize) -> core::ops::Range<usize> {
        let start = self.next_in(start, end);
        if start == end {
            return end..end;
        }
        match self {
            Self::Sparse(plan) => {
                let pos = plan.ranges.partition_point(|range| range.end <= start);
                start..plan.ranges[pos].end.min(end)
            }
            _ => start..end,
        }
    }

    pub(crate) fn split_at(&self, start: usize, end: usize, count: usize) -> usize {
        if count == 0 {
            return start;
        }
        if count == self.count(start, end) {
            return end;
        }
        let rank = self.rank(start) + count;
        match self {
            Self::Sparse(plan) => {
                let pos = plan
                    .ranges
                    .partition_point(|range| range.prefix + range.end - range.start <= rank);
                let range = &plan.ranges[pos];
                range.start + rank - range.prefix
            }
            _ => start + count,
        }
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Relative scan work, not an exact item count or elapsed-time estimate.
    #[inline]
    pub(crate) fn scan_cost(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Dense { len } => len.saturating_add(len.div_ceil(TRACKING_CHUNK_SIZE)),
            Self::Sparse(plan) => plan.slots.saturating_add(plan.chunks),
        }
    }

    #[inline]
    pub(crate) fn next_in(&self, index: usize, end: usize) -> usize {
        if index >= end {
            return end;
        }
        match self {
            Self::Empty => end,
            Self::Dense { .. } => index,
            Self::Sparse(plan) => {
                let end = end.min(plan.len);
                if index >= end {
                    return end;
                }
                let bits = usize::BITS as usize;
                let chunk = index / TRACKING_CHUNK_SIZE;
                let mut word = chunk / bits;
                let last_word = ((end - 1) / TRACKING_CHUNK_SIZE) / bits;
                let mut candidates = plan.chunk_bits[word] & (usize::MAX << (chunk % bits));
                while candidates == 0 {
                    if word == last_word {
                        return end;
                    }
                    word += 1;
                    candidates = plan.chunk_bits[word];
                }
                let next_chunk = word * bits + candidates.trailing_zeros() as usize;
                index.max(next_chunk * TRACKING_CHUNK_SIZE).min(end)
            }
        }
    }

    #[inline]
    fn rank(&self, index: usize) -> usize {
        match self {
            Self::Empty => 0,
            Self::Dense { len, .. } => index.min(*len),
            Self::Sparse(plan) => {
                let pos = plan.ranges.partition_point(|range| range.end <= index);
                plan.ranges.get(pos).map_or(plan.slots, |range| {
                    range.prefix + index.saturating_sub(range.start)
                })
            }
        }
    }

    #[inline]
    pub(crate) fn count(&self, start: usize, end: usize) -> usize {
        self.rank(end) - self.rank(start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_and_split_match_candidate_slots_for_every_subrange() {
        let len = 257;
        for mask in 0..32 {
            let plan = TrackingPlan::build(len, |chunk| mask & (1 << chunk) != 0);
            let candidate: Vec<_> = (0..len).filter(|i| mask & (1 << (i / 64)) != 0).collect();
            let mut actual = Vec::new();
            let mut index = 0;
            while index < len {
                let range = plan.next_range(index, len);
                assert!(index <= range.start && range.start <= range.end && range.end <= len);
                assert!(!range.is_empty() || range.end == len);
                actual.extend(range.clone());
                index = range.end;
            }
            assert_eq!(actual, candidate);
            for start in [0, 1, 63, 64, 65, 127, 128, 192, 256] {
                for end in [64, 127, 128, 191, 192, 256, 257] {
                    if end <= start {
                        continue;
                    }
                    let expected = candidate.iter().filter(|&&i| start <= i && i < end).count();
                    assert_eq!(plan.count(start, end), expected);
                    if expected > 1 {
                        let split = plan.split_at(start, end, expected / 2);
                        assert!(start < split && split < end);
                        assert_eq!(plan.count(start, split), expected / 2);
                        assert_eq!(plan.count(split, end), expected - expected / 2);
                    }
                }
            }
        }
    }

    #[test]
    fn dense_and_empty_plans_do_not_allocate_ranges() {
        assert!(matches!(
            TrackingPlan::build(0, |_| true),
            TrackingPlan::Empty
        ));
        assert!(matches!(
            TrackingPlan::build(4096, |_| false),
            TrackingPlan::Empty
        ));
        assert!(matches!(
            TrackingPlan::build(4097, |_| true),
            TrackingPlan::Dense { .. }
        ));
    }

    #[test]
    fn dense_threshold_stops_metadata_reads_without_changing_the_policy() {
        for chunk_count in [2usize, 31, 32, 33, 782] {
            let threshold = if chunk_count < 32 {
                chunk_count
            } else {
                chunk_count.div_ceil(2)
            };
            let mut reads = 0;
            let plan = TrackingPlan::build(chunk_count * 64 - 1, |_| {
                reads += 1;
                true
            });
            assert!(matches!(plan, TrackingPlan::Dense { .. }));
            assert_eq!(reads, threshold);

            let mut reads = 0;
            let plan = TrackingPlan::build(chunk_count * 64 - 1, |chunk| {
                reads += 1;
                chunk < threshold - 1
            });
            assert_eq!(reads, chunk_count);
            assert!(!matches!(plan, TrackingPlan::Dense { .. }));
        }
    }

    #[test]
    fn bitmap_word_boundaries_keep_counts_and_split_coverage() {
        let word_slots = usize::BITS as usize * 64;
        let len = word_slots * 3 + 7;
        let chunks = [
            0,
            usize::BITS as usize - 1,
            usize::BITS as usize,
            usize::BITS as usize * 3,
        ];
        let plan = TrackingPlan::build(len, |chunk| chunks.contains(&chunk));
        let candidate: Vec<_> = (0..len).filter(|i| chunks.contains(&(i / 64))).collect();
        for end in [
            0,
            1,
            63,
            64,
            word_slots - 1,
            word_slots,
            word_slots + 1,
            len - 1,
            len,
        ] {
            assert_eq!(
                plan.count(0, end),
                candidate.iter().filter(|&&i| i < end).count()
            );
        }
        let split = plan.split_at(0, len, candidate.len() / 2);
        assert_eq!(plan.count(0, split), candidate.len() / 2);
    }

    #[test]
    fn bounded_sparse_scan_keeps_partial_chunks_and_bitmap_word_boundaries() {
        let word_slots = usize::BITS as usize * TRACKING_CHUNK_SIZE;
        let len = word_slots * 3 + 7;
        let chunks = [0, usize::BITS as usize, usize::BITS as usize * 3];
        let plan = TrackingPlan::build(len, |chunk| chunks.contains(&chunk));
        assert!(matches!(plan, TrackingPlan::Sparse(_)));
        let boundaries = [
            0,
            1,
            63,
            64,
            word_slots - 1,
            word_slots,
            word_slots + 1,
            word_slots * 2,
            len - 1,
            len,
        ];
        for start in boundaries {
            for end in boundaries.into_iter().filter(|&end| end >= start) {
                let expected = (start..end)
                    .find(|i| chunks.contains(&(i / TRACKING_CHUNK_SIZE)))
                    .unwrap_or(end);
                assert_eq!(plan.next_in(start, end), expected, "{start}..{end}");
            }
        }
        assert_eq!(TrackingPlan::Empty.next_in(0, 64), 64);
        assert_eq!(TrackingPlan::Dense { len }.next_in(64, 64), 64);
    }

    #[test]
    fn planning_budget_skips_metadata_and_never_proves_uninspected_inputs_empty() {
        let len = 50_000;
        for max_chunks in [0, 1, 32, 512, 781] {
            let plan = TrackingPlan::build_with_budget(len, max_chunks, |_| {
                panic!("over-budget construction must not read any chunk");
            });
            assert!(matches!(plan, TrackingPlan::Dense { len: 50_000 }));
            assert!(!plan.is_empty());
            assert_eq!(plan.count(0, len), len);
        }
        let mut reads = 0;
        let plan = TrackingPlan::build_with_budget(len, 782, |_| {
            reads += 1;
            false
        });
        assert!(plan.is_empty());
        assert_eq!(reads, 782);

        let mut reads = 0;
        let plan = TrackingPlan::build_with_budget(len, 782, |chunk| {
            reads += 1;
            chunk == 781
        });
        assert_eq!(reads, 782);
        assert_eq!(plan.next_in(0, len), 49_984);
        assert_eq!(plan.count(0, len), 16);
        assert!(TrackingPlan::build_with_budget(0, 0, |_| unreachable!()).is_empty());
    }
}
