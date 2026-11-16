#![deny(clippy::all)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

pub mod cli;
pub mod diff;
pub mod error;
pub mod sandbox;
pub mod synth;
pub mod verifier;

use std::path::{Path, PathBuf};
use std::time::Duration;
use cli::{RunOptions, SynthesizerType};
use diff::{parse_unified_diff, GitClient};
use error::ReddiffError;
use sandbox::WorktreeGuard;
use synth::{HeuristicSynthesizer, LlmSynthesizer, MockSynthesizer, Synthesizer};
use verifier::DifferentialVerifier;

/// Runs the reddiff differential verification engine on the specified repository.
///
/// Returns:
/// - `Ok(Some(PathBuf))` if a differential regression is proven (passes on base, fails on target).
/// - `Ok(None)` if no regression is proven or the diff is clean.
/// - `Err(ReddiffError)` if an operational failure occurs.
pub fn run(opts: &RunOptions, repo_dir: &Path) -> Result<Option<PathBuf>, ReddiffError> {
    if !GitClient::is_inside_work_tree(repo_dir) {
        return Err(ReddiffError::Git(
            "Current directory is not a git repository".to_string(),
        ));
    }

    if !GitClient::has_commits(repo_dir) {
        return Err(ReddiffError::Git(
            "Git repository has no commits".to_string(),
        ));
    }

    let repo_root = GitClient::toplevel(repo_dir)?;
    let base_sha = GitClient::resolve_base_ref(&repo_root, opts.base.as_deref())?;

    let target_opt = if opts.target == "working-tree" {
        None
    } else {
        Some(opts.target.as_str())
    };

    let raw_diff = GitClient::diff(&repo_root, &base_sha, target_opt)?;
    let diff_summary = parse_unified_diff(&raw_diff)?;

    if diff_summary.is_empty() {
        return Ok(None);
    }

    match opts.synthesizer {
        SynthesizerType::Auto => {
            let llm = LlmSynthesizer::new();
            let heuristic = HeuristicSynthesizer::new();
            let mut combined = Vec::new();
            if llm.has_credentials() {
                if let Ok(cands) = llm.synthesize(&diff_summary) {
                    combined.extend(cands);
                }
            }
            if let Ok(cands) = heuristic.synthesize(&diff_summary) {
                combined.extend(cands);
            }
            let merged = MockSynthesizer::new(combined);
            run_with_synthesizer(opts, &repo_root, &base_sha, &diff_summary, &merged)
        }
        SynthesizerType::Heuristic => {
            let synth = HeuristicSynthesizer::new();
            run_with_synthesizer(opts, &repo_root, &base_sha, &diff_summary, &synth)
        }
        SynthesizerType::Mock => {
            let synth = MockSynthesizer::empty();
            run_with_synthesizer(opts, &repo_root, &base_sha, &diff_summary, &synth)
        }
        SynthesizerType::Llm => {
            let synth = LlmSynthesizer::new();
            run_with_synthesizer(opts, &repo_root, &base_sha, &diff_summary, &synth)
        }
    }
}

/// Executes differential verification using a provided synthesizer instance.
pub fn run_with_synthesizer(
    opts: &RunOptions,
    repo_root: &Path,
    base_sha: &str,
    diff_summary: &diff::DiffSummary,
    synthesizer: &dyn Synthesizer,
) -> Result<Option<PathBuf>, ReddiffError> {
    let candidates = synthesizer.synthesize(diff_summary)?;
    if candidates.is_empty() {
        return Ok(None);
    }

    let worktree = WorktreeGuard::create(repo_root, base_sha)?;

    let verifier = DifferentialVerifier::new(
        worktree.path().to_path_buf(),
        repo_root.to_path_buf(),
        opts.runner.clone(),
        Duration::from_secs(opts.timeout),
        opts.out.clone(),
    );

    let verify_result = verifier.verify_candidates(&candidates);

    // Clean up temporary worktree
    let cleanup_result = worktree.clean_up();

    // Propagate verification result or cleanup error
    let reproducer_path = verify_result?;
    cleanup_result?;

    Ok(reproducer_path)
}
