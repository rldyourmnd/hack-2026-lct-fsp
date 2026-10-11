//! Public repositories cloned on the server.
//!
//! A blobless clone (`--filter=blob:none`) of the default branch holds every commit and tree but
//! no file contents, so the full history, author time zones, changed paths and the current tree
//! are available at a fraction of a full clone. Git transfers are not counted against the API
//! limits. Clones live in a scratch directory and are removed after the analysis.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use chrono::DateTime;
use tokio::process::Command;

use crate::snapshot::{GitCommit, RepositoryAnalysis};

const FIELD: char = '\u{1f}';
const RECORD: char = '\u{1e}';

/// Most commits read per repository; older history beyond this is not needed for the heuristics.
const MAX_HISTORY: &str = "--max-count=50000";
const MAX_FILES: usize = 100_000;

pub struct Git {
    pub base_url: String,
    pub scratch: PathBuf,
    pub clone_timeout: Duration,
}

impl Git {
    /// Clones and reads one repository; the clone is removed afterwards.
    pub async fn analyze(
        &self,
        full_name: &str,
        emails: &BTreeSet<String>,
    ) -> Option<RepositoryAnalysis> {
        let directory = self.scratch.join(full_name.replace('/', "__"));
        let _ = tokio::fs::remove_dir_all(&directory).await;
        let url = format!("{}/{full_name}.git", self.base_url);
        let cloned = run(
            Command::new("git")
                .args([
                    "clone",
                    "--bare",
                    "--quiet",
                    "--filter=blob:none",
                    "--single-branch",
                    "--no-tags",
                ])
                .arg(&url)
                .arg(&directory)
                .env("GIT_TERMINAL_PROMPT", "0"),
            self.clone_timeout,
        )
        .await;
        let result = match cloned {
            Some(_) => read(&directory, full_name, emails).await,
            None => None,
        };
        let _ = tokio::fs::remove_dir_all(&directory).await;
        result
    }
}

async fn read(
    directory: &Path,
    full_name: &str,
    emails: &BTreeSet<String>,
) -> Option<RepositoryAnalysis> {
    let format = format!("--format={RECORD}%H{FIELD}%ae{FIELD}%aI{FIELD}%ce{FIELD}%P{FIELD}%s");
    let log = run(
        Command::new("git")
            .arg("-C")
            .arg(directory)
            // Rename detection would download file contents into the blobless clone.
            .args([
                "log",
                MAX_HISTORY,
                "--no-renames",
                "--name-only",
                &format,
                "HEAD",
            ]),
        Duration::from_secs(120),
    )
    .await?;
    let mut analysis = RepositoryAnalysis {
        full_name: full_name.into(),
        ..RepositoryAnalysis::default()
    };
    for record in log.split(RECORD).filter(|record| !record.trim().is_empty()) {
        analysis.total_commits += 1;
        let (header, files) = record.split_once('\n').unwrap_or((record, ""));
        let fields = header.split(FIELD).collect::<Vec<_>>();
        let [sha, author, authored, committer, parents, subject] = fields[..] else {
            continue;
        };
        if !emails.contains(&author.to_ascii_lowercase()) {
            continue;
        }
        let Ok(authored_at) = DateTime::parse_from_rfc3339(authored) else {
            continue;
        };
        let parents = u32::try_from(parents.split_whitespace().count()).unwrap_or(0);
        if parents <= 1 && emails.contains(&committer.to_ascii_lowercase()) {
            analysis.direct_pushes += 1;
        }
        analysis.commits.push(GitCommit {
            sha: sha.into(),
            authored_at,
            parents,
            subject: subject.into(),
            files: files
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect(),
        });
    }
    let tree = run(
        Command::new("git")
            .arg("-C")
            .arg(directory)
            .args(["ls-tree", "-r", "--name-only", "HEAD"]),
        Duration::from_secs(60),
    )
    .await
    .unwrap_or_default();
    analysis.files = tree.lines().take(MAX_FILES).map(str::to_owned).collect();
    Some(analysis)
}

/// Runs a command with a timeout; `None` on failure, a non-zero exit or the timeout.
async fn run(command: &mut Command, timeout: Duration) -> Option<String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .ok()?
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn git(directory: &Path, args: &[&str], env: &[(&str, &str)]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(directory)
            .args(args)
            .envs(env.iter().copied())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    #[tokio::test]
    async fn history_keeps_author_offsets_paths_and_direct_pushes() {
        let root = std::env::temp_dir().join(format!("fsp-git-test-{}", std::process::id()));
        let source = root.join("source/dev/service");
        tokio::fs::create_dir_all(&source).await.unwrap();
        git(&source, &["init", "--quiet", "--initial-branch=main"], &[]).await;
        tokio::fs::create_dir_all(source.join("src")).await.unwrap();
        tokio::fs::write(source.join("src/main.rs"), "fn main() {}\n")
            .await
            .unwrap();
        git(&source, &["add", "."], &[]).await;
        let mine = [
            ("GIT_AUTHOR_NAME", "Dev"),
            ("GIT_AUTHOR_EMAIL", "Dev@Example.com"),
            ("GIT_COMMITTER_NAME", "Dev"),
            ("GIT_COMMITTER_EMAIL", "dev@example.com"),
            ("GIT_AUTHOR_DATE", "2026-10-09T10:15:00+05:00"),
            ("GIT_COMMITTER_DATE", "2026-10-09T10:15:00+05:00"),
        ];
        git(&source, &["commit", "--quiet", "-m", "feat: start"], &mine).await;
        tokio::fs::write(source.join("README.md"), "# Service\n")
            .await
            .unwrap();
        git(&source, &["add", "."], &[]).await;
        let other = [
            ("GIT_AUTHOR_NAME", "Other"),
            ("GIT_AUTHOR_EMAIL", "other@example.com"),
            ("GIT_COMMITTER_NAME", "Other"),
            ("GIT_COMMITTER_EMAIL", "other@example.com"),
        ];
        git(
            &source,
            &["commit", "--quiet", "-m", "docs: readme"],
            &other,
        )
        .await;
        let tool = Git {
            base_url: format!("file://{}", root.join("source").display()),
            scratch: root.join("scratch"),
            clone_timeout: Duration::from_secs(30),
        };
        // `git clone` accepts a plain directory path through the file transport.
        tokio::fs::rename(&source, root.join("source/dev/service.git"))
            .await
            .unwrap();
        let emails = BTreeSet::from(["dev@example.com".to_owned()]);
        let analysis = tool.analyze("dev/service", &emails).await.unwrap();
        assert_eq!(analysis.total_commits, 2);
        assert_eq!(analysis.direct_pushes, 1);
        assert_eq!(analysis.commits.len(), 1);
        let commit = &analysis.commits[0];
        assert_eq!(commit.authored_at.offset().local_minus_utc(), 5 * 3600);
        assert_eq!(commit.files, ["src/main.rs"]);
        assert_eq!(analysis.files, ["README.md", "src/main.rs"]);
        assert!(!root.join("scratch/dev__service").exists());
        let _ = tokio::fs::remove_dir_all(&root).await;
    }
}
