use std::fmt;

use serde::{Deserialize, Serialize};

/// A `between` range: the stretch of a segment a rule applies to, as two linear
/// references (geodesic fractions `0..=1` of the segment's length).
///
/// Overture's JSON Schema bounds each end to `0..=1` but cannot enforce `start < end`.
/// An inverted range is kept as written; [`Between::bounds`] orders it, and the
/// evaluator says so in its explanation. See `docs/semantics.md`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "[f64; 2]", into = "[f64; 2]")]
pub struct Between {
    /// The first linear reference, as written.
    pub start: f64,
    /// The second linear reference, as written.
    pub end: f64,
}

impl Between {
    /// A range from `start` to `end`, as written.
    pub fn new(start: f64, end: f64) -> Self {
        Self { start, end }
    }

    /// `(low, high)`, whichever order the range was written in.
    pub fn bounds(&self) -> (f64, f64) {
        (self.start.min(self.end), self.start.max(self.end))
    }

    /// Whether the range was written high-to-low.
    pub fn is_inverted(&self) -> bool {
        self.start > self.end
    }

    /// Whether `fraction` falls inside the range, ends included.
    ///
    /// A 1e-9 tolerance absorbs the rounding of fractions that were computed
    /// rather than read from the data.
    pub fn contains(&self, fraction: f64) -> bool {
        const EPS: f64 = 1e-9;
        let (lo, hi) = self.bounds();
        fraction >= lo - EPS && fraction <= hi + EPS
    }
}

impl From<[f64; 2]> for Between {
    fn from([start, end]: [f64; 2]) -> Self {
        Self { start, end }
    }
}

impl From<Between> for [f64; 2] {
    fn from(b: Between) -> Self {
        [b.start, b.end]
    }
}

impl fmt::Display for Between {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:.3}, {:.3}]", self.start, self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_is_inclusive_and_order_independent() {
        let b = Between::new(0.2, 0.6);
        assert!(b.contains(0.2));
        assert!(b.contains(0.6));
        assert!(!b.contains(0.61));
        let inverted = Between::new(0.6, 0.2);
        assert!(inverted.is_inverted());
        assert!(inverted.contains(0.4));
    }

    #[test]
    fn serialises_as_a_pair() {
        let b: Between = serde_json::from_str("[0.25, 0.5]").unwrap();
        assert_eq!(b, Between::new(0.25, 0.5));
        assert_eq!(serde_json::to_string(&b).unwrap(), "[0.25,0.5]");
        assert!(serde_json::from_str::<Between>("[0.25]").is_err());
    }
}
