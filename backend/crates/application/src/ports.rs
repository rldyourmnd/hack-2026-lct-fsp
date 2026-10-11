use async_trait::async_trait;
use fsp_domain::{
    Account, CandidateIndexRecord, CandidateProfile, Company, FspParticipation, GitHubReport,
    HiringNeedRecord, JobRecord, SourceRecord,
};

use crate::PendingChallenge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumeChallenge {
    Consumed,
    Missing,
    Used,
    Expired,
    Locked,
    WrongCode,
}

#[derive(Debug)]
pub enum RepositoryError {
    /// A uniqueness constraint rejected the write (map to a 409 surface).
    Conflict,
    /// The row addressed by a guarded write did not exist or no longer
    /// satisfies the preconditions (map to a 404/409 surface per call site).
    NotFound,
    /// Transient infrastructure failure: pool exhaustion, lost connection.
    /// Retrying the request may succeed.
    Unavailable,
    Storage(String),
}

/// Outcome of claiming an idempotency scope for a request.
#[derive(Debug)]
pub enum IdempotencyClaim {
    /// No row existed: this caller runs the command and finalizes the scope
    /// with [`PlatformRepository::idempotency_finish`].
    Execute,
    /// A finished response is stored; the caller must fingerprint-check it.
    Stored(Box<StoredIdempotentResponse>),
    /// An unresolved legacy claim exists; its business effect is unknown.
    /// Refuse execution instead of taking over by age.
    InFlight,
}

/// The storage boundary. Every method is async; multi-statement use cases run
/// inside a [`RepositoryTransaction`] obtained from `transaction()` so partial
/// commits are impossible by construction.
#[async_trait]
pub trait PlatformRepository: Send + Sync {
    /// Opens a unit of work. All repository methods on the returned value share
    /// one database transaction until `commit()`; dropping it rolls back.
    /// Calling `transaction()` on a transaction returns an error.
    async fn transaction(&self) -> Result<Box<dyn RepositoryTransaction>, RepositoryError>;

    /// Serialize a logical aggregate for the lifetime of this transaction.
    async fn lock_scope(&self, scope: &str) -> Result<(), RepositoryError>;
    /// Lock the candidate row before reading a profile/index that will be changed.
    async fn lock_candidate(&self, candidate_id: &str) -> Result<(), RepositoryError>;

    async fn account_by_email(&self, email: &str) -> Result<Option<Account>, RepositoryError>;
    async fn create_account(&self, account: &Account, now: &str) -> Result<(), RepositoryError>;
    async fn create_candidate(&self, profile: &CandidateProfile) -> Result<(), RepositoryError>;
    async fn create_company(&self, company: &Company) -> Result<(), RepositoryError>;
    async fn candidate(
        &self,
        candidate_id: &str,
    ) -> Result<Option<CandidateProfile>, RepositoryError>;
    async fn save_candidate(&self, profile: &CandidateProfile) -> Result<(), RepositoryError>;
    /// Conditional write: updates only when the stored row is still at
    /// `expected_revision`. `NotFound` means a concurrent writer moved it.
    async fn save_candidate_guarded(
        &self,
        profile: &CandidateProfile,
        expected_revision: u64,
    ) -> Result<(), RepositoryError>;
    async fn company(&self, company_id: &str) -> Result<Option<Company>, RepositoryError>;
    async fn save_company(&self, company: &Company) -> Result<(), RepositoryError>;
    async fn candidates(&self) -> Result<Vec<CandidateIndexRecord>, RepositoryError>;
    /// Single-candidate index row — the hot-path lookup; `candidates()` is the
    /// full scan for listing/search use cases.
    async fn candidate_index(
        &self,
        candidate_id: &str,
    ) -> Result<Option<CandidateIndexRecord>, RepositoryError>;
    /// Persists the derived candidate index (skills, visibility flags); the
    /// row is created on first save when seeding missed the candidate.
    async fn save_candidate_index(
        &self,
        record: &CandidateIndexRecord,
    ) -> Result<(), RepositoryError>;

    /// Atomically issues a verification challenge: persists it only when the
    /// caller is under the lockout budget — the running sum of attempts on
    /// live challenges for the same email stays below five. Real delivery also
    /// enforces one issuance per minute and five per fifteen minutes.
    async fn issue_challenge(
        &self,
        challenge: &PendingChallenge,
        limit_delivery: bool,
    ) -> Result<crate::IssueChallenge, RepositoryError>;
    /// Marks `true` on the resolved account once its challenge is consumed.
    async fn mark_email_verified(&self, account_id: &str) -> Result<(), RepositoryError>;
    /// Consumes a challenge row under `FOR UPDATE`: expiry/usage/lock checks,
    /// attempt counting and the used-mark share one transaction. On
    /// `Consumed` the claimed challenge comes back so the caller can resolve
    /// or register the account inside its own unit of work.
    async fn consume_challenge(
        &self,
        challenge_id: &str,
        code_hash: &str,
        now: &str,
    ) -> Result<(ConsumeChallenge, Option<PendingChallenge>), RepositoryError>;
    async fn create_session(
        &self,
        token_hash: &str,
        account_id: &str,
        expires_at: &str,
    ) -> Result<(), RepositoryError>;
    async fn account_for_session(
        &self,
        token_hash: &str,
        now: &str,
    ) -> Result<Option<Account>, RepositoryError>;
    async fn delete_session(&self, token_hash: &str) -> Result<(), RepositoryError>;

    async fn needs(&self, company_id: &str) -> Result<Vec<HiringNeedRecord>, RepositoryError>;
    async fn need(
        &self,
        company_id: &str,
        need_id: &str,
    ) -> Result<Option<HiringNeedRecord>, RepositoryError>;
    async fn save_need(&self, need: &HiringNeedRecord) -> Result<(), RepositoryError>;
    /// Conditional write mirroring [`save_candidate_guarded`].
    async fn save_need_guarded(
        &self,
        need: &HiringNeedRecord,
        expected_revision: u64,
    ) -> Result<(), RepositoryError>;
    async fn delete_need(&self, company_id: &str, need_id: &str) -> Result<(), RepositoryError>;
    /// Published needs across all companies, i.e. the vacancy projection.
    async fn published_needs(&self) -> Result<Vec<HiringNeedRecord>, RepositoryError>;

    async fn shortlists(
        &self,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::ShortlistRecord>, RepositoryError>;
    async fn shortlist(
        &self,
        company_id: &str,
        shortlist_id: &str,
    ) -> Result<Option<fsp_domain::ShortlistRecord>, RepositoryError>;
    async fn save_shortlist(
        &self,
        shortlist: &fsp_domain::ShortlistRecord,
    ) -> Result<(), RepositoryError>;
    async fn delete_shortlist(
        &self,
        company_id: &str,
        shortlist_id: &str,
    ) -> Result<(), RepositoryError>;

    /// Invitations sent by a company, in creation order.
    async fn invitations(
        &self,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::InvitationRecord>, RepositoryError>;
    /// Invitations addressed to a candidate, in creation order.
    async fn candidate_invitations(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::InvitationRecord>, RepositoryError>;
    /// Unscoped lookup; the service applies the caller's tenancy rule.
    async fn invitation(
        &self,
        invitation_id: &str,
    ) -> Result<Option<fsp_domain::InvitationRecord>, RepositoryError>;
    async fn save_invitation(
        &self,
        invitation: &fsp_domain::InvitationRecord,
    ) -> Result<(), RepositoryError>;
    /// Conditional transition: updates only while the stored status is still
    /// in `expected_statuses` — the concurrent-decision guard.
    async fn save_invitation_guarded(
        &self,
        invitation: &fsp_domain::InvitationRecord,
        expected_statuses: &[fsp_domain::InteractionStatus],
    ) -> Result<(), RepositoryError>;

    async fn disclosure(
        &self,
        company_id: &str,
        candidate_id: &str,
    ) -> Result<Option<fsp_domain::DisclosureRecord>, RepositoryError>;
    async fn save_disclosure(
        &self,
        disclosure: &fsp_domain::DisclosureRecord,
    ) -> Result<(), RepositoryError>;
    async fn delete_disclosure(
        &self,
        company_id: &str,
        candidate_id: &str,
    ) -> Result<(), RepositoryError>;

    /// Applications addressed to a company's needs.
    async fn company_applications(
        &self,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::ApplicationRecord>, RepositoryError>;
    /// Applications sent by a candidate.
    async fn candidate_applications(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::ApplicationRecord>, RepositoryError>;
    /// Unscoped lookup; the service applies the caller's tenancy rule.
    async fn application(
        &self,
        application_id: &str,
    ) -> Result<Option<fsp_domain::ApplicationRecord>, RepositoryError>;
    async fn save_application(
        &self,
        application: &fsp_domain::ApplicationRecord,
    ) -> Result<(), RepositoryError>;
    /// Conditional transition mirroring [`save_invitation_guarded`].
    async fn save_application_guarded(
        &self,
        application: &fsp_domain::ApplicationRecord,
        expected_statuses: &[fsp_domain::InteractionStatus],
    ) -> Result<(), RepositoryError>;

    /// Tasks owned by a company.
    async fn tasks(&self, company_id: &str)
    -> Result<Vec<fsp_domain::TaskRecord>, RepositoryError>;
    /// All active tasks across companies, for the weekly offer pool.
    async fn active_tasks(&self) -> Result<Vec<fsp_domain::TaskRecord>, RepositoryError>;
    /// Unscoped lookup; the service applies the caller's tenancy rule.
    async fn task(&self, task_id: &str) -> Result<Option<fsp_domain::TaskRecord>, RepositoryError>;
    async fn save_task(&self, task: &fsp_domain::TaskRecord) -> Result<(), RepositoryError>;

    /// Responses sent by a candidate, for offers, history and freshness.
    async fn candidate_task_responses(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::TaskResponseRecord>, RepositoryError>;
    /// Responses addressed to a company's tasks.
    async fn company_task_responses(
        &self,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::TaskResponseRecord>, RepositoryError>;
    /// Unscoped lookup; the service applies the caller's tenancy rule.
    async fn task_response(
        &self,
        response_id: &str,
    ) -> Result<Option<fsp_domain::TaskResponseRecord>, RepositoryError>;
    /// Candidate's response to a specific task — the submit-time dedup read.
    async fn task_response_for(
        &self,
        candidate_id: &str,
        task_id: &str,
    ) -> Result<Option<fsp_domain::TaskResponseRecord>, RepositoryError>;
    async fn save_task_response(
        &self,
        response: &fsp_domain::TaskResponseRecord,
    ) -> Result<(), RepositoryError>;
    /// Conditional review transition: `NotFound` means another reviewer beat
    /// this request (the contract's `task_response_reviewed` surface).
    async fn review_task_response(
        &self,
        response: &fsp_domain::TaskResponseRecord,
    ) -> Result<(), RepositoryError>;

    /// Assessments owned by a candidate, newest first.
    async fn candidate_assessments(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::AssessmentRecord>, RepositoryError>;
    /// Unscoped lookup; the service applies the caller's tenancy rule.
    async fn assessment(
        &self,
        assessment_id: &str,
    ) -> Result<Option<fsp_domain::AssessmentRecord>, RepositoryError>;
    async fn save_assessment(
        &self,
        assessment: &fsp_domain::AssessmentRecord,
    ) -> Result<(), RepositoryError>;
    /// Conditional write: updates only when the stored row is still at
    /// `expected_revision` and its status is in `expected_statuses`.
    async fn save_assessment_guarded(
        &self,
        assessment: &fsp_domain::AssessmentRecord,
        expected_revision: u64,
        expected_statuses: &[fsp_domain::AssessmentStatus],
    ) -> Result<(), RepositoryError>;

    async fn save_oauth_state(
        &self,
        candidate_id: &str,
        state: &str,
        expires_at: &str,
    ) -> Result<(), RepositoryError>;
    async fn consume_oauth_state(
        &self,
        candidate_id: &str,
        state: &str,
        now: &str,
    ) -> Result<bool, RepositoryError>;
    async fn save_github_token(
        &self,
        candidate_id: &str,
        token: &str,
    ) -> Result<(), RepositoryError>;
    async fn github_token(&self, candidate_id: &str) -> Result<Option<String>, RepositoryError>;
    async fn sources(&self, candidate_id: &str) -> Result<Vec<SourceRecord>, RepositoryError>;
    /// GitHub sources of any candidate connected to `login`, compared case-insensitively.
    async fn github_sources(&self, login: &str) -> Result<Vec<SourceRecord>, RepositoryError>;
    async fn save_source(&self, source: &SourceRecord) -> Result<(), RepositoryError>;
    async fn save_job(&self, job: &JobRecord) -> Result<(), RepositoryError>;
    /// Conditional job write: persists only while `confirmed` is still FALSE —
    /// the profile-import confirm guard.
    async fn save_job_unconfirmed(&self, job: &JobRecord) -> Result<(), RepositoryError>;
    async fn job(&self, owner_id: &str, job_id: &str)
    -> Result<Option<JobRecord>, RepositoryError>;

    /// Claims an idempotency scope. `Execute` — this caller runs the command;
    /// `Stored` — a finished response exists (fingerprint-check it);
    /// `InFlight` — unresolved legacy work, execution must be refused.
    /// Only valid on a transaction. Concurrent callers wait on its scope lock;
    /// business writes and `idempotency_finish` must use the same unit of work.
    async fn idempotency_claim(
        &self,
        scope: &str,
        fingerprint: &str,
    ) -> Result<IdempotencyClaim, RepositoryError>;
    async fn idempotency_finish(
        &self,
        scope: &str,
        status: u16,
        body: &str,
    ) -> Result<(), RepositoryError>;

    /// Periodic retention sweep: deletes expired sessions, challenges and
    /// OAuth states plus finished idempotency rows older than a day.
    async fn sweep_expired(&self, now: &str) -> Result<u64, RepositoryError>;

    /// Liveness probe for the health surface.
    async fn ping(&self) -> Result<(), RepositoryError>;

    async fn seed_demo(&self, now: &str) -> Result<(), RepositoryError>;
}

/// A live unit of work: the full repository surface bound to one transaction.
/// Obtain via [`PlatformRepository::transaction`]; `commit()` persists, drop
/// rolls back.
#[async_trait]
pub trait RepositoryTransaction: PlatformRepository {
    /// A repository view sharing this transaction, for composed application commands.
    /// It becomes unusable after commit; dropping all views rolls back uncommitted work.
    fn share(&self) -> std::sync::Arc<dyn PlatformRepository>;
    async fn commit(self: Box<Self>) -> Result<(), RepositoryError>;
}

/// A stored `Idempotency-Key` response: the request fingerprint plus the
/// serialized success payload that a replayed request must receive verbatim.
#[derive(Debug, Clone)]
pub struct StoredIdempotentResponse {
    pub fingerprint: String,
    pub status: u16,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationError {
    pub code: &'static str,
    pub message: String,
}

#[async_trait]
pub trait GitHubGateway: Send + Sync {
    fn authorization_url(
        &self,
        state: &str,
        redirect_uri: &str,
    ) -> Result<String, IntegrationError>;
    async fn exchange_code(&self, code: &str) -> Result<String, IntegrationError>;
    /// The login the OAuth token belongs to; a match proves account ownership.
    async fn authenticated_login(&self, token: &str) -> Result<String, IntegrationError>;
}

/// A public-activity report and whether the profile bio carried the ownership code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubInsights {
    pub report: GitHubReport,
    pub ownership_code_found: Option<bool>,
}

/// A background analysis: still running, finished or failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsightsStatus {
    Pending {
        /// Waiting for a worker or for GitHub's rate limit to reset.
        queued: bool,
        stage: String,
        progress: u8,
    },
    Done(Box<GitHubInsights>),
    Failed(IntegrationError),
}

#[async_trait]
pub trait GitHubInsightsGateway: Send + Sync {
    /// Starts an analysis, or returns the running or fresh one.
    async fn submit(
        &self,
        login: &str,
        ownership_code: Option<&str>,
        refresh: bool,
    ) -> Result<InsightsStatus, IntegrationError>;
    /// The current analysis; `None` when the service does not know it (for example after a restart).
    async fn status(
        &self,
        login: &str,
        ownership_code: Option<&str>,
    ) -> Result<Option<InsightsStatus>, IntegrationError>;
}

#[async_trait]
pub trait LlmGateway: Send + Sync {
    async fn extract_resume(
        &self,
        pages: &[fsp_domain::PdfPage],
    ) -> Result<fsp_domain::ResumeExtraction, IntegrationError>;
    async fn assessment_help(
        &self,
        request: &fsp_domain::AssessmentHelpRequest,
    ) -> Result<fsp_domain::AssessmentHelpReply, IntegrationError>;

    async fn match_repository(
        &self,
        request: &fsp_domain::RepositoryMatchRequest,
    ) -> Result<fsp_domain::RepositoryCompetitionMatch, IntegrationError>;
    async fn analyze_hiring_flow(
        &self,
        _facts: &crate::HiringFlowFacts,
    ) -> Result<crate::HiringFlowNarrative, IntegrationError> {
        Err(IntegrationError {
            code: "bifrost_unavailable",
            message: "Сервис анализа не настроен".into(),
        })
    }
    /// A second opinion on an automatic profile estimate from anonymized evidence.
    async fn review_profile(
        &self,
        _request: &fsp_domain::ProfileReviewRequest,
    ) -> Result<fsp_domain::ProfileReviewReply, IntegrationError> {
        Err(IntegrationError {
            code: "bifrost_unavailable",
            message: "Сервис анализа не настроен".into(),
        })
    }
}

#[async_trait]
pub trait FspGateway: Send + Sync {
    async fn participation(&self, account: &str) -> Result<FspParticipation, IntegrationError>;
}

#[async_trait]
pub trait VerificationDelivery: Send + Sync {
    fn code_for(&self) -> Result<String, IntegrationError>;
    fn code_for_email(&self, _email: &str) -> Result<String, IntegrationError> {
        self.code_for()
    }
    async fn deliver(
        &self,
        email: &str,
        code: &str,
        challenge_id: &str,
    ) -> Result<(), IntegrationError>;
    fn contract_name(&self) -> &'static str;
    fn contract_name_for_email(&self, _email: &str) -> &'static str {
        self.contract_name()
    }
}

#[async_trait]
pub trait PdfGateway: Send + Sync {
    async fn extract(&self, bytes: &[u8]) -> Result<Vec<fsp_domain::PdfPage>, IntegrationError>;
}

#[async_trait]
pub trait InterviewGateway: Send + Sync {
    async fn list(&self, candidate_id: &str) -> Result<crate::InterviewsView, IntegrationError>;
    async fn start(
        &self,
        candidate_id: &str,
        input: &crate::InterviewStartInput,
    ) -> Result<crate::InterviewView, IntegrationError>;
    async fn get(&self, interview_id: &str) -> Result<crate::InterviewView, IntegrationError>;
    async fn answer(
        &self,
        interview_id: &str,
        input: &crate::InterviewAnswerInput,
    ) -> Result<crate::InterviewView, IntegrationError>;
}

#[async_trait]
pub trait SpeechSynthesisGateway: Send + Sync {
    async fn synthesize(&self, text: &str) -> Result<crate::SpeechAudioView, IntegrationError>;
}
