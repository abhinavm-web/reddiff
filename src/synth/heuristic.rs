use std::path::Path;
use regex::Regex;
use crate::diff::{DiffFile, DiffHunk, DiffSummary, LineKind};
use crate::error::ReddiffError;
use super::{CandidateTest, Synthesizer};

/// Heuristic offline synthesizer identifying common regression patterns from diff hunks.
#[derive(Debug, Clone, Default)]
pub struct HeuristicSynthesizer;

impl HeuristicSynthesizer {
    pub fn new() -> Self {
        Self
    }

    fn analyze_file(&self, file: &DiffFile) -> Vec<CandidateTest> {
        let mut candidates = Vec::new();
        let path = match file.effective_path() {
            Some(p) => p,
            None => return candidates,
        };

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        for hunk in &file.hunks {
            let hunk_candidates = self.analyze_hunk(path, &ext, hunk);
            candidates.extend(hunk_candidates);
        }

        candidates
    }

    fn analyze_hunk(&self, path: &Path, ext: &str, hunk: &DiffHunk) -> Vec<CandidateTest> {
        let mut candidates = Vec::new();
        let fn_info = find_enclosing_function(hunk);

        // Find pairs of deletions and additions
        let mut i = 0;
        while i < hunk.lines.len() {
            if hunk.lines[i].kind == LineKind::Deletion {
                let del_line = &hunk.lines[i].content;
                // Check if following or nearby line is an addition
                if i + 1 < hunk.lines.len() && hunk.lines[i + 1].kind == LineKind::Addition {
                    let add_line = &hunk.lines[i + 1].content;
                    if let Some(cand) = self.check_mutation(path, ext, &fn_info, del_line, add_line) {
                        candidates.push(cand);
                    }
                    i += 2;
                    continue;
                }
            }
            i += 1;
        }

        candidates
    }

    fn check_mutation(
        &self,
        path: &Path,
        ext: &str,
        fn_info: &Option<FunctionInfo>,
        del_line: &str,
        add_line: &str,
    ) -> Option<CandidateTest> {
        // Pattern 1: Boundary operator mutation (> vs >=, < vs <=)
        if let Some(cand) = self.check_boundary_operator(path, ext, fn_info, del_line, add_line) {
            return Some(cand);
        }

        // Pattern 2: Boolean literal flip (True -> False or False -> True)
        if let Some(cand) = self.check_boolean_flip(path, ext, fn_info, del_line, add_line) {
            return Some(cand);
        }

        // Pattern 3: Arithmetic operator mutation (+ to -, * to /)
        if let Some(cand) = self.check_arithmetic_mutation(path, ext, fn_info, del_line, add_line) {
            return Some(cand);
        }

        None
    }

    fn check_boundary_operator(
        &self,
        path: &Path,
        ext: &str,
        fn_info: &Option<FunctionInfo>,
        del_line: &str,
        add_line: &str,
    ) -> Option<CandidateTest> {
        let op_re = Regex::new(r"([><]=?)\s*(\d+)").ok()?;

        let del_cap = op_re.captures(del_line)?;
        let add_cap = op_re.captures(add_line)?;

        let del_op = del_cap.get(1)?.as_str();
        let del_val = del_cap.get(2)?.as_str();

        let add_op = add_cap.get(1)?.as_str();
        let add_val = add_cap.get(2)?.as_str();

        // Same boundary value, but comparison operator changed
        if del_val == add_val && del_op != add_op {
            let fn_name = fn_info.as_ref().map(|f| f.name.as_str()).unwrap_or("target_func");
            let boundary_num = del_val;

            // Determine baseline expected outcome:
            // If base was `> 0` and input is `0`: 0 > 0 is false.
            // If base was `< 0` and input is `0`: 0 < 0 is false.
            // If base was `>= 0` and input is `0`: 0 >= 0 is true.
            // If base was `<= 0` and input is `0`: 0 <= 0 is true.
            let (expected_bool, _) = match del_op {
                ">" | "<" => (false, true),
                ">=" | "<=" => (true, false),
                _ => (false, true),
            };

            return Some(self.build_test_script(
                path,
                ext,
                fn_name,
                &[boundary_num],
                if expected_bool { "True" } else { "False" },
                "boundary_condition_mutation",
            ));
        }

        None
    }

    fn check_boolean_flip(
        &self,
        path: &Path,
        ext: &str,
        fn_info: &Option<FunctionInfo>,
        del_line: &str,
        add_line: &str,
    ) -> Option<CandidateTest> {
        let fn_name = fn_info.as_ref().map(|f| f.name.as_str()).unwrap_or("target_func");
        let arg_count = fn_info.as_ref().map(|f| f.params.len()).unwrap_or(0);
        let dummy_args: Vec<&str> = vec!["0"; arg_count];

        if (del_line.contains("True") && add_line.contains("False"))
            || (del_line.contains("true") && add_line.contains("false"))
        {
            return Some(self.build_test_script(
                path,
                ext,
                fn_name,
                &dummy_args,
                "True",
                "boolean_flip_to_false",
            ));
        }

        if (del_line.contains("False") && add_line.contains("True"))
            || (del_line.contains("false") && add_line.contains("true"))
        {
            return Some(self.build_test_script(
                path,
                ext,
                fn_name,
                &dummy_args,
                "False",
                "boolean_flip_to_true",
            ));
        }

        None
    }

    fn check_arithmetic_mutation(
        &self,
        path: &Path,
        ext: &str,
        fn_info: &Option<FunctionInfo>,
        del_line: &str,
        add_line: &str,
    ) -> Option<CandidateTest> {
        let fn_name = fn_info.as_ref().map(|f| f.name.as_str()).unwrap_or("target_func");

        if del_line.contains('+') && add_line.contains('-') {
            // e.g. base return a + b (2+1 = 3), target return a - b (2-1 = 1)
            return Some(self.build_test_script(
                path,
                ext,
                fn_name,
                &["2", "1"],
                "3",
                "arithmetic_add_to_sub",
            ));
        }

        if del_line.contains('-') && add_line.contains('+') {
            // e.g. base return a - b (2-1 = 1), target return a + b (2+1 = 3)
            return Some(self.build_test_script(
                path,
                ext,
                fn_name,
                &["2", "1"],
                "1",
                "arithmetic_sub_to_add",
            ));
        }

        None
    }

    fn build_test_script(
        &self,
        path: &Path,
        ext: &str,
        fn_name: &str,
        args: &[&str],
        expected: &str,
        candidate_name: &str,
    ) -> CandidateTest {
        let module_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module");

        let args_str = args.join(", ");

        let (script, runner, file_ext) = match ext {
            "py" => {
                let code = format!(
                    "import sys\n\
                     import os\n\
                     sys.path.insert(0, os.path.abspath('.'))\n\
                     from {module_stem} import {fn_name}\n\
                     \n\
                     assert {fn_name}({args_str}) == {expected}\n"
                );
                (code, Some("python".to_string()), "py".to_string())
            }
            "js" | "ts" => {
                let exp_js = if expected == "True" {
                    "true"
                } else if expected == "False" {
                    "false"
                } else {
                    expected
                };
                let code = format!(
                    "const assert = require('assert');\n\
                     const mod = require('./{module_stem}');\n\
                     const fn = mod.{fn_name} || mod.default || mod;\n\
                     assert.strictEqual(fn({args_str}), {exp_js});\n"
                );
                (code, Some("node".to_string()), "js".to_string())
            }
            "sh" | "bash" => {
                let code = format!(
                    "#!/usr/bin/env bash\n\
                     source ./{module_stem}.sh\n\
                     res=$({fn_name} {args_str})\n\
                     [ \"$res\" = \"{expected}\" ]\n"
                );
                (code, Some("bash".to_string()), "sh".to_string())
            }
            _ => {
                // Default to Python syntax
                let code = format!(
                    "import sys\n\
                     import os\n\
                     sys.path.insert(0, os.path.abspath('.'))\n\
                     from {module_stem} import {fn_name}\n\
                     \n\
                     assert {fn_name}({args_str}) == {expected}\n"
                );
                (code, Some("python".to_string()), "py".to_string())
            }
        };

        CandidateTest {
            name: candidate_name.to_string(),
            script,
            file_extension: file_ext,
            runner,
            timeout_secs: Some(10),
        }
    }
}

impl Synthesizer for HeuristicSynthesizer {
    fn synthesize(&self, diff: &DiffSummary) -> Result<Vec<CandidateTest>, ReddiffError> {
        let mut candidates = Vec::new();
        for file in &diff.files {
            candidates.extend(self.analyze_file(file));
        }
        Ok(candidates)
    }
}

#[derive(Debug, Clone)]
struct FunctionInfo {
    name: String,
    params: Vec<String>,
}

fn find_enclosing_function(hunk: &DiffHunk) -> Option<FunctionInfo> {
    let fn_re = Regex::new(r"(?:def|function|fn)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(([^)]*)\)").ok()?;

    // Check hunk header first
    if let Some(caps) = fn_re.captures(&hunk.header) {
        let name = caps.get(1)?.as_str().to_string();
        let params_str = caps.get(2)?.as_str();
        let params = parse_params(params_str);
        return Some(FunctionInfo { name, params });
    }

    // Check hunk lines (context lines)
    for line in &hunk.lines {
        if line.kind == LineKind::Context {
            if let Some(caps) = fn_re.captures(&line.content) {
                let name = caps.get(1)?.as_str().to_string();
                let params_str = caps.get(2)?.as_str();
                let params = parse_params(params_str);
                return Some(FunctionInfo { name, params });
            }
        }
    }

    None
}

fn parse_params(params_str: &str) -> Vec<String> {
    params_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "self" && s != "&self" && s != "&mut self")
        .collect()
}
