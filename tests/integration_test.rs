use assert_cmd::Command;
use predicates::prelude::*;
use reddiff::cli::RunOptions;
use reddiff::diff::GitClient;
use reddiff::synth::{CandidateTest, MockSynthesizer};
use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use tempfile::TempDir;

fn init_git_repo(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let out1 = StdCommand::new("git")
        .args(["init", "-b", "main"])
        .current_dir(dir)
        .output()?;
    if !out1.status.success() {
        StdCommand::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()?;
        StdCommand::new("git")
            .args(["branch", "-M", "main"])
            .current_dir(dir)
            .output()?;
    }

    StdCommand::new("git")
        .args(["config", "user.name", "Reddiff Test"])
        .current_dir(dir)
        .output()?;
    StdCommand::new("git")
        .args(["config", "user.email", "test@reddiff.local"])
        .current_dir(dir)
        .output()?;
    Ok(())
}

#[test]
fn test_regression_detected_and_zero_prose() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let repo_dir = temp.path();
    init_git_repo(repo_dir)?;

    // 1. Commit baseline calc.py
    let baseline_code = "def is_positive(x):\n    return x > 0\n";
    fs::write(repo_dir.join("calc.py"), baseline_code)?;

    StdCommand::new("git")
        .args(["add", "calc.py"])
        .current_dir(repo_dir)
        .output()?;
    StdCommand::new("git")
        .args(["commit", "-m", "initial baseline"])
        .current_dir(repo_dir)
        .output()?;

    // 2. Introduce boundary regression in working tree: x >= 0
    let regressed_code = "def is_positive(x):\n    return x >= 0\n";
    fs::write(repo_dir.join("calc.py"), regressed_code)?;

    // 3. Run reddiff CLI binary
    let mut cmd = Command::cargo_bin("reddiff")?;
    cmd.current_dir(repo_dir)
        .args(["--base", "main", "--synthesizer", "heuristic", "--runner", "python"]);

    let assert = cmd.assert().code(1);
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    // Verify zero prose review invariant:
    // No conversational words allowed on stdout
    let forbidden_words = [
        "review", "consider", "suggest", "explanation", "warning",
        "fixed", "issue", "hello", "here is", "please", "note",
    ];
    let lower_stdout = stdout.to_lowercase();
    for word in forbidden_words {
        assert!(
            !lower_stdout.contains(word),
            "Stdout contained conversational prose word: '{word}'"
        );
    }

    // Verify stdout contains strictly the reproducer path
    let trimmed_stdout = stdout.trim();
    assert!(
        trimmed_stdout == ".reddiff/reproducer.py" || trimmed_stdout == ".reddiff\\reproducer.py",
        "Expected reproducer path on stdout, got: '{trimmed_stdout}'"
    );

    // Verify reproducer exists on disk
    let reproducer_file = repo_dir.join(".reddiff").join("reproducer.py");
    assert!(
        reproducer_file.exists(),
        "Reproducer file was not written to disk"
    );

    // Verify that the generated reproducer genuinely reproduces the bug:
    // In target tree (modified), running python reproducer.py MUST fail (non-zero)
    let target_run = StdCommand::new("python")
        .arg(&reproducer_file)
        .current_dir(repo_dir)
        .output()?;
    assert!(
        !target_run.status.success(),
        "Reproducer was expected to fail on target tree but exited 0"
    );

    // Revert calc.py to baseline and verify reproducer passes (exit 0)
    fs::write(repo_dir.join("calc.py"), baseline_code)?;
    let base_run = StdCommand::new("python")
        .arg(&reproducer_file)
        .current_dir(repo_dir)
        .output()?;
    assert!(
        base_run.status.success(),
        "Reproducer was expected to pass on baseline tree but failed"
    );

    Ok(())
}

#[test]
fn test_clean_diff_silent_exit_zero() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let repo_dir = temp.path();
    init_git_repo(repo_dir)?;

    fs::write(repo_dir.join("file.txt"), "hello world\n")?;
    StdCommand::new("git")
        .args(["add", "file.txt"])
        .current_dir(repo_dir)
        .output()?;
    StdCommand::new("git")
        .args(["commit", "-m", "commit"])
        .current_dir(repo_dir)
        .output()?;

    // Run reddiff on clean tree
    let mut cmd = Command::cargo_bin("reddiff")?;
    cmd.current_dir(repo_dir).args(["--base", "main"]);

    // Must exit 0 and emit exactly 0 bytes on stdout
    cmd.assert()
        .code(0)
        .stdout(predicate::str::is_empty());

    Ok(())
}

#[test]
fn test_check_subcommand_behavior() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let repo_dir = temp.path();
    init_git_repo(repo_dir)?;

    fs::write(repo_dir.join("file.txt"), "hello\n")?;
    StdCommand::new("git")
        .args(["add", "file.txt"])
        .current_dir(repo_dir)
        .output()?;
    StdCommand::new("git")
        .args(["commit", "-m", "commit"])
        .current_dir(repo_dir)
        .output()?;

    // Run reddiff check subcommand on clean tree
    let mut cmd = Command::cargo_bin("reddiff")?;
    cmd.current_dir(repo_dir).args(["check", "--base", "main"]);

    cmd.assert()
        .code(0)
        .stdout(predicate::str::is_empty());

    Ok(())
}

#[test]
fn test_non_git_repo_handled() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let non_git_dir = temp.path();

    let mut cmd = Command::cargo_bin("reddiff")?;
    cmd.current_dir(non_git_dir);

    // Must exit 2 (operational error), emit error to stderr, zero bytes on stdout
    cmd.assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("not a git repository"));

    Ok(())
}

#[test]
fn test_mock_synthesizer_decision_matrix() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let repo_dir = temp.path();
    init_git_repo(repo_dir)?;

    // Commit baseline
    fs::write(repo_dir.join("test.txt"), "base\n")?;
    StdCommand::new("git")
        .args(["add", "test.txt"])
        .current_dir(repo_dir)
        .output()?;
    StdCommand::new("git")
        .args(["commit", "-m", "commit"])
        .current_dir(repo_dir)
        .output()?;

    // Dirty the tree so diff is not empty
    fs::write(repo_dir.join("test.txt"), "target\n")?;

    let base_sha = GitClient::resolve_ref(repo_dir, "HEAD")?;
    let raw_diff = GitClient::diff(repo_dir, &base_sha, None)?;
    let summary = reddiff::diff::parse_unified_diff(&raw_diff)?;

    let opts = RunOptions {
        repo: None,
        base: Some("main".to_string()),
        target: "working-tree".to_string(),
        runner: Some("python".to_string()),
        timeout: 10,
        out: None,
        synthesizer: reddiff::cli::SynthesizerType::Mock,
    };

    // Case 1: Candidate fails on base (script exits 1 unconditionally)
    // python -c "import sys; sys.exit(1)"
    let failing_base_cand = CandidateTest {
        name: "fail_on_base".to_string(),
        script: "import sys; sys.exit(1)\n".to_string(),
        file_extension: "py".to_string(),
        runner: Some("python".to_string()),
        timeout_secs: Some(5),
    };
    let synth1 = MockSynthesizer::single(failing_base_cand);
    let res1 = reddiff::run_with_synthesizer(&opts, repo_dir, &base_sha, &summary, &synth1)?;
    assert!(res1.is_none(), "Candidate failing on base should be discarded");

    // Case 2: Candidate passes on both base and target (script exits 0 unconditionally)
    let passing_both_cand = CandidateTest {
        name: "pass_on_both".to_string(),
        script: "import sys; sys.exit(0)\n".to_string(),
        file_extension: "py".to_string(),
        runner: Some("python".to_string()),
        timeout_secs: Some(5),
    };
    let synth2 = MockSynthesizer::single(passing_both_cand);
    let res2 = reddiff::run_with_synthesizer(&opts, repo_dir, &base_sha, &summary, &synth2)?;
    assert!(res2.is_none(), "Candidate passing on both should be discarded");

    // Case 3: Candidate passes on base and fails on target!
    // Script checks content of test.txt: if "base" in content -> exit 0, else -> exit 1
    let differential_cand = CandidateTest {
        name: "differential_proven".to_string(),
        script: "\
with open('test.txt', 'r') as f:
    content = f.read()
assert 'base' in content
"
        .to_string(),
        file_extension: "py".to_string(),
        runner: Some("python".to_string()),
        timeout_secs: Some(5),
    };
    let synth3 = MockSynthesizer::single(differential_cand);
    let res3 = reddiff::run_with_synthesizer(&opts, repo_dir, &base_sha, &summary, &synth3)?;
    assert!(res3.is_some(), "Differential regression must be accepted");

    Ok(())
}
