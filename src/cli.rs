use std::path::PathBuf;
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SynthesizerType {
    #[default]
    Heuristic,
    Mock,
    Llm,
}

/// Command-line arguments for reddiff.
#[derive(Parser, Debug, Clone)]
#[command(
    name = "reddiff",
    author,
    version,
    about = "Adversarial diff breaker with zero prose review comments",
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[command(flatten)]
    pub opts: RunOptions,
}

impl Cli {
    pub fn effective_opts(&self) -> &RunOptions {
        match &self.command {
            Some(Commands::Check(opts)) => opts,
            None => &self.opts,
        }
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Inspect diff and synthesize adversarial regression reproducer
    Check(RunOptions),
}

#[derive(Args, Debug, Clone)]
pub struct RunOptions {
    /// Base git reference representing baseline invariant (e.g. main, HEAD~1)
    #[arg(short = 'b', long)]
    pub base: Option<String>,

    /// Target git reference to inspect (commit, branch, or working-tree)
    #[arg(short = 't', long, default_value = "working-tree")]
    pub target: String,

    /// Test runner command (e.g. "python", "node", "cargo test")
    #[arg(short = 'r', long)]
    pub runner: Option<String>,

    /// Execution timeout per test in seconds
    #[arg(long, default_value_t = 30)]
    pub timeout: u64,

    /// Destination file path for synthesized reproducer
    #[arg(short = 'o', long)]
    pub out: Option<PathBuf>,

    /// Synthesis backend to generate candidate tests
    #[arg(short = 's', long, value_enum, default_value = "heuristic")]
    pub synthesizer: SynthesizerType,
}
