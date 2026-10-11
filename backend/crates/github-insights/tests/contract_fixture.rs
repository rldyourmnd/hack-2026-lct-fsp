//! Golden contract fixtures and heuristic checks on one synthetic account.
//!
//! The report and its employer projection are written to `contracts/fixtures/`. The mock suite
//! validates them against the OpenAPI schemas and the frontend renders them, so the Rust
//! serialization and the contract cannot drift apart. Regenerate with `FSP_UPDATE_FIXTURES=1`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use chrono::{DateTime, Duration, FixedOffset, TimeZone, Utc};
use fsp_domain::{GitHubDimensionId, GitHubReport, StackCategory};
use fsp_github_insights::analysis::build_report;
use fsp_github_insights::snapshot::{
    GitCommit, PullRequestFact, PullRequestState, RepositoryAnalysis, RepositoryMeta,
    SampledRepository, SearchTotals, Snapshot,
};

const OWNED: &str = "owner/project";
const ORGANIZATION: &str = "acme-labs/platform";
const EXTERNAL: &str = "other/library";

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).unwrap()
}

fn commit(index: usize, at: DateTime<FixedOffset>, subject: &str, files: &[&str]) -> GitCommit {
    GitCommit {
        sha: format!("{index:040x}"),
        authored_at: at,
        parents: 1,
        subject: subject.into(),
        files: files.iter().map(|file| (*file).to_owned()).collect(),
    }
}

/// Weekday work in UTC+3 over the last year: a morning session, often an afternoon one.
fn owned_commits() -> Vec<GitCommit> {
    let offset = FixedOffset::east_opt(3 * 3600).unwrap();
    let start = offset.with_ymd_and_hms(2025, 10, 13, 0, 0, 0).unwrap();
    let mut commits = Vec::new();
    for day in (0..360).filter(|day| day % 7 < 5 && day % 9 != 0) {
        let date = start + Duration::days(day);
        let mut times = vec![(10, 5), (11, 20)];
        if day % 2 == 0 {
            times.extend([(15, 0), (16, 10)]);
        }
        for (hour, minute) in times {
            let index = commits.len();
            let files: &[&str] = match index % 4 {
                0 => &["src/api.rs", "tests/api.rs"],
                1 => &["web/src/App.tsx"],
                2 => &["src/main.rs", "Cargo.toml"],
                _ => &["src/storage.rs"],
            };
            commits.push(commit(
                index,
                date + Duration::hours(hour) + Duration::minutes(minute),
                &format!("feat(api): handle request kind {index}"),
                files,
            ));
        }
    }
    // Earlier years: occasional evening work.
    for week in 0..40 {
        let at = offset.with_ymd_and_hms(2023, 2, 1, 20, 30, 0).unwrap() + Duration::weeks(week);
        let index = 10_000 + commits.len();
        commits.push(commit(
            index,
            at,
            "fix: tidy the parser",
            &["src/parser.rs"],
        ));
    }
    commits
}

fn organization_commits() -> Vec<GitCommit> {
    let offset = FixedOffset::east_opt(3 * 3600).unwrap();
    (0..60)
        .map(|index| {
            // Weekdays from Monday, 2 March 2026.
            let at = offset.with_ymd_and_hms(2026, 3, 2, 13, 0, 0).unwrap()
                + Duration::days(index / 5 * 7 + index % 5)
                + Duration::minutes(7 * index);
            commit(
                20_000 + index as usize,
                at,
                "feat(platform): add an ingestion endpoint",
                &["app/api/routes.py", "tests/test_routes.py"],
            )
        })
        .collect()
}

fn external_commits() -> Vec<GitCommit> {
    let offset = FixedOffset::east_opt(3 * 3600).unwrap();
    (0..2)
        .map(|index| {
            commit(
                30_000 + index,
                offset.with_ymd_and_hms(2026, 6, 1, 18, 0, 0).unwrap()
                    + Duration::days(index as i64),
                "Fix connection pool reuse",
                &["pool/pool.go"],
            )
        })
        .collect()
}

fn meta(
    name: &str,
    organization: bool,
    language: &str,
    stars: u32,
    topics: &[&str],
) -> RepositoryMeta {
    RepositoryMeta {
        full_name: name.into(),
        owner: name.split('/').next().unwrap().into(),
        owner_is_organization: organization,
        stars,
        forks: stars / 10,
        size_kib: 2_000,
        language: Some(language.into()),
        topics: topics.iter().map(|topic| (*topic).to_owned()).collect(),
        description: Some(format!("{language} project")),
        license: Some("MIT".into()),
        default_branch: Some("main".into()),
        pushed_at: Some(now() - Duration::days(2)),
        ..RepositoryMeta::default()
    }
}

fn pull_request(repository: &str, days_ago: i64, state: PullRequestState) -> PullRequestFact {
    PullRequestFact {
        url: format!("https://github.com/{repository}/pull/{days_ago}"),
        repository: repository.into(),
        owner: repository.split('/').next().unwrap().into(),
        created_at: now() - Duration::days(days_ago),
        state,
    }
}

fn snapshot() -> Snapshot {
    let owned = owned_commits();
    let organization = organization_commits();
    let external = external_commits();
    let year_ago = now() - Duration::days(365);
    let mut yearly = BTreeMap::new();
    for commit in owned.iter().chain(&organization).chain(&external) {
        *yearly
            .entry(chrono::Datelike::year(&commit.authored_at))
            .or_insert(0) += 1;
    }
    let last_year = owned
        .iter()
        .chain(&organization)
        .chain(&external)
        .filter(|commit| commit.utc() >= year_ago)
        .count() as u32;
    // Search sees twenty commits more than the clones: work in repositories not analyzed.
    *yearly.get_mut(&2026).unwrap() += 20;
    Snapshot {
        login: "owner".into(),
        created_at: Some(Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap()),
        now: now(),
        followers: 25,
        display_name: Some("Иван Петров".into()),
        organizations: vec!["acme-labs".into()],
        emails: BTreeSet::from(["owner@users.noreply.github.com".into()]),
        totals: SearchTotals {
            commits: Some(yearly.values().sum()),
            yearly_commits: yearly,
            owner_commits: BTreeMap::from([("acme-labs".into(), 75)]),
            commits_last_year: Some(last_year + 20),
            pull_requests: Some(25),
            pull_requests_last_year: Some(20),
            approved_pull_requests: Some(8),
            reviews_of_others: Some(5),
            independent_reviews: Some(2),
            issues: Some(7),
            external_pull_requests: Some(3),
            external_merged: Some(2),
        },
        pull_requests: (0..10)
            .map(|index| {
                pull_request(
                    if index % 2 == 0 { OWNED } else { ORGANIZATION },
                    3 + index * 20,
                    PullRequestState::Merged,
                )
            })
            .collect(),
        external_pull_requests: vec![
            pull_request(EXTERNAL, 120, PullRequestState::Merged),
            pull_request(EXTERNAL, 125, PullRequestState::Merged),
            pull_request(EXTERNAL, 30, PullRequestState::Open),
        ],
        repositories: BTreeMap::from([
            (OWNED.into(), meta(OWNED, false, "Rust", 12, &["rust", "web", "mit"])),
            (
                ORGANIZATION.into(),
                meta(ORGANIZATION, true, "Python", 40, &["platform", "agpl-3"]),
            ),
            (EXTERNAL.into(), meta(EXTERNAL, false, "Go", 9_000, &["database"])),
        ]),
        sampled: BTreeMap::from([
            (
                OWNED.into(),
                SampledRepository {
                    commits: 60,
                    owner: "owner".into(),
                    organization: false,
                },
            ),
            (
                ORGANIZATION.into(),
                SampledRepository {
                    commits: 30,
                    owner: "acme-labs".into(),
                    organization: true,
                },
            ),
            (
                EXTERNAL.into(),
                SampledRepository {
                    commits: 2,
                    owner: "other".into(),
                    organization: false,
                },
            ),
        ]),
        analyzed: vec![
            RepositoryAnalysis {
                full_name: OWNED.into(),
                total_commits: owned.len() as u32 + 20,
                direct_pushes: owned.len() as u32,
                files: [
                    "Cargo.toml",
                    "Cargo.lock",
                    "README.md",
                    "LICENSE",
                    "Dockerfile",
                    "rustfmt.toml",
                    ".github/workflows/ci.yml",
                    ".github/dependabot.yml",
                    "src/main.rs",
                    "src/api.rs",
                    "src/storage.rs",
                    "migrations/0001_init.sql",
                    "docs/adr/0001-storage.md",
                    "tests/api.rs",
                    "web/package.json",
                    "web/bun.lock",
                    "web/src/App.tsx",
                ]
                .map(String::from)
                .to_vec(),
                manifests: vec![
                    (
                        "Cargo.toml".into(),
                        "[package]\nname = \"project\"\n\n[dependencies]\naxum = \"0.8\"\ntokio = { version = \"1\", features = [\"full\"] }\nserde = \"1\"\nsqlx = { version = \"0.8\", features = [\"postgres\"] }\n".into(),
                    ),
                    (
                        "web/package.json".into(),
                        r#"{"dependencies":{"react":"19.0.0","@tanstack/react-query":"5.0.0"},"devDependencies":{"vite":"7.0.0","typescript":"5.9.0"}}"#.into(),
                    ),
                ],
                commits: owned,
            },
            RepositoryAnalysis {
                full_name: ORGANIZATION.into(),
                total_commits: 80,
                direct_pushes: 40,
                files: [
                    "pyproject.toml",
                    "uv.lock",
                    "README.md",
                    "app/api/routes.py",
                    "tests/test_routes.py",
                    ".github/workflows/test.yml",
                ]
                .map(String::from)
                .to_vec(),
                manifests: vec![(
                    "pyproject.toml".into(),
                    "[project]\nname = \"platform\"\ndependencies = [\"fastapi>=0.115\", \"sqlalchemy>=2\", \"httpx>=0.28\"]\n".into(),
                )],
                commits: organization,
            },
            RepositoryAnalysis {
                full_name: EXTERNAL.into(),
                total_commits: 500,
                direct_pushes: 0,
                files: ["go.mod", "README.md", "pool/pool.go"].map(String::from).to_vec(),
                manifests: vec![(
                    "go.mod".into(),
                    "module example.com/library\n\ngo 1.24\n\nrequire github.com/redis/go-redis/v9 v9.7.0\n".into(),
                )],
                commits: external,
            },
        ],
        ..Snapshot::default()
    }
}

fn check(name: &str, value: &serde_json::Value) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/fixtures")
        .join(name);
    let rendered = format!("{}\n", serde_json::to_string_pretty(value).unwrap());
    if std::env::var_os("FSP_UPDATE_FIXTURES").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, rendered).unwrap();
        return;
    }
    let stored = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(
        stored, rendered,
        "{name} is stale; regenerate with FSP_UPDATE_FIXTURES=1"
    );
}

fn report() -> GitHubReport {
    build_report(&snapshot())
}

#[test]
fn contract_fixtures_match_the_serialized_report() {
    let report = report();
    check(
        "github-report.json",
        &serde_json::to_value(&report).unwrap(),
    );
    check(
        "github-signal.json",
        &serde_json::to_value(report.employer_signal(true)).unwrap(),
    );
}

#[test]
fn the_report_is_deterministic() {
    assert_eq!(report(), report());
}

#[test]
fn activity_counts_exact_public_totals_and_local_rhythm() {
    let report = report();
    let coverage = &report.coverage;
    assert_eq!(
        coverage.commits_last_year_total,
        coverage.commits_last_year_found + 20
    );
    assert_eq!(
        report.activity.commits_last_year,
        coverage.commits_last_year_total
    );
    assert!(
        coverage
            .coverage_percent
            .is_some_and(|percent| percent >= 90)
    );
    assert_eq!(
        report
            .activity
            .yearly
            .iter()
            .map(|year| year.year)
            .collect::<Vec<_>>(),
        [2023, 2025, 2026]
    );
    let rhythm = report.rhythm.as_ref().unwrap();
    // Every commit is authored in UTC+3 on a weekday between 10:00 and 17:00.
    assert_eq!(rhythm.timezone_offsets.len(), 1);
    assert_eq!(rhythm.timezone_offsets[0].offset_minutes, 180);
    assert_eq!(rhythm.weekend_percent, 0);
    assert!(rhythm.office_hours_percent >= 95);
    // Two commits 75 minutes apart are one session: 75 minutes plus the half-hour credit.
    assert!(rhythm.median_minutes_per_active_day >= 105);
    assert!(rhythm.median_minutes_per_active_day <= 4 * 60);
    // Gaps of up to two hours join a day's commits into one session; nights always break it.
    assert!(rhythm.longest_session_minutes <= 8 * 60);
}

#[test]
fn ownership_and_collaboration_separate_own_and_independent_work() {
    let report = report();
    assert_eq!(report.ownership.led_repositories, 2);
    assert_eq!(report.ownership.own_organizations, 1);
    let organization = &report.organizations.contributed[0];
    assert_eq!(organization.login, "acme-labs");
    assert!(organization.affiliated);
    // The owner search counts more commits than the cloned history holds.
    assert_eq!(organization.commits, 75);
    let collaboration = &report.collaboration;
    assert_eq!(collaboration.external_merged, 2);
    assert_eq!(collaboration.external_repositories, 1);
    assert_eq!(collaboration.notable_contributions[0].repository, EXTERNAL);
    let external = report
        .repositories
        .iter()
        .find(|repository| repository.name_with_owner == EXTERNAL)
        .unwrap();
    assert!(!external.led && !external.owned);
    assert_eq!(external.user_share_percent, 0);
    assert!(report.flags.is_empty(), "{:?}", report.flags);
    for dimension in [
        GitHubDimensionId::Consistency,
        GitHubDimensionId::Engagement,
        GitHubDimensionId::Ownership,
        GitHubDimensionId::Craft,
    ] {
        assert!(report.dimensions.iter().any(|item| item.id == dimension));
    }
}

#[test]
fn scope_counts_sustained_systems_areas_layers_and_design_work() {
    let report = report();
    let scope = report
        .dimensions
        .iter()
        .find(|item| item.id == GitHubDimensionId::Scope)
        .unwrap();
    let metric = |id: &str| {
        scope
            .metrics
            .iter()
            .find(|metric| metric.id == id)
            .unwrap()
            .value
    };
    // The owned project spans a year of the account's own commits; the organization one is short.
    assert_eq!(metric("sustained_systems"), 1);
    // Migrations and an architecture decision record are design work; vendored copies are not.
    assert_eq!(metric("design_artifacts"), 1);
    assert!(metric("engineering_areas") >= 1);
    assert!(metric("system_layers") >= 1);
}

#[test]
fn design_artifacts_are_decisions_contracts_and_migrations() {
    use fsp_github_insights::analysis::design_artifact;
    for path in [
        "docs/adr/0001-storage.md",
        "docs/decisions/0002.md",
        "ARCHITECTURE.md",
        "docs/architecture/overview.md",
        "api/openapi.yaml",
        "proto/service.proto",
        "db/migrate/20240101_create_users.rb",
        "app/migrations/0001_initial.py",
    ] {
        assert!(design_artifact(path), "{path}");
    }
    for path in [
        "README.md",
        "src/main.rs",
        "node_modules/pkg/migrations/1.sql",
        "docs/usage.md",
    ] {
        assert!(!design_artifact(path), "{path}");
    }
}

#[test]
fn languages_stack_and_portrait_come_from_the_candidates_own_changes() {
    let report = report();
    assert_eq!(report.languages[0].language, "Rust");
    let stack = report
        .stack
        .iter()
        .map(|item| (item.name.as_str(), item.category))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(stack.get("Tokio"), Some(&StackCategory::Backend));
    assert_eq!(stack.get("React"), Some(&StackCategory::Frontend));
    assert!(stack.contains_key("FastAPI"));
    let skills = report
        .technologies
        .iter()
        .map(|item| item.skill_id.as_str())
        .collect::<BTreeSet<_>>();
    assert!(skills.contains("rust") && skills.contains("react"));
    // Two commits in someone else's Go library do not make Go the candidate's language.
    assert!(
        report
            .languages
            .iter()
            .all(|share| share.language != "Go" || share.percent <= 1)
    );
    assert!(!report.profile.archetypes.is_empty());
    assert!(report.profile.summary.contains("acme-labs"));
    assert!(!report.profile.public_summary.contains("acme-labs"));
    // License topics are not focus topics.
    assert!(
        report
            .profile
            .focus_topics
            .iter()
            .all(|topic| topic != "mit" && topic != "agpl-3")
    );
}

#[test]
fn the_employer_signal_names_no_account_project_or_organization() {
    let report = report();
    let signal = serde_json::to_string(&report.employer_signal(false)).unwrap();
    for private in [
        "\"owner\"",
        "owner/",
        "acme-labs",
        "github.com",
        "other/",
        "@",
    ] {
        assert!(!signal.contains(private), "the signal leaks {private}");
    }
}
