use std::path::{Path, PathBuf};
use std::time::Duration;
use crate::error::ReddiffError;
use crate::sandbox::ProcessRunner;
use crate::synth::CandidateTest;

pub struct Shrinker;

impl Shrinker {
    /// Minimizes reproducer test lines, writes to `.reddiff/reproducer.<ext>` (or `out_path`),
    /// and returns the path to the written reproducer.
    pub fn shrink_and_write(
        candidate: &CandidateTest,
        runner: &str,
        base_dir: &Path,
        target_dir: &Path,
        timeout: Duration,
        out_path: Option<&Path>,
    ) -> Result<PathBuf, ReddiffError> {
        let mut best_script = candidate.script.clone();

        // Step 1: Try stripping comment lines and extraneous whitespace
        let stripped = strip_comments_and_blanks(&best_script, &candidate.file_extension);
        if verify_script(&stripped, &candidate.file_extension, runner, base_dir, target_dir, timeout, out_path) {
            best_script = stripped;
        }

        // Step 2: Line-by-line delta reduction (up to 30 lines)
        let lines: Vec<&str> = best_script.lines().collect();
        if lines.len() <= 30 {
            let mut current_lines = lines;
            let mut idx = 0;
            while idx < current_lines.len() {
                if is_essential_preamble(current_lines[idx]) {
                    idx += 1;
                    continue;
                }

                // Try removing line at idx
                let mut trial_lines = current_lines.clone();
                trial_lines.remove(idx);
                let trial_script = trial_lines.join("\n");

                if verify_script(&trial_script, &candidate.file_extension, runner, base_dir, target_dir, timeout, out_path) {
                    current_lines = trial_lines;
                } else {
                    idx += 1;
                }
            }
            best_script = current_lines.join("\n");
        }

        // Ensure trailing newline
        if !best_script.ends_with('\n') {
            best_script.push('\n');
        }

        // Step 3: Write to destination file
        let final_path = match out_path {
            Some(p) => p.to_path_buf(),
            None => {
                let default_dir = target_dir.join(".reddiff");
                default_dir.join(format!("reproducer.{}", candidate.file_extension))
            }
        };

        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&final_path, best_script)?;

        Ok(final_path)
    }
}

fn is_essential_preamble(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("sys.path")
        || trimmed.starts_with("const ")
        || trimmed.starts_with("require(")
        || trimmed.starts_with("#!/")
}

fn strip_comments_and_blanks(script: &str, ext: &str) -> String {
    let mut out_lines = Vec::new();
    for line in script.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if (ext == "py" || ext == "sh") && trimmed.starts_with('#') {
            continue;
        }
        if (ext == "js" || ext == "ts" || ext == "rs") && trimmed.starts_with("//") {
            continue;
        }
        out_lines.push(line);
    }
    out_lines.join("\n")
}

fn verify_script(
    script: &str,
    ext: &str,
    runner: &str,
    base_dir: &Path,
    target_dir: &Path,
    timeout: Duration,
    out_path: Option<&Path>,
) -> bool {
    let temp_filename = format!("_rd_shrink_{}.{}", std::process::id(), ext);
    let sub_rel = out_path
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".reddiff"));

    let base_sub = base_dir.join(&sub_rel);
    let target_sub = target_dir.join(&sub_rel);

    let _ = std::fs::create_dir_all(&base_sub);
    let _ = std::fs::create_dir_all(&target_sub);

    let base_file = base_sub.join(&temp_filename);
    let target_file = target_sub.join(&temp_filename);

    // Verify on base
    if std::fs::write(&base_file, script).is_err() {
        return false;
    }
    let base_res = ProcessRunner::run_script(runner, &base_file, base_dir, timeout);
    let _ = std::fs::remove_file(&base_file);

    let base_ok = match base_res {
        Ok(out) => out.success(),
        Err(_) => false,
    };

    if !base_ok {
        return false;
    }

    // Verify on target
    if std::fs::write(&target_file, script).is_err() {
        return false;
    }
    let target_res = ProcessRunner::run_script(runner, &target_file, target_dir, timeout);
    let _ = std::fs::remove_file(&target_file);

    match target_res {
        Ok(out) => !out.success(),
        Err(_) => false,
    }
}
