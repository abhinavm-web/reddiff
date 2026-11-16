use serde_json::json;
use crate::diff::DiffSummary;
use crate::error::ReddiffError;
use super::prompt::{construct_user_prompt, SYSTEM_PROMPT};
use super::{CandidateTest, Synthesizer};

/// Online LLM synthesizer querying OpenAI-compatible endpoints with zero prose extraction.
#[derive(Debug, Clone)]
pub struct LlmSynthesizer {
    api_key: Option<String>,
    base_url: String,
    model: String,
}

impl Default for LlmSynthesizer {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmSynthesizer {
    pub fn new() -> Self {
        let api_key = std::env::var("REDDIFF_API_KEY")
            .or_else(|_| std::env::var("OPENAI_API_KEY"))
            .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
            .ok();
        let is_anthropic_only = std::env::var("REDDIFF_API_KEY").is_err()
            && std::env::var("OPENAI_API_KEY").is_err()
            && std::env::var("ANTHROPIC_API_KEY").is_ok();
        let base_url = std::env::var("REDDIFF_BASE_URL")
            .or_else(|_| std::env::var("OPENAI_BASE_URL"))
            .unwrap_or_else(|_| {
                if is_anthropic_only {
                    "https://api.anthropic.com/v1".to_string()
                } else {
                    "https://api.openai.com/v1".to_string()
                }
            });
        let model = std::env::var("REDDIFF_MODEL")
            .or_else(|_| std::env::var("OPENAI_MODEL"))
            .unwrap_or_else(|_| {
                if is_anthropic_only {
                    "claude-3-5-sonnet-20241022".to_string()
                } else {
                    "gpt-4o-mini".to_string()
                }
            });

        Self {
            api_key,
            base_url,
            model,
        }
    }

    pub fn has_credentials(&self) -> bool {
        self.api_key.is_some()
    }

    pub fn with_config(api_key: String, base_url: String, model: String) -> Self {
        Self {
            api_key: Some(api_key),
            base_url,
            model,
        }
    }
}

impl Synthesizer for LlmSynthesizer {
    fn synthesize(&self, diff: &DiffSummary) -> Result<Vec<CandidateTest>, ReddiffError> {
        let api_key = self
            .api_key
            .as_deref()
            .ok_or_else(|| ReddiffError::Llm("OPENAI_API_KEY environment variable not set".to_string()))?;

        // Format a compact representation of the diff
        let mut diff_repr = String::new();
        for file in &diff.files {
            let path_str = file
                .effective_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            diff_repr.push_str(&format!("--- a/{path_str}\n+++ b/{path_str}\n"));
            for hunk in &file.hunks {
                diff_repr.push_str(&format!("{}\n", hunk.header));
                for line in &hunk.lines {
                    let prefix = match line.kind {
                        crate::diff::LineKind::Context => " ",
                        crate::diff::LineKind::Addition => "+",
                        crate::diff::LineKind::Deletion => "-",
                    };
                    diff_repr.push_str(&format!("{prefix}{}\n", line.content));
                }
            }
        }

        let user_prompt = construct_user_prompt(&diff_repr);
        let is_anthropic = self.base_url.contains("anthropic.com");

        let response = if is_anthropic {
            let endpoint = format!("{}/messages", self.base_url.trim_end_matches('/'));
            let payload = json!({
                "model": self.model,
                "max_tokens": 1024,
                "system": SYSTEM_PROMPT,
                "messages": [
                    {"role": "user", "content": user_prompt}
                ],
                "temperature": 0.1
            });
            ureq::post(&endpoint)
                .set("x-api-key", api_key)
                .set("anthropic-version", "2023-06-01")
                .set("Content-Type", "application/json")
                .send_json(payload)
                .map_err(|e| ReddiffError::Llm(format!("HTTP request failed: {e}")))?
        } else {
            let endpoint = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
            let payload = json!({
                "model": self.model,
                "messages": [
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": user_prompt}
                ],
                "temperature": 0.1
            });
            ureq::post(&endpoint)
                .set("Authorization", &format!("Bearer {api_key}"))
                .set("Content-Type", "application/json")
                .send_json(payload)
                .map_err(|e| ReddiffError::Llm(format!("HTTP request failed: {e}")))?
        };

        let body_str = response
            .into_string()
            .map_err(|e| ReddiffError::Llm(format!("Failed to read response body: {e}")))?;

        let res_json: serde_json::Value = serde_json::from_str(&body_str)?;
        let content = res_json["choices"][0]["message"]["content"]
            .as_str()
            .or_else(|| res_json["content"][0]["text"].as_str())
            .ok_or_else(|| ReddiffError::Llm("Missing content in LLM response".to_string()))?;

        let candidates = extract_candidate_tests(content);
        Ok(candidates)
    }
}

/// Strictly extracts code blocks from an LLM response, stripping all surrounding prose.
pub fn extract_candidate_tests(llm_response: &str) -> Vec<CandidateTest> {
    let mut candidates = Vec::new();
    let fence = "```";
    let mut cursor = llm_response;

    while let Some(start) = cursor.find(fence) {
        let after_fence = &cursor[start + fence.len()..];
        if let Some(newline) = after_fence.find('\n') {
            let lang = after_fence[..newline].trim().to_lowercase();
            let body = &after_fence[newline + 1..];
            if let Some(end) = body.find(fence) {
                let code = body[..end].trim().to_string();
                let (ext, runner) = match lang.as_str() {
                    "python" | "py" => ("py", Some("python".to_string())),
                    "javascript" | "js" => ("js", Some("node".to_string())),
                    "typescript" | "ts" => ("ts", Some("node".to_string())),
                    "rust" | "rs" => ("rs", None),
                    "bash" | "sh" => ("sh", Some("bash".to_string())),
                    _ => ("py", Some("python".to_string())),
                };

                candidates.push(CandidateTest {
                    name: format!("llm_candidate_{}", candidates.len() + 1),
                    script: code,
                    file_extension: ext.to_string(),
                    runner,
                    timeout_secs: Some(15),
                });
                cursor = &body[end + fence.len()..];
                continue;
            }
        }
        break;
    }

    candidates
}
