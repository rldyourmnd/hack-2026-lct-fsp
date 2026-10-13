use fsp_domain::{
    Company, Criticality, EvidencePolicy, EvidenceSource, EvidenceStatus, Grade, JobRecord,
    MatchScore, RepositoryMetrics, Role, SourceProvider, SourceRecord, Specialization,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub actor_id: String,
    pub role: Role,
    pub email_verified: bool,
    pub candidate_id: Option<String>,
    pub company_id: Option<String>,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InterviewStartInput {
    pub blueprint_id: String,
    pub max_turns: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InterviewAnswerInput {
    pub question_id: String,
    pub answer: String,
    pub response_mode: InterviewResponseMode,
    pub revision: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InterviewSpeechInput {
    pub question_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechAudioView {
    pub audio_base64: String,
    pub mime_type: String,
    pub provider: String,
    pub voice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InterviewTranscriptionInput {
    pub question_id: String,
    pub audio_base64: String,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechTranscriptView {
    pub text: String,
    pub provider: String,
    pub duration_seconds: f32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterviewResponseMode {
    #[default]
    Text,
    Voice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterviewStatusView {
    Active,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStateView {
    Unknown,
    Observed,
    Partial,
    Confirmed,
    Contradicted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompetencyKindView {
    Technical,
    Behavioral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewQuestionView {
    pub id: String,
    pub node_id: String,
    pub prompt: String,
    pub sequence: u8,
    #[serde(default)]
    pub recommended_response_mode: InterviewResponseMode,
    pub delivery: InterviewDeliveryView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewDeliveryView {
    pub speech_text: String,
    pub avatar_cue: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewNodeView {
    pub id: String,
    pub label: String,
    pub kind: CompetencyKindView,
    pub criticality: u8,
    pub prerequisites: Vec<String>,
    pub state: EvidenceStateView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewEvidenceView {
    pub node_id: String,
    pub state: EvidenceStateView,
    pub question_id: String,
    pub answer_excerpt: String,
    pub rationale: String,
    pub evaluator: String,
    pub rubric_version: String,
    #[serde(default)]
    pub response_mode: InterviewResponseMode,
    #[serde(default = "default_interview_signal_strength")]
    pub signal_strength: u8,
}

fn default_interview_signal_strength() -> u8 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewView {
    pub id: String,
    pub candidate_id: String,
    pub blueprint_id: String,
    pub status: InterviewStatusView,
    pub revision: u32,
    pub current_question: Option<InterviewQuestionView>,
    pub nodes: Vec<InterviewNodeView>,
    pub evidence: Vec<InterviewEvidenceView>,
    pub model_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewsView {
    pub items: Vec<InterviewView>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmailChallengeInput {
    pub email: String,
    pub role: Role,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailChallengeView {
    pub challenge_id: String,
    pub expires_at: String,
    pub delivery: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmailVerifyInput {
    pub challenge_id: String,
    pub code: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueChallenge {
    Issued,
    Locked,
    RateLimited,
}

#[derive(Debug, Clone)]
pub struct PendingChallenge {
    pub id: String,
    pub email: String,
    pub role: Role,
    pub code_hash: String,
    pub expires_at: String,
    pub attempts: u8,
    pub used: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    pub id: String,
    pub alias: String,
    pub email: String,
    pub email_verified: bool,
    pub name: String,
    pub industry: &'static str,
    pub specialization: Specialization,
    pub selected_grade: Grade,
    pub stack: Vec<String>,
    pub team_role: String,
    pub experience_months: Option<u32>,
    pub experience_since: Option<u16>,
    pub experience: String,
    pub soft_skills: Vec<String>,
    pub consents: fsp_domain::Consents,
    pub verified_grade: Option<Grade>,
    pub verified_specialization: Option<Specialization>,
    pub grade_confirmed_at: Option<String>,
    pub category: CategoryView,
    /// The estimate from GitHub and FSP evidence, with its factors and the LLM review.
    pub estimate: Option<fsp_domain::ProfileEstimate>,
    /// Questionnaire fields last filled from evidence.
    pub auto_fields: Vec<String>,
    pub freshness: FreshnessView,
    pub onboarding: OnboardingView,
    pub assessment_eligibility: GradeEligibilityView,
    pub revision: u64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryView {
    pub specialization: Option<Specialization>,
    pub grade: Option<Grade>,
    pub eligible: bool,
    /// A test result or an estimate; `None` without a category.
    pub source: Option<fsp_domain::CategorySource>,
    /// The estimate's confidence; `None` for a test result.
    pub confidence: Option<fsp_domain::SignalConfidence>,
}

/// What employers see of an estimate: the score and its factors, without names or the LLM text.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmployerEstimateView {
    pub score: u8,
    pub computed_at: String,
    pub factors: Vec<EmployerEstimateFactorView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmployerEstimateFactorView {
    pub id: String,
    pub label: String,
    pub points: u8,
    pub max: u8,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreshnessView {
    pub last_signal_at: Option<String>,
    pub last_signal: Option<&'static str>,
    pub fresh: bool,
    pub window_days: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingView {
    pub email_verified: bool,
    pub questionnaire_complete: bool,
    pub assessment_passed: bool,
    pub ready_for_publication: bool,
    pub visible_to_employers: bool,
    pub blockers: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GradeEligibilityView {
    pub selected_specialization: Specialization,
    pub selected_grade: Grade,
    pub can_start_selected: bool,
    pub selected_reason: Option<&'static str>,
    pub can_start_upgrade: bool,
    pub next_eligible_at: Option<String>,
    pub cooldown_days: u16,
    pub reason: Option<&'static str>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfilePatch {
    #[serde(default, deserialize_with = "present_patch_value")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub industry: Option<String>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub specialization: Option<Specialization>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub selected_grade: Option<Grade>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub stack: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub team_role: Option<String>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub experience_months: Option<Option<u32>>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub experience_since: Option<Option<u16>>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub experience: Option<String>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub soft_skills: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub consents: Option<fsp_domain::Consents>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyView {
    pub id: String,
    pub display_name: String,
    pub business_area: String,
    pub description: String,
    pub contact_email: String,
    pub website: Option<String>,
    pub ready_to_invite: bool,
}

impl From<Company> for CompanyView {
    fn from(company: Company) -> Self {
        let ready_to_invite = company.ready_to_invite();
        Self {
            id: company.id,
            display_name: company.display_name,
            business_area: company.business_area,
            description: company.description,
            contact_email: company.contact_email,
            website: company.website,
            ready_to_invite,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompanyPatch {
    #[serde(default, deserialize_with = "present_patch_value")]
    pub display_name: Option<String>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub business_area: Option<String>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub contact_email: Option<String>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub website: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NeedsView {
    pub items: Vec<NeedView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NeedView {
    pub id: String,
    pub company_id: String,
    pub title: String,
    pub specialization: Specialization,
    pub grade: Grade,
    pub work_mode: String,
    pub salary: SalaryView,
    pub description: String,
    pub requirements: Vec<RequirementView>,
    pub published: bool,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortlistInput {
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortlistsView {
    pub items: Vec<ShortlistSummaryView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortlistSummaryView {
    pub id: String,
    pub title: String,
    pub count: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortlistMemberView {
    pub candidate_id: String,
    pub alias: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortlistView {
    pub id: String,
    pub title: String,
    pub count: usize,
    pub created_at: String,
    pub updated_at: String,
    pub members: Vec<ShortlistMemberView>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvitationInput {
    pub candidate_id: String,
    pub need_id: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptInvitationInput {
    pub disclose: Vec<String>,
}

/// The need fields frozen into an invitation, as the contract exposes them.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitationNeedView {
    pub id: String,
    pub title: String,
    pub work_mode: String,
    pub salary: SalaryView,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitationView {
    pub id: String,
    pub candidate_id: String,
    pub candidate_alias: String,
    pub company: CompanyCardView,
    pub need: InvitationNeedView,
    pub message: String,
    pub status: fsp_domain::InteractionStatus,
    pub created_at: String,
    pub viewed_at: Option<String>,
    pub responded_at: Option<String>,
    pub disclosure_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitationsView {
    pub items: Vec<InvitationView>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DisclosureFieldsView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_links: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisclosureView {
    pub candidate_id: String,
    pub company_id: String,
    pub granted: bool,
    pub invitation_id: Option<String>,
    pub application_id: Option<String>,
    pub granted_at: Option<String>,
    pub fields: DisclosureFieldsView,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplicationInput {
    pub vacancy_id: String,
    pub message: String,
    pub disclose: Vec<String>,
}

/// The vacancy card frozen into an application, as the contract exposes it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationVacancyView {
    pub id: String,
    pub title: String,
    pub specialization: Specialization,
    pub grade: Grade,
    pub work_mode: String,
    pub salary: SalaryView,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationView {
    pub id: String,
    pub candidate_id: String,
    pub candidate_alias: String,
    pub company: CompanyCardView,
    pub vacancy: ApplicationVacancyView,
    pub message: String,
    pub status: fsp_domain::InteractionStatus,
    pub created_at: String,
    pub viewed_at: Option<String>,
    pub responded_at: Option<String>,
    pub disclosure_fields: Vec<String>,
    pub r#match: CandidateMatchView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationsView {
    pub items: Vec<ApplicationView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HiringFlowStageView {
    pub id: &'static str,
    pub label: &'static str,
    pub count: usize,
    pub share_percent: u8,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VacancyFlowView {
    pub vacancy_id: String,
    pub title: String,
    pub total: usize,
    pub awaiting_review: usize,
    pub awaiting_decision: usize,
    pub accepted: usize,
    pub closed: usize,
    pub required_coverage_percent: u8,
    pub grade_match_percent: u8,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HiringBottleneckView {
    pub kind: &'static str,
    pub severity: &'static str,
    pub title: String,
    pub evidence: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HiringAssistantView {
    pub mode: &'static str,
    pub summary: String,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HiringFlowAnalyticsView {
    pub generated_at: String,
    pub total: usize,
    pub stages: Vec<HiringFlowStageView>,
    pub median_time_to_view_hours: Option<u32>,
    pub median_time_to_decision_hours: Option<u32>,
    pub acceptance_rate_percent: u8,
    pub vacancies: Vec<VacancyFlowView>,
    pub bottlenecks: Vec<HiringBottleneckView>,
    pub assistant: HiringAssistantView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HiringFlowFacts {
    pub total: usize,
    pub stages: Vec<HiringFlowStageView>,
    pub median_time_to_view_hours: Option<u32>,
    pub median_time_to_decision_hours: Option<u32>,
    pub acceptance_rate_percent: u8,
    pub vacancies: Vec<VacancyFlowView>,
    pub bottlenecks: Vec<HiringBottleneckView>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HiringFlowNarrative {
    pub summary: String,
    pub actions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskInput {
    pub title: String,
    pub prompt: String,
    pub skill_id: String,
    pub grade: Grade,
    pub estimate_minutes: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskView {
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub skill_id: String,
    pub grade: Grade,
    pub estimate_minutes: u32,
    pub status: fsp_domain::TaskStatus,
    pub created_at: String,
    pub response_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TasksView {
    pub items: Vec<TaskView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfferedCompanyView {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfferedTaskView {
    pub id: String,
    pub company: OfferedCompanyView,
    pub title: String,
    pub prompt: String,
    pub skill_id: String,
    pub grade: Grade,
    pub estimate_minutes: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskResponseInput {
    pub mode: fsp_domain::TaskMode,
    pub answer: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponseView {
    pub id: String,
    pub candidate_id: String,
    pub candidate_alias: String,
    pub task: OfferedTaskView,
    pub mode: fsp_domain::TaskMode,
    pub answer: String,
    pub status: fsp_domain::TaskResponseStatus,
    pub feedback: Option<String>,
    pub submitted_at: String,
    pub reviewed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponsesView {
    pub items: Vec<TaskResponseView>,
}

/// Review outcomes the contract allows; `submitted` is not a valid outcome.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TaskReviewOutcome {
    Accepted,
    NeedsWork,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskReviewInput {
    pub outcome: TaskReviewOutcome,
    pub feedback: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPeriodView {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MyTasksView {
    pub period: TaskPeriodView,
    pub current: Option<OfferedTaskView>,
    pub current_response: Option<TaskResponseView>,
    pub reason: Option<&'static str>,
    pub history: Vec<TaskResponseView>,
    pub freshness: FreshnessView,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssessmentStartInput {
    pub specialization: Specialization,
    pub grade: Grade,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssistantInput {
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantReplyView {
    pub message: String,
    pub simulation: bool,
    pub ai_allowed: bool,
    pub external_ai_allowed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentView {
    pub id: String,
    pub specialization: Specialization,
    pub grade: Grade,
    pub status: fsp_domain::AssessmentStatus,
    pub task: fsp_domain::AssessmentTask,
    pub answer: fsp_domain::AssessmentAnswer,
    pub checks: Vec<fsp_domain::AssessmentCheck>,
    pub result: Option<fsp_domain::AssessmentResultData>,
    pub created_at: String,
    pub updated_at: String,
    pub revision: u64,
    pub simulation: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentsView {
    pub items: Vec<AssessmentView>,
}

/// The contract's `GapPlan`: requirements of a need the candidate does not
/// yet meet, with the accepted path to close each gap.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GapPlanView {
    pub candidate_id: String,
    pub need_id: String,
    pub items: Vec<GapItemView>,
    pub simulation: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GapItemView {
    pub skill_id: String,
    /// `needs_verification` or `not_evidenced` — `meets` never appears.
    pub status: &'static str,
    pub reason: &'static str,
    pub action: &'static str,
    pub accepted_competition_ids: Vec<String>,
    pub assessment_available: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportConfirmInput {
    pub profile: ProfilePatch,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SalaryView {
    pub from: u64,
    pub to: u64,
    pub currency: &'static str,
    pub period: &'static str,
    pub tax_basis: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementView {
    pub skill_id: String,
    pub criticality: Criticality,
    pub minimum_grade: Grade,
    pub evidence_policy: EvidencePolicyView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidencePolicyView {
    pub accept_assessments: bool,
    pub accept_github: bool,
    pub accept_fsp: bool,
    pub competition_selection: String,
    pub competition_ids: Vec<String>,
}

impl From<&EvidencePolicy> for EvidencePolicyView {
    fn from(value: &EvidencePolicy) -> Self {
        Self {
            accept_assessments: value.accept_assessments,
            accept_github: value.accept_github,
            accept_fsp: value.accept_fsp,
            competition_selection: value.competition_selection.clone(),
            competition_ids: value.competition_ids.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum SalaryCurrency {
    #[serde(rename = "RUB")]
    Rub,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum SalaryPeriod {
    #[serde(rename = "month")]
    Month,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum SalaryTaxBasis {
    #[serde(rename = "gross")]
    Gross,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SalaryInput {
    pub from: u64,
    pub to: u64,
    pub currency: SalaryCurrency,
    pub period: SalaryPeriod,
    pub tax_basis: SalaryTaxBasis,
}

/// The contract's `Requirement`/`EvidencePolicy` arrive camelCased; the
/// domain types stay snake_case for stored JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidencePolicyInput {
    pub accept_assessments: bool,
    pub accept_github: bool,
    pub accept_fsp: bool,
    pub competition_selection: String,
    pub competition_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequirementInput {
    pub skill_id: String,
    pub criticality: fsp_domain::Criticality,
    pub minimum_grade: Grade,
    pub evidence_policy: EvidencePolicyInput,
}

impl RequirementInput {
    pub fn into_domain(self) -> fsp_domain::Requirement {
        fsp_domain::Requirement {
            skill_id: self.skill_id,
            criticality: self.criticality,
            minimum_grade: self.minimum_grade,
            evidence_policy: fsp_domain::EvidencePolicy {
                accept_assessments: self.evidence_policy.accept_assessments,
                accept_github: self.evidence_policy.accept_github,
                accept_fsp: self.evidence_policy.accept_fsp,
                competition_selection: self.evidence_policy.competition_selection,
                competition_ids: self.evidence_policy.competition_ids,
            },
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NeedInput {
    pub title: String,
    pub specialization: Specialization,
    pub grade: Grade,
    pub work_mode: String,
    pub salary: SalaryInput,
    pub description: String,
    pub requirements: Vec<RequirementInput>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NeedPatch {
    #[serde(default, deserialize_with = "present_patch_value")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub specialization: Option<Specialization>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub grade: Option<Grade>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub work_mode: Option<String>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub salary: Option<SalaryInput>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "present_patch_value")]
    pub requirements: Option<Vec<RequirementInput>>,
}

impl NeedPatch {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.specialization.is_none()
            && self.grade.is_none()
            && self.work_mode.is_none()
            && self.salary.is_none()
            && self.description.is_none()
            && self.requirements.is_none()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyCardView {
    pub id: String,
    pub display_name: String,
    pub business_area: String,
    pub description: String,
    pub contact_email: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VacancyRequirementView {
    pub skill_id: String,
    pub criticality: Criticality,
    pub minimum_grade: Grade,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationEligibilityView {
    pub can_apply: bool,
    pub reason: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VacancyView {
    pub id: String,
    pub company: CompanyCardView,
    pub title: String,
    pub specialization: Specialization,
    pub grade: Grade,
    pub work_mode: String,
    pub salary: SalaryView,
    pub description: String,
    pub requirements: Vec<VacancyRequirementView>,
    pub published_at: String,
    pub updated_at: String,
    pub r#match: Option<CandidateMatchView>,
    pub application_id: Option<String>,
    pub eligibility: ApplicationEligibilityView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VacancyPageView {
    pub items: Vec<VacancyView>,
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub has_next_page: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidatePageView {
    pub items: Vec<AnonymousCandidateView>,
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub has_next_page: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnonymousCandidateView {
    pub id: String,
    pub alias: String,
    pub specialization: Specialization,
    /// The category grade: confirmed by a test or estimated from evidence.
    pub grade: Grade,
    pub category_source: fsp_domain::CategorySource,
    /// The estimate's confidence; `None` for a test result.
    pub category_confidence: Option<fsp_domain::SignalConfidence>,
    /// The test-confirmed grade, when there is one.
    pub verified_grade: Option<Grade>,
    pub grade_confirmed_at: Option<String>,
    pub estimate: Option<EmployerEstimateView>,
    pub work_modes: Vec<String>,
    pub available_for_offers: bool,
    pub skills: Vec<SkillView>,
    pub source_kinds: Vec<EvidenceSource>,
    pub fsp_verified: bool,
    pub freshness: FreshnessView,
    pub r#match: Option<CandidateMatchView>,
    /// Anonymous public-GitHub signal: no login, names, links or calendars.
    pub github: Option<fsp_domain::GitHubSignal>,
    pub fsp: Option<EmployerFspView>,
    pub review: Option<EmployerReviewView>,
    pub simulation: bool,
}

/// Aggregates only: no account, team, exact placement, protocol or external identifiers.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmployerFspView {
    pub verified_participation: bool,
    pub published_results: usize,
    pub podium_results: usize,
    pub relevant_results: usize,
    pub last_synced_at: Option<String>,
    pub synthetic: bool,
}

/// Only validated model classifications cross the anonymous boundary. Free-form reasoning and
/// recommendations may repeat user-authored personal details and remain in the owner's profile.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmployerReviewView {
    pub specialization: Specialization,
    pub grade: Grade,
    pub agrees: bool,
    pub computed_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillView {
    pub skill_id: String,
    pub label: String,
    pub status: EvidenceStatus,
    pub evidence_grade: Option<Grade>,
    pub evidence_sources: Vec<EvidenceSource>,
    pub summary: String,
    pub evidence_count: usize,
    pub confirmed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateMatchView {
    pub need_id: String,
    pub status: &'static str,
    pub grade_matches: bool,
    pub required_met: usize,
    pub required_total: usize,
    pub desired_met: usize,
    pub desired_total: usize,
    pub required_evidenced: usize,
    pub desired_evidenced: usize,
    pub requirements: Vec<MatchRequirementView>,
    pub explanation: String,
    pub algorithm_version: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchRequirementView {
    pub skill_id: String,
    pub criticality: Criticality,
    pub status: &'static str,
    pub evidence_status: EvidenceStatus,
    pub minimum_grade: Grade,
    pub evidence_grade: Option<Grade>,
    pub evidence_sources: Vec<EvidenceSource>,
    pub accepted_sources: Vec<EvidenceSource>,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct CandidateSearch {
    pub need_id: Option<String>,
    pub recommended: bool,
    pub specialization: Option<Specialization>,
    pub grade: Option<Grade>,
    /// Test-confirmed or estimated categories only.
    pub category_source: Option<fsp_domain::CategorySource>,
    pub skill: Option<String>,
    pub fsp_verified: bool,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone, Default)]
pub struct VacancySearch {
    pub specialization: Option<Specialization>,
    pub grade: Option<Grade>,
    pub work_mode: Option<String>,
    pub skill: Option<String>,
    pub page: usize,
    pub page_size: usize,
}

pub fn match_status(score: &MatchScore, requirements: &[MatchRequirementView]) -> &'static str {
    if requirements
        .iter()
        .any(|item| item.criticality == Criticality::Required && item.status == "not_evidenced")
    {
        "missing_required"
    } else if score.grade_matches && score.required_met == score.required_total {
        "meets_required"
    } else {
        "needs_verification"
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceConnectInput {
    pub account: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcesView {
    pub items: Vec<SourceView>,
    pub simulation: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceView {
    pub id: String,
    pub provider: SourceProvider,
    pub status: &'static str,
    pub synthetic: bool,
    pub active_job_id: Option<String>,
    pub account_label: Option<String>,
    pub last_synced_at: Option<String>,
    pub counts: SourceCountsView,
    pub repositories: Vec<RepositoryView>,
    pub active_days: Option<u32>,
    pub observation_period: Option<ObservationPeriodView>,
    pub verified_participation: bool,
    pub competition_ids: Vec<String>,
    pub notice: String,
    pub fsp_results: Vec<fsp_domain::FspResult>,
    /// GitHub only: ownership state, the public-activity report and the employer projection.
    pub github: Option<GitHubSourceView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubSourceView {
    pub ownership: GitHubOwnershipView,
    pub report: Option<fsp_domain::GitHubReport>,
    /// Exactly what employers see before any disclosure.
    pub employer_preview: Option<fsp_domain::GitHubSignal>,
    /// The background analysis while it runs; `None` when none is in flight.
    pub analysis: Option<GitHubAnalysisView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubAnalysisView {
    /// `queued`, `running` or `failed`.
    pub state: &'static str,
    pub stage: String,
    pub progress: u8,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubOwnershipView {
    pub verified: bool,
    pub method: Option<fsp_domain::OwnershipMethod>,
    pub verified_at: Option<String>,
    /// The candidate's code; adding it to the GitHub bio proves ownership on the next update.
    pub code: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCountsView {
    pub found: usize,
    pub processed: usize,
    pub queued: usize,
    pub unavailable: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryView {
    pub id: String,
    pub name: String,
    pub url: String,
    pub technologies: Vec<String>,
    pub status: &'static str,
    pub metrics: RepositoryMetrics,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservationPeriodView {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    /// Contract-shaped result payload; `kind` discriminates the variant.
    pub result: Option<serde_json::Value>,
    pub error: Option<JobErrorView>,
    pub poll_after_ms: u32,
    pub simulation: bool,
}

/// Typed constructors for `Job.result`; serialized values persist on the job
/// record so polling replays them without recomputation. Serialize-only DTO,
/// so variant size does not matter.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobResultView {
    SourceSync {
        source: SourceView,
    },
    Checks {
        assessment_id: String,
        checks: Vec<fsp_domain::AssessmentCheck>,
        simulation: bool,
    },
    Assessment {
        assessment: AssessmentView,
    },
    /// Extracted fields remain a proposal until explicit confirmation.
    ProfileImport {
        preview: serde_json::Value,
        parser: &'static str,
        simulation: bool,
        evidence: Vec<fsp_domain::ImportEvidence>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobErrorView {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

pub fn source_view(
    record: SourceRecord,
    ownership_code: &str,
    progress: Option<GitHubAnalysisView>,
) -> SourceView {
    let analysis = record.analysis;
    let github = (record.provider == SourceProvider::Github).then(|| {
        let ownership = analysis
            .as_ref()
            .and_then(|analysis| analysis.ownership.clone());
        let report = analysis
            .as_ref()
            .and_then(|analysis| analysis.report.clone());
        GitHubSourceView {
            employer_preview: report
                .as_ref()
                .map(|report| report.employer_signal(ownership.is_some())),
            report,
            analysis: progress,
            ownership: GitHubOwnershipView {
                verified: ownership.is_some(),
                method: ownership.as_ref().map(|ownership| ownership.method),
                verified_at: ownership.map(|ownership| ownership.verified_at),
                code: ownership_code.into(),
            },
        }
    });
    let repositories = analysis
        .as_ref()
        .map(|value| {
            value
                .repositories
                .iter()
                .map(|repository| RepositoryView {
                    id: repository.id.clone(),
                    name: repository.name.clone(),
                    url: repository.url.clone(),
                    technologies: repository.technologies.clone(),
                    status: "processed",
                    metrics: repository.metrics.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let observation_period = analysis.as_ref().and_then(|value| {
        Some(ObservationPeriodView {
            from: value.observed_from.clone()?,
            to: value.observed_to.clone()?,
        })
    });
    SourceView {
        id: record.id,
        provider: record.provider,
        status: if record.account_label.is_some() {
            "connected"
        } else {
            "not_connected"
        },
        synthetic: record.synthetic,
        active_job_id: record.active_job_id,
        account_label: record.account_label,
        last_synced_at: record.last_synced_at,
        counts: SourceCountsView {
            found: repositories.len(),
            processed: repositories.len(),
            queued: 0,
            unavailable: 0,
        },
        repositories,
        active_days: analysis.as_ref().map(|value| value.active_days),
        observation_period,
        verified_participation: record.verified_participation,
        competition_ids: record.competition_ids,
        fsp_results: record.fsp_results,
        notice: if record.synthetic {
            "Вымышленные данные ФСП. Командный результат не подтверждает индивидуальные навыки или грейд.".into()
        } else if analysis.is_some() {
            "Только публичные данные GitHub: приватные репозитории не анализируются. Метрики — сигналы активности, а не оценка навыков.".into()
        } else {
            String::new()
        },
        github,
    }
}

pub fn job_view(record: JobRecord, source: Option<SourceView>) -> JobView {
    // The mock asks clients to poll in-flight jobs every 250 ms and stops polling once a job is
    // terminal. A GitHub analysis runs for minutes, so its source sync is polled every two seconds.
    let terminal = record.status == "succeeded" || record.status == "failed";
    let poll_after_ms = match (terminal, record.kind.as_str()) {
        (true, _) => 0,
        (false, "source_sync") => 2_000,
        (false, _) => 250,
    };
    let result = record
        .result_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        .or_else(|| {
            source.map(|source| {
                serde_json::to_value(JobResultView::SourceSync { source }).unwrap_or_default()
            })
        });
    let simulation = result
        .as_ref()
        .and_then(|value: &serde_json::Value| {
            value
                .get("simulation")
                .or_else(|| {
                    value
                        .get("assessment")
                        .and_then(|assessment| assessment.get("simulation"))
                })
                .or_else(|| {
                    value
                        .get("source")
                        .and_then(|source| source.get("synthetic"))
                })
        })
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(record.kind != "profile_import");
    JobView {
        id: record.id,
        kind: record.kind,
        status: record.status,
        created_at: record.created_at,
        updated_at: record.updated_at,
        result,
        error: record.error_code.map(|code| JobErrorView {
            code,
            message: record.error_message.unwrap_or_default(),
            retryable: true,
        }),
        poll_after_ms,
        simulation,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsView {
    pub items: Vec<SelfSkillView>,
    pub simulation: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfSkillView {
    pub skill_id: String,
    pub label: String,
    pub status: EvidenceStatus,
    pub evidence_grade: Option<Grade>,
    pub evidence_sources: Vec<EvidenceSource>,
    pub summary: String,
    pub evidence_count: usize,
    pub confirmed_at: Option<String>,
    pub evidence: Vec<EvidenceView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceView {
    pub id: String,
    pub source: EvidenceSource,
    pub title: String,
    pub url: Option<String>,
    pub reference: Option<String>,
    pub observed_at: String,
    pub scope: &'static str,
    pub synthetic: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphView {
    pub audience: &'static str,
    pub nodes: Vec<GraphNodeView>,
    pub edges: Vec<GraphEdgeView>,
    pub simulation: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNodeView {
    pub id: String,
    /// `role`, `concept` (a catalog skill category), `skill`, `source` (GitHub, FSP, a test).
    pub kind: String,
    pub label: String,
    pub skill_id: Option<String>,
    pub status: Option<String>,
    /// The level or the scale behind the node, e.g. «Основная · 49 проектов».
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdgeView {
    pub id: String,
    pub from: String,
    pub to: String,
    pub relation: String,
}

// Missing fields use the default outer None. Present values deserialize the
// inner type, preserving explicit null only for nullable contract properties.
fn present_patch_value<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
