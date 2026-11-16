use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::warn;
use crate::diff::GitClient;
use crate::error::ReddiffError;

/// RAII guard for a temporary git worktree.
/// Ensures worktrees are cleanly removed even upon error or drop.
pub struct WorktreeGuard {
    repo_root: PathBuf,
    worktree_path: PathBuf,
    active: bool,
}

impl WorktreeGuard {
    /// Creates a new detached worktree at a temporary path pointing to `commit_ref`.
    pub fn create(repo_root: &Path, commit_ref: &str) -> Result<Self, ReddiffError> {
        let ts = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_nanos(),
            Err(_) => 0,
        };
        let dir_name = format!("rd_wt_{}_{}", std::process::id(), ts);
        let worktree_path = std::env::temp_dir().join(dir_name);

        GitClient::worktree_add(repo_root, &worktree_path, commit_ref)?;

        Ok(Self {
            repo_root: repo_root.to_path_buf(),
            worktree_path,
            active: true,
        })
    }

    /// Path to the isolated worktree directory.
    pub fn path(&self) -> &Path {
        &self.worktree_path
    }

    /// Explicit cleanup returning any error encountered.
    pub fn clean_up(mut self) -> Result<(), ReddiffError> {
        self.remove_internal()
    }

    fn remove_internal(&mut self) -> Result<(), ReddiffError> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        Self::remove_with_retry(&self.repo_root, &self.worktree_path)
    }

    fn remove_with_retry(repo_root: &Path, worktree_path: &Path) -> Result<(), ReddiffError> {
        const MAX_RETRIES: usize = 5;
        let mut backoff = Duration::from_millis(50);

        for _ in 0..MAX_RETRIES {
            let res = GitClient::worktree_remove(repo_root, worktree_path);
            if res.is_ok() {
                return Ok(());
            }
            std::thread::sleep(backoff);
            backoff *= 2;
        }

        // Fallback: If git worktree remove failed after retries,
        // clear read-only attributes, remove directory, and invoke git worktree prune.
        if worktree_path.exists() {
            clear_readonly_recursive(worktree_path);
            let _ = std::fs::remove_dir_all(worktree_path);
        }

        let _ = GitClient::worktree_prune(repo_root);

        if worktree_path.exists() {
            Err(ReddiffError::Worktree {
                path: worktree_path.to_path_buf(),
                message: "Worktree directory could not be removed after retries".to_string(),
            })
        } else {
            Ok(())
        }
    }
}

impl Drop for WorktreeGuard {
    fn drop(&mut self) {
        if self.active {
            if let Err(e) = Self::remove_with_retry(&self.repo_root, &self.worktree_path) {
                warn!(?e, "Failed to clean up worktree during Drop");
            }
            self.active = false;
        }
    }
}

#[allow(clippy::permissions_set_readonly_false)]
fn clear_readonly_recursive(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                clear_readonly_recursive(&path);
            }
            if let Ok(metadata) = std::fs::metadata(&path) {
                let mut perms = metadata.permissions();
                if perms.readonly() {
                    perms.set_readonly(false);
                    let _ = std::fs::set_permissions(&path, perms);
                }
            }
        }
    }
}
