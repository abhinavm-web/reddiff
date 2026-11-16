use super::{CandidateTest, Synthesizer};
use crate::diff::DiffSummary;
use crate::error::ReddiffError;

/// Deterministic mock synthesizer for offline tests and testing the decision engine.
#[derive(Debug, Clone, Default)]
pub struct MockSynthesizer {
    candidates: Vec<CandidateTest>,
}

impl MockSynthesizer {
    pub fn new(candidates: Vec<CandidateTest>) -> Self {
        Self { candidates }
    }

    pub fn single(candidate: CandidateTest) -> Self {
        Self {
            candidates: vec![candidate],
        }
    }

    pub fn empty() -> Self {
        Self {
            candidates: Vec::new(),
        }
    }
}

impl Synthesizer for MockSynthesizer {
    fn synthesize(&self, _diff: &DiffSummary) -> Result<Vec<CandidateTest>, ReddiffError> {
        Ok(self.candidates.clone())
    }
}
