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
fn n1_amends_head_to_remove_added_eof_newline() {
    let repo_dir = unique_temp_dir("git-fix-eof-newline-n1");
    fs::create_dir_all(&repo_dir).unwrap();

    run_git(&repo_dir, &["init"]);
    run_git(&repo_dir, &["config", "user.name", "Test User"]);
    run_git(&repo_dir, &["config", "user.email", "test@example.com"]);

    let file_path = repo_dir.join("a.txt");
    fs::write(&file_path, b"hello").unwrap();
    run_git(&repo_dir, &["add", "a.txt"]);
    run_git(&repo_dir, &["commit", "-m", "add a"]);

    fs::write(&file_path, b"hello\n").unwrap();
    run_git(&repo_dir, &["add", "a.txt"]);
    run_git(&repo_dir, &["commit", "-m", "add eof newline"]);

    let old_head = String::from_utf8(git_stdout(&repo_dir, &["rev-parse", "HEAD"]))
        .unwrap()
        .trim()
        .to_string();

    let bin = env!("CARGO_BIN_EXE_git-fix-eof-newline");
    let status = Command::new(bin)
        .current_dir(&repo_dir)
        .args(["--n", "1"])
        .status()
        .unwrap();
    assert!(status.success());

    let new_head = String::from_utf8(git_stdout(&repo_dir, &["rev-parse", "HEAD"]))
        .unwrap()
        .trim()
        .to_string();
    assert_ne!(old_head, new_head);

    let bytes = git_stdout(&repo_dir, &["show", "HEAD:a.txt"]);
    assert_eq!(bytes, b"hello");

    fs::remove_dir_all(&repo_dir).unwrap();
}
