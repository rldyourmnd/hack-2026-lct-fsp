//! Pure domain types and rules for the reverse-hiring platform.

pub mod catalog;
mod documents;
mod estimate;
mod github;
mod knowledge;
mod llm;
mod matching;
mod sources;

pub use documents::*;
pub use estimate::*;
pub use github::*;
pub use knowledge::*;
pub use llm::*;
pub use matching::*;
pub use sources::*;

use serde::{Deserialize, Serialize};

/// Assessed level. The order is meaningful: a verified grade never goes down, and a need's grade is a
/// minimum the candidate's verified grade has to reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Grade {
    Intern,
    Junior,
    Middle,
    Senior,
    Lead,
}

impl Grade {
    pub const ALL: [Self; 5] = [
        Self::Intern,
        Self::Junior,
        Self::Middle,
        Self::Senior,
        Self::Lead,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Intern => "intern",
            Self::Junior => "junior",
            Self::Middle => "middle",
            Self::Senior => "senior",
            Self::Lead => "lead",
        }
    }

    /// The catalog label, e.g. «Мидл».
    #[must_use]
    pub fn label(self) -> &'static str {
        catalog::grade_label(self)
    }
}

/// The catalog's specializations (`data/catalog.json`), in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Specialization {
    Backend,
    Frontend,
    Fullstack,
    Ios,
    Android,
    MobileCrossplatform,
    Devops,
    DataEngineering,
    DataScience,
    MlEngineering,
    QaManual,
    QaAutomation,
    Security,
    Embedded,
    Systems,
    Gamedev,
    Onec,
    Erp,
    SystemAnalysis,
    BusinessAnalysis,
    DataAnalytics,
    Dba,
    Sysadmin,
    Design,
    Management,
}

impl Specialization {
    pub const ALL: [Self; 25] = [
        Self::Backend,
        Self::Frontend,
        Self::Fullstack,
        Self::Ios,
        Self::Android,
        Self::MobileCrossplatform,
        Self::Devops,
        Self::DataEngineering,
        Self::DataScience,
        Self::MlEngineering,
        Self::QaManual,
        Self::QaAutomation,
        Self::Security,
        Self::Embedded,
        Self::Systems,
        Self::Gamedev,
        Self::Onec,
        Self::Erp,
        Self::SystemAnalysis,
        Self::BusinessAnalysis,
        Self::DataAnalytics,
        Self::Dba,
        Self::Sysadmin,
        Self::Design,
        Self::Management,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Backend => "backend",
            Self::Frontend => "frontend",
            Self::Fullstack => "fullstack",
            Self::Ios => "ios",
            Self::Android => "android",
            Self::MobileCrossplatform => "mobile_crossplatform",
            Self::Devops => "devops",
            Self::DataEngineering => "data_engineering",
            Self::DataScience => "data_science",
            Self::MlEngineering => "ml_engineering",
            Self::QaManual => "qa_manual",
            Self::QaAutomation => "qa_automation",
            Self::Security => "security",
            Self::Embedded => "embedded",
            Self::Systems => "systems",
            Self::Gamedev => "gamedev",
            Self::Onec => "onec",
            Self::Erp => "erp",
            Self::SystemAnalysis => "system_analysis",
            Self::BusinessAnalysis => "business_analysis",
            Self::DataAnalytics => "data_analytics",
            Self::Dba => "dba",
            Self::Sysadmin => "sysadmin",
            Self::Design => "design",
            Self::Management => "management",
        }
    }

    /// The catalog label, e.g. «Разработка на 1С».
    #[must_use]
    pub fn label(self) -> &'static str {
        catalog::specialization(self).label.as_str()
    }

    /// The short catalog label, e.g. «1С».
    #[must_use]
    pub fn short_label(self) -> &'static str {
        catalog::specialization(self).short.as_str()
    }
}

impl std::str::FromStr for Specialization {
    type Err = UnknownValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|item| item.as_str() == value)
            .ok_or_else(|| UnknownValue(value.to_owned()))
    }
}

impl std::str::FromStr for Grade {
    type Err = UnknownValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|item| item.as_str() == value)
            .ok_or_else(|| UnknownValue(value.to_owned()))
    }
}

/// A grade or specialization id outside the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownValue(pub String);

impl std::fmt::Display for UnknownValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "unknown catalog value: {}", self.0)
    }
}

/// An account has exactly one role, fixed at registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Candidate,
    Employer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: String,
    pub email: String,
    pub role: Role,
    pub candidate_id: Option<String>,
    pub company_id: Option<String>,
    pub email_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consents {
    pub processing: bool,
    pub publication: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateProfile {
    pub id: String,
    pub alias: String,
    pub email: String,
    pub name: String,
    pub specialization: Specialization,
    pub selected_grade: Grade,
    pub stack: Vec<String>,
    pub team_role: String,
    pub experience_months: Option<u32>,
    /// The year the candidate started in IT (study, own projects, work); experience counts from it.
    pub experience_since: Option<u16>,
    pub experience: String,
    pub soft_skills: Vec<String>,
    pub consents: Consents,
    pub verified_grade: Option<Grade>,
    pub verified_specialization: Option<Specialization>,
    pub grade_confirmed_at: Option<String>,
    /// The latest estimate from GitHub and FSP evidence.
    pub estimate: Option<ProfileEstimate>,
    /// Questionnaire fields last filled from evidence; editing a field removes it.
    pub auto_fields: Vec<String>,
    pub revision: u64,
    pub updated_at: String,
}

/// Where a candidate's category comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategorySource {
    Assessment,
    Estimate,
}

/// The employer-visible category: a passed test, otherwise an estimate of at least medium confidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Category {
    pub specialization: Specialization,
    pub grade: Grade,
    pub source: CategorySource,
    /// The estimate's confidence; `None` for a test result.
    pub confidence: Option<SignalConfidence>,
}

impl Category {
    /// Strength of the confirmation: a test, then high and medium estimates.
    #[must_use]
    pub fn strength(&self) -> u8 {
        match (self.source, self.confidence) {
            (CategorySource::Assessment, _) => 3,
            (CategorySource::Estimate, Some(SignalConfidence::High)) => 2,
            (CategorySource::Estimate, _) => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateIndexRecord {
    pub profile: CandidateProfile,
    pub email_verified: bool,
    pub work_modes: Vec<String>,
    pub available_for_offers: bool,
    pub skills: Vec<SkillEvidence>,
    pub source_kinds: Vec<EvidenceSource>,
    pub fsp_verified: bool,
    pub last_signal_at: Option<String>,
}

impl CandidateIndexRecord {
    #[must_use]
    pub fn rankable(&self) -> Option<RankableCandidate> {
        let category = self.profile.category()?;
        Some(RankableCandidate {
            id: self.profile.id.clone(),
            specialization: category.specialization,
            grade: category.grade,
            confirmation: category.strength(),
            fsp_verified: self.fsp_verified,
            skills: self.skills.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HiringNeedRecord {
    pub id: String,
    pub company_id: String,
    pub title: String,
    pub need: HiringNeed,
    pub work_mode: String,
    pub salary_from: u64,
    pub salary_to: u64,
    pub description: String,
    pub published: bool,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub revision: u64,
}

impl CandidateProfile {
    #[must_use]
    pub fn questionnaire_complete(&self) -> bool {
        !self.name.trim().is_empty()
            && !self.stack.is_empty()
            && !self.team_role.trim().is_empty()
            && !self.experience.trim().is_empty()
    }

    /// The category a passed test gives, otherwise an estimate with at least medium confidence.
    #[must_use]
    pub fn category(&self) -> Option<Category> {
        if let (Some(specialization), Some(grade)) =
            (self.verified_specialization, self.verified_grade)
        {
            return Some(Category {
                specialization,
                grade,
                source: CategorySource::Assessment,
                confidence: None,
            });
        }
        let estimate = self.estimate.as_ref()?;
        if estimate.confidence == SignalConfidence::Low {
            return None;
        }
        // The candidate may choose among the specializations the evidence supports.
        let chosen = estimate
            .specializations
            .iter()
            .any(|share| share.id == self.specialization);
        Some(Category {
            specialization: if chosen {
                self.specialization
            } else {
                estimate.specialization?
            },
            grade: estimate.grade?,
            source: CategorySource::Estimate,
            confidence: Some(estimate.confidence),
        })
    }

    /// Mirrors the mock's `onboarding.visibleToEmployers`: verified e-mail,
    /// complete questionnaire, a category (test or estimate) and both consents.
    #[must_use]
    pub fn visible_to_employers(&self, email_verified: bool) -> bool {
        email_verified
            && self.questionnaire_complete()
            && self.category().is_some()
            && self.consents.processing
            && self.consents.publication
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Company {
    pub id: String,
    pub display_name: String,
    pub business_area: String,
    pub description: String,
    pub contact_email: String,
    pub website: Option<String>,
}

/// An employer's shortlist of candidate ids, in insertion order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortlistRecord {
    pub id: String,
    pub company_id: String,
    pub title: String,
    pub member_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Lifecycle shared by employer invitations and candidate applications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InteractionStatus {
    Sent,
    Viewed,
    Accepted,
    Declined,
    Withdrawn,
}

impl InteractionStatus {
    /// Terminal states cannot transition further; re-issuing the same
    /// terminal outcome is the caller's idempotent replay.
    #[must_use]
    pub fn terminal(self) -> bool {
        matches!(self, Self::Accepted | Self::Declined | Self::Withdrawn)
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sent => "sent",
            Self::Viewed => "viewed",
            Self::Accepted => "accepted",
            Self::Declined => "declined",
            Self::Withdrawn => "withdrawn",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "sent" => Some(Self::Sent),
            "viewed" => Some(Self::Viewed),
            "accepted" => Some(Self::Accepted),
            "declined" => Some(Self::Declined),
            "withdrawn" => Some(Self::Withdrawn),
            _ => None,
        }
    }
}

/// The company card frozen into an invitation or application snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyCard {
    pub id: String,
    pub display_name: String,
    pub business_area: String,
    pub description: String,
    pub contact_email: String,
}

/// The need fields frozen into an invitation snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeedSnapshot {
    pub id: String,
    pub title: String,
    pub work_mode: String,
    pub salary_from: u64,
    pub salary_to: u64,
    pub description: String,
}

/// An employer's invitation to a candidate, with the company card and the
/// need frozen at creation so later edits do not rewrite sent terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationRecord {
    pub id: String,
    pub company_id: String,
    pub candidate_id: String,
    pub candidate_alias: String,
    pub company: CompanyCard,
    pub need: NeedSnapshot,
    pub need_definition: HiringNeedRecord,
    pub message: String,
    pub status: InteractionStatus,
    pub created_at: String,
    pub viewed_at: Option<String>,
    pub responded_at: Option<String>,
    pub disclosure_fields: Vec<String>,
}

/// The vacancy fields frozen into an application snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VacancySnapshot {
    pub id: String,
    pub title: String,
    pub specialization: Specialization,
    pub grade: Grade,
    pub work_mode: String,
    pub salary_from: u64,
    pub salary_to: u64,
    pub description: String,
}

/// A candidate's application to a published need, with the company card,
/// the vacancy card and the full need frozen at submission time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRecord {
    pub id: String,
    pub company_id: String,
    pub candidate_id: String,
    pub candidate_alias: String,
    pub company: CompanyCard,
    pub vacancy: VacancySnapshot,
    pub need_definition: HiringNeedRecord,
    pub message: String,
    pub status: InteractionStatus,
    pub created_at: String,
    pub viewed_at: Option<String>,
    pub responded_at: Option<String>,
    pub disclosure_fields: Vec<String>,
}

/// Lifecycle of an employer's weekly task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Active,
    Archived,
}

impl TaskStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

/// The response format a candidate picks for a weekly task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskMode {
    Solution,
    Approach,
}

impl TaskMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Solution => "solution",
            Self::Approach => "approach",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "solution" => Some(Self::Solution),
            "approach" => Some(Self::Approach),
            _ => None,
        }
    }
}

/// Review lifecycle of a candidate's task response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskResponseStatus {
    Submitted,
    Accepted,
    NeedsWork,
}

impl TaskResponseStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Accepted => "accepted",
            Self::NeedsWork => "needs_work",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "submitted" => Some(Self::Submitted),
            "accepted" => Some(Self::Accepted),
            "needs_work" => Some(Self::NeedsWork),
            _ => None,
        }
    }
}

/// A weekly task an employer publishes for candidates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRecord {
    pub id: String,
    pub company_id: String,
    pub title: String,
    pub prompt: String,
    pub skill_id: String,
    pub grade: Grade,
    pub estimate_minutes: u32,
    pub status: TaskStatus,
    pub created_at: String,
}

/// The offered-task card frozen into a task response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfferedTaskSnapshot {
    pub id: String,
    pub company_id: String,
    pub company_display_name: String,
    pub title: String,
    pub prompt: String,
    pub skill_id: String,
    pub grade: Grade,
    pub estimate_minutes: u32,
}

/// A candidate's answer to a weekly task, with the offered task frozen at
/// submission time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskResponseRecord {
    pub id: String,
    pub candidate_id: String,
    pub candidate_alias: String,
    pub task: OfferedTaskSnapshot,
    pub mode: TaskMode,
    pub answer: String,
    pub status: TaskResponseStatus,
    pub feedback: Option<String>,
    pub submitted_at: String,
    pub reviewed_at: Option<String>,
}

/// Lifecycle of a grade assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentStatus {
    Draft,
    InProgress,
    Submitted,
    Evaluating,
    Passed,
    Failed,
    ServiceError,
}

impl AssessmentStatus {
    /// The mock's `activeAssessment` set.
    #[must_use]
    pub fn active(self) -> bool {
        matches!(
            self,
            Self::Draft | Self::InProgress | Self::Submitted | Self::Evaluating
        )
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::InProgress => "in_progress",
            Self::Submitted => "submitted",
            Self::Evaluating => "evaluating",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::ServiceError => "service_error",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "in_progress" => Some(Self::InProgress),
            "submitted" => Some(Self::Submitted),
            "evaluating" => Some(Self::Evaluating),
            "passed" => Some(Self::Passed),
            "failed" => Some(Self::Failed),
            "service_error" => Some(Self::ServiceError),
            _ => None,
        }
    }
}

/// The assessment task card issued to a candidate — the mock's `TASK`
/// fixture with a per-grade variant id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentTask {
    pub id: String,
    pub family: String,
    pub version: String,
    pub variant_id: String,
    pub title: String,
    pub prompt: String,
    pub language: String,
    pub starter_code: String,
    pub acceptance_criteria: Vec<String>,
    pub rubric: Vec<RubricCriterion>,
    pub ai_allowed: bool,
    pub external_ai_allowed: bool,
    pub execution: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub questions: Vec<KnowledgeQuestion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RubricCriterion {
    pub id: String,
    pub label: String,
    pub weight: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssessmentAnswer {
    pub code: String,
    pub explanation: String,
    pub external_ai_used: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub responses: Vec<KnowledgeResponse>,
}

/// One automated check over the submitted answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentCheck {
    pub id: String,
    pub label: String,
    /// `passed`, `failed` or `not_run`.
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RubricOutcome {
    pub criterion_id: String,
    pub label: String,
    /// `met`, `partial` or `not_met`.
    pub outcome: String,
    pub feedback: String,
}

/// The contract's `AssessmentResult` blob.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentResultData {
    pub assessment_id: String,
    /// `passed`, `failed` or `service_error`.
    pub outcome: String,
    pub confirmed_grade: Option<Grade>,
    pub previous_grade: Option<Grade>,
    pub grade_changed: bool,
    pub rubric: Vec<RubricOutcome>,
    pub checks: Vec<AssessmentCheck>,
    pub feedback: String,
    pub next_eligible_at: Option<String>,
    pub simulation: bool,
}

/// A grade assessment attempt owned by a candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssessmentRecord {
    pub id: String,
    pub candidate_id: String,
    pub specialization: Specialization,
    pub grade: Grade,
    pub status: AssessmentStatus,
    pub task: AssessmentTask,
    pub answer: AssessmentAnswer,
    pub checks: Vec<AssessmentCheck>,
    pub result: Option<AssessmentResultData>,
    pub created_at: String,
    pub updated_at: String,
    pub revision: u64,
}

/// The aggregated disclosure grant a company holds over a candidate.
/// Row existence means `granted: true`; an absent row renders the
/// not-granted disclosure shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisclosureRecord {
    pub candidate_id: String,
    pub company_id: String,
    pub invitation_id: Option<String>,
    pub application_id: Option<String>,
    pub granted_at: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub source_links: Vec<String>,
}

impl Company {
    #[must_use]
    pub fn ready_to_invite(&self) -> bool {
        !self.display_name.trim().is_empty()
            && !self.business_area.trim().is_empty()
            && !self.description.trim().is_empty()
            && !self.contact_email.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grades_are_ordered_from_junior_to_senior() {
        assert!(Grade::Junior < Grade::Middle);
        assert!(Grade::Middle < Grade::Senior);
    }

    #[test]
    fn wire_names_match_the_contract() {
        let names =
            serde_json::to_value((Grade::Middle, Specialization::Fullstack, Role::Employer))
                .unwrap();
        assert_eq!(
            names,
            serde_json::json!(["middle", "fullstack", "employer"])
        );
    }

    #[test]
    fn publication_requires_a_complete_verified_profile_and_both_consents() {
        let mut profile = CandidateProfile {
            id: "candidate-1".into(),
            alias: "Кандидат №001".into(),
            email: "candidate@example.test".into(),
            name: "Кандидат".into(),
            specialization: Specialization::Backend,
            selected_grade: Grade::Junior,
            stack: vec!["rust".into()],
            team_role: "Разработчик".into(),
            experience_months: Some(12),
            experience_since: None,
            experience: "Два года в продуктовой разработке".into(),
            soft_skills: Vec::new(),
            consents: Consents {
                processing: true,
                publication: true,
            },
            verified_grade: Some(Grade::Junior),
            verified_specialization: Some(Specialization::Backend),
            grade_confirmed_at: None,
            estimate: None,
            auto_fields: Vec::new(),
            revision: 1,
            updated_at: "2026-10-06T00:00:00Z".into(),
        };
        assert!(profile.visible_to_employers(true));
        assert!(!profile.visible_to_employers(false));
        profile.consents.publication = false;
        assert!(!profile.visible_to_employers(true));
    }

    #[test]
    fn an_estimate_of_medium_confidence_gives_a_category_below_a_test() {
        let report: GitHubReport = serde_json::from_str(include_str!(
            "../../../../contracts/fixtures/github-report.json"
        ))
        .unwrap();
        let estimate = estimate_profile(&EstimateInput {
            github: Some(&report),
            fsp: &[],
            stated_experience_months: None,
            stated_team_role: None,
            now: "2026-10-10T00:00:00Z",
        });
        let mut profile = CandidateProfile {
            id: "candidate-2".into(),
            alias: "Кандидат №002".into(),
            email: "estimate@example.test".into(),
            name: "Кандидат".into(),
            specialization: Specialization::Backend,
            selected_grade: Grade::Junior,
            stack: vec!["rust".into()],
            team_role: "Разработчик".into(),
            experience_months: None,
            experience_since: None,
            experience: "Публичные проекты".into(),
            soft_skills: Vec::new(),
            consents: Consents {
                processing: true,
                publication: true,
            },
            verified_grade: None,
            verified_specialization: None,
            grade_confirmed_at: None,
            estimate: Some(estimate.clone()),
            auto_fields: Vec::new(),
            revision: 1,
            updated_at: "2026-10-10T00:00:00Z".into(),
        };
        let category = profile.category().unwrap();
        assert_eq!(category.source, CategorySource::Estimate);
        assert_eq!(Some(category.grade), estimate.grade);
        assert!(profile.visible_to_employers(true));
        assert!(category.strength() < 3);
        // The candidate may choose among the supported specializations, not beyond them.
        let supported = estimate
            .specializations
            .iter()
            .map(|share| share.id)
            .find(|id| Some(*id) != estimate.specialization);
        if let Some(supported) = supported {
            profile.specialization = supported;
            assert_eq!(profile.category().unwrap().specialization, supported);
        }
        profile.specialization = Specialization::Design;
        assert_eq!(
            Some(profile.category().unwrap().specialization),
            estimate.specialization
        );
        // A low-confidence estimate gives no category.
        if let Some(stored) = profile.estimate.as_mut() {
            stored.confidence = SignalConfidence::Low;
        }
        assert!(profile.category().is_none());
        // A test result wins over any estimate.
        profile.verified_grade = Some(Grade::Middle);
        profile.verified_specialization = Some(Specialization::Frontend);
        let category = profile.category().unwrap();
        assert_eq!(category.source, CategorySource::Assessment);
        assert_eq!(category.strength(), 3);
    }
}
