//! Collects a [`Snapshot`] of public facts without a token.
//!
//! Stages: the profile and public organizations (two REST requests), commit search by period for
//! the account's commit identities, repositories and yearly totals, the newest pull requests,
//! commit search per owner, repository metadata through repository search, blobless clones of the
//! repositories the account works in, manifests from raw file downloads, and pull-request counts.
//! The account's own organizations are known only after the clones (direct pushes), so the search
//! for pull requests into other owners' projects runs last.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::sync::Arc;
use std::task::Poll;

use chrono::{DateTime, Datelike, Duration, Utc};
use serde::Deserialize;
use tokio::sync::Semaphore;

use crate::client::{FetchError, GitHubClient, encode_query};
use crate::git::Git;
use crate::snapshot::{
    PullRequestFact, PullRequestState, RepositoryMeta, SampledRepository, Snapshot,
};
use crate::tech::manifest_paths;

#[derive(Debug, Clone)]
pub struct Limits {
    pub repositories: usize,
    pub manifests_per_repository: usize,
    pub clone_concurrency: usize,
    /// Repositories larger than this are not cloned (GitHub's size in KiB).
    pub max_repository_kib: u32,
    pub organizations: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            repositories: 80,
            manifests_per_repository: 12,
            clone_concurrency: 6,
            max_repository_kib: 1_500_000,
            organizations: 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectError {
    UnknownAccount,
    Fetch(FetchError),
}

impl From<FetchError> for CollectError {
    fn from(error: FetchError) -> Self {
        match error {
            FetchError::NotFound => Self::UnknownAccount,
            other => Self::Fetch(other),
        }
    }
}

/// Reports the current stage and an approximate percentage.
pub type Progress = Arc<dyn Fn(&str, u8) + Send + Sync>;

pub struct Collector {
    pub client: Arc<GitHubClient>,
    pub git: Arc<Git>,
    pub limits: Limits,
}

/// The snapshot plus the profile bio, read for the ownership check and never stored.
pub struct Collected {
    pub snapshot: Snapshot,
    pub bio: Option<String>,
}

impl Collector {
    /// Reads only the profile: the canonical login and the bio for an ownership check.
    pub async fn profile(&self, login: &str) -> Result<(String, Option<String>), CollectError> {
        let user: UserProfile = self.client.rest(&format!("/users/{login}")).await?;
        if user.kind != "User" {
            return Err(CollectError::UnknownAccount);
        }
        Ok((user.login, user.bio))
    }

    pub async fn collect(
        &self,
        login: &str,
        progress: Progress,
    ) -> Result<Collected, CollectError> {
        let now = Utc::now();
        let year_ago = (now - Duration::days(365)).format("%Y-%m-%d").to_string();
        progress("Профиль и организации", 3);
        let user: UserProfile = self.client.rest(&format!("/users/{login}")).await?;
        if user.kind != "User" {
            return Err(CollectError::UnknownAccount);
        }
        let login = user.login.clone();
        let mut snapshot = Snapshot {
            login: login.clone(),
            created_at: parse_time(&user.created_at),
            now,
            followers: user.followers,
            display_name: user
                .name
                .clone()
                .map(|name| name.trim().to_owned())
                .filter(|name| !name.is_empty()),
            ..Snapshot::default()
        };
        snapshot
            .emails
            .insert(format!("{login}@users.noreply.github.com").to_ascii_lowercase());
        snapshot
            .emails
            .insert(format!("{}+{login}@users.noreply.github.com", user.id).to_ascii_lowercase());
        match self
            .client
            .rest::<Vec<Named>>(&format!("/users/{login}/orgs?per_page=100"))
            .await
        {
            Ok(organizations) => {
                snapshot.organizations = organizations.into_iter().map(|item| item.login).collect();
            }
            Err(error) => self.miss(&mut snapshot, "organizations", &error)?,
        }

        // Commit search by window: repositories across the whole history, identities and exact
        // public totals (the last four quarters, then calendar years back to the account's start).
        progress("Поиск коммитов по периодам", 6);
        let windows = windows(now, snapshot.created_at);
        let mut seen = BTreeSet::new();
        let mut last_year = 0u32;
        let mut last_year_known = true;
        let mut earlier = 0u32;
        for (index, window) in windows.iter().enumerate() {
            let query = format!("author:{login} author-date:{}", window.range);
            match self.commit_search(&query, 1, 100).await {
                Ok(result) => {
                    match window.kind {
                        WindowKind::Quarter => last_year += result.total_count,
                        WindowKind::Year(year) => {
                            snapshot
                                .totals
                                .yearly_commits
                                .insert(year, result.total_count);
                        }
                        WindowKind::Earlier => earlier = result.total_count,
                    }
                    absorb_commits(&mut snapshot, &mut seen, result.items);
                }
                Err(error) => {
                    if window.kind == WindowKind::Quarter {
                        last_year_known = false;
                    }
                    self.miss(&mut snapshot, "commit search", &error)?;
                }
            }
            progress(
                "Поиск коммитов по периодам",
                6 + u8::try_from(index * 8 / windows.len().max(1)).unwrap_or(8),
            );
        }
        snapshot.totals.commits_last_year = last_year_known.then_some(last_year);
        snapshot.totals.commits =
            Some(snapshot.totals.yearly_commits.values().sum::<u32>() + earlier);

        // The newest pull requests name more of the owners the account works with.
        progress("Pull request’ы", 14);
        match self
            .issue_search(&format!("type:pr author:{login} sort:created-desc"), 100)
            .await
        {
            Ok(result) => {
                snapshot.totals.pull_requests = Some(result.total_count);
                snapshot.pull_requests = pull_requests(result.items);
            }
            Err(error) => self.miss(&mut snapshot, "pull request search", &error)?,
        }
        // Owners: public memberships first, then those holding sampled commits or pull requests;
        // a search per owner gives its exact commit total and a sample of its repositories.
        let mut weights = BTreeMap::<String, u32>::new();
        for item in snapshot.sampled.values() {
            if item.organization {
                *weights.entry(item.owner.clone()).or_default() += item.commits;
            }
        }
        for pull in &snapshot.pull_requests {
            if !snapshot.is_self(&pull.owner) {
                *weights.entry(pull.owner.clone()).or_default() += 1;
            }
        }
        let mut owners = snapshot.organizations.clone();
        let mut ranked = weights
            .into_iter()
            .filter(|(owner, _)| !owners.contains(owner))
            .collect::<Vec<_>>();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        owners.extend(ranked.into_iter().map(|(owner, _)| owner));
        owners.truncate(self.limits.organizations);
        progress("Коммиты в организациях", 16);
        for owner in &owners {
            match self
                .commit_search(&format!("author:{login} user:{owner}"), 1, 100)
                .await
            {
                Ok(result) => {
                    snapshot
                        .totals
                        .owner_commits
                        .insert(owner.clone(), result.total_count);
                    absorb_commits(&mut snapshot, &mut seen, result.items);
                }
                Err(error) => self.miss(&mut snapshot, "organization commit search", &error)?,
            }
        }
        let organizations = owners
            .into_iter()
            .filter(|owner| {
                snapshot.organizations.contains(owner)
                    || snapshot
                        .sampled
                        .values()
                        .any(|item| &item.owner == owner && item.organization)
            })
            .collect::<Vec<_>>();

        // Repository metadata through repository search (no REST budget).
        progress("Метаданные репозиториев", 22);
        let mut metadata_queries = vec![format!("user:{login} fork:true")];
        metadata_queries.extend(
            organizations
                .iter()
                .map(|organization| format!("org:{organization}")),
        );
        for query in metadata_queries {
            match self.repository_search(&query).await {
                Ok(items) => {
                    for item in items {
                        snapshot.repositories.insert(item.full_name.clone(), item);
                    }
                }
                Err(error) => self.miss(&mut snapshot, "repository search", &error)?,
            }
        }
        let missing = snapshot
            .sampled
            .keys()
            .filter(|name| !snapshot.repositories.contains_key(*name))
            .cloned()
            .collect::<Vec<_>>();
        for batch in batches(&missing) {
            match self.repository_search(&batch).await {
                Ok(items) => {
                    for item in items {
                        snapshot.repositories.insert(item.full_name.clone(), item);
                    }
                }
                Err(error) => self.miss(&mut snapshot, "repository metadata", &error)?,
            }
        }

        // Clones of the repositories the account works in.
        let targets = self.clone_targets(&snapshot);
        if targets.len() < snapshot.sampled.len() {
            snapshot.notes.push(format!(
                "Разобраны репозитории с наибольшим вкладом: {} из {} найденных.",
                targets.len(),
                snapshot.sampled.len()
            ));
        }
        let total = targets.len().max(1);
        let done = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let permits = Arc::new(Semaphore::new(self.limits.clone_concurrency));
        let emails = snapshot.emails.clone();
        let clones = targets.iter().map(|name| {
            let permits = permits.clone();
            let done = done.clone();
            let progress = progress.clone();
            let emails = &emails;
            async move {
                let _permit = permits.acquire().await.ok()?;
                let result = self.git.analyze(name, emails).await;
                let finished = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                progress(
                    "Клонирование и разбор истории",
                    30 + u8::try_from(finished * 50 / total).unwrap_or(50),
                );
                result
            }
        });
        let analyzed = join_all(clones).await;
        let failed = analyzed.iter().filter(|result| result.is_none()).count();
        if failed > 0 {
            snapshot.partial = true;
            snapshot.notes.push(format!(
                "Не удалось склонировать за отведённое время репозиториев: {failed}."
            ));
        }
        // Repositories cloned on spec (busy organizations) count only where the account has commits.
        let login = snapshot.login.clone();
        snapshot.analyzed = analyzed
            .into_iter()
            .flatten()
            .filter(|repository| {
                repository.user_commits() > 0
                    || repository
                        .full_name
                        .split('/')
                        .next()
                        .is_some_and(|owner| owner.eq_ignore_ascii_case(&login))
            })
            .collect();

        // Manifests of the analyzed repositories.
        progress("Манифесты и зависимости", 82);
        let requests = snapshot
            .analyzed
            .iter()
            .enumerate()
            .flat_map(|(index, repository)| {
                manifest_paths(&repository.files, self.limits.manifests_per_repository)
                    .into_iter()
                    .map(move |path| (index, path))
            })
            .collect::<Vec<_>>();
        let permits = Arc::new(Semaphore::new(8));
        let downloads = requests.iter().map(|(index, path)| {
            let permits = permits.clone();
            let name = snapshot.analyzed[*index].full_name.clone();
            async move {
                let _permit = permits.acquire().await.ok()?;
                self.client
                    .raw_file(&name, path)
                    .await
                    .map(|text| (*index, path.clone(), text))
            }
        });
        for (index, path, text) in join_all(downloads).await.into_iter().flatten() {
            snapshot.analyzed[index].manifests.push((path, text));
        }

        // Pull requests, approvals and reviews.
        progress("Pull request’ы и ревью", 88);
        for (query, slot) in [
            (format!("type:pr author:{login} created:>={year_ago}"), 0),
            (format!("type:pr author:{login} review:approved"), 1),
            (format!("type:pr reviewed-by:{login} -author:{login}"), 2),
            (format!("type:issue author:{login}"), 3),
        ] {
            match self.issue_search(&query, 1).await {
                Ok(result) => {
                    let total = Some(result.total_count);
                    match slot {
                        0 => snapshot.totals.pull_requests_last_year = total,
                        1 => snapshot.totals.approved_pull_requests = total,
                        2 => snapshot.totals.reviews_of_others = total,
                        _ => snapshot.totals.issues = total,
                    }
                }
                Err(error) => self.miss(&mut snapshot, "contribution count", &error)?,
            }
        }

        // Other owners' projects, now that the account's organizations are known.
        progress("Вклад в независимые проекты", 94);
        // Owners where merged pull requests span three repositories or exceed thirty are the
        // account's own as well (it holds merge rights there); the search repeats without them.
        let mut external = String::new();
        for _ in 0..3 {
            let exclusions = exclusions(&snapshot);
            external = format!("type:pr author:{login} -user:{login}{exclusions}");
            match self.issue_search(&external, 100).await {
                Ok(result) => {
                    snapshot.totals.external_pull_requests = Some(result.total_count);
                    snapshot.external_pull_requests = pull_requests(result.items);
                }
                Err(error) => {
                    self.miss(&mut snapshot, "external pull requests", &error)?;
                    break;
                }
            }
            let mut per_owner: BTreeMap<String, (BTreeSet<String>, u32)> = BTreeMap::new();
            for pull in snapshot
                .external_pull_requests
                .iter()
                .filter(|pull| pull.state == PullRequestState::Merged)
            {
                let entry = per_owner.entry(pull.owner.clone()).or_default();
                entry.0.insert(pull.repository.clone());
                entry.1 += 1;
            }
            let found = per_owner
                .into_iter()
                .filter(|(_, (repositories, merged))| repositories.len() >= 3 || *merged >= 30)
                .map(|(owner, _)| owner)
                .filter(|owner| !snapshot.merge_rights.contains(owner))
                .collect::<Vec<_>>();
            if found.is_empty() {
                break;
            }
            snapshot.merge_rights.extend(found);
        }
        match self.issue_search(&format!("{external} is:merged"), 1).await {
            Ok(result) => snapshot.totals.external_merged = Some(result.total_count),
            Err(error) => self.miss(&mut snapshot, "external merges", &error)?,
        }
        match self
            .issue_search(
                &format!(
                    "type:pr reviewed-by:{login} -author:{login} -user:{login}{}",
                    exclusions(&snapshot)
                ),
                1,
            )
            .await
        {
            Ok(result) => snapshot.totals.independent_reviews = Some(result.total_count),
            Err(error) => self.miss(&mut snapshot, "independent reviews", &error)?,
        }
        // Metadata for external repositories that accepted pull requests (stars of notable work).
        let external_repositories = snapshot
            .external_pull_requests
            .iter()
            .filter(|pull| pull.state == PullRequestState::Merged)
            .map(|pull| pull.repository.clone())
            .filter(|name| !snapshot.repositories.contains_key(name))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(16)
            .collect::<Vec<_>>();
        for batch in batches(&external_repositories) {
            if let Ok(items) = self.repository_search(&batch).await {
                for item in items {
                    snapshot.repositories.insert(item.full_name.clone(), item);
                }
            }
        }
        if snapshot.partial {
            snapshot.notes.push(
                "Часть данных GitHub не удалось получить: показатели рассчитаны по доступной выборке."
                    .into(),
            );
        }
        progress("Расчёт показателей", 98);
        Ok(Collected {
            snapshot,
            bio: user.bio,
        })
    }

    /// A rate limit beyond the wait budget aborts so the job can retry later; other failures
    /// only mark the snapshot partial.
    fn miss(
        &self,
        snapshot: &mut Snapshot,
        stage: &str,
        error: &FetchError,
    ) -> Result<(), CollectError> {
        tracing::warn!(stage, ?error, "github stage failed");
        if let FetchError::RateLimited { .. } = error {
            return Err(CollectError::Fetch(error.clone()));
        }
        snapshot.partial = true;
        Ok(())
    }

    fn clone_targets(&self, snapshot: &Snapshot) -> Vec<String> {
        let fits = |name: &str| {
            snapshot
                .repositories
                .get(name)
                .is_none_or(|meta| !meta.fork && meta.size_kib <= self.limits.max_repository_kib)
        };
        let mut ranked = snapshot
            .sampled
            .iter()
            .filter(|(name, _)| fits(name))
            .map(|(name, item)| (name.clone(), item.commits))
            .collect::<Vec<_>>();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut targets = ranked
            .into_iter()
            .map(|(name, _)| name)
            .take(self.limits.repositories)
            .collect::<Vec<_>>();
        // Owned repositories without sampled commits, most recently pushed first.
        let mut owned = snapshot
            .repositories
            .values()
            .filter(|meta| snapshot.is_self(&meta.owner) && fits(&meta.full_name))
            .collect::<Vec<_>>();
        owned.sort_by_key(|meta| std::cmp::Reverse(meta.pushed_at));
        // Repositories pushed within two years in owners where the account has at least twenty public
        // commits: newest-first samples miss older projects of busy organizations.
        let since = snapshot.now - Duration::days(730);
        let busy = |owner: &str| {
            snapshot
                .totals
                .owner_commits
                .iter()
                .any(|(known, total)| known.eq_ignore_ascii_case(owner) && *total >= 20)
        };
        let mut organizational = snapshot
            .repositories
            .values()
            .filter(|meta| {
                busy(&meta.owner)
                    && meta.pushed_at.is_some_and(|at| at >= since)
                    && fits(&meta.full_name)
            })
            .collect::<Vec<_>>();
        organizational.sort_by_key(|meta| std::cmp::Reverse(meta.pushed_at));
        for meta in owned.into_iter().chain(organizational) {
            if targets.len() >= self.limits.repositories {
                break;
            }
            if !targets.contains(&meta.full_name) {
                targets.push(meta.full_name.clone());
            }
        }
        targets
    }

    async fn commit_search(
        &self,
        query: &str,
        page: u32,
        per_page: u32,
    ) -> Result<Search<CommitItem>, FetchError> {
        self.client
            .search(&format!(
                "/search/commits?q={}&sort=author-date&order=desc&per_page={per_page}&page={page}",
                encode_query(query)
            ))
            .await
    }

    async fn issue_search(
        &self,
        query: &str,
        per_page: u32,
    ) -> Result<Search<IssueItem>, FetchError> {
        self.client
            .search(&format!(
                "/search/issues?q={}&per_page={per_page}",
                encode_query(query)
            ))
            .await
    }

    async fn repository_search(&self, query: &str) -> Result<Vec<RepositoryMeta>, FetchError> {
        let result: Search<RepositoryItem> = self
            .client
            .search(&format!(
                "/search/repositories?q={}&sort=updated&per_page=100",
                encode_query(query)
            ))
            .await?;
        Ok(result
            .items
            .into_iter()
            .filter(|item| !item.private)
            .map(RepositoryItem::into_meta)
            .collect())
    }
}

/// Adds sampled commits once each: windows and owner searches overlap.
fn absorb_commits(snapshot: &mut Snapshot, seen: &mut BTreeSet<String>, items: Vec<CommitItem>) {
    for item in items {
        if item.repository.private || !seen.insert(item.sha.clone()) {
            continue;
        }
        if item
            .author
            .as_ref()
            .is_some_and(|author| author.login.eq_ignore_ascii_case(&snapshot.login))
        {
            snapshot
                .emails
                .insert(item.commit.author.email.to_ascii_lowercase());
        }
        snapshot
            .sampled
            .entry(item.repository.full_name.clone())
            .or_insert_with(|| SampledRepository {
                commits: 0,
                owner: item.repository.owner.login.clone(),
                organization: item.repository.owner.kind == "Organization",
            })
            .commits += 1;
    }
}

/// Owners outside "other people's projects": public organizations, owners where the account
/// pushes directly, and owners where it commits to three or more repositories.
pub fn affiliated_owners(snapshot: &Snapshot) -> BTreeMap<String, bool> {
    let mut owners = BTreeMap::new();
    for organization in &snapshot.organizations {
        owners.insert(organization.clone(), true);
    }
    let mut per_owner: BTreeMap<&str, (u32, bool)> = BTreeMap::new();
    for item in snapshot.sampled.values() {
        let entry = per_owner
            .entry(&item.owner)
            .or_insert((0, item.organization));
        entry.0 += 1;
    }
    for (owner, (repositories, organization)) in per_owner {
        if repositories >= 3 && !snapshot.is_self(owner) {
            owners.insert(owner.to_owned(), organization);
        }
    }
    for owner in &snapshot.merge_rights {
        owners.entry(owner.clone()).or_insert(true);
    }
    for repository in snapshot
        .analyzed
        .iter()
        .filter(|repository| repository.direct_pushes > 0)
    {
        let owner = repository.full_name.split('/').next().unwrap_or_default();
        if !snapshot.is_self(owner) {
            let organization = snapshot
                .repositories
                .get(&repository.full_name)
                .is_none_or(|meta| meta.owner_is_organization);
            owners.insert(owner.to_owned(), organization);
        }
    }
    owners
}

fn exclusions(snapshot: &Snapshot) -> String {
    let mut text = String::new();
    // GitHub rejects search queries over 256 characters; the analysis filters the rest.
    for (owner, organization) in affiliated_owners(snapshot) {
        let qualifier = format!(" -{}:{owner}", if organization { "org" } else { "user" });
        if text.len() + qualifier.len() > 160 {
            break;
        }
        text.push_str(&qualifier);
    }
    text
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowKind {
    /// One of the last four quarters; together they give the last year's total.
    Quarter,
    Year(i32),
    /// Everything before the searched calendar years.
    Earlier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Window {
    range: String,
    kind: WindowKind,
}

/// Commit-search windows, newest first: four quarters of the last year, then calendar years back
/// to the account's creation (at most eight) and one window for anything older.
fn windows(now: DateTime<Utc>, created: Option<DateTime<Utc>>) -> Vec<Window> {
    let day = |time: DateTime<Utc>| time.format("%Y-%m-%d").to_string();
    let mut windows = Vec::new();
    for quarter in 0..4 {
        let to = now - Duration::days(91 * quarter);
        let from = if quarter == 3 {
            now - Duration::days(364)
        } else {
            now - Duration::days(91 * (quarter + 1) - 1)
        };
        windows.push(Window {
            range: format!("{}..{}", day(from), day(to)),
            kind: WindowKind::Quarter,
        });
    }
    let first_year = created.map_or(now.year() - 8, |created| created.year());
    let mut year = now.year();
    while year >= first_year && year > now.year() - 8 {
        let to = if year == now.year() {
            day(now)
        } else {
            format!("{year}-12-31")
        };
        windows.push(Window {
            range: format!("{year}-01-01..{to}"),
            kind: WindowKind::Year(year),
        });
        year -= 1;
    }
    if year >= first_year {
        windows.push(Window {
            range: format!("<{}-01-01", year + 1),
            kind: WindowKind::Earlier,
        });
    }
    windows
}

/// `repo:` queries that fit GitHub's 256-character limit.
fn batches(names: &[String]) -> Vec<String> {
    let mut batches = Vec::new();
    let mut current = String::new();
    for name in names {
        let qualifier = format!("repo:{name}");
        if !current.is_empty() && current.len() + qualifier.len() + 1 > 240 {
            batches.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(&qualifier);
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

fn pull_requests(items: Vec<IssueItem>) -> Vec<PullRequestFact> {
    items
        .into_iter()
        .filter_map(|item| {
            let repository = item.repository_url.rsplit("/repos/").next()?.to_owned();
            let owner = repository.split('/').next()?.to_owned();
            Some(PullRequestFact {
                url: item.html_url,
                owner,
                repository,
                created_at: parse_time(&item.created_at)?,
                state: if item
                    .pull_request
                    .as_ref()
                    .is_some_and(|pull| pull.merged_at.is_some())
                {
                    PullRequestState::Merged
                } else if item.state == "open" {
                    PullRequestState::Open
                } else {
                    PullRequestState::Closed
                },
            })
        })
        .collect()
}

/// Polls all futures together; semaphores bound the real concurrency.
pub async fn join_all<F: Future>(futures: impl IntoIterator<Item = F>) -> Vec<F::Output> {
    let mut futures = futures.into_iter().map(Box::pin).collect::<Vec<_>>();
    let mut output = futures.iter().map(|_| None).collect::<Vec<_>>();
    std::future::poll_fn(|context| {
        let mut pending = false;
        for (future, slot) in futures.iter_mut().zip(output.iter_mut()) {
            if slot.is_none() {
                match future.as_mut().poll(context) {
                    Poll::Ready(value) => *slot = Some(value),
                    Poll::Pending => pending = true,
                }
            }
        }
        if pending {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    })
    .await;
    output.into_iter().flatten().collect()
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

#[derive(Deserialize)]
struct UserProfile {
    login: String,
    id: u64,
    #[serde(rename = "type")]
    kind: String,
    created_at: String,
    #[serde(default)]
    followers: u32,
    bio: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct Named {
    login: String,
}

#[derive(Deserialize)]
pub struct Search<T> {
    total_count: u32,
    items: Vec<T>,
}

#[derive(Deserialize)]
struct CommitItem {
    sha: String,
    commit: CommitBody,
    author: Option<Named>,
    repository: MinimalRepository,
}

#[derive(Deserialize)]
struct CommitBody {
    author: Identity,
}

#[derive(Deserialize)]
struct Identity {
    email: String,
}

#[derive(Deserialize)]
struct MinimalRepository {
    full_name: String,
    #[serde(default)]
    private: bool,
    owner: Owner,
}

#[derive(Deserialize)]
struct Owner {
    login: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct IssueItem {
    html_url: String,
    repository_url: String,
    state: String,
    created_at: String,
    pull_request: Option<PullRequestLink>,
}

#[derive(Deserialize)]
struct PullRequestLink {
    merged_at: Option<String>,
}

#[derive(Deserialize)]
struct RepositoryItem {
    full_name: String,
    #[serde(default)]
    private: bool,
    owner: Owner,
    #[serde(default)]
    fork: bool,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    stargazers_count: u32,
    #[serde(default)]
    forks_count: u32,
    #[serde(default)]
    size: u32,
    language: Option<String>,
    #[serde(default)]
    topics: Vec<String>,
    description: Option<String>,
    license: Option<License>,
    default_branch: Option<String>,
    pushed_at: Option<String>,
}

#[derive(Deserialize)]
struct License {
    spdx_id: Option<String>,
}

impl RepositoryItem {
    fn into_meta(self) -> RepositoryMeta {
        RepositoryMeta {
            owner_is_organization: self.owner.kind == "Organization",
            owner: self.owner.login,
            full_name: self.full_name,
            fork: self.fork,
            archived: self.archived,
            stars: self.stargazers_count,
            forks: self.forks_count,
            size_kib: self.size,
            language: self.language,
            topics: self.topics,
            description: self
                .description
                .filter(|description| !description.trim().is_empty()),
            license: self
                .license
                .and_then(|license| license.spdx_id)
                .filter(|id| id != "NOASSERTION"),
            default_branch: self.default_branch,
            pushed_at: self.pushed_at.as_deref().and_then(parse_time),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_cover_the_last_year_and_every_calendar_year() {
        let now = "2026-10-10T12:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let created = "2022-04-01T00:00:00Z".parse::<DateTime<Utc>>().ok();
        let windows = windows(now, created);
        let ranges = windows
            .iter()
            .map(|window| window.range.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ranges,
            [
                "2026-07-12..2026-10-10",
                "2026-04-12..2026-07-11",
                "2026-01-11..2026-04-11",
                "2025-10-11..2026-01-10",
                "2026-01-01..2026-10-10",
                "2025-01-01..2025-12-31",
                "2024-01-01..2024-12-31",
                "2023-01-01..2023-12-31",
                "2022-01-01..2022-12-31",
            ]
        );
        assert_eq!(windows[4].kind, WindowKind::Year(2026));
        let old = "2010-01-01T00:00:00Z".parse::<DateTime<Utc>>().ok();
        let windows = super::windows(now, old);
        assert_eq!(windows.len(), 13);
        assert_eq!(windows[11].range, "2019-01-01..2019-12-31");
        assert_eq!(windows[12].range, "<2019-01-01");
        assert_eq!(windows[12].kind, WindowKind::Earlier);
    }

    #[test]
    fn clone_targets_add_owned_and_busy_organization_repositories() {
        let now = "2026-10-10T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let meta = |name: &str, days: i64, fork: bool| RepositoryMeta {
            full_name: name.into(),
            owner: name.split('/').next().unwrap().into(),
            fork,
            size_kib: 100,
            pushed_at: Some(now - Duration::days(days)),
            ..RepositoryMeta::default()
        };
        let mut snapshot = Snapshot {
            login: "dev".into(),
            now,
            ..Snapshot::default()
        };
        snapshot.sampled.insert(
            "acme/api".into(),
            SampledRepository {
                commits: 9,
                owner: "acme".into(),
                organization: true,
            },
        );
        snapshot.totals.owner_commits.insert("acme".into(), 400);
        snapshot.totals.owner_commits.insert("quiet".into(), 3);
        for item in [
            meta("acme/api", 1, false),
            meta("acme/old-tool", 400, false),
            meta("acme/ancient", 1000, false),
            meta("acme/vendor", 5, true),
            meta("quiet/lib", 2, false),
            meta("dev/site", 30, false),
        ] {
            snapshot.repositories.insert(item.full_name.clone(), item);
        }
        let collector = Collector {
            client: Arc::new(
                GitHubClient::with_endpoints(
                    None,
                    "http://127.0.0.1:9".into(),
                    "http://127.0.0.1:9".into(),
                )
                .unwrap(),
            ),
            git: Arc::new(Git {
                base_url: String::new(),
                scratch: std::env::temp_dir(),
                clone_timeout: std::time::Duration::from_secs(1),
            }),
            limits: Limits::default(),
        };
        assert_eq!(
            collector.clone_targets(&snapshot),
            ["acme/api", "dev/site", "acme/old-tool"]
        );
    }

    #[test]
    fn repository_batches_stay_under_the_query_limit() {
        let names = (0..30)
            .map(|index| format!("organization-name/repository-{index:02}"))
            .collect::<Vec<_>>();
        let batches = batches(&names);
        assert!(batches.len() > 1);
        assert!(batches.iter().all(|batch| batch.len() <= 240));
        assert_eq!(
            batches
                .iter()
                .map(|batch| batch.matches("repo:").count())
                .sum::<usize>(),
            30
        );
    }

    #[test]
    fn pull_request_states_and_repositories_come_from_search_items() {
        let items = vec![
            IssueItem {
                html_url: "https://github.com/a/b/pull/1".into(),
                repository_url: "https://api.github.com/repos/a/b".into(),
                state: "closed".into(),
                created_at: "2026-01-02T03:04:05Z".into(),
                pull_request: Some(PullRequestLink {
                    merged_at: Some("2026-01-03T00:00:00Z".into()),
                }),
            },
            IssueItem {
                html_url: "https://github.com/a/b/pull/2".into(),
                repository_url: "https://api.github.com/repos/a/b".into(),
                state: "open".into(),
                created_at: "2026-01-02T03:04:05Z".into(),
                pull_request: Some(PullRequestLink { merged_at: None }),
            },
        ];
        let facts = pull_requests(items);
        assert_eq!(facts[0].repository, "a/b");
        assert_eq!(facts[0].owner, "a");
        assert_eq!(facts[0].state, PullRequestState::Merged);
        assert_eq!(facts[1].state, PullRequestState::Open);
    }
}
