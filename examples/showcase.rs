#![deny(clippy::all)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

use reddiff::diff::parse_unified_diff;
use reddiff::synth::{HeuristicSynthesizer, Synthesizer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== reddiff: Zero-Prose Adversarial Diff Breaker Showcase ===");

    let sample_diff = "\
diff --git a/src/quota.py b/src/quota.py
index 1111111..2222222 100644
--- a/src/quota.py
+++ b/src/quota.py
@@ -1,3 +1,3 @@
 def can_withdraw(balance):
-    return balance >= 100
+    return balance > 100
";

    let summary = parse_unified_diff(sample_diff)?;
    let synth = HeuristicSynthesizer::new();
    let candidates = synth.synthesize(&summary)?;

    println!(
        "Parsed {} modified file(s); synthesized {} adversarial candidate reproducer(s).",
        summary.files.len(),
        candidates.len()
    );
    if let Some(first) = candidates.first() {
        println!("--- Candidate #1 ({}) ---", first.name);
        println!("{}", first.script.trim());
    }

    Ok(())
}
