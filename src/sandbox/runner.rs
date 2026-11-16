use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};
use crate::error::ReddiffError;

#[derive(Debug, Clone)]
pub struct ExecutionOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub duration: Duration,
}

impl ExecutionOutput {
    pub fn success(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out
    }
}

pub struct ProcessRunner;

impl ProcessRunner {
    /// Executes a command with arguments in a given working directory with timeout and concurrent IO draining.
    pub fn run(
        cmd: &str,
        args: &[&str],
        working_dir: &Path,
        timeout: Duration,
    ) -> Result<ExecutionOutput, ReddiffError> {
        let start = Instant::now();
        let mut child = Command::new(cmd)
            .args(args)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| ReddiffError::Runner(format!("Failed to spawn command '{cmd}': {e}")))?;

        let mut stdout_pipe = child
            .stdout
            .take()
            .ok_or_else(|| ReddiffError::Runner("Failed to capture child stdout pipe".to_string()))?;
        let mut stderr_pipe = child
            .stderr
            .take()
            .ok_or_else(|| ReddiffError::Runner("Failed to capture child stderr pipe".to_string()))?;

        // Concurrently drain stdout and stderr to prevent pipe buffer deadlocks
        let stdout_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stdout_pipe.read_to_end(&mut buf);
            buf
        });

        let stderr_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stderr_pipe.read_to_end(&mut buf);
            buf
        });

        // Poll with try_wait and sleep until timeout or exit
        let poll_interval = Duration::from_millis(20);
        let mut timed_out = false;
        let mut status: Option<ExitStatus> = None;

        loop {
            match child.try_wait() {
                Ok(Some(s)) => {
                    status = Some(s);
                    break;
                }
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        timed_out = true;
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                    std::thread::sleep(poll_interval);
                }
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ReddiffError::Runner(format!("Error polling child process: {e}")));
                }
            }
        }

        // Reader threads will now hit EOF because the child is terminated / exited
        let stdout_bytes: Vec<u8> = stdout_thread.join().unwrap_or_default();
        let stderr_bytes: Vec<u8> = stderr_thread.join().unwrap_or_default();

        let duration = start.elapsed();
        let exit_code = status.and_then(|s| s.code());

        Ok(ExecutionOutput {
            exit_code,
            stdout: String::from_utf8_lossy(&stdout_bytes).into_owned(),
            stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
            timed_out,
            duration,
        })
    }

    /// Runs a test script file using the configured runner string (e.g. "python", "node", "bash").
    pub fn run_script(
        runner_str: &str,
        script_path: &Path,
        working_dir: &Path,
        timeout: Duration,
    ) -> Result<ExecutionOutput, ReddiffError> {
        let tokens: Vec<&str> = runner_str.split_whitespace().collect();
        if tokens.is_empty() {
            return Err(ReddiffError::Runner("Empty runner command specified".to_string()));
        }

        let cmd = tokens[0];
        let mut args: Vec<&str> = tokens[1..].to_vec();

        let path_str = script_path
            .to_str()
            .ok_or_else(|| ReddiffError::Runner("Script path is not valid UTF-8".to_string()))?;
        args.push(path_str);

        Self::run(cmd, &args, working_dir, timeout)
    }
}
