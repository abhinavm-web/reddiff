# reddiff

Differential diff breaker that emits zero prose review comments and outputs only verified runnable regression reproducers.

[![CI](https://github.com/abhinavm-web/reddiff/actions/workflows/ci.yml/badge.svg)](https://github.com/abhinavm-web/reddiff/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/abhinavm-web/reddiff)](https://github.com/abhinavm-web/reddiff/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](Cargo.toml)

<p align="center">
  <img src="assets/demo.svg" width="860" alt="reddiff terminal demo">
</p>

## Overview

Automated PR review bots often produce speculative prose comments with high false-positive rates. `reddiff` enforces a binary output contract:

`reddiff` writes nothing to `stdout` (`exit 0`) unless it synthesizes and verifies an executable test script that **passes twice consecutively on the base git ref (`main` / `HEAD~1`)** and **fails twice consecutively on the target tree (`working-tree` / `HEAD`)**.

```text
  git diff (Base vs Target)
         |
         v
  +------------------------------------------+
  |         reddiff Verification Loop        |
  |  1. Parse unified diff and changed hunks |
  |  2. Materialize isolated git worktrees   |
  |  3. Synthesize candidate test harnesses  |
  |  4. 2x Base PASS + 2x Target FAIL + shrink|
  +------------------+-----------------------+
                     |
          +----------+----------+
          v                     v
    No Regression       Differential Failure Verified
   (0 bytes stdout,     (Writes minimal reproducer &
     exit code 0)        prints path + exit code 1)
```

## How It Works

1. **Unified Diff Inspection**: Extracts modified files, added/removed lines, and hunk coordinates between `--base` (e.g., `main` or `HEAD~1`) and `--target` (`working-tree` or a commit/branch).
2. **Isolated Dual-Worktree Execution**: Materializes temporary isolated git worktrees for the baseline and target trees. Child runners execute in a scrubbed environment (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `REDDIFF_API_KEY`, `GITHUB_TOKEN`, `AWS_*` stripped; `TZ=UTC`, `PYTHONHASHSEED=0`, isolated `HOME`/`TMPDIR`).
3. **Adversarial Candidate Synthesis**: Defaults to `auto`, which combines LLM-synthesized test candidates (when API keys are set) with offline heuristic detectors (boundary comparisons, zero-division guard removals, boolean flips, arithmetic mutations).
4. **2-Run Determinism Gate and Shrinking**: Candidates must pass 2 consecutive runs on Base (`exit 0`) and fail 2 consecutive runs on Target (`exit != 0`). Confirmed regressions are line-delta minimized, written to disk, and reported via file path on `stdout` (`exit 1`).

## Installation

Install prebuilt binaries, download an archive from [GitHub Releases](https://github.com/abhinavm-web/reddiff/releases/latest), or build from source:

```bash
# macOS (Apple Silicon) and Linux (x86_64)
curl -fsSL https://raw.githubusercontent.com/abhinavm-web/reddiff/main/install.sh | sh
```

```powershell
# Windows (PowerShell)
irm https://raw.githubusercontent.com/abhinavm-web/reddiff/main/install.ps1 | iex
```

From source:

```bash
git clone https://github.com/abhinavm-web/reddiff.git
cd reddiff
cargo run --example showcase
cargo install --path .
```

## Usage

### Inspect uncommitted changes or branch diffs

```bash
# Check working tree against HEAD (exits 0 silently if no regression is proven)
reddiff check

# Compare against main and write verified reproducer to .reddiff/poc.py
reddiff check --base main --target working-tree --out .reddiff/poc.py
```

### Specify repository, runner, or synthesis backend

```bash
reddiff check -C /path/to/repo --base HEAD~1 --runner "python3" --timeout 15
reddiff check --base main --synthesizer llm --out reproducer.py
```

### Exit codes and output contract

| Outcome | Exit Code | `stdout` |
| :--- | :--- | :--- |
| Clean diff or no verified PoC | `0` | Empty (`0 bytes`) |
| Verified regression PoC (2x Base PASS, 2x Target FAIL) | `1` | Reproducer file path only |
| Git or operational error | `2` | Error diagnostic on `stderr` |

## Benchmarks

Reproduce locally:

```bash
cargo run --release --example benchmark
```

| Benchmark Check | Measured Result |
| :--- | :--- |
| Regression cases caught (boundary, zero-div, bool flip, arithmetic) | 4 / 4 (100% recall) |
| Clean refactoring diffs (`0 bytes stdout`, exit `0`) | 0 / 2 false positives (0.0%) |
| Simulated flaky test rejection (2-run determinism gate) | 100% rejected |
| Host credential isolation (`OPENAI_API_KEY` scrubbed) | Verified |

## Architecture

```text
src/
├── cli.rs              # Clap CLI flags (check, -C/--repo, --base, --target, --runner, --out, --synthesizer)
├── error.rs            # Typed error definitions (ReddiffError)
├── diff/
│   ├── git.rs          # Git diff extraction between refs and working tree
│   └── parser.rs       # Unified diff parser (FileDiff, DiffHunk, LineChange)
├── sandbox/
│   ├── worktree.rs     # Ephemeral git worktree materialization
│   └── runner.rs       # Timeout-enforced, env-scrubbed subprocess execution harness
├── synth/
│   ├── heuristic.rs    # Boundary, zero-division, boolean, and arithmetic candidate generator
│   ├── llm.rs          # OpenAI/Anthropic-compatible adversarial script synthesizer
│   ├── prompt.rs       # Diff-to-reproducer prompt builder
│   └── mock.rs         # Deterministic synthesizer for integration test verification
└── verifier/
    ├── mod.rs          # 2-run Base-PASS / Target-FAIL differential gatekeeper
    └── shrinker.rs     # Delta-debugging script minimizer
```

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

## See Also

| Repository | Description |
| :--- | :--- |
| **[agent-fork](https://github.com/abhinavm-web/agent-fork)** | Workspace checkpointing, rewind, parallel worktrees, and cassette recording |
| **[ctx-firewall](https://github.com/abhinavm-web/ctx-firewall)** | Active LLM context proxy and terminal profiler with KV-cache prefix pinning |
| **[diffmem](https://github.com/abhinavm-web/diffmem)** | Git diff-of-diffs episodic memory daemon and editor rule sync |
| **[jitpipe](https://github.com/abhinavm-web/jitpipe)** | Sample-driven compiled Unix pipes (`sgrep`, `sawk`, `sjq`) |

## License

[MIT](LICENSE)
