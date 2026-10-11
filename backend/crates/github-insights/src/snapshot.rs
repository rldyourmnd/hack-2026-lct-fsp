//! Normalized public facts collected for one account. Heuristics read only this structure, so they
//! are pure and testable without GitHub.
//!
//! Everything here is public: GitHub search and profile data, and the default branches of public
//! repositories cloned on the server. No token is required.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, FixedOffset, Utc};

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub login: String,
    pub created_at: Option<DateTime<Utc>>,
    pub now: DateTime<Utc>,
    pub followers: u32,
    /// The public profile name, when set.
    pub display_name: Option<String>,
    /// Public organization memberships.
    pub organizations: Vec<String>,
    /// Lower-case commit identities that GitHub attributes to the account.
    pub emails: BTreeSet<String>,
    pub totals: SearchTotals,
    /// Authored pull requests across public repositories, most recent first.
    pub pull_requests: Vec<PullRequestFact>,
    /// Authored pull requests into other owners' repositories, outside the account's organizations.
    pub external_pull_requests: Vec<PullRequestFact>,
    pub repositories: BTreeMap<String, RepositoryMeta>,
    /// Repositories found by commit search with the number of sampled commits there.
    pub sampled: BTreeMap<String, SampledRepository>,
    pub analyzed: Vec<RepositoryAnalysis>,
    /// Owners where the account merges pull requests widely: its own organizations as well.
    pub merge_rights: BTreeSet<String>,
    pub partial: bool,
    pub notes: Vec<String>,
}

impl Snapshot {
    pub fn is_self(&self, owner: &str) -> bool {
        owner.eq_ignore_ascii_case(&self.login)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SampledRepository {
    pub commits: u32,
    pub owner: String,
    pub organization: bool,
}

/// Counts from GitHub search over all public repositories; `None` when the search failed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchTotals {
    /// Public commits per calendar year from period searches.
    pub yearly_commits: BTreeMap<i32, u32>,
    /// Public commits under each searched owner (organization or user), from owner searches.
    pub owner_commits: BTreeMap<String, u32>,
    pub commits: Option<u32>,
    pub commits_last_year: Option<u32>,
    pub pull_requests: Option<u32>,
    pub pull_requests_last_year: Option<u32>,
    pub approved_pull_requests: Option<u32>,
    pub reviews_of_others: Option<u32>,
    pub independent_reviews: Option<u32>,
    pub issues: Option<u32>,
    pub external_pull_requests: Option<u32>,
    pub external_merged: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestState {
    Open,
    Closed,
    Merged,
}

#[derive(Debug, Clone)]
pub struct PullRequestFact {
    pub url: String,
    pub repository: String,
    pub owner: String,
    pub created_at: DateTime<Utc>,
    pub state: PullRequestState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepositoryMeta {
    pub full_name: String,
    pub owner: String,
    pub owner_is_organization: bool,
    pub fork: bool,
    pub archived: bool,
    pub stars: u32,
    pub forks: u32,
    pub size_kib: u32,
    pub language: Option<String>,
    pub topics: Vec<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    pub default_branch: Option<String>,
    pub pushed_at: Option<DateTime<Utc>>,
}

/// One cloned repository: its default-branch history and current tree.
#[derive(Debug, Clone, Default)]
pub struct RepositoryAnalysis {
    pub full_name: String,
    pub total_commits: u32,
    /// The account's commits that it also committed itself (pushed directly, not through a merge).
    pub direct_pushes: u32,
    pub commits: Vec<GitCommit>,
    /// Paths of the default branch tree.
    pub files: Vec<String>,
    /// Manifest path and text.
    pub manifests: Vec<(String, String)>,
}

impl RepositoryAnalysis {
    pub fn user_commits(&self) -> u32 {
        u32::try_from(self.commits.len()).unwrap_or(u32::MAX)
    }

    pub fn last_commit(&self) -> Option<DateTime<Utc>> {
        self.commits.iter().map(GitCommit::utc).max()
    }

    pub fn share_percent(&self) -> u8 {
        crate::analysis::percent(self.user_commits(), self.total_commits.max(1))
    }
}

#[derive(Debug, Clone)]
pub struct GitCommit {
    pub sha: String,
    /// Author time with the author's own UTC offset.
    pub authored_at: DateTime<FixedOffset>,
    pub parents: u32,
    pub subject: String,
    pub files: Vec<String>,
}

impl GitCommit {
    pub fn is_merge(&self) -> bool {
        self.parents > 1
    }

    pub fn utc(&self) -> DateTime<Utc> {
        self.authored_at.with_timezone(&Utc)
    }
}
