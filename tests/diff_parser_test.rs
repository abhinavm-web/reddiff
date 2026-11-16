use reddiff::diff::{parse_unified_diff, FileStatus, LineKind};
use std::path::Path;

#[test]
fn test_parse_clean_diff() -> Result<(), Box<dyn std::error::Error>> {
    let raw = "";
    let summary = parse_unified_diff(raw)?;
    assert!(summary.is_empty());
    assert_eq!(summary.files_changed, 0);
    assert_eq!(summary.insertions, 0);
    assert_eq!(summary.deletions, 0);
    Ok(())
}

#[test]
fn test_parse_modified_file_with_hunk() -> Result<(), Box<dyn std::error::Error>> {
    let raw = "\
diff --git a/src/calc.py b/src/calc.py
index e69de29..d95f3ad 100644
--- a/src/calc.py
+++ b/src/calc.py
@@ -1,3 +1,3 @@ def is_positive(x):
 context line
-return x > 0
+return x >= 0
";
    let summary = parse_unified_diff(raw)?;
    assert_eq!(summary.files_changed, 1);
    assert_eq!(summary.insertions, 1);
    assert_eq!(summary.deletions, 1);

    let file = &summary.files[0];
    assert_eq!(file.status, FileStatus::Modified);
    assert_eq!(file.old_path.as_deref(), Some(Path::new("src/calc.py")));
    assert_eq!(file.new_path.as_deref(), Some(Path::new("src/calc.py")));
    assert_eq!(file.hunks.len(), 1);

    let hunk = &file.hunks[0];
    assert_eq!(hunk.old_start, 1);
    assert_eq!(hunk.old_count, 3);
    assert_eq!(hunk.new_start, 1);
    assert_eq!(hunk.new_count, 3);
    assert_eq!(hunk.lines.len(), 3);

    assert_eq!(hunk.lines[0].kind, LineKind::Context);
    assert_eq!(hunk.lines[0].content, "context line");

    assert_eq!(hunk.lines[1].kind, LineKind::Deletion);
    assert_eq!(hunk.lines[1].content, "return x > 0");

    assert_eq!(hunk.lines[2].kind, LineKind::Addition);
    assert_eq!(hunk.lines[2].content, "return x >= 0");

    Ok(())
}

#[test]
fn test_parse_new_file_added() -> Result<(), Box<dyn std::error::Error>> {
    let raw = "\
diff --git a/new_file.txt b/new_file.txt
new file mode 100644
index 0000000..fa49b07
--- /dev/null
+++ b/new_file.txt
@@ -0,0 +1,2 @@
+line 1
+line 2
";
    let summary = parse_unified_diff(raw)?;
    assert_eq!(summary.files_changed, 1);
    assert_eq!(summary.insertions, 2);
    assert_eq!(summary.deletions, 0);

    let file = &summary.files[0];
    assert_eq!(file.status, FileStatus::Added);
    assert!(file.old_path.is_none());
    assert_eq!(file.new_path.as_deref(), Some(Path::new("new_file.txt")));

    Ok(())
}

#[test]
fn test_parse_deleted_file() -> Result<(), Box<dyn std::error::Error>> {
    let raw = "\
diff --git a/old_file.txt b/old_file.txt
deleted file mode 100644
index fa49b07..0000000
--- a/old_file.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-line 1
-line 2
";
    let summary = parse_unified_diff(raw)?;
    assert_eq!(summary.files_changed, 1);
    assert_eq!(summary.insertions, 0);
    assert_eq!(summary.deletions, 2);

    let file = &summary.files[0];
    assert_eq!(file.status, FileStatus::Deleted);
    assert_eq!(file.old_path.as_deref(), Some(Path::new("old_file.txt")));
    assert!(file.new_path.is_none());

    Ok(())
}

#[test]
fn test_parse_renamed_file() -> Result<(), Box<dyn std::error::Error>> {
    let raw = "\
diff --git a/source.txt b/dest.txt
similarity index 100%
rename from source.txt
rename to dest.txt
";
    let summary = parse_unified_diff(raw)?;
    assert_eq!(summary.files_changed, 1);
    let file = &summary.files[0];
    assert_eq!(file.status, FileStatus::Renamed);
    assert_eq!(file.similarity, Some(100));
    assert_eq!(file.old_path.as_deref(), Some(Path::new("source.txt")));
    assert_eq!(file.new_path.as_deref(), Some(Path::new("dest.txt")));

    Ok(())
}

#[test]
fn test_parse_binary_file_differ() -> Result<(), Box<dyn std::error::Error>> {
    let raw = "\
diff --git a/image.png b/image.png
index e2613b3..cdb95ea 100644
Binary files a/image.png and b/image.png differ
";
    let summary = parse_unified_diff(raw)?;
    assert_eq!(summary.files_changed, 1);
    let file = &summary.files[0];
    assert!(file.is_binary);

    Ok(())
}

#[test]
fn test_parse_single_line_hunk_coordinates() -> Result<(), Box<dyn std::error::Error>> {
    let raw = "\
diff --git a/a.txt b/a.txt
index 111..222 100644
--- a/a.txt
+++ b/a.txt
@@ -1 +1 @@
-old
+new
";
    let summary = parse_unified_diff(raw)?;
    let hunk = &summary.files[0].hunks[0];
    assert_eq!(hunk.old_start, 1);
    assert_eq!(hunk.old_count, 1);
    assert_eq!(hunk.new_start, 1);
    assert_eq!(hunk.new_count, 1);

    Ok(())
}
