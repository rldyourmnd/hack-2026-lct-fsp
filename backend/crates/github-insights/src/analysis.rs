//! Heuristics: a [`Snapshot`] of public facts becomes a [`GitHubReport`].
//!
//! Every metric is a documented, deterministic function of public data (see
//! `docs/github-insights.md`). Scores map metrics onto 0–100 with fixed thresholds and saturate.
//! Integrity flags never add points: they lower confidence and explain anomalies. Agent-assisted
//! work is ordinary work: commits made through AI agents under the account count like any other.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDate, Timelike, Utc};
use fsp_domain::{
    Archetype, FlagSeverity, GITHUB_INSIGHTS_VERSION, GitHubActivity, GitHubCollaboration,
    GitHubContributedOrganization, GitHubCoverage, GitHubCraft, GitHubDay, GitHubDeveloperProfile,
    GitHubDimension, GitHubDimensionId, GitHubFlag, GitHubIndex, GitHubMetric,
    GitHubNotableContribution, GitHubOffsetShare, GitHubOrganization, GitHubOrganizations,
    GitHubOwnershipSummary, GitHubProject, GitHubReport, GitHubRepositoryInsight, GitHubRhythm,
    GitHubStackItem, GitHubTechnology, GitHubYear, HygieneSignal, SignalBand, SignalConfidence,
    StackCategory,
};

use crate::collect::affiliated_owners;
use crate::profile::{self, RepositoryKind, SummaryInput};
use crate::snapshot::{GitCommit, PullRequestState, RepositoryAnalysis, Snapshot};
use crate::tech::{self, Detected, Tech, language_of};

/// A gap of up to two hours between commits continues a work session (git-hours convention).
const SESSION_GAP_MINUTES: i64 = 120;
/// Work before the first commit of a session is unknown; credit a conservative half hour.
const FIRST_COMMIT_MINUTES: i64 = 30;

/// Everything derived once and shared by the sections of the report.
struct Facts<'a> {
    snapshot: &'a Snapshot,
    today: NaiveDate,
    /// The account's commits across analyzed repositories, each commit once.
    commits: Vec<(&'a RepositoryAnalysis, &'a GitCommit)>,
    detected: BTreeMap<&'a str, Detected>,
    affiliated: BTreeMap<String, bool>,
    led: BTreeSet<&'a str>,
}

impl<'a> Facts<'a> {
    fn new(snapshot: &'a Snapshot) -> Self {
        let mut seen = HashSet::new();
        let mut commits = Vec::new();
        for repository in &snapshot.analyzed {
            for commit in &repository.commits {
                if seen.insert(commit.sha.as_str()) {
                    commits.push((repository, commit));
                }
            }
        }
        commits.sort_by_key(|(_, commit)| commit.utc());
        let detected = snapshot
            .analyzed
            .iter()
            .map(|repository| {
                let topics = snapshot
                    .repositories
                    .get(&repository.full_name)
                    .map(|meta| meta.topics.clone())
                    .unwrap_or_default();
                (
                    repository.full_name.as_str(),
                    tech::detect(&repository.files, &repository.manifests, &topics),
                )
            })
            .collect();
        let led = snapshot
            .analyzed
            .iter()
            .filter(|repository| {
                repository.user_commits() >= 10 && repository.share_percent() >= 50
            })
            .map(|repository| repository.full_name.as_str())
            .collect();
        Self {
            snapshot,
            today: snapshot.now.date_naive(),
            commits,
            detected,
            affiliated: affiliated_owners(snapshot),
            led,
        }
    }

    fn owner(name: &str) -> &str {
        name.split('/').next().unwrap_or_default()
    }

    fn owned(&self, name: &str) -> bool {
        self.snapshot.is_self(Self::owner(name))
    }

    fn independent(&self, name: &str) -> bool {
        let owner = Self::owner(name);
        !self.snapshot.is_self(owner)
            && !self
                .affiliated
                .keys()
                .any(|known| known.eq_ignore_ascii_case(owner))
            && !self.led.contains(name)
    }

    /// Repositories the account substantially develops: owned, led or with ten commits.
    fn substantive(&self, repository: &RepositoryAnalysis) -> bool {
        let fork = self
            .snapshot
            .repositories
            .get(&repository.full_name)
            .is_some_and(|meta| meta.fork);
        !fork
            && (self.owned(&repository.full_name)
                || self.led.contains(repository.full_name.as_str())
                || repository.user_commits() >= 10)
    }

    fn year_ago(&self) -> DateTime<Utc> {
        self.snapshot.now - Duration::days(365)
    }
}

pub fn build_report(snapshot: &Snapshot) -> GitHubReport {
    let facts = Facts::new(snapshot);
    let activity = Activity::compute(&facts);
    let rhythm = Rhythm::compute(&facts);
    let collaboration = collaboration(&facts);
    let ownership = ownership(&facts);
    let craft = Craft::compute(&facts);
    let (stack, technologies) = stack(&facts);
    let impact = Impact::compute(&facts, &collaboration);
    let flags = flags(&facts, &activity, rhythm.as_ref(), &craft);

    let languages = languages(&facts);
    let developer = developer_profile(&facts, &languages, &stack, &ownership, &activity);

    let dimensions = vec![
        consistency_dimension(&activity),
        engagement_dimension(&activity, rhythm.as_ref()),
        longevity_dimension(&facts, &activity),
        collaboration_dimension(&collaboration),
        ownership_dimension(&facts, &ownership),
        craft_dimension(&craft),
        impact_dimension(&impact),
        scope_dimension(&facts, &developer, &stack),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let weight_sum: u32 = dimensions
        .iter()
        .map(|dimension| u32::from(dimension.weight))
        .sum();
    let weighted: u32 = dimensions
        .iter()
        .map(|dimension| u32::from(dimension.weight) * u32::from(dimension.score))
        .sum();
    let score = (weighted + weight_sum / 2)
        .checked_div(weight_sum)
        .unwrap_or(0) as u8;
    let coverage = coverage(&facts);
    let confidence = confidence(&facts, &activity, &coverage, &flags);
    let highlights = highlights(
        &activity,
        rhythm.as_ref(),
        &collaboration,
        &ownership,
        &craft,
        &impact,
    );

    GitHubReport {
        algorithm_version: GITHUB_INSIGHTS_VERSION.into(),
        generated_at: iso(snapshot.now),
        login: snapshot.login.clone(),
        display_name: snapshot.display_name.clone(),
        profile_url: format!("https://github.com/{}", snapshot.login),
        account_created_at: snapshot.created_at.map(iso).unwrap_or_default(),
        coverage,
        index: GitHubIndex {
            score,
            band: SignalBand::from_score(score),
            confidence,
        },
        dimensions,
        profile: developer,
        activity: activity.report.clone(),
        rhythm: rhythm.as_ref().map(|rhythm| rhythm.report.clone()),
        collaboration,
        ownership,
        organizations: organizations(&facts),
        languages,
        stack,
        technologies,
        craft: craft.report.clone(),
        repositories: repositories(&facts, &craft),
        flags,
        highlights,
    }
}

fn iso(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// ---------------------------------------------------------------------------------------------
// Activity

struct Activity {
    report: GitHubActivity,
    active_months_24: u32,
    sustained_years: u32,
    /// Goh–Barabási burstiness of weekly activity over the last year, in hundredths (−100..100).
    burstiness: Option<i32>,
    days_per_active_week: Option<u32>,
    days_since_last: Option<i64>,
}

fn week_of(date: NaiveDate) -> NaiveDate {
    date - Duration::days(i64::from(date.weekday().num_days_from_monday()))
}

impl Activity {
    fn compute(facts: &Facts<'_>) -> Self {
        let today = facts.today;
        let window_start = today - Duration::days(364);
        // Local calendar days of commits plus the days pull requests were opened.
        let mut days: BTreeMap<NaiveDate, u32> = BTreeMap::new();
        for (_, commit) in &facts.commits {
            *days.entry(commit.authored_at.date_naive()).or_default() += 1;
        }
        for pull in &facts.snapshot.pull_requests {
            *days.entry(pull.created_at.date_naive()).or_default() += 1;
        }
        let active = days
            .iter()
            .filter(|(date, count)| **count > 0 && **date <= today)
            .map(|(date, _)| *date)
            .collect::<Vec<_>>();
        let in_window = active
            .iter()
            .copied()
            .filter(|date| *date >= window_start)
            .collect::<Vec<_>>();
        let sum = |from: NaiveDate, to: NaiveDate| -> u32 {
            days.range(from..=to).map(|(_, count)| count).sum()
        };
        let year_ago = facts.year_ago();
        let found_last_year = facts
            .commits
            .iter()
            .filter(|(_, commit)| commit.utc() >= year_ago)
            .count() as u32;
        // The exact public total from commit search; the analyzed histories when search failed.
        let commits_last_year = facts
            .snapshot
            .totals
            .commits_last_year
            .map_or(found_last_year, |total| total.max(found_last_year));
        let window_weeks = in_window
            .iter()
            .map(|date| week_of(*date))
            .collect::<BTreeSet<_>>();
        let weeks = active
            .iter()
            .map(|date| week_of(*date))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let (longest_daily, current_daily) = runs(&active, 1, today);
        let (longest_weekly, current_weekly) = runs(&weeks, 7, week_of(today));
        let longest_gap = in_window
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).num_days() - 1)
            .max()
            .unwrap_or(0)
            .max(0) as u32;
        let recent = sum(today - Duration::days(181), today);
        let previous = sum(today - Duration::days(363), today - Duration::days(182));
        let trend_percent = match (previous, recent) {
            (0, 0) => None,
            (0, _) => Some(100),
            (previous, recent) => {
                Some(((i64::from(recent) - i64::from(previous)) * 100 / i64::from(previous)) as i32)
            }
        };
        let months_24 = active
            .iter()
            .filter(|date| **date > today - Duration::days(730))
            .map(|date| (date.year(), date.month()))
            .collect::<BTreeSet<_>>()
            .len() as u32;
        let mut weekly_counts = BTreeMap::new();
        for (date, count) in days.range(window_start..=today) {
            *weekly_counts.entry(week_of(*date)).or_insert(0u32) += count;
        }
        let first_week = week_of(window_start);
        let counts = (0..52)
            .map(|week| {
                weekly_counts
                    .get(&(first_week + Duration::days(7 * week)))
                    .copied()
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>();
        let burstiness = (!in_window.is_empty()).then(|| burstiness(&counts));
        let mut days_per_week = BTreeMap::new();
        for date in &in_window {
            *days_per_week.entry(week_of(*date)).or_insert(0u32) += 1;
        }
        let mut yearly: BTreeMap<i32, (u32, BTreeSet<NaiveDate>)> = BTreeMap::new();
        for (_, commit) in &facts.commits {
            let date = commit.authored_at.date_naive();
            let entry = yearly.entry(date.year()).or_default();
            entry.0 += 1;
            entry.1.insert(date);
        }
        // Public commit totals per year come from period searches over all repositories;
        // active days need commit dates and come from the analyzed histories.
        let searched = &facts.snapshot.totals.yearly_commits;
        let mut years = yearly.keys().copied().collect::<BTreeSet<_>>();
        years.extend(
            searched
                .iter()
                .filter(|(_, count)| **count > 0)
                .map(|(year, _)| *year),
        );
        let yearly = years
            .into_iter()
            .map(|year| {
                let (cloned, active) = yearly.get(&year).cloned().unwrap_or_default();
                GitHubYear {
                    year,
                    commits: searched.get(&year).copied().unwrap_or(0).max(cloned),
                    active_days: active.len() as u32,
                    pull_requests: None,
                }
            })
            .collect::<Vec<_>>();
        let active_years = yearly.iter().filter(|year| year.commits > 0).count() as u32;
        let sustained_years = yearly.iter().filter(|year| year.active_days >= 50).count() as u32;
        let calendar = (0..365)
            .map(|offset| {
                let date = window_start + Duration::days(offset);
                GitHubDay {
                    date: date.format("%Y-%m-%d").to_string(),
                    count: days.get(&date).copied().unwrap_or(0),
                }
            })
            .collect();
        let last = active.last().copied();
        Self {
            report: GitHubActivity {
                active_years,
                commits_last_year,
                pull_requests_last_year: facts
                    .snapshot
                    .totals
                    .pull_requests_last_year
                    .unwrap_or_else(|| {
                        facts
                            .snapshot
                            .pull_requests
                            .iter()
                            .filter(|pull| pull.created_at >= year_ago)
                            .count() as u32
                    }),
                active_days_last_year: in_window.len() as u32,
                active_weeks_last_year: (window_weeks.len() as u32).min(52),
                longest_daily_streak: longest_daily,
                current_daily_streak: current_daily,
                longest_weekly_streak: longest_weekly,
                current_weekly_streak: current_weekly,
                longest_gap_days_last_year: longest_gap,
                last_active_on: last.map(|date| date.format("%Y-%m-%d").to_string()),
                trend_percent,
                calendar,
                yearly,
            },
            active_months_24: months_24,
            sustained_years,
            burstiness,
            days_per_active_week: median(days_per_week.values().copied().collect()),
            days_since_last: last.map(|date| (today - date).num_days()),
        }
    }
}

/// Longest run and the run that is still current (ending at `current` or one step before), for
/// sorted distinct dates spaced `step` days apart.
fn runs(dates: &[NaiveDate], step: i64, current: NaiveDate) -> (u32, u32) {
    let mut longest = 0;
    let mut run = 0;
    let mut previous: Option<NaiveDate> = None;
    for date in dates {
        run = match previous {
            Some(before) if (*date - before).num_days() == step => run + 1,
            _ => 1,
        };
        longest = longest.max(run);
        previous = Some(*date);
    }
    let ongoing = previous.is_some_and(|last| (current - last).num_days() <= step);
    (longest, if ongoing { run } else { 0 })
}

/// B = (σ − μ) / (σ + μ): −1 is perfectly regular, 0 is random (Poisson-like), 1 is bursty.
fn burstiness(counts: &[u32]) -> i32 {
    let n = counts.len() as f64;
    let mean = counts.iter().map(|value| f64::from(*value)).sum::<f64>() / n;
    let variance = counts
        .iter()
        .map(|value| (f64::from(*value) - mean).powi(2))
        .sum::<f64>()
        / n;
    let sigma = variance.sqrt();
    if sigma + mean == 0.0 {
        return 0;
    }
    ((sigma - mean) / (sigma + mean) * 100.0).round() as i32
}

// ---------------------------------------------------------------------------------------------
// Rhythm

struct Rhythm {
    report: GitHubRhythm,
    top_minute_share: u8,
}

impl Rhythm {
    /// The last year's commits in the author's own local time; two years when the last is thin.
    fn compute(facts: &Facts<'_>) -> Option<Self> {
        let mut since = facts.year_ago();
        let mut sample = facts
            .commits
            .iter()
            .map(|(_, commit)| *commit)
            .filter(|commit| commit.utc() >= since)
            .collect::<Vec<_>>();
        if sample.len() < 20 {
            since = facts.snapshot.now - Duration::days(730);
            sample = facts
                .commits
                .iter()
                .map(|(_, commit)| *commit)
                .filter(|commit| commit.utc() >= since)
                .collect();
        }
        if sample.len() < 10 {
            return None;
        }
        let total = sample.len() as u32;
        let mut hourly = [0u32; 24];
        let mut weekdays = [0u32; 7];
        let (mut office, mut evening, mut night, mut weekend) = (0, 0, 0, 0);
        let mut offsets: BTreeMap<i32, u32> = BTreeMap::new();
        let mut minutes: HashMap<(u32, u32), u32> = HashMap::new();
        for commit in &sample {
            let local = commit.authored_at;
            let hour = local.hour();
            let weekday = local.weekday().num_days_from_monday();
            hourly[hour as usize] += 1;
            weekdays[weekday as usize] += 1;
            *offsets
                .entry(local.offset().local_minus_utc() / 60)
                .or_insert(0) += 1;
            *minutes.entry((hour, local.minute())).or_insert(0) += 1;
            if weekday >= 5 {
                weekend += 1;
            } else if (9..19).contains(&hour) {
                office += 1;
            }
            if hour >= 19 {
                evening += 1;
            }
            if hour < 6 {
                night += 1;
            }
        }
        // Sessions in UTC order and in the local time of their first commit. The half-hour credit
        // goes to the day a session starts; its span is split across the local days it covers.
        let mut sessions: Vec<(DateTime<FixedOffset>, DateTime<FixedOffset>)> = Vec::new();
        let mut start = sample[0].authored_at;
        let mut last = sample[0].utc();
        for commit in sample.iter().skip(1) {
            if (commit.utc() - last).num_minutes() <= SESSION_GAP_MINUTES {
                last = commit.utc();
                continue;
            }
            sessions.push((start, last.with_timezone(start.offset())));
            start = commit.authored_at;
            last = commit.utc();
        }
        sessions.push((start, last.with_timezone(start.offset())));
        let mut day_minutes: BTreeMap<NaiveDate, i64> = BTreeMap::new();
        for (begin, end) in &sessions {
            *day_minutes.entry(begin.date_naive()).or_insert(0) += FIRST_COMMIT_MINUTES;
            let mut cursor = *begin;
            while cursor < *end {
                let midnight = cursor
                    .date_naive()
                    .succ_opt()
                    .and_then(|day| day.and_hms_opt(0, 0, 0))
                    .and_then(|time| time.and_local_timezone(*cursor.offset()).single())
                    .unwrap_or(*end);
                let until = midnight.min(*end);
                *day_minutes.entry(cursor.date_naive()).or_insert(0) +=
                    (until - cursor).num_minutes();
                cursor = until;
            }
        }
        let mut week_minutes: BTreeMap<NaiveDate, i64> = BTreeMap::new();
        for (day, minutes) in &day_minutes {
            *week_minutes.entry(week_of(*day)).or_insert(0) += minutes;
        }
        let mut daily = day_minutes
            .values()
            .map(|minutes| (*minutes).clamp(0, 24 * 60) as u32)
            .collect::<Vec<_>>();
        daily.sort_unstable();
        let active_days = daily.len() as u32;
        let p90 = daily
            .get((daily.len() * 9).div_ceil(10).saturating_sub(1))
            .copied()
            .unwrap_or(0);
        let weekly = week_minutes
            .values()
            .map(|minutes| (*minutes).clamp(0, 7 * 24 * 60) as u32)
            .collect::<Vec<_>>();
        let top_minute = minutes.values().copied().max().unwrap_or(0);
        Some(Self {
            report: GitHubRhythm {
                sample_size: total,
                timezone_offsets: offsets
                    .iter()
                    .map(|(offset, count)| GitHubOffsetShare {
                        offset_minutes: *offset,
                        percent: percent(*count, total),
                    })
                    .filter(|share| share.percent > 0)
                    .collect(),
                hourly_percent: hourly.iter().map(|count| percent(*count, total)).collect(),
                weekday_percent: weekdays
                    .iter()
                    .map(|count| percent(*count, total))
                    .collect(),
                office_hours_percent: percent(office, total),
                evening_percent: percent(evening, total),
                night_percent: percent(night, total),
                weekend_percent: percent(weekend, total),
                active_days,
                median_minutes_per_active_day: median(daily).unwrap_or(0),
                p90_minutes_per_active_day: p90,
                median_minutes_per_active_week: median(weekly),
                sessions_per_active_day_tenths: (sessions.len() as u32 * 10 + active_days / 2)
                    .checked_div(active_days)
                    .unwrap_or(0),
                longest_session_minutes: sessions
                    .iter()
                    .map(|(begin, end)| {
                        ((*end - *begin).num_minutes() + FIRST_COMMIT_MINUTES).clamp(0, 24 * 60)
                            as u32
                    })
                    .max()
                    .unwrap_or(0),
            },
            top_minute_share: percent(top_minute, total),
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Collaboration and ownership

fn collaboration(facts: &Facts<'_>) -> GitHubCollaboration {
    let totals = &facts.snapshot.totals;
    let external = facts
        .snapshot
        .external_pull_requests
        .iter()
        .filter(|pull| facts.independent(&pull.repository))
        .collect::<Vec<_>>();
    let merged_sample = external
        .iter()
        .filter(|pull| pull.state == PullRequestState::Merged)
        .count() as u32;
    let decided = external
        .iter()
        .filter(|pull| pull.state != PullRequestState::Open)
        .count() as u32;
    let mut accepted: BTreeMap<String, GitHubNotableContribution> = BTreeMap::new();
    let entry = |name: &str| -> GitHubNotableContribution {
        GitHubNotableContribution {
            repository: name.into(),
            url: format!("https://github.com/{name}"),
            stars: facts
                .snapshot
                .repositories
                .get(name)
                .map_or(0, |meta| meta.stars),
            merged_pull_requests: 0,
            commits: 0,
        }
    };
    for pull in external
        .iter()
        .filter(|pull| pull.state == PullRequestState::Merged)
    {
        accepted
            .entry(pull.repository.clone())
            .or_insert_with(|| entry(&pull.repository))
            .merged_pull_requests += 1;
    }
    for repository in facts.snapshot.analyzed.iter().filter(|repository| {
        repository.user_commits() > 0 && facts.independent(&repository.full_name)
    }) {
        accepted
            .entry(repository.full_name.clone())
            .or_insert_with(|| entry(&repository.full_name))
            .commits += repository.user_commits();
    }
    let external_repositories = accepted.len() as u32;
    let mut notable = accepted
        .into_values()
        .filter(|item| item.stars >= 100)
        .collect::<Vec<_>>();
    notable.sort_by(|a, b| b.stars.cmp(&a.stars).then(a.repository.cmp(&b.repository)));
    notable.truncate(10);
    GitHubCollaboration {
        pull_requests_total: totals
            .pull_requests
            .unwrap_or(facts.snapshot.pull_requests.len() as u32),
        approved_pull_requests: totals.approved_pull_requests.unwrap_or(0),
        reviews_of_others: totals.reviews_of_others.unwrap_or(0),
        independent_reviews: totals.independent_reviews.unwrap_or(0),
        issues_total: totals.issues.unwrap_or(0),
        external_pull_requests: totals
            .external_pull_requests
            .unwrap_or(external.len() as u32),
        external_merged: totals.external_merged.unwrap_or(merged_sample),
        external_merge_rate_percent: (decided >= 3).then(|| percent(merged_sample, decided)),
        external_repositories,
        notable_contributions: notable,
    }
}

fn ownership(facts: &Facts<'_>) -> GitHubOwnershipSummary {
    let year_ago = facts.year_ago();
    let own_organizations = facts
        .affiliated
        .iter()
        .filter(|(_, organization)| **organization)
        .map(|(owner, _)| owner.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let analyzed = &facts.snapshot.analyzed;
    GitHubOwnershipSummary {
        led_repositories: facts.led.len() as u32,
        maintained_repositories: analyzed
            .iter()
            .filter(|repository| {
                (facts.owned(&repository.full_name)
                    || facts.led.contains(repository.full_name.as_str()))
                    && repository
                        .last_commit()
                        .is_some_and(|last| last >= year_ago)
            })
            .count() as u32,
        own_organizations: own_organizations.len() as u32,
        own_organization_repositories: analyzed
            .iter()
            .filter(|repository| {
                repository.user_commits() > 0
                    && own_organizations
                        .contains(&Facts::owner(&repository.full_name).to_ascii_lowercase())
            })
            .count() as u32,
        direct_push_repositories: analyzed
            .iter()
            .filter(|repository| repository.direct_pushes > 0)
            .count() as u32,
    }
}

// ---------------------------------------------------------------------------------------------
// Craft

struct Craft {
    report: GitHubCraft,
    hygiene: BTreeMap<String, Vec<HygieneSignal>>,
    message_sample: u32,
    repeated_message_share: u8,
}

fn hygiene(
    repository: &RepositoryAnalysis,
    meta: Option<&crate::snapshot::RepositoryMeta>,
) -> Vec<HygieneSignal> {
    let files = repository
        .files
        .iter()
        .map(|path| path.to_ascii_lowercase())
        .filter(|path| {
            !path
                .split('/')
                .any(|segment| segment == "node_modules" || segment == "vendor")
        })
        .collect::<Vec<_>>();
    let root =
        |test: &dyn Fn(&str) -> bool| files.iter().any(|path| !path.contains('/') && test(path));
    let any = |test: &dyn Fn(&str) -> bool| files.iter().any(|path| test(path));
    let name = |path: &str| path.rsplit('/').next().unwrap_or(path).to_owned();
    let mut signals = Vec::new();
    if root(&|path| path.starts_with("readme")) {
        signals.push(HygieneSignal::Readme);
    }
    if meta.is_some_and(|meta| meta.license.is_some())
        || root(&|path| {
            path.starts_with("license")
                || path.starts_with("licence")
                || path.starts_with("copying")
        })
    {
        signals.push(HygieneSignal::License);
    }
    if any(&|path| {
        path.starts_with(".github/workflows/")
            || matches!(
                path,
                ".gitlab-ci.yml"
                    | ".travis.yml"
                    | "azure-pipelines.yml"
                    | "jenkinsfile"
                    | ".drone.yml"
                    | "bitbucket-pipelines.yml"
            )
            || path.starts_with(".circleci/")
            || path.starts_with(".woodpecker")
            || path.starts_with(".buildkite/")
    }) {
        signals.push(HygieneSignal::Ci);
    }
    if any(&|path| {
        let file = name(path);
        path.split('/').any(|segment| {
            matches!(
                segment,
                "tests" | "test" | "__tests__" | "spec" | "specs" | "e2e" | "testing"
            )
        }) && path.contains('/')
            || file.ends_with("_test.go")
            || file.ends_with("_test.py")
            || file.starts_with("test_") && file.ends_with(".py")
            || [
                ".test.ts",
                ".test.tsx",
                ".test.js",
                ".test.jsx",
                ".spec.ts",
                ".spec.tsx",
                ".spec.js",
            ]
            .iter()
            .any(|suffix| file.ends_with(suffix))
            || file.ends_with("test.java")
            || file.ends_with("tests.cs")
            || matches!(file.as_str(), "pytest.ini" | "conftest.py" | "nextest.toml")
    }) {
        signals.push(HygieneSignal::Tests);
    }
    const LOCKFILES: &[&str] = &[
        "cargo.lock",
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "bun.lock",
        "bun.lockb",
        "poetry.lock",
        "uv.lock",
        "pipfile.lock",
        "go.sum",
        "composer.lock",
        "gemfile.lock",
        "mix.lock",
        "flake.lock",
    ];
    if any(&|path| LOCKFILES.contains(&name(path).as_str())) {
        signals.push(HygieneSignal::Lockfile);
    }
    const LINT: &[&str] = &[
        "rustfmt.toml",
        ".rustfmt.toml",
        "clippy.toml",
        ".clippy.toml",
        "deny.toml",
        "biome.json",
        "biome.jsonc",
        ".editorconfig",
        "ruff.toml",
        ".ruff.toml",
        ".pre-commit-config.yaml",
        ".golangci.yml",
        ".golangci.yaml",
        ".rubocop.yml",
        ".flake8",
        ".pylintrc",
        ".clang-format",
        ".clang-tidy",
        ".swiftlint.yml",
    ];
    if any(&|path| {
        let file = name(path);
        LINT.contains(&file.as_str())
            || [
                ".eslintrc",
                "eslint.config.",
                ".prettierrc",
                "prettier.config.",
                ".stylelintrc",
            ]
            .iter()
            .any(|prefix| file.starts_with(prefix))
    }) {
        signals.push(HygieneSignal::Lint);
    }
    if any(&|path| {
        matches!(
            path,
            ".github/dependabot.yml"
                | ".github/dependabot.yaml"
                | "renovate.json"
                | ".github/renovate.json"
        ) || name(path).starts_with(".renovaterc")
    }) {
        signals.push(HygieneSignal::DependencyUpdates);
    }
    if any(&|path| {
        let file = name(path);
        file == "dockerfile"
            || file == "containerfile"
            || file.starts_with("dockerfile.")
            || file.starts_with("docker-compose")
            || file == "compose.yml"
            || file == "compose.yaml"
    }) {
        signals.push(HygieneSignal::Container);
    }
    if meta.is_some_and(|meta| meta.description.is_some()) {
        signals.push(HygieneSignal::Description);
    }
    signals
}

const CONVENTIONAL_TYPES: &[&str] = &[
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert",
];

pub(crate) fn is_conventional(headline: &str) -> bool {
    let Some((head, rest)) = headline.split_once(':') else {
        return false;
    };
    if !rest.starts_with(' ') || rest.trim().is_empty() {
        return false;
    }
    let head = head.strip_suffix('!').unwrap_or(head);
    let kind = match head.split_once('(') {
        Some((kind, scope)) if scope.ends_with(')') && scope.len() > 1 => kind,
        Some(_) => return false,
        None => head,
    };
    CONVENTIONAL_TYPES.contains(&kind.to_ascii_lowercase().as_str())
}

/// A headline says what changed: long enough, not a placeholder, not a web-editor default.
pub(crate) fn is_descriptive(headline: &str) -> bool {
    let text = headline.trim();
    let lower = text.to_lowercase();
    const PLACEHOLDERS: &[&str] = &[
        "update",
        "updates",
        "fix",
        "fixes",
        "wip",
        "test",
        "tests",
        "changes",
        "change",
        "minor",
        "commit",
        ".",
        "-",
        "tmp",
        "temp",
        "asdf",
        "save",
        "stuff",
        "misc",
        "cleanup",
        "refactor",
        "initial commit",
        "first commit",
        "add files via upload",
    ];
    if text.chars().count() < 12 || PLACEHOLDERS.contains(&lower.as_str()) {
        return false;
    }
    let mut words = text.split_whitespace();
    let verb = words.next().unwrap_or_default();
    let object = words.next();
    let single_file = object.is_some_and(|word| word.contains('.')) && words.next().is_none();
    !(["Update", "Create", "Delete", "Rename"].contains(&verb) && single_file)
}

impl Craft {
    fn compute(facts: &Facts<'_>) -> Self {
        let hygiene_map = facts
            .snapshot
            .analyzed
            .iter()
            .filter(|repository| facts.substantive(repository))
            .map(|repository| {
                (
                    repository.full_name.clone(),
                    hygiene(
                        repository,
                        facts.snapshot.repositories.get(&repository.full_name),
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let total = hygiene_map.len() as u32;
        let share = |signal: HygieneSignal| {
            percent(
                hygiene_map
                    .values()
                    .filter(|signals| signals.contains(&signal))
                    .count() as u32,
                total,
            )
        };
        let all = facts
            .commits
            .iter()
            .map(|(_, commit)| *commit)
            .collect::<Vec<_>>();
        let regular = all
            .iter()
            .filter(|commit| !commit.is_merge())
            .collect::<Vec<_>>();
        let sample = regular.len() as u32;
        let enough = sample >= 10;
        let files = regular
            .iter()
            .map(|commit| commit.files.len() as u32)
            .collect::<Vec<_>>();
        let mut headlines: HashMap<&str, u32> = HashMap::new();
        for commit in &regular {
            *headlines.entry(commit.subject.trim()).or_insert(0) += 1;
        }
        let top_headline = headlines.values().copied().max().unwrap_or(0);
        let of = |count: usize| enough.then(|| percent(count as u32, sample));
        Self {
            report: GitHubCraft {
                substantive_repositories: total,
                readme_percent: share(HygieneSignal::Readme),
                license_percent: share(HygieneSignal::License),
                ci_percent: share(HygieneSignal::Ci),
                tests_percent: share(HygieneSignal::Tests),
                lockfile_percent: share(HygieneSignal::Lockfile),
                lint_percent: share(HygieneSignal::Lint),
                dependency_updates_percent: share(HygieneSignal::DependencyUpdates),
                container_percent: share(HygieneSignal::Container),
                conventional_commits_percent: of(regular
                    .iter()
                    .filter(|commit| is_conventional(&commit.subject))
                    .count()),
                descriptive_messages_percent: of(regular
                    .iter()
                    .filter(|commit| is_descriptive(&commit.subject))
                    .count()),
                median_files_per_commit: if enough { median(files.clone()) } else { None },
                large_commits_percent: of(files.iter().filter(|count| **count > 50).count()),
                merge_commits_percent: (all.len() >= 10).then(|| {
                    percent(
                        all.iter().filter(|commit| commit.is_merge()).count() as u32,
                        all.len() as u32,
                    )
                }),
            },
            hygiene: hygiene_map,
            message_sample: sample,
            repeated_message_share: percent(top_headline, sample),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Stack and languages

/// Repositories, reasons and the latest own commit per technology.
type Evidence = (u32, BTreeSet<String>, Option<DateTime<Utc>>);

fn stack(facts: &Facts<'_>) -> (Vec<GitHubStackItem>, Vec<GitHubTechnology>) {
    let mut found: BTreeMap<Tech, Evidence> = BTreeMap::new();
    for repository in &facts.snapshot.analyzed {
        // Technologies count where the account works: owned repositories or three own commits.
        if !(facts.owned(&repository.full_name) || repository.user_commits() >= 3) {
            continue;
        }
        let Some(detected) = facts.detected.get(repository.full_name.as_str()) else {
            continue;
        };
        for (technology, reasons) in detected {
            let entry = found.entry(*technology).or_default();
            entry.0 += 1;
            entry.1.extend(reasons.iter().cloned());
            entry.2 = entry.2.max(repository.last_commit());
        }
    }
    let mut items = found
        .iter()
        .map(
            |(technology, (repositories, reasons, last))| GitHubStackItem {
                name: technology.name.into(),
                category: technology.category,
                repositories: *repositories,
                signals: reasons.iter().take(4).cloned().collect(),
                last_used_at: last.map(iso),
            },
        )
        .collect::<Vec<_>>();
    items.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then(b.repositories.cmp(&a.repositories))
            .then(a.name.cmp(&b.name))
    });
    let mut catalog: BTreeMap<&str, Evidence> = BTreeMap::new();
    for (technology, (repositories, reasons, last)) in &found {
        if let Some(skill) = technology.catalog_skill() {
            let entry = catalog.entry(skill).or_default();
            entry.0 = entry.0.max(*repositories);
            entry.1.extend(reasons.iter().cloned());
            entry.2 = entry.2.max(*last);
        }
    }
    let technologies = catalog
        .into_iter()
        .map(|(skill, (repositories, reasons, last))| GitHubTechnology {
            skill_id: skill.into(),
            repositories,
            signals: reasons.into_iter().take(5).collect(),
            last_pushed_at: last.map(iso),
        })
        .collect();
    (items, technologies)
}

fn languages(facts: &Facts<'_>) -> Vec<fsp_domain::GitHubLanguageShare> {
    let mut changes: BTreeMap<&'static str, u32> = BTreeMap::new();
    for (_, commit) in facts
        .commits
        .iter()
        .filter(|(_, commit)| !commit.is_merge())
    {
        for file in &commit.files {
            if let Some(language) = language_of(file) {
                *changes.entry(language).or_default() += 1;
            }
        }
    }
    profile::languages(&changes)
}

fn developer_profile(
    facts: &Facts<'_>,
    languages: &[fsp_domain::GitHubLanguageShare],
    stack: &[GitHubStackItem],
    ownership: &GitHubOwnershipSummary,
    activity: &Activity,
) -> GitHubDeveloperProfile {
    let mut areas: BTreeMap<Archetype, (u32, BTreeMap<&'static str, u32>)> = BTreeMap::new();
    for (repository, commit) in facts
        .commits
        .iter()
        .filter(|(_, commit)| !commit.is_merge())
    {
        let kind = facts
            .detected
            .get(repository.full_name.as_str())
            .map(RepositoryKind::of)
            .unwrap_or_default();
        for file in &commit.files {
            if let Some(area) = profile::area(file, kind) {
                let entry = areas.entry(area).or_default();
                entry.0 += 1;
                if let Some(language) = language_of(file) {
                    *entry.1.entry(language).or_default() += 1;
                }
            }
        }
    }
    let mut stack_by_area: BTreeMap<Archetype, Vec<&str>> = BTreeMap::new();
    for item in stack {
        if let Some(area) = profile::archetype_of(&item.name, item.category) {
            stack_by_area
                .entry(area)
                .or_default()
                .push(item.name.as_str());
        }
    }
    let archetypes = profile::archetypes(&areas, &stack_by_area);
    // Technologies shared by several projects; a small account shows its single-project stack.
    let mut ranked = stack
        .iter()
        .filter(|item| {
            !matches!(
                item.category,
                fsp_domain::StackCategory::Language | fsp_domain::StackCategory::Quality
            )
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|a, b| {
        b.repositories
            .cmp(&a.repositories)
            .then(a.category.cmp(&b.category))
            .then(a.name.cmp(&b.name))
    });
    let shared = ranked.iter().filter(|item| item.repositories >= 2).count();
    let key_technologies = ranked
        .into_iter()
        .take(shared.clamp(4, 8))
        .map(|item| item.name.as_str())
        .collect::<Vec<_>>();
    let own_organizations = facts
        .affiliated
        .iter()
        .filter(|(_, organization)| **organization)
        .map(|(owner, _)| owner.clone())
        .collect::<Vec<_>>();
    let mut topic_counts: BTreeMap<String, u32> = BTreeMap::new();
    for repository in &facts.snapshot.analyzed {
        if !(facts.owned(&repository.full_name)
            || facts.led.contains(repository.full_name.as_str()))
        {
            continue;
        }
        if let Some(meta) = facts.snapshot.repositories.get(&repository.full_name) {
            for topic in meta
                .topics
                .iter()
                .filter(|topic| profile::meaningful_topic(topic))
            {
                *topic_counts.entry(topic.clone()).or_default() += 1;
            }
        }
    }
    let mut topics = topic_counts.into_iter().collect::<Vec<_>>();
    topics.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let focus_topics = topics
        .into_iter()
        .take(8)
        .map(|(topic, _)| topic)
        .collect::<Vec<_>>();
    let (summary, public_summary) = profile::summaries(&SummaryInput {
        archetypes: &archetypes,
        languages,
        key_technologies: &key_technologies,
        led_repositories: ownership.led_repositories,
        own_organizations: &own_organizations,
        focus_topics: &focus_topics,
        commits_last_year: activity.report.commits_last_year,
    });
    let mut projects = facts
        .snapshot
        .analyzed
        .iter()
        .filter(|repository| {
            facts.led.contains(repository.full_name.as_str())
                || (facts.owned(&repository.full_name) && repository.user_commits() > 0)
        })
        .collect::<Vec<_>>();
    projects.sort_by_key(|repository| std::cmp::Reverse(repository.user_commits()));
    let top_projects = projects
        .into_iter()
        .take(8)
        .map(|repository| {
            let meta = facts.snapshot.repositories.get(&repository.full_name);
            GitHubProject {
                name_with_owner: repository.full_name.clone(),
                url: format!("https://github.com/{}", repository.full_name),
                description: meta.and_then(|meta| meta.description.clone()),
                primary_language: meta.and_then(|meta| meta.language.clone()),
                stars: meta.map_or(0, |meta| meta.stars),
                user_commits: repository.user_commits(),
                user_share_percent: repository.share_percent(),
                last_commit_at: repository.last_commit().map(iso),
            }
        })
        .collect();
    GitHubDeveloperProfile {
        summary,
        public_summary,
        archetypes,
        focus_topics,
        top_projects,
    }
}

// ---------------------------------------------------------------------------------------------
// Impact

struct Impact {
    maintained_stars: u32,
    owned_forks: u32,
    notable_external: u32,
    followers: u32,
}

impl Impact {
    fn compute(facts: &Facts<'_>, collaboration: &GitHubCollaboration) -> Self {
        let meta = |name: &str| facts.snapshot.repositories.get(name);
        Self {
            maintained_stars: facts
                .snapshot
                .analyzed
                .iter()
                .filter(|repository| facts.substantive(repository))
                .map(|repository| meta(&repository.full_name).map_or(0, |meta| meta.stars))
                .sum(),
            owned_forks: facts
                .snapshot
                .repositories
                .values()
                .filter(|meta| facts.snapshot.is_self(&meta.owner) && !meta.fork)
                .map(|meta| meta.forks)
                .sum(),
            notable_external: collaboration
                .notable_contributions
                .iter()
                .filter(|item| {
                    item.stars >= 1000 && (item.commits >= 3 || item.merged_pull_requests >= 1)
                })
                .count() as u32,
            followers: facts.snapshot.followers,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Dimensions

struct MetricSpec {
    id: &'static str,
    label: &'static str,
    value: i64,
    display: String,
    public_display: String,
    score: u8,
    weight: u8,
}

fn dimension(
    id: GitHubDimensionId,
    label: &str,
    weight: u8,
    summaries: [&str; 4],
    metrics: Vec<MetricSpec>,
) -> Option<GitHubDimension> {
    let total: u32 = metrics.iter().map(|metric| u32::from(metric.weight)).sum();
    if total == 0 {
        return None;
    }
    let weighted: u32 = metrics
        .iter()
        .map(|metric| u32::from(metric.weight) * u32::from(metric.score))
        .sum();
    let score = (weighted + total / 2).checked_div(total).unwrap_or(0) as u8;
    let summary = match score {
        75..=100 => summaries[0],
        50..=74 => summaries[1],
        25..=49 => summaries[2],
        _ => summaries[3],
    };
    Some(GitHubDimension {
        id,
        label: label.into(),
        score,
        weight,
        summary: summary.into(),
        metrics: metrics
            .into_iter()
            .map(|metric| GitHubMetric {
                id: metric.id.into(),
                label: metric.label.into(),
                value: metric.value,
                display: metric.display,
                public_display: metric.public_display,
                score: metric.score,
            })
            .collect(),
    })
}

fn count_metric(
    id: &'static str,
    label: &'static str,
    value: u32,
    unit: (&str, &str, &str),
    score: u8,
    weight: u8,
) -> MetricSpec {
    MetricSpec {
        id,
        label,
        value: i64::from(value),
        display: with_unit(group(value), value, unit),
        public_display: with_unit(approx(value), approx_value(value), unit),
        score,
        weight,
    }
}

fn percent_metric(
    id: &'static str,
    label: &'static str,
    value: u8,
    score: u8,
    weight: u8,
) -> MetricSpec {
    MetricSpec {
        id,
        label,
        value: i64::from(value),
        display: format!("{value}%"),
        public_display: format!("≈{}%", round_to(u32::from(value), 5)),
        score,
        weight,
    }
}

const DAYS: (&str, &str, &str) = ("день", "дня", "дней");
const WEEKS: (&str, &str, &str) = ("неделя", "недели", "недель");
const MONTHS: (&str, &str, &str) = ("месяц", "месяца", "месяцев");
const YEARS: (&str, &str, &str) = ("год", "года", "лет");
const ITEMS: (&str, &str, &str) = ("", "", "");

fn consistency_dimension(activity: &Activity) -> Option<GitHubDimension> {
    let report = &activity.report;
    let regularity = activity
        .burstiness
        .map_or(0, |b| ramp(f64::from(80 - b.clamp(0, 80)), 0.0, 80.0));
    let weeks = report.active_weeks_last_year;
    dimension(
        GitHubDimensionId::Consistency,
        "Регулярность",
        15,
        [
            "Стабильная регулярная активность на протяжении года",
            "Активность в основном регулярная",
            "Активность эпизодическая",
            "Регулярной публичной активности почти нет",
        ],
        vec![
            MetricSpec {
                id: "active_weeks",
                label: "Активных недель за год",
                value: i64::from(weeks),
                display: format!("{weeks} из 52"),
                public_display: format!("≈{} из 52", round_to(weeks, 5).min(52)),
                score: ramp(f64::from(weeks), 0.0, 40.0),
                weight: 35,
            },
            count_metric(
                "longest_weekly_streak",
                "Самая длинная серия активных недель",
                report.longest_weekly_streak,
                WEEKS,
                ramp(f64::from(report.longest_weekly_streak), 0.0, 26.0),
                20,
            ),
            count_metric(
                "current_weekly_streak",
                "Текущая серия активных недель",
                report.current_weekly_streak,
                WEEKS,
                ramp(f64::from(report.current_weekly_streak), 0.0, 12.0),
                15,
            ),
            count_metric(
                "active_months_24",
                "Активных месяцев за два года",
                activity.active_months_24,
                MONTHS,
                ramp(f64::from(activity.active_months_24), 0.0, 20.0),
                15,
            ),
            MetricSpec {
                id: "regularity",
                label: "Равномерность по неделям",
                value: i64::from(activity.burstiness.unwrap_or(0)),
                display: regularity_label(activity.burstiness).into(),
                public_display: regularity_label(activity.burstiness).into(),
                score: regularity,
                weight: 15,
            },
        ],
    )
}

fn regularity_label(burstiness: Option<i32>) -> &'static str {
    match burstiness {
        None => "нет данных",
        Some(value) if value <= 0 => "равномерная",
        Some(value) if value <= 40 => "умеренные всплески",
        Some(_) => "редкие всплески",
    }
}

fn engagement_dimension(activity: &Activity, rhythm: Option<&Rhythm>) -> Option<GitHubDimension> {
    let report = &activity.report;
    let mut metrics = vec![
        count_metric(
            "commits_last_year",
            "Публичных коммитов за год",
            report.commits_last_year,
            ITEMS,
            log_ramp(f64::from(report.commits_last_year), 1000.0),
            15,
        ),
        count_metric(
            "active_days_last_year",
            "Активных дней за год",
            report.active_days_last_year,
            DAYS,
            ramp(f64::from(report.active_days_last_year), 0.0, 180.0),
            20,
        ),
        count_metric(
            "longest_daily_streak",
            "Самая длинная серия дней подряд",
            report.longest_daily_streak,
            DAYS,
            ramp(f64::from(report.longest_daily_streak), 0.0, 30.0),
            15,
        ),
    ];
    if let Some(days) = activity.days_per_active_week {
        metrics.push(count_metric(
            "days_per_active_week",
            "Дней в активную неделю (медиана)",
            days,
            DAYS,
            ramp(f64::from(days), 0.0, 5.0),
            10,
        ));
    }
    if let Some(rhythm) = rhythm.filter(|rhythm| rhythm.report.sample_size >= 20) {
        let minutes = rhythm.report.median_minutes_per_active_day;
        metrics.push(MetricSpec {
            id: "hours_per_active_day",
            label: "Время в активный день (медиана)",
            value: i64::from(minutes),
            display: hours_exact(minutes),
            public_display: fsp_domain::hours_band(minutes),
            // A full working day of commit sessions earns the maximum.
            score: ramp(f64::from(minutes), 0.0, 360.0),
            weight: 15,
        });
        if let Some(weekly) = rhythm.report.median_minutes_per_active_week {
            metrics.push(MetricSpec {
                id: "hours_per_active_week",
                label: "Время в активную неделю (медиана)",
                value: i64::from(weekly),
                display: hours_exact(weekly),
                public_display: fsp_domain::week_band(weekly),
                score: ramp(f64::from(weekly), 0.0, 1800.0),
                weight: 10,
            });
        }
    }
    let since = activity.days_since_last;
    metrics.push(MetricSpec {
        id: "days_since_last_activity",
        label: "Дней с последней активности",
        value: since.unwrap_or(-1),
        display: since.map_or_else(|| "нет активности".into(), |days| days.to_string()),
        public_display: since.map_or_else(
            || "нет активности".into(),
            |days| match days {
                0..=7 => "до недели".into(),
                8..=31 => "до месяца".into(),
                32..=180 => "до полугода".into(),
                _ => "больше полугода".into(),
            },
        ),
        score: since.map_or(0, |days| inverse_ramp(days as f64, 14.0, 180.0)),
        weight: 15,
    });
    dimension(
        GitHubDimensionId::Engagement,
        "Вовлечённость",
        15,
        [
            "Высокая вовлечённость: много активных дней, длинные серии и рабочие часы",
            "Заметная вовлечённость",
            "Умеренная вовлечённость",
            "Низкая публичная вовлечённость",
        ],
        metrics,
    )
}

fn longevity_dimension(facts: &Facts<'_>, activity: &Activity) -> Option<GitHubDimension> {
    let age_years = facts.snapshot.created_at.map_or(0, |created| {
        ((facts.today - created.date_naive()).num_days() / 365) as u32
    });
    dimension(
        GitHubDimensionId::Longevity,
        "Стаж",
        10,
        [
            "Многолетняя устойчивая активность",
            "Несколько лет активности",
            "Небольшой публичный стаж",
            "Публичная история только начинается",
        ],
        vec![
            count_metric(
                "account_age",
                "Возраст аккаунта",
                age_years,
                YEARS,
                ramp(f64::from(age_years), 0.0, 6.0),
                30,
            ),
            count_metric(
                "active_years",
                "Лет с публичными коммитами",
                activity.report.active_years,
                YEARS,
                ramp(f64::from(activity.report.active_years), 0.0, 5.0),
                40,
            ),
            count_metric(
                "sustained_years",
                "Лет с 50+ активными днями",
                activity.sustained_years,
                YEARS,
                ramp(f64::from(activity.sustained_years), 0.0, 4.0),
                30,
            ),
        ],
    )
}

fn collaboration_dimension(report: &GitHubCollaboration) -> Option<GitHubDimension> {
    let mut metrics = vec![
        count_metric(
            "approved_pull_requests",
            "PR, одобренные другими людьми в ревью",
            report.approved_pull_requests,
            ITEMS,
            log_ramp(f64::from(report.approved_pull_requests), 25.0),
            35,
        ),
        count_metric(
            "independent_reviews",
            "Ревью PR в независимых репозиториях",
            report.independent_reviews,
            ITEMS,
            log_ramp(f64::from(report.independent_reviews), 50.0),
            25,
        ),
        count_metric(
            "external_repositories",
            "Независимых проектов с принятым вкладом",
            report.external_repositories,
            ITEMS,
            ramp(f64::from(report.external_repositories), 0.0, 10.0),
            20,
        ),
        count_metric(
            "external_merged",
            "Принятых PR в независимые проекты",
            report.external_merged,
            ITEMS,
            log_ramp(f64::from(report.external_merged), 25.0),
            20,
        ),
    ];
    if let Some(rate) = report.external_merge_rate_percent {
        metrics.push(percent_metric(
            "external_merge_rate",
            "Доля принятых PR в независимые проекты",
            rate,
            ramp(f64::from(rate), 30.0, 90.0),
            10,
        ));
    }
    dimension(
        GitHubDimensionId::Collaboration,
        "Совместная работа",
        15,
        [
            "Регулярная работа с другими людьми: ревью и принятые изменения",
            "Есть подтверждённая совместная работа",
            "Совместная работа подтверждается слабо",
            "Публичных следов совместной работы почти нет",
        ],
        metrics,
    )
}

fn ownership_dimension(
    facts: &Facts<'_>,
    report: &GitHubOwnershipSummary,
) -> Option<GitHubDimension> {
    let pulls = facts.snapshot.totals.pull_requests_last_year.unwrap_or(0);
    dimension(
        GitHubDimensionId::Ownership,
        "Собственные проекты и организации",
        15,
        [
            "Ведёт много живых проектов и развивает свои организации",
            "Ведёт собственные проекты",
            "Несколько собственных проектов",
            "Собственных публичных проектов почти нет",
        ],
        vec![
            count_metric(
                "led_repositories",
                "Проектов, где пишет большую часть кода",
                report.led_repositories,
                ITEMS,
                ramp(f64::from(report.led_repositories), 0.0, 10.0),
                25,
            ),
            count_metric(
                "maintained_repositories",
                "Проектов с коммитами за год",
                report.maintained_repositories,
                ITEMS,
                ramp(f64::from(report.maintained_repositories), 0.0, 8.0),
                20,
            ),
            count_metric(
                "own_organizations",
                "Собственных организаций",
                report.own_organizations,
                ITEMS,
                ramp(f64::from(report.own_organizations), 0.0, 3.0),
                15,
            ),
            count_metric(
                "own_organization_repositories",
                "Репозиториев в собственных организациях",
                report.own_organization_repositories,
                ITEMS,
                ramp(f64::from(report.own_organization_repositories), 0.0, 15.0),
                20,
            ),
            count_metric(
                "pull_requests_last_year",
                "Pull request’ов за год",
                pulls,
                ITEMS,
                log_ramp(f64::from(pulls), 300.0),
                20,
            ),
        ],
    )
}

fn craft_dimension(craft: &Craft) -> Option<GitHubDimension> {
    let report = &craft.report;
    if report.substantive_repositories == 0 {
        return None;
    }
    let practice = |id, label, value: u8, weight| percent_metric(id, label, value, value, weight);
    let mut metrics = vec![
        practice("readme", "Проекты с README", report.readme_percent, 10),
        practice("license", "Проекты с лицензией", report.license_percent, 10),
        practice("ci", "Проекты с CI", report.ci_percent, 20),
        practice("tests", "Проекты с тестами", report.tests_percent, 20),
        practice(
            "lockfile",
            "Зафиксированные зависимости",
            report.lockfile_percent,
            10,
        ),
        practice("lint", "Линтеры и форматтеры", report.lint_percent, 10),
        practice(
            "dependency_updates",
            "Автообновление зависимостей",
            report.dependency_updates_percent,
            5,
        ),
    ];
    if let Some(value) = report.descriptive_messages_percent {
        metrics.push(percent_metric(
            "descriptive_messages",
            "Содержательные сообщения коммитов",
            value,
            value,
            10,
        ));
    }
    if let Some(files) = report.median_files_per_commit {
        // Focused commits score best; empty or sweeping medians score less.
        let score = match files {
            0 => 0,
            1..=10 => 100,
            11..=30 => 60,
            _ => 20,
        };
        metrics.push(MetricSpec {
            id: "median_files_per_commit",
            label: "Файлов в коммите (медиана)",
            value: i64::from(files),
            display: files.to_string(),
            public_display: approx(files),
            score,
            weight: 5,
        });
    }
    dimension(
        GitHubDimensionId::Craft,
        "Инженерная культура",
        15,
        [
            "Проекты оформлены зрело: CI, тесты, зависимости и стиль",
            "Хорошие инженерные практики в большинстве проектов",
            "Инженерные практики применяются выборочно",
            "Инженерных практик в публичных проектах почти не видно",
        ],
        metrics,
    )
}

/// Own work that spans this many days makes a project a sustained system, not an experiment.
const SUSTAINED_SYSTEM_DAYS: i64 = 180;

/// Scope and complexity, the axis engineering level frameworks rank first: systems the account
/// sustains, the engineering areas and system layers it covers, and design work beyond code.
fn scope_dimension(
    facts: &Facts<'_>,
    developer: &GitHubDeveloperProfile,
    stack: &[GitHubStackItem],
) -> Option<GitHubDimension> {
    let substantive = facts
        .snapshot
        .analyzed
        .iter()
        .filter(|repository| facts.substantive(repository))
        .collect::<Vec<_>>();
    let sustained = substantive
        .iter()
        .filter(|repository| {
            (facts.owned(&repository.full_name)
                || facts.led.contains(repository.full_name.as_str()))
                && repository.user_commits() >= 10
                && own_span_days(repository) >= SUSTAINED_SYSTEM_DAYS
        })
        .count() as u32;
    let areas = developer
        .archetypes
        .iter()
        .filter(|area| area.percent >= 15)
        .count() as u32;
    let layers = stack
        .iter()
        .filter(|item| item.category != StackCategory::Language && item.repositories >= 2)
        .map(|item| item.category)
        .collect::<BTreeSet<_>>()
        .len() as u32;
    let designed = substantive
        .iter()
        .filter(|repository| repository.files.iter().any(|path| design_artifact(path)))
        .count() as u32;
    dimension(
        GitHubDimensionId::Scope,
        "Масштаб и сложность",
        10,
        [
            "Развивает долгоживущие многослойные системы с проектной документацией",
            "Заметный масштаб: несколько направлений и долгие проекты",
            "Масштаб работы пока ограничен",
            "Масштаб и сложность работы почти не видны",
        ],
        vec![
            count_metric(
                "sustained_systems",
                "Своих проектов, которые развивает полгода и дольше",
                sustained,
                ITEMS,
                ramp(f64::from(sustained), 0.0, 5.0),
                35,
            ),
            count_metric(
                "engineering_areas",
                "Направлений разработки с долей от 15%",
                areas,
                ITEMS,
                ramp(f64::from(areas), 0.0, 3.0),
                20,
            ),
            count_metric(
                "system_layers",
                "Слоёв системы, используемых в двух и более проектах",
                layers,
                ITEMS,
                ramp(f64::from(layers), 0.0, 6.0),
                20,
            ),
            count_metric(
                "design_artifacts",
                "Проектов с архитектурными решениями, контрактами API или миграциями",
                designed,
                ITEMS,
                ramp(f64::from(designed), 0.0, 5.0),
                25,
            ),
        ],
    )
}

/// Days between the account's first and last commit in a repository.
fn own_span_days(repository: &RepositoryAnalysis) -> i64 {
    let first = repository.commits.iter().map(GitCommit::utc).min();
    let last = repository.commits.iter().map(GitCommit::utc).max();
    match (first, last) {
        (Some(first), Some(last)) => (last - first).num_days(),
        _ => 0,
    }
}

/// Architecture decision records and design documents, interface contracts (OpenAPI, AsyncAPI,
/// GraphQL, Protocol Buffers) and database schema migrations.
pub fn design_artifact(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    if tech::is_vendored(&lower) {
        return false;
    }
    let mut segments = lower.split('/').collect::<Vec<_>>();
    let name = segments.pop().unwrap_or_default();
    segments.iter().any(|directory| {
        matches!(
            *directory,
            "adr" | "adrs" | "decisions" | "rfc" | "rfcs" | "migrations" | "migrate" | "migration"
        )
    }) || lower.starts_with("docs/architecture")
        || lower.starts_with("docs/design")
        || name.ends_with(".proto")
        || matches!(
            name,
            "architecture.md"
                | "openapi.json"
                | "openapi.yaml"
                | "openapi.yml"
                | "swagger.json"
                | "swagger.yaml"
                | "asyncapi.yaml"
                | "asyncapi.yml"
                | "schema.graphql"
        )
}

fn impact_dimension(impact: &Impact) -> Option<GitHubDimension> {
    dimension(
        GitHubDimensionId::Impact,
        "Признание",
        5,
        [
            "Проекты и вклад широко используются",
            "Заметное признание сообщества",
            "Небольшое признание сообщества",
            "Признание сообщества пока не видно",
        ],
        vec![
            count_metric(
                "maintained_stars",
                "Звёзды основных проектов",
                impact.maintained_stars,
                ITEMS,
                log_ramp(f64::from(impact.maintained_stars), 500.0),
                35,
            ),
            count_metric(
                "owned_forks",
                "Форки собственных проектов",
                impact.owned_forks,
                ITEMS,
                log_ramp(f64::from(impact.owned_forks), 100.0),
                15,
            ),
            count_metric(
                "notable_external",
                "Принятый вклад в независимые проекты от 1000 звёзд",
                impact.notable_external,
                ITEMS,
                ramp(f64::from(impact.notable_external), 0.0, 3.0),
                40,
            ),
            count_metric(
                "followers",
                "Подписчики",
                impact.followers,
                ITEMS,
                log_ramp(f64::from(impact.followers), 200.0),
                10,
            ),
        ],
    )
}

// ---------------------------------------------------------------------------------------------
// Flags, confidence and narrative

fn flags(
    facts: &Facts<'_>,
    activity: &Activity,
    rhythm: Option<&Rhythm>,
    craft: &Craft,
) -> Vec<GitHubFlag> {
    let mut flags = Vec::new();
    let mut push = |id: &str, severity, title: &str, detail: String| {
        flags.push(GitHubFlag {
            id: id.into(),
            severity,
            title: title.into(),
            detail,
        });
    };
    let future = facts
        .commits
        .iter()
        .filter(|(_, commit)| commit.utc() > facts.snapshot.now + Duration::days(1))
        .count();
    if future > 0 {
        push(
            "future_dated",
            FlagSeverity::Warning,
            "Коммиты из будущего",
            format!("{future} коммитов датированы позже текущего момента: даты задаются вручную."),
        );
    }
    if let Some(rhythm) = rhythm
        && rhythm.report.sample_size >= 20
        && rhythm.top_minute_share >= 40
    {
        push(
            "uniform_commit_minute",
            FlagSeverity::Warning,
            "Коммиты в одну и ту же минуту",
            format!(
                "{}% коммитов сделаны в одну и ту же минуту суток: так выглядит генерация по расписанию.",
                rhythm.top_minute_share
            ),
        );
    }
    if craft.message_sample >= 20 && craft.repeated_message_share >= 50 {
        push(
            "repeated_messages",
            FlagSeverity::Warning,
            "Одинаковые сообщения коммитов",
            format!(
                "{}% коммитов имеют одно и то же сообщение.",
                craft.repeated_message_share
            ),
        );
    }
    // Graph padding: an unknown repository of the account holds most of its commits, which change
    // a single file and repeat one message.
    let total = facts.commits.len() as u32;
    for repository in &facts.snapshot.analyzed {
        let stars = facts
            .snapshot
            .repositories
            .get(&repository.full_name)
            .map_or(0, |meta| meta.stars);
        let own =
            facts.owned(&repository.full_name) || facts.led.contains(repository.full_name.as_str());
        let commits = &repository.commits;
        if !own || stars >= 10 || commits.len() < 30 || percent(commits.len() as u32, total) < 80 {
            continue;
        }
        let single = commits
            .iter()
            .filter(|commit| commit.files.len() <= 1)
            .count() as u32;
        let mut subjects: HashMap<&str, u32> = HashMap::new();
        for commit in commits {
            *subjects.entry(commit.subject.trim()).or_insert(0) += 1;
        }
        let top = subjects.values().copied().max().unwrap_or(0);
        if percent(single, commits.len() as u32) >= 80 && percent(top, commits.len() as u32) >= 30 {
            push(
                "padding_repository",
                FlagSeverity::Warning,
                "Активность сосредоточена в одном репозитории из однотипных микрокоммитов",
                "Большая часть коммитов — правки одного файла с повторяющимся сообщением; такой вклад не считается содержательным.".into(),
            );
            break;
        }
    }
    if let Some(created) = facts.snapshot.created_at
        && (facts.snapshot.now - created).num_days() < 120
        && activity.report.commits_last_year >= 300
    {
        push(
            "new_account_burst",
            FlagSeverity::Info,
            "Новый аккаунт с большим объёмом",
            "Аккаунту меньше четырёх месяцев; устойчивость активности пока не проверить.".into(),
        );
    }
    let owned = facts
        .snapshot
        .repositories
        .values()
        .filter(|meta| facts.snapshot.is_self(&meta.owner))
        .collect::<Vec<_>>();
    let forks = owned.iter().filter(|meta| meta.fork).count() as u32;
    if owned.len() >= 10 && percent(forks, owned.len() as u32) >= 70 {
        push(
            "fork_heavy",
            FlagSeverity::Info,
            "Много форков без собственной работы",
            format!(
                "{forks} из {} собственных репозиториев — форки; форки не оцениваются.",
                owned.len()
            ),
        );
    }
    let year_ago = facts.year_ago();
    let found = facts
        .commits
        .iter()
        .filter(|(_, commit)| commit.utc() >= year_ago)
        .count() as u32;
    if let Some(total) = facts.snapshot.totals.commits_last_year
        && total >= 50
        && percent(found, total) < 50
    {
        push(
            "partial_coverage",
            FlagSeverity::Info,
            "Проанализирована часть публичных коммитов",
            format!(
                "В проанализированных репозиториях найдено {} из {} публичных коммитов за год; остальные — в репозиториях за пределами выборки.",
                group(found),
                group(total)
            ),
        );
    }
    if activity.report.active_days_last_year < 10 && facts.commits.len() < 10 {
        push(
            "low_public_data",
            FlagSeverity::Info,
            "Мало публичных данных",
            "Публичной активности слишком мало для уверенных выводов; это не оценка навыков."
                .into(),
        );
    }
    flags
}

fn coverage(facts: &Facts<'_>) -> GitHubCoverage {
    let totals = &facts.snapshot.totals;
    let year_ago = facts.year_ago();
    let found_last_year = facts
        .commits
        .iter()
        .filter(|(_, commit)| commit.utc() >= year_ago)
        .count() as u32;
    let first = facts
        .commits
        .first()
        .map(|(_, commit)| commit.utc())
        .or(facts.snapshot.created_at)
        .unwrap_or(facts.snapshot.now);
    GitHubCoverage {
        from: iso(first),
        to: iso(facts.snapshot.now),
        repositories_analyzed: facts.snapshot.analyzed.len() as u32,
        commits_analyzed: facts.commits.len() as u32,
        commits_public_total: totals.commits.unwrap_or(0),
        commits_last_year_found: found_last_year,
        commits_last_year_total: totals.commits_last_year.unwrap_or(found_last_year),
        coverage_percent: totals
            .commits_last_year
            .filter(|total| *total > 0)
            .map(|total| percent(found_last_year.min(total), total)),
        partial: facts.snapshot.partial,
        notes: facts.snapshot.notes.clone(),
    }
}

fn confidence(
    facts: &Facts<'_>,
    activity: &Activity,
    coverage: &GitHubCoverage,
    flags: &[GitHubFlag],
) -> SignalConfidence {
    let analyzed = facts.commits.len();
    let days = activity.report.active_days_last_year;
    let mut level: u8 = if analyzed >= 150 && days >= 60 && activity.report.active_years >= 2 {
        2
    } else if analyzed >= 30 && days >= 15 {
        1
    } else {
        0
    };
    if flags
        .iter()
        .any(|flag| flag.severity == FlagSeverity::Warning)
    {
        level = level.saturating_sub(1);
    }
    if facts.snapshot.partial
        || coverage
            .coverage_percent
            .is_some_and(|percent| percent < 30)
    {
        level = level.min(1);
    }
    match level {
        2 => SignalConfidence::High,
        1 => SignalConfidence::Medium,
        _ => SignalConfidence::Low,
    }
}

fn highlights(
    activity: &Activity,
    rhythm: Option<&Rhythm>,
    collaboration: &GitHubCollaboration,
    ownership: &GitHubOwnershipSummary,
    craft: &Craft,
    impact: &Impact,
) -> Vec<String> {
    let mut items = Vec::new();
    let report = &activity.report;
    if report.active_weeks_last_year >= 10 {
        items.push(format!(
            "Активность около {} из 52 недель за последний год",
            round_to(report.active_weeks_last_year, 5).min(52)
        ));
    }
    if report.longest_daily_streak >= 7 {
        items.push(format!(
            "Самая длинная серия — {} подряд",
            with_unit(
                approx(report.longest_daily_streak),
                approx_value(report.longest_daily_streak),
                DAYS
            )
        ));
    }
    if let Some(rhythm) = rhythm.filter(|rhythm| rhythm.report.sample_size >= 20) {
        items.push(format!(
            "Обычно {} работы в активный день",
            fsp_domain::hours_band(rhythm.report.median_minutes_per_active_day)
        ));
    }
    if ownership.led_repositories > 0 {
        let projects = with_unit(
            approx(ownership.led_repositories),
            approx_value(ownership.led_repositories),
            (
                "собственный проект",
                "собственных проекта",
                "собственных проектов",
            ),
        );
        let organizations = if ownership.own_organizations > 0 {
            format!(
                " в {} {}",
                ownership.own_organizations,
                plural(
                    ownership.own_organizations,
                    ("организации", "организациях", "организациях")
                )
            )
        } else {
            String::new()
        };
        items.push(format!("Ведёт {projects}{organizations}"));
    }
    if collaboration.approved_pull_requests >= 5 {
        items.push(format!(
            "PR, одобренные другими людьми: {}",
            approx(collaboration.approved_pull_requests)
        ));
    }
    if impact.notable_external > 0 {
        items.push("Принятый вклад в крупные независимые open source-проекты".into());
    }
    if craft.report.substantive_repositories >= 2 && craft.report.ci_percent >= 50 {
        items.push(format!(
            "CI настроен примерно в {}% основных проектов",
            round_to(u32::from(craft.report.ci_percent), 10)
        ));
    }
    items.truncate(7);
    items
}

fn organizations(facts: &Facts<'_>) -> GitHubOrganizations {
    let mut contributed: BTreeMap<String, (BTreeSet<String>, u32)> = BTreeMap::new();
    for (name, item) in facts
        .snapshot
        .sampled
        .iter()
        .filter(|(_, item)| item.organization)
    {
        contributed
            .entry(item.owner.clone())
            .or_default()
            .0
            .insert(name.clone());
    }
    for repository in &facts.snapshot.analyzed {
        let owner = Facts::owner(&repository.full_name);
        if let Some(entry) = contributed.get_mut(owner) {
            entry.0.insert(repository.full_name.clone());
            entry.1 += repository.user_commits();
        }
    }
    let mut contributed = contributed
        .into_iter()
        .map(
            |(login, (repositories, commits))| GitHubContributedOrganization {
                url: format!("https://github.com/{login}"),
                affiliated: facts
                    .affiliated
                    .keys()
                    .any(|known| known.eq_ignore_ascii_case(&login)),
                repositories: repositories.len() as u32,
                // The owner search counts every public commit there, beyond the cloned histories.
                commits: facts
                    .snapshot
                    .totals
                    .owner_commits
                    .get(&login)
                    .map_or(commits, |total| commits.max(*total)),
                login,
            },
        )
        .collect::<Vec<_>>();
    contributed.sort_by(|a, b| b.commits.cmp(&a.commits).then(a.login.cmp(&b.login)));
    contributed.truncate(15);
    GitHubOrganizations {
        memberships: facts
            .snapshot
            .organizations
            .iter()
            .map(|login| GitHubOrganization {
                login: login.clone(),
                url: format!("https://github.com/{login}"),
            })
            .collect(),
        contributed,
    }
}

fn repositories(facts: &Facts<'_>, craft: &Craft) -> Vec<GitHubRepositoryInsight> {
    let mut items = facts
        .snapshot
        .analyzed
        .iter()
        .map(|repository| {
            let meta = facts.snapshot.repositories.get(&repository.full_name);
            let detected = facts.detected.get(repository.full_name.as_str());
            GitHubRepositoryInsight {
                url: format!("https://github.com/{}", repository.full_name),
                owned: facts.owned(&repository.full_name),
                led: facts.led.contains(repository.full_name.as_str()),
                fork: meta.is_some_and(|meta| meta.fork),
                archived: meta.is_some_and(|meta| meta.archived),
                stars: meta.map_or(0, |meta| meta.stars),
                forks: meta.map_or(0, |meta| meta.forks),
                primary_language: meta.and_then(|meta| meta.language.clone()),
                description: meta.and_then(|meta| meta.description.clone()),
                pushed_at: meta.and_then(|meta| meta.pushed_at).map(iso),
                total_commits: repository.total_commits,
                user_commits: repository.user_commits(),
                user_share_percent: repository.share_percent(),
                last_user_commit_at: repository.last_commit().map(iso),
                hygiene: craft
                    .hygiene
                    .get(&repository.full_name)
                    .cloned()
                    .unwrap_or_else(|| hygiene(repository, meta)),
                technologies: detected
                    .map(|detected| {
                        detected
                            .keys()
                            .filter_map(|technology| technology.catalog_skill())
                            .map(str::to_owned)
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect()
                    })
                    .unwrap_or_default(),
                stack: detected
                    .map(|detected| {
                        detected
                            .keys()
                            .filter(|technology| {
                                technology.category != fsp_domain::StackCategory::Language
                            })
                            .map(|technology| technology.name.to_owned())
                            .take(8)
                            .collect()
                    })
                    .unwrap_or_default(),
                name_with_owner: repository.full_name.clone(),
            }
        })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| {
        b.user_commits
            .cmp(&a.user_commits)
            .then(a.name_with_owner.cmp(&b.name_with_owner))
    });
    items
}

// ---------------------------------------------------------------------------------------------
// Numbers

fn ramp(value: f64, zero: f64, full: f64) -> u8 {
    if full <= zero {
        return 0;
    }
    ((value - zero) / (full - zero) * 100.0)
        .clamp(0.0, 100.0)
        .round() as u8
}

/// Saturating logarithmic scale: `full` and above score 100.
fn log_ramp(value: f64, full: f64) -> u8 {
    if value <= 0.0 {
        return 0;
    }
    ((1.0 + value).ln() / (1.0 + full).ln() * 100.0)
        .clamp(0.0, 100.0)
        .round() as u8
}

/// 100 up to `full_until`, falling linearly to 0 at `zero_at`.
fn inverse_ramp(value: f64, full_until: f64, zero_at: f64) -> u8 {
    100 - ramp(value, full_until, zero_at)
}

pub(crate) fn percent(part: u32, total: u32) -> u8 {
    if total == 0 {
        return 0;
    }
    ((u64::from(part) * 100 + u64::from(total) / 2) / u64::from(total)).min(100) as u8
}

fn median(mut values: Vec<u32>) -> Option<u32> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let middle = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2
    } else {
        values[middle]
    })
}

fn round_to(value: u32, step: u32) -> u32 {
    (value + step / 2) / step * step
}

/// Rounded down, so a public figure never overstates: two significant figures from 100, a
/// multiple of five from 10, exact below 10.
fn approx_value(value: u32) -> u32 {
    let step = match value {
        0..=9 => 1,
        10..=99 => 5,
        _ => 10u32.pow(value.ilog10() - 1),
    };
    value / step * step
}

fn approx(value: u32) -> String {
    let rounded = approx_value(value);
    if rounded == value && value < 10 {
        group(value)
    } else {
        format!("≈{}", group(rounded))
    }
}

/// Thousands separated by a no-break space, as in Russian typography.
fn group(value: u32) -> String {
    let digits = value.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push('\u{a0}');
        }
        grouped.push(digit);
    }
    grouped
}

fn plural(value: u32, forms: (&str, &str, &str)) -> String {
    let (one, few, many) = forms;
    let tail = value % 100;
    let form = if (11..=14).contains(&tail) {
        many
    } else {
        match value % 10 {
            1 => one,
            2..=4 => few,
            _ => many,
        }
    };
    form.into()
}

fn with_unit(number: String, value: u32, unit: (&str, &str, &str)) -> String {
    if unit.0.is_empty() {
        number
    } else {
        format!("{number} {}", plural(value, unit))
    }
}

fn hours_exact(minutes: u32) -> String {
    format!("{} ч {:02} мин", minutes / 60, minutes % 60)
}
