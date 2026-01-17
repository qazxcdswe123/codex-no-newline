use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let pid = std::process::id();
    std::env::temp_dir().join(format!("{prefix}-{pid}-{nanos}"))
}

fn run_git(repo_dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(repo_dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

fn run_git_env(repo_dir: &Path, args: &[&str], envs: &HashMap<&str, &str>) {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_dir).args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let status = cmd.status().unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

fn git_stdout(repo_dir: &Path, args: &[&str]) -> Vec<u8> {
    let out = Command::new("git")
        .current_dir(repo_dir)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {:?} failed", args);
    out.stdout
}

#[test]
fn n2_filters_by_author_email() {
    let repo_dir = unique_temp_dir("git-fix-eof-newline-n2-filter");
    fs::create_dir_all(&repo_dir).unwrap();

    run_git(&repo_dir, &["init"]);
    run_git(&repo_dir, &["config", "user.name", "Test User"]);
    run_git(&repo_dir, &["config", "user.email", "test@example.com"]);

    let file_path = repo_dir.join("a.txt");
    fs::write(&file_path, b"x").unwrap();
    run_git(&repo_dir, &["add", "a.txt"]);
    run_git(&repo_dir, &["commit", "-m", "base"]);

    fs::write(&file_path, b"x1\n").unwrap();
    run_git(&repo_dir, &["add", "a.txt"]);
    let mut envs = HashMap::new();
    envs.insert("GIT_AUTHOR_NAME", "Alice");
    envs.insert("GIT_AUTHOR_EMAIL", "alice@example.com");
    run_git_env(&repo_dir, &["commit", "-m", "alice change"], &envs);

    fs::write(&file_path, b"x2\n").unwrap();
    run_git(&repo_dir, &["add", "a.txt"]);
    let mut envs = HashMap::new();
    envs.insert("GIT_AUTHOR_NAME", "Bob");
    envs.insert("GIT_AUTHOR_EMAIL", "bob@example.com");
    run_git_env(&repo_dir, &["commit", "-m", "bob change"], &envs);

    let bin = env!("CARGO_BIN_EXE_git-fix-eof-newline");
    let status = Command::new(bin)
        .current_dir(&repo_dir)
        .args(["--n", "2", "--author-email", "alice@example.com"])
        .status()
        .unwrap();
    assert!(status.success());

    let log = String::from_utf8(git_stdout(
        &repo_dir,
        &["log", "-2", "--format=%H%x00%ae%x00%s"],
    ))
    .unwrap();

    let mut alice_commit = None;
    let mut bob_commit = None;
    for line in log.lines() {
        let mut parts = line.split('\0');
        let hash = parts.next().unwrap_or("").to_string();
        let email = parts.next().unwrap_or("").to_string();
        let subject = parts.next().unwrap_or("").to_string();
        if email.contains("alice@example.com") || subject == "alice change" {
            alice_commit = Some(hash.clone());
        }
        if email.contains("bob@example.com") || subject == "bob change" {
            bob_commit = Some(hash);
        }
    }

    let alice_commit = alice_commit.expect("missing alice commit");
    let bob_commit = bob_commit.expect("missing bob commit");

    let alice_bytes = git_stdout(&repo_dir, &["show", &format!("{alice_commit}:a.txt")]);
    assert!(!alice_bytes.ends_with(b"\n"));

    let bob_bytes = git_stdout(&repo_dir, &["show", &format!("{bob_commit}:a.txt")]);
    assert!(bob_bytes.ends_with(b"\n"));

    fs::remove_dir_all(&repo_dir).unwrap();
}
