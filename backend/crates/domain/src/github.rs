//! Public GitHub activity report produced by the insights service.
//!
//! Every value describes observable public activity. A report never confirms a skill or a grade: the
//! knowledge assessment remains the only confirmation. Integers keep the report deterministic and
//! comparable (`Eq`); shares are whole percent, durations are minutes, scores are 0–100.

use serde::{Deserialize, Serialize};

pub const GITHUB_INSIGHTS_VERSION: &str = "github-insights-v3";

/// Normalizes a GitHub login typed as `login`, `@login` or a profile URL.
///
/// GitHub logins are 1–39 ASCII letters, digits or single inner hyphens. Repository, gist and
/// other non-profile URLs are rejected rather than guessed.
pub fn parse_github_login(input: &str) -> Option<String> {
    let mut value = input.trim();
    for prefix in ["https://", "http://"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            value = rest;
        }
    }
    if let Some(rest) = value.strip_prefix("www.") {
        value = rest;
    }
    let candidate = if let Some(path) = value.strip_prefix("github.com/") {
        let path = path.split(['?', '#']).next().unwrap_or_default();
        let mut segments = path.split('/').filter(|segment| !segment.is_empty());
        let login = segments.next()?;
        if segments.next().is_some() {
            return None;
        }
        login
    } else if value.contains('/') || value.contains('.') {
        return None;
    } else {
        value.strip_prefix('@').unwrap_or(value)
    };
    let bytes = candidate.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= 39
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
        && bytes.first() != Some(&b'-')
        && bytes.last() != Some(&b'-')
        && !candidate.contains("--");
    valid.then(|| candidate.to_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalBand {
    Weak,
    Moderate,
    Strong,
    VeryStrong,
}

impl SignalBand {
    #[must_use]
    pub fn from_score(score: u8) -> Self {
        match score {
            0..=24 => Self::Weak,
            25..=49 => Self::Moderate,
            50..=74 => Self::Strong,
            _ => Self::VeryStrong,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalConfidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHubDimensionId {
    Consistency,
    Engagement,
    Longevity,
    Collaboration,
    Ownership,
    Craft,
    Impact,
    /// Scope and complexity: sustained systems, engineering areas, system layers and design work.
    Scope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlagSeverity {
    Info,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackCategory {
    Language,
    Backend,
    Frontend,
    Data,
    Infrastructure,
    Quality,
    Ai,
    Mobile,
    Tooling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Archetype {
    Backend,
    Frontend,
    Platform,
    Systems,
    Data,
    Mobile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubReport {
    pub algorithm_version: String,
    pub generated_at: String,
    pub login: String,
    /// The public profile name, when the account sets one.
    #[serde(default)]
    pub display_name: Option<String>,
    pub profile_url: String,
    pub account_created_at: String,
    pub coverage: GitHubCoverage,
    pub index: GitHubIndex,
    pub dimensions: Vec<GitHubDimension>,
    /// Who the developer is: roles, summary, focus areas and leading projects.
    pub profile: GitHubDeveloperProfile,
    pub activity: GitHubActivity,
    pub rhythm: Option<GitHubRhythm>,
    pub collaboration: GitHubCollaboration,
    pub ownership: GitHubOwnershipSummary,
    pub organizations: GitHubOrganizations,
    /// Languages of the files the account itself changed.
    pub languages: Vec<GitHubLanguageShare>,
    /// Languages, frameworks, databases, infrastructure and tools with their artifacts.
    pub stack: Vec<GitHubStackItem>,
    /// Catalog skills (`rust`, `postgresql`, `docker`, `redis`, `typescript`, `react`).
    pub technologies: Vec<GitHubTechnology>,
    pub craft: GitHubCraft,
    pub repositories: Vec<GitHubRepositoryInsight>,
    pub flags: Vec<GitHubFlag>,
    /// Non-identifying sentences: no repository, organization or account names.
    pub highlights: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubCoverage {
    pub from: String,
    pub to: String,
    pub repositories_analyzed: u32,
    /// The account's commits found in the analyzed repositories' full histories.
    pub commits_analyzed: u32,
    /// All public commits GitHub search attributes to the account.
    pub commits_public_total: u32,
    pub commits_last_year_found: u32,
    pub commits_last_year_total: u32,
    pub coverage_percent: Option<u8>,
    pub partial: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubIndex {
    pub score: u8,
    pub band: SignalBand,
    pub confidence: SignalConfidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubDimension {
    pub id: GitHubDimensionId,
    pub label: String,
    pub score: u8,
    /// Share of the index, in percent.
    pub weight: u8,
    pub summary: String,
    pub metrics: Vec<GitHubMetric>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubMetric {
    pub id: String,
    pub label: String,
    pub value: i64,
    /// Exact value for the candidate.
    pub display: String,
    /// Rounded value for employers, so the metric cannot fingerprint the account.
    pub public_display: String,
    pub score: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubDeveloperProfile {
    /// For the candidate; may name projects and organizations.
    pub summary: String,
    /// The same portrait without names, for employers.
    pub public_summary: String,
    pub archetypes: Vec<GitHubArchetypeShare>,
    pub focus_topics: Vec<String>,
    pub top_projects: Vec<GitHubProject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubArchetypeShare {
    pub id: Archetype,
    pub label: String,
    pub percent: u8,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubProject {
    pub name_with_owner: String,
    pub url: String,
    pub description: Option<String>,
    pub primary_language: Option<String>,
    pub stars: u32,
    pub user_commits: u32,
    pub user_share_percent: u8,
    pub last_commit_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubActivity {
    pub active_years: u32,
    pub commits_last_year: u32,
    pub pull_requests_last_year: u32,
    pub active_days_last_year: u32,
    pub active_weeks_last_year: u32,
    pub longest_daily_streak: u32,
    pub current_daily_streak: u32,
    pub longest_weekly_streak: u32,
    pub current_weekly_streak: u32,
    pub longest_gap_days_last_year: u32,
    pub last_active_on: Option<String>,
    /// Commits of the last six months against the six before, in percent change.
    pub trend_percent: Option<i32>,
    pub calendar: Vec<GitHubDay>,
    pub yearly: Vec<GitHubYear>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubDay {
    pub date: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubYear {
    pub year: i32,
    pub commits: u32,
    pub active_days: u32,
    /// Pull requests opened that year, when GitHub search was asked for it.
    pub pull_requests: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRhythm {
    pub sample_size: u32,
    pub timezone_offsets: Vec<GitHubOffsetShare>,
    /// 24 values, local hour of the author's commit time.
    pub hourly_percent: Vec<u8>,
    /// 7 values, Monday first.
    pub weekday_percent: Vec<u8>,
    pub office_hours_percent: u8,
    pub evening_percent: u8,
    pub night_percent: u8,
    pub weekend_percent: u8,
    pub active_days: u32,
    pub median_minutes_per_active_day: u32,
    pub p90_minutes_per_active_day: u32,
    pub median_minutes_per_active_week: Option<u32>,
    pub sessions_per_active_day_tenths: u32,
    pub longest_session_minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubOffsetShare {
    pub offset_minutes: i32,
    pub percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubCollaboration {
    pub pull_requests_total: u32,
    /// Pull requests approved in review: GitHub forbids approving one's own pull request.
    pub approved_pull_requests: u32,
    pub reviews_of_others: u32,
    pub independent_reviews: u32,
    pub issues_total: u32,
    pub external_pull_requests: u32,
    pub external_merged: u32,
    pub external_merge_rate_percent: Option<u8>,
    pub external_repositories: u32,
    pub notable_contributions: Vec<GitHubNotableContribution>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubNotableContribution {
    pub repository: String,
    pub url: String,
    pub stars: u32,
    pub merged_pull_requests: u32,
    pub commits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubOwnershipSummary {
    /// Repositories where the account wrote at least half of the commits.
    pub led_repositories: u32,
    /// Owned or led repositories with commits in the last year.
    pub maintained_repositories: u32,
    /// Organizations the account builds: public membership, direct pushes or wide write access.
    pub own_organizations: u32,
    pub own_organization_repositories: u32,
    pub direct_push_repositories: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubOrganizations {
    /// Public memberships only; GitHub hides private memberships from everyone else.
    pub memberships: Vec<GitHubOrganization>,
    /// Organizations whose public repositories hold the account's commits.
    pub contributed: Vec<GitHubContributedOrganization>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubOrganization {
    pub login: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubContributedOrganization {
    pub login: String,
    pub url: String,
    pub repositories: u32,
    pub commits: u32,
    /// The account's own organization: it writes there directly or holds membership.
    pub affiliated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubLanguageShare {
    pub language: String,
    pub percent: u8,
    pub files_changed: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubStackItem {
    pub name: String,
    pub category: StackCategory,
    pub repositories: u32,
    pub signals: Vec<String>,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubTechnology {
    /// Catalog skill id (`rust`, `postgresql`, `docker`, `redis`, `typescript`, `react`).
    pub skill_id: String,
    pub repositories: u32,
    pub signals: Vec<String>,
    pub last_pushed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubCraft {
    pub substantive_repositories: u32,
    pub readme_percent: u8,
    pub license_percent: u8,
    pub ci_percent: u8,
    pub tests_percent: u8,
    pub lockfile_percent: u8,
    pub lint_percent: u8,
    pub dependency_updates_percent: u8,
    pub container_percent: u8,
    pub conventional_commits_percent: Option<u8>,
    pub descriptive_messages_percent: Option<u8>,
    pub median_files_per_commit: Option<u32>,
    pub large_commits_percent: Option<u8>,
    pub merge_commits_percent: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HygieneSignal {
    Readme,
    License,
    Ci,
    Tests,
    Lockfile,
    Lint,
    DependencyUpdates,
    Container,
    Description,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRepositoryInsight {
    pub name_with_owner: String,
    pub url: String,
    pub owned: bool,
    pub led: bool,
    pub fork: bool,
    pub archived: bool,
    pub stars: u32,
    pub forks: u32,
    pub primary_language: Option<String>,
    pub description: Option<String>,
    pub pushed_at: Option<String>,
    pub total_commits: u32,
    pub user_commits: u32,
    pub user_share_percent: u8,
    pub last_user_commit_at: Option<String>,
    pub hygiene: Vec<HygieneSignal>,
    pub technologies: Vec<String>,
    pub stack: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubFlag {
    pub id: String,
    pub severity: FlagSeverity,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipMethod {
    ProfileCode,
    Oauth,
}

/// Proof that the candidate controls the connected account. Without it the report stays a claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubOwnership {
    pub login: String,
    pub method: OwnershipMethod,
    pub verified_at: String,
}

/// What an employer sees before any disclosure: no names, links, calendars or exact counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubSignal {
    pub algorithm_version: String,
    pub generated_at: String,
    pub ownership_verified: bool,
    pub index: GitHubIndex,
    pub summary: String,
    pub archetypes: Vec<GitHubSignalArchetype>,
    pub dimensions: Vec<GitHubSignalDimension>,
    pub languages: Vec<GitHubSignalLanguage>,
    pub stack: Vec<GitHubSignalStackItem>,
    pub rhythm: Option<GitHubSignalRhythm>,
    pub highlights: Vec<String>,
    pub flags: Vec<GitHubFlag>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubSignalArchetype {
    pub id: Archetype,
    pub label: String,
    pub percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubSignalLanguage {
    pub language: String,
    pub percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubSignalStackItem {
    /// Named `technology`, not `name`: anonymous views carry no `name` keys at all.
    pub technology: String,
    pub category: StackCategory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubSignalDimension {
    pub id: GitHubDimensionId,
    pub label: String,
    pub score: u8,
    pub summary: String,
    pub metrics: Vec<GitHubSignalMetric>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubSignalMetric {
    pub id: String,
    pub label: String,
    pub display: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubSignalRhythm {
    pub hours_per_active_day: String,
    pub hours_per_active_week: Option<String>,
    pub office_hours_percent: u8,
    pub weekend_percent: u8,
}

impl GitHubReport {
    /// The anonymous employer projection.
    #[must_use]
    pub fn employer_signal(&self, ownership_verified: bool) -> GitHubSignal {
        let round5 = |percent: u8| ((u16::from(percent) + 2) / 5 * 5).min(100) as u8;
        GitHubSignal {
            algorithm_version: self.algorithm_version.clone(),
            generated_at: self.generated_at.get(..10).unwrap_or_default().to_owned(),
            ownership_verified,
            index: self.index.clone(),
            summary: self.profile.public_summary.clone(),
            archetypes: self
                .profile
                .archetypes
                .iter()
                .map(|archetype| GitHubSignalArchetype {
                    id: archetype.id,
                    label: archetype.label.clone(),
                    percent: round5(archetype.percent),
                })
                .filter(|archetype| archetype.percent > 0)
                .collect(),
            dimensions: self
                .dimensions
                .iter()
                .map(|dimension| GitHubSignalDimension {
                    id: dimension.id,
                    label: dimension.label.clone(),
                    score: dimension.score,
                    summary: dimension.summary.clone(),
                    metrics: dimension
                        .metrics
                        .iter()
                        .map(|metric| GitHubSignalMetric {
                            id: metric.id.clone(),
                            label: metric.label.clone(),
                            display: metric.public_display.clone(),
                        })
                        .collect(),
                })
                .collect(),
            languages: self
                .languages
                .iter()
                .take(6)
                .map(|language| GitHubSignalLanguage {
                    language: language.language.clone(),
                    percent: round5(language.percent),
                })
                .filter(|language| language.percent > 0)
                .collect(),
            stack: self
                .stack
                .iter()
                .filter(|item| item.category != StackCategory::Language)
                .take(16)
                .map(|item| GitHubSignalStackItem {
                    technology: item.name.clone(),
                    category: item.category,
                })
                .collect(),
            rhythm: self.rhythm.as_ref().map(|rhythm| GitHubSignalRhythm {
                hours_per_active_day: hours_band(rhythm.median_minutes_per_active_day),
                hours_per_active_week: rhythm.median_minutes_per_active_week.map(week_band),
                office_hours_percent: round5(rhythm.office_hours_percent),
                weekend_percent: round5(rhythm.weekend_percent),
            }),
            highlights: self.highlights.clone(),
            flags: self
                .flags
                .iter()
                .filter(|flag| flag.severity == FlagSeverity::Warning)
                .cloned()
                .collect(),
        }
    }
}

/// Five-hour bands for weekly time.
#[must_use]
pub fn week_band(minutes: u32) -> String {
    let hours = minutes / 60;
    if hours < 5 {
        return "менее 5 ч".into();
    }
    let low = hours / 5 * 5;
    format!("{low}–{} ч", low + 5)
}

/// Half-hour bands: an exact daily duration is not needed to compare engagement.
#[must_use]
pub fn hours_band(minutes: u32) -> String {
    if minutes < 30 {
        return "менее 0,5 ч".into();
    }
    let low = minutes / 30 * 30;
    format!("{}–{} ч", half_hours(low), half_hours(low + 30))
}

fn half_hours(minutes: u32) -> String {
    let whole = minutes / 60;
    if minutes.is_multiple_of(60) {
        whole.to_string()
    } else {
        format!("{whole},5")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logins_are_parsed_from_names_handles_and_profile_urls() {
        for (input, expected) in [
            ("octocat", Some("octocat")),
            ("  @Octo-Cat ", Some("Octo-Cat")),
            ("https://github.com/octocat", Some("octocat")),
            ("http://www.github.com/octocat/", Some("octocat")),
            ("github.com/octocat?tab=repositories", Some("octocat")),
            ("https://github.com/octocat/hello-world", None),
            ("https://gitlab.com/octocat", None),
            ("-octocat", None),
            ("octo--cat", None),
            ("octo_cat", None),
            ("", None),
            ("a".repeat(40).as_str(), None),
        ] {
            assert_eq!(parse_github_login(input).as_deref(), expected, "{input}");
        }
    }

    #[test]
    fn time_bands_hide_exact_durations() {
        assert_eq!(hours_band(10), "менее 0,5 ч");
        assert_eq!(hours_band(95), "1,5–2 ч");
        assert_eq!(hours_band(120), "2–2,5 ч");
        assert_eq!(week_band(130), "менее 5 ч");
        assert_eq!(week_band(1_380), "20–25 ч");
    }

    #[test]
    fn bands_follow_documented_thresholds() {
        assert_eq!(SignalBand::from_score(24), SignalBand::Weak);
        assert_eq!(SignalBand::from_score(25), SignalBand::Moderate);
        assert_eq!(SignalBand::from_score(74), SignalBand::Strong);
        assert_eq!(SignalBand::from_score(75), SignalBand::VeryStrong);
    }
}
