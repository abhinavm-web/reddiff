pub mod git;
pub mod parser;

pub use git::GitClient;
pub use parser::{parse_unified_diff, DiffFile, DiffHunk, DiffLine, DiffSummary, FileStatus, LineKind};
