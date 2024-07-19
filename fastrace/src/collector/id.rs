// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::Cell;

/// An identifier for a trace, which groups a set of related spans together.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct TraceId(pub u128);

/// An identifier for a span within a trace.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct SpanId(pub u64);

impl SpanId {
    #[inline]
    /// Create a non-zero `SpanId`
    pub(crate) fn next_id() -> SpanId {
        LOCAL_ID_GENERATOR
            .try_with(|g| {
                let (prefix, mut suffix) = g.get();

                suffix = suffix.wrapping_add(1);

                g.set((prefix, suffix));

                SpanId(((prefix as u64) << 32) | (suffix as u64))
            })
            .unwrap_or_else(|_| SpanId(rand::random()))
    }
}

thread_local! {
    static LOCAL_ID_GENERATOR: Cell<(u32, u32)> = Cell::new((rand::random(), 0))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    #[allow(clippy::needless_collect)]
    fn unique_id() {
        let handles = std::iter::repeat_with(|| {
            std::thread::spawn(|| {
                std::iter::repeat_with(SpanId::next_id)
                    .take(1000)
                    .collect::<Vec<_>>()
            })
        })
        .take(32)
        .collect::<Vec<_>>();

        let k = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect::<HashSet<_>>();

        assert_eq!(k.len(), 32 * 1000);
    }
}
