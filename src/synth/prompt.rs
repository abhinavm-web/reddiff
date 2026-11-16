/// Zero-prose LLM system prompt enforcing code-only response.
pub const SYSTEM_PROMPT: &str = "\
You are an adversarial test synthesizer. Your job is to output a single runnable reproducer test script that exposes a regression in the provided diff.
CRITICAL RULES:
1. Output ONLY executable code enclosed in triple backticks with language identifier (e.g. ```python\\n...\\n```).
2. NEVER output conversational prose, greetings, markdown text outside the code block, or explanation of the bug.
3. The test must exit with code 0 on the base code and exit with non-zero on the target code.
4. Make the test self-contained with zero external dependencies.";

/// Constructs the user prompt containing the diff text.
pub fn construct_user_prompt(diff_text: &str) -> String {
    format!(
        "Synthesize a minimal adversarial test script that passes on the pre-diff code and fails on the post-diff code.\n\n\
         GIT DIFF:\n\
         ```diff\n\
         {diff_text}\n\
         ```\n\n\
         Remember: Output strictly the code block. Zero prose."
    )
}
