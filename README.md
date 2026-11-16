# reddiff

**Adversarial diff breaker with one rule: No review comment without a runnable crash PoC.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](#roadmap--mvp-milestones)
[![Architecture: MVP](https://img.shields.io/badge/Architecture-MVP-orange.svg)](#roadmap--mvp-milestones)

---

## The Problem

Generating 2,000 lines of code with an AI takes 30 seconds; proving it doesn't contain subtle race conditions, auth bypasses, integer overflows, or broken state transitions takes hours.

Meanwhile, developers have severe **AI code review fatigue**. Existing AI PR reviewers flood diffs with 15 paragraphs of speculative prose (*"Consider checking if `user` could be null here"*), false positives, and stylistic nitpicks. Asking the same coding agent to write unit tests doesn't help either—it writes happy-path tests that mirror the exact assumptions of the bug.

## The Unclaimed Wedge: How It Works

`reddiff` enforces one ruthless constraint:

> **Silence unless proven broken.** `reddiff` is strictly forbidden from outputting prose review comments. It may only speak when it synthesizes and runs a minimal, reproducible failing test or exploit script that **passes on `main` (or satisfies the baseline invariant)** and **crashes or breaks the invariant on your `git diff`**.

Instead of a passive linter, `reddiff` spins up an isolated adversarial breaker agent against your uncommitted diff or PR:

1. **Invariant Extraction**: Parses the `git diff` and surrounding callers to infer state, concurrency, boundary, and authorization invariants.
2. **Adversarial Fuzz & Race Synthesis**: Generates property-based test harnesses (`Hypothesis`, `fast-check`, `proptest`), concurrent request storms, and malformed edge payloads inside an isolated sandbox.
3. **Differential Verification**: Runs the generated PoC against both the base state and your diff, shrinking the failing input to a minimal 5–15 line reproducer.
4. **Auto-Pipe to Fix**: Outputs a standalone failing test file you can commit directly—or pipe straight back into your coding agent to fix.

```text
  git diff (HEAD vs main)
         │
         ▼
  ┌──────────────────────────────────────────┐
  │        reddiff Adversarial Engine        │
  │  1. Infer invariants & attack surface    │
  │  2. Synthesize PBT / race / edge inputs  │
  │  3. Execute in isolated sandbox          │
  └──────────────────┬───────────────────────┘
                     │
          ┌──────────┴──────────┐
          ▼                     ▼
     No PoC Found          Crash PoC Verified!
   (Exits silently)    Emits minimal runnable test
                       + pipes reproducer to agent
```

### Target CLI Experience (`< 60 second aha moment`)

> Note: Below is the target MVP CLI specification currently under construction.

```bash
# Run against your uncommitted diff or staged PR changes
reddiff check

# Output (only appears when a real reproducible break is found):
# ✗ INVARIANT BROKEN in src/billing/credits.ts (verified in 11.4s)
#   Rule: Balance must never go negative under concurrent redemptions
#   Minimal PoC written to: .reddiff/poc_concurrent_redeem.test.ts
#
#   await Promise.all([redeem(user, 100), redeem(user, 100)]);
#   expect(await getBalance(user)).toBeGreaterThanOrEqual(0); // Got: -100
#
# Run `reddiff fix --agent claude` to send this failing PoC to your coding agent.
```

## Roadmap & MVP Milestones

- [ ] **Milestone 1 — Diff & Sandbox Runner**: Isolated worktree/container execution harness that runs commands against both base (`main`) and target (`HEAD`/working tree).
- [ ] **Milestone 2 — Invariant & Harness Generator**: Language adapters for TypeScript/JS (`vitest`/`fast-check`), Python (`pytest`/`hypothesis`), and Rust (`cargo test`/`proptest`).
- [ ] **Milestone 3 — Shrunk PoC Gatekeeper**: Strict verifier that discards flaky or non-reproducible attempts and minimizes passing-to-failing PoCs before emitting any output.
- [ ] **Milestone 4 — GitHub Action & Agent Pipe**: Zero-noise PR check mode + `reddiff fix` loop for automated repair verification.

## License

[MIT](LICENSE)
