use crate::tracking::TrackingTimestamp;
use alloc::vec::Vec;
use core::ops::Range;
use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(test)]
mod tests;

pub(crate) const TRACKING_CHUNK_SIZE: usize = 64;

/// An upper bound: stale entries can cause extra checks, never missed components.
#[derive(Default)]
pub(crate) struct ChunkTimestamp(AtomicU64);

impl ChunkTimestamp {
    #[inline]
    pub(crate) fn mark_mut(&mut self, timestamp: TrackingTimestamp) {
        let value = self.0.get_mut();
        *value = (*value).max(timestamp.get());
    }

    #[inline]
    pub(crate) fn mark(&self, timestamp: TrackingTimestamp) {
        let timestamp = timestamp.get();
        if self.0.load(Ordering::Relaxed) < timestamp {
            // Concurrent writers share one exclusive storage borrow and timestamp.
            self.0.store(timestamp, Ordering::Relaxed);
        }
    }

    #[cfg(test)]
    pub(crate) fn changed_since(&self, last: TrackingTimestamp) -> bool {
        last.get() < self.0.load(Ordering::Relaxed)
    }
}

/// Updates summaries after timestamps are added or rows move between chunks.
pub(super) fn refresh_chunks(
    chunks: &mut Vec<ChunkTimestamp>,
    timestamps: &[TrackingTimestamp],
    range: Range<usize>,
) {
    chunks.resize_with(
        timestamps.len().div_ceil(TRACKING_CHUNK_SIZE),
        ChunkTimestamp::default,
    );
    if range.is_empty() || timestamps.is_empty() {
        return;
    }
    let end = range.end.min(timestamps.len());
    for (chunk, summary) in chunks
        .iter_mut()
        .enumerate()
        .take(end.div_ceil(TRACKING_CHUNK_SIZE))
        .skip(range.start / TRACKING_CHUNK_SIZE)
    {
        let start = range.start.max(chunk * TRACKING_CHUNK_SIZE);
        let end = end.min((chunk + 1) * TRACKING_CHUNK_SIZE);
        let max = timestamps[start..end]
            .iter()
            .map(|t| t.get())
            .max()
            .unwrap_or(0);
        summary.mark_mut(TrackingTimestamp::new(max));
    }
}

/// Modification bounds; raw chunk values are only exposed as write handles.
#[derive(Default)]
pub(crate) struct ModificationSummary {
    chunks: Vec<ChunkTimestamp>,
    whole: ChunkTimestamp,
}

impl ModificationSummary {
    pub(crate) fn len(&self) -> usize {
        self.chunks.len()
    }
    pub(crate) fn capacity(&self) -> usize {
        self.chunks.capacity()
    }

    pub(super) fn clear(&mut self) {
        self.chunks.clear();
        self.whole = ChunkTimestamp::default();
    }

    /// Opaque marking handles; candidate reads go through this summary.
    #[inline]
    pub(crate) fn writers(&self) -> &[ChunkTimestamp] {
        &self.chunks
    }

    /// A handle for per-item marking, not a candidate test.
    #[inline]
    pub(crate) fn get(&self, index: usize) -> Option<&ChunkTimestamp> {
        self.chunks.get(index)
    }

    fn materialize_whole(&mut self) {
        let whole = *self.whole.0.get_mut();
        if whole != 0 {
            for chunk in &mut self.chunks {
                chunk.mark_mut(TrackingTimestamp::new(whole));
            }
            *self.whole.0.get_mut() = 0;
        }
    }

    pub(crate) fn push(&mut self, chunk: ChunkTimestamp) {
        // Keep the previous whole-storage mark from making a new chunk a candidate.
        self.materialize_whole();
        self.chunks.push(chunk);
    }

    pub(crate) fn refresh(&mut self, timestamps: &[TrackingTimestamp], range: Range<usize>) {
        if timestamps.len().div_ceil(TRACKING_CHUNK_SIZE) > self.chunks.len() {
            self.materialize_whole();
        }
        refresh_chunks(&mut self.chunks, timestamps, range);
    }

    #[inline(always)]
    pub(crate) fn prepare_range(
        &self,
        range: Range<usize>,
        len: usize,
        current: TrackingTimestamp,
    ) {
        if range.is_empty() {
            return;
        }
        if range.start == 0 && range.end == len {
            self.whole.mark(current);
        } else {
            for chunk in &self.chunks
                [range.start / TRACKING_CHUNK_SIZE..=(range.end - 1) / TRACKING_CHUNK_SIZE]
            {
                chunk.mark(current);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn all_changed(&self, last: TrackingTimestamp) -> bool {
        self.whole.0.load(Ordering::Relaxed) > last.get()
    }

    #[cfg(test)]
    pub(crate) fn changed(&self, index: usize, last: TrackingTimestamp) -> bool {
        self.all_changed(last) || self.chunks[index].0.load(Ordering::Relaxed) > last.get()
    }

    #[cfg(test)]
    fn upper(&self, chunk: &ChunkTimestamp) -> u64 {
        self.whole
            .0
            .load(Ordering::Relaxed)
            .max(chunk.0.load(Ordering::Relaxed))
    }
}

// Indexing exposes marking operations; timestamp reads stay inside this module.
impl core::ops::Index<usize> for ModificationSummary {
    type Output = ChunkTimestamp;
    fn index(&self, index: usize) -> &Self::Output {
        &self.chunks[index]
    }
}
impl core::ops::IndexMut<usize> for ModificationSummary {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.chunks[index]
    }
}
