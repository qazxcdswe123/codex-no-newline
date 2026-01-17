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

#[test]
fn n0_fixes_added_eof_newline_in_worktree() {
    let repo_dir = unique_temp_dir("git-fix-eof-newline-n0");
    fs::create_dir_all(&repo_dir).unwrap();

    run_git(&repo_dir, &["init"]);
    run_git(&repo_dir, &["config", "user.name", "Test User"]);
    run_git(&repo_dir, &["config", "user.email", "test@example.com"]);

    let file_path = repo_dir.join("a.txt");
    fs::write(&file_path, b"hello").unwrap();
    run_git(&repo_dir, &["add", "a.txt"]);
    run_git(&repo_dir, &["commit", "-m", "add a"]);

    fs::write(&file_path, b"hello\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_git-fix-eof-newline");
    let status = Command::new(bin)
        .current_dir(&repo_dir)
        .args(["--n", "0"])
        .status()
        .unwrap();
    assert!(status.success());

    let bytes = fs::read(&file_path).unwrap();
    assert!(!bytes.ends_with(b"\n"));

    fs::remove_dir_all(&repo_dir).unwrap();
}
