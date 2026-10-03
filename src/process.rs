//! All target/tool arguments are passed as argv, never shell programs.
use anyhow::{Context, Result, ensure};
use std::{
    fs::File,
    io::{Read, Seek},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn command(program: &str, args: &[String], cwd: Option<&Path>) -> Command {
    let mut c = Command::new(program);
    c.args(args).stdin(Stdio::null());
    if let Some(p) = cwd {
        c.current_dir(p);
    }
    if program != "gh" {
        for key in [
            "GH_TOKEN",
            "GITHUB_TOKEN",
            "GITHUB_OAUTH_TOKEN",
            "GITHUB_TOKEN_CMD",
            "ATTIC_TOKEN",
            "CACHIX_AUTH_TOKEN",
            "CACHIX_SIGNING_KEY",
            "NIX_CONFIG",
            "NIX_PATH",
            "NIX_USER_CONF_FILES",
        ] {
            c.env_remove(key);
        }
    }
    c.env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
    c
}
pub fn run(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    log: Option<&Path>,
) -> Result<String> {
    run_command(command(program, args, cwd), log)
}

pub fn run_command(mut c: Command, log: Option<&Path>) -> Result<String> {
    let program = c.get_program().to_string_lossy().into_owned();
    let mut stdout = tempfile::tempfile()?;
    let stderr = if let Some(p) = log {
        File::create(p)?
    } else {
        tempfile::tempfile()?
    };
    c.stdout(Stdio::from(stdout.try_clone()?))
        .stderr(Stdio::from(stderr.try_clone()?));
    let mut child = c
        .spawn()
        .with_context(|| format!("cannot start {program}"))?;
    let timeout = if program == "gh" {
        60
    } else if program == "git" {
        300
    } else {
        2100
    };
    let deadline = Instant::now() + Duration::from_secs(timeout);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline
            || stdout.metadata()?.len() > 16 * 1024 * 1024
            || stderr.metadata()?.len() > 64 * 1024 * 1024
        {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("{program} exceeded time/output limits");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    // Tool stderr can contain target-controlled strings or credentials: never echo it in a privileged job.
    ensure!(
        status.success(),
        "{program} failed ({:?}); see retained tool log",
        status.code()
    );
    ensure!(
        stdout.metadata()?.len() <= 16 * 1024 * 1024,
        "tool output too large"
    );
    stdout.rewind()?;
    let mut text = String::new();
    stdout.read_to_string(&mut text)?;
    Ok(text)
}
pub fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| (*s).into()).collect()
}
pub fn nix_args(values: &[&str]) -> Vec<String> {
    let mut a = args(values);
    a.extend(args(&[
        "--option",
        "accept-flake-config",
        "false",
        "--option",
        "allow-import-from-derivation",
        "false",
        "--option",
        "builders",
        "",
        "--option",
        "max-jobs",
        "2",
        "--option",
        "cores",
        "2",
        "--option",
        "max-silent-time",
        "600",
        "--option",
        "timeout",
        "1800",
        "--option",
        "eval-cache",
        "false",
    ]));
    a
}
