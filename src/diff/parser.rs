use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use regex::Regex;
use crate::error::ReddiffError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
    ModeChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineKind {
    Context,
    Addition,
    Deletion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: LineKind,
    pub content: String,
    pub old_line_no: Option<usize>,
    pub new_line_no: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffFile {
    pub old_path: Option<PathBuf>,
    pub new_path: Option<PathBuf>,
    pub old_mode: Option<u32>,
    pub new_mode: Option<u32>,
    pub status: FileStatus,
    pub is_binary: bool,
    pub similarity: Option<u8>,
    pub hunks: Vec<DiffHunk>,
}

impl DiffFile {
    pub fn effective_path(&self) -> Option<&Path> {
        self.new_path.as_deref().or(self.old_path.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSummary {
    pub files: Vec<DiffFile>,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

impl DiffSummary {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() || (self.insertions == 0 && self.deletions == 0 && self.files.iter().all(|f| f.hunks.is_empty() && !f.is_binary))
    }

    pub fn total_changes(&self) -> usize {
        self.insertions + self.deletions
    }
}

/// Parses raw unified diff output from git into typed `DiffSummary`.
pub fn parse_unified_diff(raw_diff: &str) -> Result<DiffSummary, ReddiffError> {
    let hunk_re = Regex::new(r"^@@\s+-(\d+)(?:,(\d+))?\s+\+(\d+)(?:,(\d+))?\s+@@(.*)$")
        .map_err(|e| ReddiffError::DiffParse(format!("Regex compile error: {e}")))?;

    let mut files = Vec::new();
    let mut current_file: Option<DiffFile> = None;
    let mut current_hunk: Option<DiffHunk> = None;
    let mut total_insertions = 0;
    let mut total_deletions = 0;

    let mut current_old_line = 0;
    let mut current_new_line = 0;

    for line in raw_diff.lines() {
        if let Some(paths_part) = line.strip_prefix("diff --git ") {
            // Close existing hunk and file
            if let Some(hunk) = current_hunk.take() {
                if let Some(ref mut file) = current_file {
                    file.hunks.push(hunk);
                }
            }
            if let Some(file) = current_file.take() {
                files.push(file);
            }

            // Extract file paths from diff --git a/... b/...
            let (old_p, new_p) = parse_git_diff_header_paths(paths_part);

            current_file = Some(DiffFile {
                old_path: old_p,
                new_path: new_p,
                old_mode: None,
                new_mode: None,
                status: FileStatus::Modified,
                is_binary: false,
                similarity: None,
                hunks: Vec::new(),
            });
            continue;
        }

        if let Some(ref mut file) = current_file {
            if line.starts_with("new file mode ") {
                file.status = FileStatus::Added;
                file.old_path = None;
                continue;
            }
            if line.starts_with("deleted file mode ") {
                file.status = FileStatus::Deleted;
                file.new_path = None;
                continue;
            }
            if let Some(rest) = line.strip_prefix("similarity index ") {
                file.status = FileStatus::Renamed;
                let num_str = rest.trim().trim_end_matches('%');
                if let Ok(pct) = num_str.parse::<u8>() {
                    file.similarity = Some(pct);
                }
                continue;
            }
            if let Some(p) = line.strip_prefix("rename from ") {
                file.old_path = Some(PathBuf::from(p.trim()));
                file.status = FileStatus::Renamed;
                continue;
            }
            if let Some(p) = line.strip_prefix("rename to ") {
                file.new_path = Some(PathBuf::from(p.trim()));
                file.status = FileStatus::Renamed;
                continue;
            }
            if line.starts_with("Binary files ") && line.ends_with(" differ") {
                file.is_binary = true;
                continue;
            }
            if let Some(raw_path) = line.strip_prefix("--- ") {
                let path_str = raw_path.trim();
                if path_str == "/dev/null" {
                    file.status = FileStatus::Added;
                    file.old_path = None;
                } else {
                    let cleaned = strip_prefix(path_str, "a/");
                    file.old_path = Some(PathBuf::from(cleaned));
                }
                continue;
            }
            if let Some(raw_path) = line.strip_prefix("+++ ") {
                let path_str = raw_path.trim();
                if path_str == "/dev/null" {
                    file.status = FileStatus::Deleted;
                    file.new_path = None;
                } else {
                    let cleaned = strip_prefix(path_str, "b/");
                    file.new_path = Some(PathBuf::from(cleaned));
                }
                continue;
            }

            if line.starts_with("@@ ") {
                // Hunk header
                if let Some(hunk) = current_hunk.take() {
                    file.hunks.push(hunk);
                }

                if let Some(caps) = hunk_re.captures(line) {
                    let old_start: usize = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
                    let old_count: usize = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
                    let new_start: usize = caps.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
                    let new_count: usize = caps.get(4).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
                    let header = caps.get(5).map(|m| m.as_str().trim().to_string()).unwrap_or_default();

                    current_old_line = old_start;
                    current_new_line = new_start;

                    current_hunk = Some(DiffHunk {
                        old_start,
                        old_count,
                        new_start,
                        new_count,
                        header,
                        lines: Vec::new(),
                    });
                }
                continue;
            }

            // If we are inside a hunk
            if let Some(ref mut hunk) = current_hunk {
                if let Some(stripped) = line.strip_prefix('+') {
                    total_insertions += 1;
                    hunk.lines.push(DiffLine {
                        kind: LineKind::Addition,
                        content: stripped.to_string(),
                        old_line_no: None,
                        new_line_no: Some(current_new_line),
                    });
                    current_new_line += 1;
                } else if let Some(stripped) = line.strip_prefix('-') {
                    total_deletions += 1;
                    hunk.lines.push(DiffLine {
                        kind: LineKind::Deletion,
                        content: stripped.to_string(),
                        old_line_no: Some(current_old_line),
                        new_line_no: None,
                    });
                    current_old_line += 1;
                } else if let Some(stripped) = line.strip_prefix(' ') {
                    hunk.lines.push(DiffLine {
                        kind: LineKind::Context,
                        content: stripped.to_string(),
                        old_line_no: Some(current_old_line),
                        new_line_no: Some(current_new_line),
                    });
                    current_old_line += 1;
                    current_new_line += 1;
                } else if line.starts_with('\\') {
                    // e.g. \ No newline at end of file
                    // Ignore without advancing line numbers
                }
            }
        }
    }

    // Flush last hunk and file
    if let Some(hunk) = current_hunk.take() {
        if let Some(ref mut file) = current_file {
            file.hunks.push(hunk);
        }
    }
    if let Some(file) = current_file.take() {
        files.push(file);
    }

    let files_changed = files.len();
    Ok(DiffSummary {
        files,
        files_changed,
        insertions: total_insertions,
        deletions: total_deletions,
    })
}

fn strip_prefix<'a>(s: &'a str, prefix: &str) -> &'a str {
    if let Some(rest) = s.strip_prefix(prefix) {
        rest
    } else {
        s
    }
}

fn parse_git_diff_header_paths(paths_line: &str) -> (Option<PathBuf>, Option<PathBuf>) {
    // Usually "a/path b/path"
    // Handle paths with spaces or quotes if present
    let trimmed = paths_line.trim();
    if let Some(idx) = trimmed.find(" b/") {
        let old_p = strip_prefix(&trimmed[..idx], "a/");
        let new_p = strip_prefix(&trimmed[idx + 1..], "b/");
        (Some(PathBuf::from(old_p)), Some(PathBuf::from(new_p)))
    } else {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 2 {
            let old_p = strip_prefix(parts[0], "a/");
            let new_p = strip_prefix(parts[1], "b/");
            (Some(PathBuf::from(old_p)), Some(PathBuf::from(new_p)))
        } else {
            (None, None)
        }
    }
}
