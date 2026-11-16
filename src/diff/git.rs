use std::path::{Path, PathBuf};
use std::process::Command;
use crate::error::ReddiffError;

/// Type-safe wrapper for git CLI operations.
pub struct GitClient;

impl GitClient {
    /// Checks whether the specified directory is inside a git work tree.
    pub fn is_inside_work_tree(repo_dir: &Path) -> bool {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["rev-parse", "--is-inside-work-tree"])
            .output();

        match output {
            Ok(out) => out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true",
            Err(_) => false,
        }
    }

    /// Gets the top-level directory of the current git repository.
    pub fn toplevel(repo_dir: &Path) -> Result<PathBuf, ReddiffError> {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .map_err(|e| ReddiffError::Git(format!("Failed to execute git rev-parse: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ReddiffError::Git(format!(
                "Repository root detection failed: {}",
                stderr.trim()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(PathBuf::from(stdout))
    }

    /// Checks if the git repository has any commits (HEAD resolvable).
    pub fn has_commits(repo_dir: &Path) -> bool {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["rev-parse", "--verify", "HEAD"])
            .output();

        match output {
            Ok(out) => out.status.success(),
            Err(_) => false,
        }
    }

    /// Verifies and resolves a git reference to its 40-character commit SHA.
    pub fn resolve_ref(repo_dir: &Path, git_ref: &str) -> Result<String, ReddiffError> {
        let spec = format!("{git_ref}^{{commit}}");
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["rev-parse", "--verify", &spec])
            .output()
            .map_err(|e| ReddiffError::Git(format!("Failed to execute git rev-parse: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ReddiffError::Git(format!(
                "Reference '{git_ref}' could not be resolved: {}",
                stderr.trim()
            )));
        }

        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(sha)
    }

    /// Resolves the base reference using fallbacks if none was explicitly provided.
    pub fn resolve_base_ref(repo_dir: &Path, requested: Option<&str>) -> Result<String, ReddiffError> {
        if let Some(r) = requested {
            return Self::resolve_ref(repo_dir, r);
        }

        let candidates = ["main", "origin/main", "master", "origin/master", "HEAD~1"];
        for candidate in candidates {
            if let Ok(sha) = Self::resolve_ref(repo_dir, candidate) {
                return Ok(sha);
            }
        }

        // Fallback to merge-base against HEAD if possible, or HEAD directly
        if let Ok(head) = Self::resolve_ref(repo_dir, "HEAD") {
            return Ok(head);
        }

        Err(ReddiffError::Git(
            "Could not automatically resolve base reference (tried main, origin/main, master, HEAD~1, HEAD)".to_string(),
        ))
    }

    /// Finds the merge-base between base and target refs.
    pub fn merge_base(repo_dir: &Path, base: &str, target: &str) -> Result<String, ReddiffError> {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["merge-base", base, target])
            .output()
            .map_err(|e| ReddiffError::Git(format!("Failed to execute git merge-base: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ReddiffError::Git(format!(
                "Failed to find merge-base between '{base}' and '{target}': {}",
                stderr.trim()
            )));
        }

        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(sha)
    }

    /// Generates unified diff between base and target (or working tree).
    pub fn diff(
        repo_dir: &Path,
        base_ref: &str,
        target_ref: Option<&str>,
    ) -> Result<String, ReddiffError> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repo_dir);
        cmd.args(["-c", "core.quotepath=off", "diff", "-U3", "-M", "--no-color"]);

        match target_ref {
            None | Some("working-tree") => {
                cmd.arg(base_ref);
                cmd.arg("--");
            }
            Some(t) => {
                let range = format!("{base_ref}...{t}");
                cmd.arg(&range);
                cmd.arg("--");
            }
        }

        let output = cmd
            .output()
            .map_err(|e| ReddiffError::Git(format!("Failed to execute git diff: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ReddiffError::Git(format!("git diff failed: {}", stderr.trim())));
        }

        let diff_text = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(diff_text)
    }

    /// Adds a detached worktree pointing to a specific commit.
    pub fn worktree_add(repo_dir: &Path, worktree_dir: &Path, commit_ref: &str) -> Result<(), ReddiffError> {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["worktree", "add", "--detach"])
            .arg(worktree_dir)
            .arg(commit_ref)
            .output()
            .map_err(|e| ReddiffError::Git(format!("Failed to execute git worktree add: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ReddiffError::Worktree {
                path: worktree_dir.to_path_buf(),
                message: format!("git worktree add failed: {}", stderr.trim()),
            });
        }

        Ok(())
    }

    /// Removes a worktree forcefully.
    pub fn worktree_remove(repo_dir: &Path, worktree_dir: &Path) -> Result<(), ReddiffError> {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["worktree", "remove", "--force"])
            .arg(worktree_dir)
            .output()
            .map_err(|e| ReddiffError::Git(format!("Failed to execute git worktree remove: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ReddiffError::Worktree {
                path: worktree_dir.to_path_buf(),
                message: format!("git worktree remove failed: {}", stderr.trim()),
            });
        }

        Ok(())
    }

    /// Prunes worktree metadata.
    pub fn worktree_prune(repo_dir: &Path) -> Result<(), ReddiffError> {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["worktree", "prune"])
            .output()
            .map_err(|e| ReddiffError::Git(format!("Failed to execute git worktree prune: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ReddiffError::Git(format!("git worktree prune failed: {}", stderr.trim())));
        }

        Ok(())
    }
}
