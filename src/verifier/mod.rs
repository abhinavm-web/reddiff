pub mod shrinker;

use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, info};
use crate::error::ReddiffError;
use crate::sandbox::ProcessRunner;
use crate::synth::CandidateTest;
pub use shrinker::Shrinker;

/// Differential verification engine evaluating candidates across base and target trees.
pub struct DifferentialVerifier {
    base_dir: PathBuf,
    target_dir: PathBuf,
    runner_override: Option<String>,
    timeout: Duration,
    out_path: Option<PathBuf>,
}

impl DifferentialVerifier {
    pub fn new(
        base_dir: PathBuf,
        target_dir: PathBuf,
        runner_override: Option<String>,
        timeout: Duration,
        out_path: Option<PathBuf>,
    ) -> Self {
        Self {
            base_dir,
            target_dir,
            runner_override,
            timeout,
            out_path,
        }
    }

    /// Evaluates candidate tests against the differential table:
    /// Base MUST succeed (exit 0) AND Target MUST fail (exit non-zero or timeout).
    /// Returns the PathBuf of the minimized reproducer if regression is proven.
    pub fn verify_candidates(
        &self,
        candidates: &[CandidateTest],
    ) -> Result<Option<PathBuf>, ReddiffError> {
        for candidate in candidates {
            debug!(name = %candidate.name, "Evaluating candidate test");

            let runner = self.resolve_runner(candidate)?;
            let timeout = candidate
                .timeout_secs
                .map(Duration::from_secs)
                .unwrap_or(self.timeout);

            // Step 1: Base Tree Verification (Must PASS)
            let temp_filename = format!("_rd_eval_{}.{}", std::process::id(), candidate.file_extension);
            let base_file = self.base_dir.join(&temp_filename);

            std::fs::write(&base_file, &candidate.script)?;
            let base_result = ProcessRunner::run_script(&runner, &base_file, &self.base_dir, timeout);
            let _ = std::fs::remove_file(&base_file);

            let base_output = base_result?;
            if !base_output.success() {
                debug!(
                    name = %candidate.name,
                    exit_code = ?base_output.exit_code,
                    timed_out = base_output.timed_out,
                    "Candidate failed on base tree; discarding"
                );
                continue;
            }

            // Step 2: Target Tree Verification (Must FAIL)
            let target_file = self.target_dir.join(&temp_filename);
            std::fs::write(&target_file, &candidate.script)?;
            let target_result = ProcessRunner::run_script(&runner, &target_file, &self.target_dir, timeout);
            let _ = std::fs::remove_file(&target_file);

            let target_output = target_result?;
            if target_output.success() {
                debug!(
                    name = %candidate.name,
                    "Candidate passed on target tree; no regression demonstrated; discarding"
                );
                continue;
            }

            // Step 3: Verified regression found! Shrink and write reproducer
            info!(
                name = %candidate.name,
                "Differential regression successfully proven"
            );

            let reproducer_path = Shrinker::shrink_and_write(
                candidate,
                &runner,
                &self.base_dir,
                &self.target_dir,
                timeout,
                self.out_path.as_deref(),
            )?;

            return Ok(Some(reproducer_path));
        }

        Ok(None)
    }

    fn resolve_runner(&self, candidate: &CandidateTest) -> Result<String, ReddiffError> {
        if let Some(ref r) = self.runner_override {
            return Ok(r.clone());
        }
        if let Some(ref r) = candidate.runner {
            return Ok(r.clone());
        }

        // Infer runner from file extension or repository
        match candidate.file_extension.as_str() {
            "py" => Ok("python".to_string()),
            "js" => Ok("node".to_string()),
            "ts" => Ok("node".to_string()),
            "sh" => Ok("bash".to_string()),
            "rs" => Ok("cargo test".to_string()),
            other => Err(ReddiffError::Runner(format!(
                "Cannot infer test runner for extension '{other}'. Specify --runner."
            ))),
        }
    }
}
