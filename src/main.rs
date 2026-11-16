#![deny(clippy::all)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

use clap::Parser;
use reddiff::cli::Cli;

fn main() {
    // Tracing subscriber sends diagnostic logs strictly to stderr
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            if e.use_stderr() {
                eprintln!("{e}");
                std::process::exit(2);
            } else {
                print!("{e}");
                std::process::exit(0);
            }
        }
    };

    let opts = cli.effective_opts();
    let current_dir = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: failed to get current working directory: {e}");
            std::process::exit(2);
        }
    };

    let repo_root = match reddiff::diff::GitClient::toplevel(&current_dir) {
        Ok(r) => r,
        Err(_) => current_dir.clone(),
    };

    match reddiff::run(opts, &current_dir) {
        Ok(Some(reproducer_path)) => {
            // Proven regression!
            // Format relative path if inside repo_root, else absolute
            let display_path = match reproducer_path.strip_prefix(&repo_root) {
                Ok(rel) => rel.display().to_string(),
                Err(_) => reproducer_path.display().to_string(),
            };

            // Zero-prose invariant: output strictly the path, zero conversational commentary
            println!("{display_path}");
            std::process::exit(1);
        }
        Ok(None) => {
            // Clean diff or no regression proven: exactly 0 bytes to stdout and exit 0
            std::process::exit(0);
        }
        Err(err) => {
            // Operational error: emit strictly to stderr and exit 2
            eprintln!("error: {err}");
            std::process::exit(2);
        }
    }
}
