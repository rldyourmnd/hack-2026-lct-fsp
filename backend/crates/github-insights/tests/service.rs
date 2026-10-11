//! The service end to end without the network: a local fake of GitHub's REST, search and raw
//! endpoints, and public repositories cloned from local directories over `file://`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path as UrlPath, Query};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use fsp_github_insights::client::GitHubClient;
use fsp_github_insights::collect::{Collector, Limits};
use fsp_github_insights::git::Git;
use fsp_github_insights::{InsightsState, router};
use serde_json::{Value, json};
use tokio::process::Command;

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

/// A public repository with `commits` commits by dev@example.com, served as `{name}.git`.
async fn repository(root: &Path, name: &str, files: &[(&str, &str)], commits: u32) {
    let work = root.join("work").join(name);
    tokio::fs::create_dir_all(&work).await.unwrap();
    git(&work, &["init", "--quiet", "--initial-branch=main"], &[]).await;
    for (path, text) in files {
        let path = work.join(path);
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(path, text).await.unwrap();
    }
    for index in 0..commits {
        tokio::fs::write(work.join("src/lib.rs"), format!("// change {index}\n"))
            .await
            .unwrap();
        git(&work, &["add", "."], &[]).await;
        let date = format!(
            "2026-09-{:02}T10:{:02}:00+05:00",
            1 + index % 28,
            index % 60
        );
        let author = [
            ("GIT_AUTHOR_NAME", "Dev"),
            ("GIT_AUTHOR_EMAIL", "dev@example.com"),
            ("GIT_COMMITTER_NAME", "Dev"),
            ("GIT_COMMITTER_EMAIL", "dev@example.com"),
            ("GIT_AUTHOR_DATE", date.as_str()),
            ("GIT_COMMITTER_DATE", date.as_str()),
        ];
        git(
            &work,
            &["commit", "--quiet", "-m", &format!("feat: change {index}")],
            &author,
        )
        .await;
    }
    let served = root.join("source").join(format!("{name}.git"));
    tokio::fs::create_dir_all(served.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::rename(&work, served).await.unwrap();
}

fn commit_item(repository: &str, organization: bool) -> Value {
    json!({
        "sha": format!("{:040x}", repository.len()),
        "commit": { "author": { "email": "dev@example.com" } },
        "author": { "login": "Dev" },
        "repository": {
            "full_name": repository,
            "private": false,
            "owner": {
                "login": repository.split('/').next().unwrap(),
                "type": if organization { "Organization" } else { "User" },
            },
        },
    })
}

fn repository_item(name: &str, organization: bool, language: &str) -> Value {
    json!({
        "full_name": name,
        "private": false,
        "owner": {
            "login": name.split('/').next().unwrap(),
            "type": if organization { "Organization" } else { "User" },
        },
        "fork": false,
        "archived": false,
        "stargazers_count": 3,
        "forks_count": 0,
        "size": 10,
        "language": language,
        "topics": ["cli"],
        "description": "A tool",
        "license": { "spdx_id": "MIT" },
        "default_branch": "main",
        "pushed_at": "2026-09-28T10:00:00Z",
    })
}

fn pull_item(repository: &str, number: u32, merged: bool) -> Value {
    json!({
        "html_url": format!("https://github.com/{repository}/pull/{number}"),
        "repository_url": format!("https://api.github.com/repos/{repository}"),
        "state": if merged { "closed" } else { "open" },
        "created_at": "2026-09-20T10:00:00Z",
        "pull_request": { "merged_at": merged.then_some("2026-09-21T10:00:00Z") },
    })
}

async fn user(UrlPath(login): UrlPath<String>) -> Response {
    match login.as_str() {
        "dev" | "Dev" => Json(json!({
            "login": "Dev",
            "id": 7,
            "type": "User",
            "created_at": "2020-01-01T00:00:00Z",
            "followers": 3,
            "bio": "Rust developer · fsp-0123456789",
        }))
        .into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn organizations() -> Json<Value> {
    Json(json!([{ "login": "acme" }]))
}

async fn commits(Query(query): Query<HashMap<String, String>>) -> Json<Value> {
    let q = query.get("q").cloned().unwrap_or_default();
    if q.contains("user:acme") {
        return Json(json!({ "total_count": 12, "items": [commit_item("acme/service", true)] }));
    }
    Json(json!({ "total_count": 5, "items": [commit_item("Dev/tool", false)] }))
}

async fn issues(Query(query): Query<HashMap<String, String>>) -> Json<Value> {
    let q = query.get("q").cloned().unwrap_or_default();
    if q.contains("sort:created-desc") {
        return Json(json!({ "total_count": 1, "items": [pull_item("acme/service", 1, true)] }));
    }
    if q.contains("-user:Dev") && q.starts_with("type:pr author:Dev") {
        return Json(json!({ "total_count": 1, "items": [pull_item("other/library", 9, true)] }));
    }
    Json(json!({ "total_count": 2, "items": [] }))
}

async fn repositories() -> Json<Value> {
    Json(json!({
        "total_count": 2,
        "items": [
            repository_item("Dev/tool", false, "Rust"),
            repository_item("acme/service", true, "Rust"),
        ],
    }))
}

async fn raw(UrlPath((owner, name, path)): UrlPath<(String, String, String)>) -> Response {
    match (owner.as_str(), name.as_str(), path.as_str()) {
        ("Dev", "tool", "Cargo.toml") => {
            "[package]\nname = \"tool\"\n\n[dependencies]\nclap = \"4\"\ntokio = \"1\"\n"
                .into_response()
        }
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

async fn insights(root: &Path) -> String {
    let github = serve(
        Router::new()
            .route("/users/{login}", get(user))
            .route("/users/{login}/orgs", get(organizations))
            .route("/search/commits", get(commits))
            .route("/search/issues", get(issues))
            .route("/search/repositories", get(repositories))
            .route("/{owner}/{name}/HEAD/{*path}", get(raw)),
    )
    .await;
    let client = GitHubClient::with_endpoints(None, github.clone(), github)
        .unwrap()
        .with_search_interval(Duration::from_millis(5));
    let collector = Collector {
        client: Arc::new(client),
        git: Arc::new(Git {
            base_url: format!("file://{}", root.join("source").display()),
            scratch: root.join("scratch"),
            clone_timeout: Duration::from_secs(30),
        }),
        limits: Limits::default(),
    };
    serve(router(Arc::new(InsightsState::new(Some(collector))))).await
}

async fn finished(http: &reqwest::Client, base: &str, login: &str, code: &str) -> Value {
    for _ in 0..200 {
        let status: Value = http
            .get(format!("{base}/v1/analyses/{login}?ownershipCode={code}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if matches!(status["state"].as_str(), Some("done" | "failed")) {
            return status;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("the analysis did not finish");
}

fn scratch_root() -> PathBuf {
    std::env::temp_dir().join(format!("fsp-insights-service-{}", std::process::id()))
}

#[tokio::test]
async fn an_account_is_analyzed_in_the_background_from_public_data() {
    let root = scratch_root();
    let _ = tokio::fs::remove_dir_all(&root).await;
    repository(
        &root,
        "Dev/tool",
        &[
            ("Cargo.toml", "[package]\nname = \"tool\"\n"),
            ("README.md", "# Tool\n"),
            (".github/workflows/ci.yml", "on: push\n"),
            ("src/lib.rs", ""),
        ],
        12,
    )
    .await;
    repository(&root, "acme/service", &[("src/lib.rs", "")], 4).await;
    let base = insights(&root).await;
    let http = reqwest::Client::new();

    let queued: Value = http
        .post(format!("{base}/v1/analyses"))
        .json(&json!({ "login": "https://github.com/Dev", "ownershipCode": "fsp-0123456789" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(matches!(
        queued["state"].as_str(),
        Some("queued" | "running")
    ));
    let done = finished(&http, &base, "dev", "fsp-0123456789").await;
    assert_eq!(done["state"], "done", "{done}");
    assert_eq!(done["ownershipCodeFound"], true);
    let report = &done["report"];
    assert_eq!(report["login"], "Dev");
    assert_eq!(
        report["algorithmVersion"],
        fsp_domain::GITHUB_INSIGHTS_VERSION
    );
    assert_eq!(report["coverage"]["repositoriesAnalyzed"], 2);
    let repositories = report["repositories"].as_array().unwrap();
    let tool = repositories
        .iter()
        .find(|repository| repository["nameWithOwner"] == "Dev/tool")
        .unwrap();
    assert_eq!(tool["userCommits"], 12);
    assert_eq!(tool["owned"], true);
    assert!(
        tool["stack"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "clap (CLI)")
    );
    let acme = &report["organizations"]["contributed"][0];
    assert_eq!(acme["login"], "acme");
    assert_eq!(acme["commits"], 12);
    assert_eq!(acme["affiliated"], true);
    assert_eq!(report["rhythm"]["timezoneOffsets"][0]["offsetMinutes"], 300);
    // The clones are removed after the analysis.
    let mut scratch = tokio::fs::read_dir(root.join("scratch")).await.unwrap();
    assert!(scratch.next_entry().await.unwrap().is_none());

    // Another code is not in the bio; a repeated request reuses the fresh analysis.
    let other = finished(&http, &base, "dev", "fsp-9999999999").await;
    assert_eq!(other["ownershipCodeFound"], false);
    let again: Value = http
        .post(format!("{base}/v1/analyses"))
        .json(&json!({ "login": "dev" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(again["state"], "done");

    // A mistyped login is rejected at once instead of after a queued analysis.
    let missing = http
        .post(format!("{base}/v1/analyses"))
        .json(&json!({ "login": "ghost" }))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);
    let missing: Value = missing.json().await.unwrap();
    assert_eq!(missing["code"], "github_account_not_found");

    let invalid = http
        .post(format!("{base}/v1/analyses"))
        .json(&json!({ "login": "not a login" }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
    let unknown = http
        .get(format!("{base}/v1/analyses/nobody"))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown.status(), reqwest::StatusCode::NOT_FOUND);
    let _ = tokio::fs::remove_dir_all(&root).await;
}
