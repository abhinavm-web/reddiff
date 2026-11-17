#![deny(clippy::all)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

use reddiff::cli::{RunOptions, SynthesizerType};
use reddiff::diff::{parse_unified_diff, GitClient};
use reddiff::sandbox::WorktreeGuard;
use reddiff::synth::{CandidateTest, MockSynthesizer};
use reddiff::verifier::DifferentialVerifier;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

fn init_repo(dir: &Path, filename: &str, base_code: &str) -> Result<(), Box<dyn std::error::Error>> {
    let _ = Command::new("git").args(["init", "-b", "main"]).current_dir(dir).output()?;
    let _ = Command::new("git").args(["config", "user.name", "Bench"]).current_dir(dir).output()?;
    let _ = Command::new("git").args(["config", "user.email", "bench@example.com"]).current_dir(dir).output()?;
    fs::write(dir.join(filename), base_code)?;
    let _ = Command::new("git").args(["add", filename]).current_dir(dir).output()?;
    let _ = Command::new("git").args(["commit", "-m", "base"]).current_dir(dir).output()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== reddiff Accuracy, Flake-Rejection & Sandbox Benchmark ===");
    let start = Instant::now();

    let regressions = [
        (
            "boundary_off_by_one",
            "quota.py",
            "def allow(x):\n    return x >= 10\n",
            "def allow(x):\n    return x > 10\n",
        ),
        (
            "zero_division_guard_removed",
            "ratio.py",
            "def safe_div(a, b):\n    return a / (b if b != 0 else 1)\n",
            "def safe_div(a, b):\n    return a / b\n",
        ),
        (
            "boolean_default_flip",
            "auth.py",
            "def is_admin():\n    return False\n",
            "def is_admin():\n    return True\n",
        ),
        (
            "arithmetic_sign_flip",
            "ledger.py",
            "def credit(a, b):\n    return a + b\n",
            "def credit(a, b):\n    return a - b\n",
        ),
    ];

    let mut caught = 0usize;
    for (label, file, base, target) in regressions {
        let dir = tempfile::tempdir()?;
        init_repo(dir.path(), file, base)?;
        fs::write(dir.path().join(file), target)?;
        let opts = RunOptions {
            repo: None,
            base: Some("main".to_string()),
            target: "working-tree".to_string(),
            runner: Some("python".to_string()),
            timeout: 10,
            out: None,
            synthesizer: SynthesizerType::Heuristic,
        };
        let found = reddiff::run(&opts, dir.path())?;
        if found.is_some() {
            caught += 1;
        }
        println!("  [regression] {:<30} -> {}", label, if found.is_some() { "CAUGHT (PoC verified)" } else { "MISSED" });
    }

    // Clean refactors (identical behavior, whitespace/comment changes): expect 0 false positives
    let clean_cases = [
        ("clean_whitespace_refactor", "mod.py", "def ok(x):\n    return x > 0\n", "def ok(x):\n    # comment\n    return x > 0\n"),
        ("clean_doc_edit", "doc.py", "def ping():\n    return True\n", "def ping():\n    \"\"\"ping\"\"\"\n    return True\n"),
    ];
    let mut clean_fp = 0usize;
    for (label, file, base, target) in clean_cases {
        let dir = tempfile::tempdir()?;
        init_repo(dir.path(), file, base)?;
        fs::write(dir.path().join(file), target)?;
        let opts = RunOptions {
            repo: None,
            base: Some("main".to_string()),
            target: "working-tree".to_string(),
            runner: Some("python".to_string()),
            timeout: 10,
            out: None,
            synthesizer: SynthesizerType::Heuristic,
        };
        let found = reddiff::run(&opts, dir.path())?;
        if found.is_some() {
            clean_fp += 1;
        }
        println!("  [clean diff] {:<30} -> {}", label, if found.is_none() { "SILENT (0 false positive)" } else { "FALSE POSITIVE" });
    }

    // Flaky candidate + secret scrubbing verification
    let flake_dir = tempfile::tempdir()?;
    init_repo(flake_dir.path(), "app.py", "VAL = 1\n")?;
    fs::write(flake_dir.path().join("app.py"), "VAL = 2\n")?;
    let base_sha = GitClient::resolve_ref(flake_dir.path(), "HEAD")?;
    let raw_diff = GitClient::diff(flake_dir.path(), &base_sha, None)?;
    let summary = parse_unified_diff(&raw_diff)?;

    // Candidate reads a marker file inside the scrubbed TEMP dir so first run passes, second run fails (flaky!)
    let flaky_cand = CandidateTest {
        name: "flaky_probe".to_string(),
        script: "import os, sys\np = os.path.join(os.environ['TMPDIR'], 'flake_state')\nif not os.path.exists(p):\n    open(p, 'w').write('1')\n    sys.exit(0)\nsys.exit(1)\n".to_string(),
        file_extension: "py".to_string(),
        runner: Some("python".to_string()),
        timeout_secs: Some(5),
    };
    let opts = RunOptions {
        repo: None,
        base: Some("main".to_string()),
        target: "working-tree".to_string(),
        runner: Some("python".to_string()),
        timeout: 5,
        out: None,
        synthesizer: SynthesizerType::Mock,
    };
    let flake_res = reddiff::run_with_synthesizer(&opts, flake_dir.path(), &base_sha, &summary, &MockSynthesizer::single(flaky_cand))?;
    let flake_rejected = flake_res.is_none();

    // Secret-scrubbing candidate: fails on target ONLY if host secret is visible
    std::env::set_var("OPENAI_API_KEY", "sk-benchmark-secret-must-be-scrubbed");
    let wt = WorktreeGuard::create(flake_dir.path(), &base_sha)?;
    let verifier = DifferentialVerifier::new(wt.path().to_path_buf(), flake_dir.path().to_path_buf(), Some("python".to_string()), Duration::from_secs(5), None);
    let secret_probe = CandidateTest {
        name: "secret_scrub_probe".to_string(),
        script: "import os, sys\nsys.exit(0 if 'OPENAI_API_KEY' not in os.environ else 1)\n".to_string(),
        file_extension: "py".to_string(),
        runner: Some("python".to_string()),
        timeout_secs: Some(5),
    };
    let secret_leaked = verifier.verify_candidates(&[secret_probe])?.is_some();
    wt.clean_up()?;

    let elapsed = start.elapsed().as_secs_f64();
    println!("Regressions caught:        {}/4", caught);
    println!("Clean-diff false positives:{}/2", clean_fp);
    println!("Flaky test rejected (2x):  {}", if flake_rejected { "YES (100%)" } else { "NO" });
    println!("Host secrets scrubbed:     {}", if !secret_leaked { "YES (OPENAI_API_KEY stripped)" } else { "NO" });
    println!("Benchmark runtime:         {:.2}s", elapsed);

    assert_eq!(caught, 4);
    assert_eq!(clean_fp, 0);
    assert!(flake_rejected);
    assert!(!secret_leaked);
    Ok(())
}
