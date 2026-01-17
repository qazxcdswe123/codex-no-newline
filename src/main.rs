use codex_no_newline::{added_eof_newline, strip_one_trailing_newline};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

#[derive(Debug, Clone)]
struct Args {
    n: usize,
    dry_run: bool,
    in_rebase: bool,
    in_filter_branch: bool,
    author_name: Option<String>,
    author_email: Option<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args = parse_args(std::env::args_os().collect())?;

    ensure_in_git_worktree()?;

    if args.in_filter_branch {
        return run_filter_branch_step(&args);
    }

    match (args.n, args.in_rebase) {
        (0, false) => run_n0(&args),
        (0, true) => Err("--in-rebase cannot be used with --n 0".to_string()),
        (1, _) => run_n1(&args),
        (_, true) => Err("--in-rebase can only be used with --n 1".to_string()),
        _ => run_n_gt1(&args),
    }
}

fn parse_args(argv: Vec<std::ffi::OsString>) -> Result<Args, String> {
    let mut args = Args {
        n: 1,
        dry_run: false,
        in_rebase: false,
        in_filter_branch: false,
        author_name: None,
        author_email: None,
    };

    let _bin = argv.get(0).cloned();
    let mut i = 1;
    while i < argv.len() {
        let a = argv[i].to_string_lossy().to_string();
        match a.as_str() {
            "--n" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| "--n requires an integer argument".to_string())?
                    .to_string_lossy()
                    .to_string();
                args.n = v
                    .parse::<usize>()
                    .map_err(|_| format!("invalid --n value: {v}"))?;
                i += 2;
            }
            "--dry-run" => {
                args.dry_run = true;
                i += 1;
            }
            "--in-rebase" => {
                args.in_rebase = true;
                i += 1;
            }
            "--in-filter-branch" => {
                args.in_filter_branch = true;
                i += 1;
            }
            "--author-name" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| "--author-name requires a value".to_string())?
                    .to_string_lossy()
                    .to_string();
                args.author_name = Some(v);
                i += 2;
            }
            "--author-email" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| "--author-email requires a value".to_string())?
                    .to_string_lossy()
                    .to_string();
                args.author_email = Some(v);
                i += 2;
            }
            "--help" | "-h" => {
                return Err(usage());
            }
            other => {
                return Err(format!("unknown argument: {other}\n\n{}", usage()));
            }
        }
    }

    Ok(args)
}

fn usage() -> String {
    [
        "Usage:",
        "  git-fix-eof-newline [--n <int>] [--dry-run] [--author-name <substr>] [--author-email <substr>]",
        "",
        "Options:",
        "  --n <int>           Check the last n commits (0 = uncommitted diff; default 1)",
        "  --dry-run           Print what would change without modifying anything",
        "  --in-filter-branch  Internal: run as git filter-branch tree-filter",
        "  --author-name <s>   Only process commits whose author name contains s",
        "  --author-email <s>  Only process commits whose author email contains s",
    ]
    .join("\n")
}

fn ensure_in_git_worktree() -> Result<(), String> {
    let out = git_output(&["rev-parse", "--is-inside-work-tree"])?;
    if out.trim() != "true" {
        return Err("not inside a git worktree".to_string());
    }
    Ok(())
}

fn git_output(args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git {:?} failed: {}", args, stderr.trim()));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("git output was not valid UTF-8: {e}"))
}

fn git_output_bytes(args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git {:?} failed: {}", args, stderr.trim()));
    }
    Ok(out.stdout)
}

fn paths_from_zbytes(zbytes: &[u8]) -> Vec<PathBuf> {
    zbytes
        .split(|b| *b == 0u8)
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(String::from_utf8_lossy(s).to_string()))
        .collect()
}

fn run_n0(args: &Args) -> Result<(), String> {
    let unstaged = paths_from_zbytes(&git_output_bytes(&["diff", "--name-only", "-z"])?);
    let staged = paths_from_zbytes(&git_output_bytes(&[
        "diff",
        "--cached",
        "--name-only",
        "-z",
    ])?);

    let unstaged_set: BTreeSet<PathBuf> = unstaged.into_iter().collect();
    let staged_set: BTreeSet<PathBuf> = staged.into_iter().collect();

    let partial: Vec<PathBuf> = unstaged_set.intersection(&staged_set).cloned().collect();
    for p in partial {
        eprintln!(
            "skipping partially-staged file: {}",
            p.as_os_str().to_string_lossy()
        );
    }

    let mut handled_any = false;

    for p in unstaged_set.difference(&staged_set) {
        if fix_path_against_head(p, FixTarget::Worktree, args.dry_run)? {
            handled_any = true;
        }
    }

    for p in staged_set.difference(&unstaged_set) {
        if fix_path_against_head(p, FixTarget::Index, args.dry_run)? {
            handled_any = true;
        }
    }

    if args.dry_run && handled_any {
        return Ok(());
    }

    Ok(())
}

enum FixTarget {
    Worktree,
    Index,
}

fn fix_path_against_head(path: &Path, target: FixTarget, dry_run: bool) -> Result<bool, String> {
    let head_oid = rev_parse_oid(&format!("HEAD:{}", path.as_os_str().to_string_lossy()))?;
    let old_bytes = blob_bytes_limited(&head_oid)?;

    let new_bytes = match target {
        FixTarget::Worktree => match fs::read(path) {
            Ok(b) => b,
            Err(_) => return Ok(false),
        },
        FixTarget::Index => {
            let idx_oid = rev_parse_oid(&format!(":{}", path.display()))?;
            blob_bytes_limited(&idx_oid)?
        }
    };

    if !added_eof_newline(&old_bytes, &new_bytes) {
        return Ok(false);
    }

    if dry_run {
        let label = match target {
            FixTarget::Worktree => "worktree",
            FixTarget::Index => "index",
        };
        println!(
            "n=0 match ({label}): {}",
            path.as_os_str().to_string_lossy()
        );
        return Ok(true);
    }

    match target {
        FixTarget::Worktree => strip_worktree_file(path),
        FixTarget::Index => {
            strip_worktree_file(path)?;
            git_add_path(path)?;
            Ok(())
        }
    }?;

    Ok(true)
}

fn strip_worktree_file(path: &Path) -> Result<(), String> {
    let mut bytes =
        fs::read(path).map_err(|e| format!("failed to read file {}: {e}", path.display()))?;
    if !strip_one_trailing_newline(&mut bytes) {
        return Ok(());
    }
    fs::write(path, bytes).map_err(|e| format!("failed to write file {}: {e}", path.display()))?;
    Ok(())
}

fn git_add_path(path: &Path) -> Result<(), String> {
    let status = Command::new("git")
        .args(["add", "--"])
        .arg(path)
        .status()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !status.success() {
        return Err(format!("git add failed: {}", path.display()));
    }
    Ok(())
}

fn run_n1(args: &Args) -> Result<(), String> {
    if !args.in_rebase {
        ensure_clean_worktree()?;
    } else if args.n != 1 {
        return Err("--in-rebase can only be used with --n 1".to_string());
    }

    if !commit_matches_author_filter("HEAD", args)? {
        return Ok(());
    }

    let (head, parent) = head_and_first_parent()?;
    let changed = changed_paths_in_commit(&head)?;

    let mut paths_to_fix: Vec<PathBuf> = Vec::new();
    for path in changed {
        let old_oid = match rev_parse_oid(&format!("{parent}:{}", path.display())) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let new_oid = match rev_parse_oid(&format!("{head}:{}", path.display())) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let old_bytes = match blob_bytes_limited(&old_oid) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let new_bytes = match blob_bytes_limited(&new_oid) {
            Ok(b) => b,
            Err(_) => continue,
        };

        if added_eof_newline(&old_bytes, &new_bytes) {
            paths_to_fix.push(path);
        }
    }

    if paths_to_fix.is_empty() {
        return Ok(());
    }

    for path in &paths_to_fix {
        if args.dry_run {
            println!("n=1 match: {}", path.display());
            continue;
        }
        strip_worktree_file(path)?;
        git_add_path(path)?;
    }

    if args.dry_run {
        return Ok(());
    }

    let status = Command::new("git")
        .args(["commit", "--amend", "--no-edit", "--allow-empty"])
        .status()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !status.success() {
        return Err("git commit --amend failed".to_string());
    }

    Ok(())
}

fn run_filter_branch_step(args: &Args) -> Result<(), String> {
    if args.n != 1 {
        return Err("--in-filter-branch can only be used with --n 1".to_string());
    }
    let commit = filter_branch_commit();

    if !commit_matches_author_filter(&commit, args)? {
        return Ok(());
    }
    let parent = first_parent_of_commit(&commit)?;
    let changed = changed_paths_in_commit(&commit)?;

    let mut changed_any = false;
    for path in changed {
        let old_oid = match rev_parse_oid(&format!("{parent}:{}", path.display())) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let old_bytes = match blob_bytes_limited(&old_oid) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let new_bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };

        if added_eof_newline(&old_bytes, &new_bytes) {
            if !args.dry_run {
                strip_worktree_file(&path)?;
                changed_any = true;
            }
        }
    }

    if changed_any {
        let status = Command::new("git")
            .args(["add", "-A"])
            .status()
            .map_err(|e| format!("failed to run git: {e}"))?;
        if !status.success() {
            return Err("git add -A failed".to_string());
        }
    }

    Ok(())
}

fn filter_branch_commit() -> String {
    std::env::var("GIT_COMMIT").unwrap_or_else(|_| "HEAD".to_string())
}

fn ensure_clean_worktree() -> Result<(), String> {
    let out = git_output(&["status", "--porcelain"])?;
    if !out.trim().is_empty() {
        return Err("working tree is not clean; refusing to amend commits".to_string());
    }
    Ok(())
}

fn head_and_first_parent() -> Result<(String, String), String> {
    let out = git_output(&["rev-list", "--parents", "-n", "1", "HEAD"])?;
    let mut parts = out.split_whitespace();
    let head = parts
        .next()
        .ok_or_else(|| "failed to parse HEAD".to_string())?
        .to_string();
    let parent = parts
        .next()
        .ok_or_else(|| "HEAD has no parent (cannot run --n 1 on an initial commit)".to_string())?
        .to_string();
    Ok((head, parent))
}

fn changed_paths_in_commit(commit: &str) -> Result<Vec<PathBuf>, String> {
    let out = git_output(&["diff-tree", "--no-commit-id", "--name-status", "-r", commit])?;
    let mut paths = Vec::new();
    for line in out.lines() {
        let mut parts = line.split('\t');
        let status = match parts.next() {
            Some(s) => s,
            None => continue,
        };
        if status != "M" {
            continue;
        }
        if let Some(path) = parts.next() {
            paths.push(PathBuf::from(path));
        }
    }
    Ok(paths)
}

fn commit_matches_author_filter(commit: &str, args: &Args) -> Result<bool, String> {
    if args.author_name.is_none() && args.author_email.is_none() {
        return Ok(true);
    }
    let out = git_output(&["show", "-s", "--format=%an%x00%ae", commit])?;
    let mut parts = out.split('\0');
    let name = parts.next().unwrap_or("").trim();
    let email = parts.next().unwrap_or("").trim();

    if let Some(needle) = &args.author_name {
        if !name.to_lowercase().contains(&needle.to_lowercase()) {
            return Ok(false);
        }
    }
    if let Some(needle) = &args.author_email {
        if !email.to_lowercase().contains(&needle.to_lowercase()) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn rev_parse_oid(spec: &str) -> Result<String, String> {
    Ok(git_output(&["rev-parse", spec])?.trim().to_string())
}

fn blob_bytes_limited(oid: &str) -> Result<Vec<u8>, String> {
    let size_s = git_output(&["cat-file", "-s", oid])?;
    let size: u64 = size_s
        .trim()
        .parse()
        .map_err(|_| format!("failed to parse blob size: {}", size_s.trim()))?;
    if size > 10_000_000 {
        return Err(format!("blob too large, skipping: {oid} ({size} bytes)"));
    }
    git_output_bytes(&["cat-file", "-p", oid])
}

fn run_n_gt1(args: &Args) -> Result<(), String> {
    if args.n == 0 {
        return Err("internal error: run_n_gt1 received --n 0".to_string());
    }
    if args.n == 1 {
        return run_n1(args);
    }

    ensure_clean_worktree()?;
    ensure_not_in_rebase()?;

    let commits = recent_first_parent_commits(args.n)?;

    let mut needs_fix: Vec<String> = Vec::new();
    for commit in &commits {
        if !commit_matches_author_filter(commit, args)? {
            continue;
        }
        if commit_has_added_eof_newline(commit)? {
            needs_fix.push(commit.clone());
        }
    }

    if needs_fix.is_empty() {
        return Ok(());
    }

    let earliest = needs_fix
        .first()
        .ok_or_else(|| "internal error: needs_fix is empty".to_string())?;
    let base = first_parent_of_commit(earliest)?;

    if args.dry_run {
        println!("will run filter-branch starting at base: {base}");
        for c in needs_fix {
            println!("n>1 match commit: {c}");
        }
        return Ok(());
    }

    let tree_filter_cmd = build_filter_branch_tree_filter_command(args)?;
    let rev_range = format!("{base}..HEAD");
    let status = Command::new("git")
        .args([
            "filter-branch",
            "-f",
            "--prune-empty",
            "--tree-filter",
            &tree_filter_cmd,
            &rev_range,
        ])
        .env("FILTER_BRANCH_SQUELCH_WARNING", "1")
        .status()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !status.success() {
        return Err("git filter-branch failed".to_string());
    }

    Ok(())
}

fn ensure_not_in_rebase() -> Result<(), String> {
    let rebase_apply = git_output(&["rev-parse", "--git-path", "rebase-apply"])?;
    let rebase_merge = git_output(&["rev-parse", "--git-path", "rebase-merge"])?;
    let apply_path = PathBuf::from(rebase_apply.trim());
    let merge_path = PathBuf::from(rebase_merge.trim());
    if apply_path.exists() || merge_path.exists() {
        return Err("detected an ongoing rebase; refusing to start another rebase".to_string());
    }
    Ok(())
}

fn recent_first_parent_commits(n: usize) -> Result<Vec<String>, String> {
    let out = git_output(&["rev-list", "--first-parent", "-n", &n.to_string(), "HEAD"])?;
    let mut commits: Vec<String> = out
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect();
    commits.reverse();
    Ok(commits)
}

fn commit_has_added_eof_newline(commit: &str) -> Result<bool, String> {
    let parent = first_parent_of_commit(commit)?;
    let changed = changed_paths_in_commit(commit)?;
    for path in changed {
        let old_oid = match rev_parse_oid(&format!("{parent}:{}", path.display())) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let new_oid = match rev_parse_oid(&format!("{commit}:{}", path.display())) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let old_bytes = match blob_bytes_limited(&old_oid) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let new_bytes = match blob_bytes_limited(&new_oid) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if added_eof_newline(&old_bytes, &new_bytes) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn first_parent_of_commit(commit: &str) -> Result<String, String> {
    let out = git_output(&["rev-list", "--parents", "-n", "1", commit])?;
    let parts: Vec<&str> = out.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(format!("{commit} has no parent"));
    }
    if parts.len() > 2 {
        return Err(format!("{commit} is a merge commit; not supported"));
    }
    Ok(parts[1].to_string())
}

fn build_filter_branch_tree_filter_command(args: &Args) -> Result<String, String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("failed to locate current executable: {e}"))?;
    let exe_s = exe.to_string_lossy().to_string();
    let mut parts: Vec<String> = Vec::new();
    parts.push(sh_quote(&exe_s));
    parts.push("--in-filter-branch".to_string());
    parts.push("--n".to_string());
    parts.push("1".to_string());

    if let Some(v) = &args.author_name {
        parts.push("--author-name".to_string());
        parts.push(sh_quote(v));
    }
    if let Some(v) = &args.author_email {
        parts.push("--author-email".to_string());
        parts.push(sh_quote(v));
    }

    Ok(parts.join(" "))
}

fn sh_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let mut out = String::from("'");
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}
