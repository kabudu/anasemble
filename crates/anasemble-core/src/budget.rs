//! Hard resource limits. Exhaustion is a refusal, not success.

use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Caller-supplied ceilings for a unit of protocol work.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    max_nodes: u32,
    max_edges: u32,
    max_depth: u32,
    max_bytes: u64,
    deadline_ms: u32,
}

impl Budget {
    /// Construct a budget. Every field must be strictly positive.
    pub fn new(
        max_nodes: u32,
        max_edges: u32,
        max_depth: u32,
        max_bytes: u64,
        deadline_ms: u32,
    ) -> Result<Self, CoreError> {
        if max_nodes == 0 || max_edges == 0 || max_depth == 0 || max_bytes == 0 || deadline_ms == 0
        {
            return Err(CoreError::Bound);
        }
        Ok(Self {
            max_nodes,
            max_edges,
            max_depth,
            max_bytes,
            deadline_ms,
        })
    }

    /// Default bounded protocol budget.
    #[must_use]
    pub fn standard() -> Self {
        Self::new(10_000, 20_000, 16, 8 * 1024 * 1024, 5_000).expect("standard budget is non-zero")
    }

    /// Node ceiling.
    #[must_use]
    pub fn max_nodes(self) -> u32 {
        self.max_nodes
    }

    /// Edge ceiling.
    #[must_use]
    pub fn max_edges(self) -> u32 {
        self.max_edges
    }

    /// Traversal depth ceiling.
    #[must_use]
    pub fn max_depth(self) -> u32 {
        self.max_depth
    }

    /// Working-set byte ceiling.
    #[must_use]
    pub fn max_bytes(self) -> u64 {
        self.max_bytes
    }

    /// Deadline in milliseconds.
    #[must_use]
    pub fn deadline_ms(self) -> u32 {
        self.deadline_ms
    }

    /// True when a unit of work would exceed a named ceiling.
    #[must_use]
    pub fn would_exceed(self, nodes: u32, edges: u32, depth: u32, bytes: u64) -> bool {
        nodes > self.max_nodes
            || edges > self.max_edges
            || depth > self.max_depth
            || bytes > self.max_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_limit_is_refused() {
        assert!(Budget::new(0, 1, 1, 1, 1).is_err());
    }

    #[test]
    fn exhaustion_is_explicit() {
        let budget = Budget::new(2, 2, 1, 8, 10).unwrap();
        assert!(budget.would_exceed(3, 1, 1, 1));
        assert!(!budget.would_exceed(2, 2, 1, 8));
    }
}
