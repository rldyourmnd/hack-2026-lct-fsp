mod commands;
mod graphs;
mod imports;
mod sources;
pub use commands::Commands;

use std::sync::Arc;

use chrono::{Datelike, Duration, Utc};
use fsp_domain::{
    Account, CandidateIndexRecord, CandidateProfile, Company, Consents, EvidenceStatus, Grade,
    HiringNeedRecord, InteractionStatus, Requirement, Role, Specialization, rank_candidates, score,
};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    AcceptInvitationInput, AnonymousCandidateView, ApplicationEligibilityView, ApplicationInput,
    ApplicationVacancyView, ApplicationView, ApplicationsView, AssessmentStartInput,
    AssessmentView, AssessmentsView, AssistantInput, AssistantReplyView, CandidateMatchView,
    CandidatePageView, CandidateSearch, CategoryView, CompanyCardView, CompanyPatch, CompanyView,
    ConsumeChallenge, DisclosureFieldsView, DisclosureView, EmailChallengeInput,
    EmailChallengeView, EmailVerifyInput, EmployerEstimateFactorView, EmployerEstimateView,
    EvidencePolicyView, EvidenceView, FreshnessView, FspGateway, GapItemView, GapPlanView,
    GitHubGateway, GradeEligibilityView, GraphEdgeView, GraphNodeView, GraphView,
    HiringAssistantView, HiringFlowAnalyticsView, IdempotencyClaim, ImportConfirmInput,
    IntegrationError, InterviewAnswerInput, InterviewGateway, InterviewSpeechInput,
    InterviewStartInput, InterviewTranscriptionInput, InterviewView, InterviewsView,
    InvitationInput, InvitationNeedView, InvitationView, InvitationsView, JobResultView, JobView,
    LlmGateway, MatchRequirementView, MyTasksView, NeedInput, NeedPatch, NeedView, NeedsView,
    OfferedCompanyView, OfferedTaskView, OnboardingView, PendingChallenge, PlatformRepository,
    ProfilePatch, ProfileView, RepositoryError, RequirementInput, RequirementView, SalaryView,
    SelfSkillView, SessionView, ShortlistInput, ShortlistMemberView, ShortlistSummaryView,
    ShortlistView, ShortlistsView, SkillView, SkillsView, SourceConnectInput, SourceView,
    SourcesView, SpeechAudioView, SpeechRecognitionGateway, SpeechSynthesisGateway,
    SpeechTranscriptView, TaskInput, TaskPeriodView, TaskResponseInput, TaskResponseView,
    TaskResponsesView, TaskReviewInput, TaskReviewOutcome, TaskView, TasksView, VacancyPageView,
    VacancyRequirementView, VacancySearch, VacancyView, VerificationDelivery, job_view,
    match_status, source_view,
};

#[derive(Debug)]
pub struct ApplicationError {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

impl ApplicationError {
    fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    fn storage(error: RepositoryError) -> Self {
        tracing::error!(error = ?error, "storage operation failed");
        Self::new(500, "internal_error", "Сервис временно недоступен")
    }
}

/// Outcome of an `Idempotency-Key` scoped command: either the stored response
/// is replayed verbatim or the fresh operation result is returned.
pub enum Idempotent<T> {
    Replayed { status: u16, body: String },
    Fresh(T),
}

/// The request context that scopes an idempotent command: operation name, the
/// concrete path, the client key, the parsed body and the success status the
/// replayed response must keep.
pub struct IdempotencyContext<'a> {
    pub operation: &'static str,
    pub path: &'a str,
    pub key: &'a str,
    pub body: &'a serde_json::Value,
    pub status: u16,
}

#[derive(Clone)]
pub struct PlatformService {
    repository: Arc<dyn PlatformRepository>,
    delivery: Arc<dyn VerificationDelivery>,
    github: Option<Arc<dyn GitHubGateway>>,
    github_insights: Option<Arc<dyn crate::GitHubInsightsGateway>>,
    pdf: Option<Arc<dyn crate::PdfGateway>>,
    llm: Option<Arc<dyn LlmGateway>>,
    fsp: Option<Arc<dyn FspGateway>>,
    interviews: Option<Arc<dyn InterviewGateway>>,
    speech: Option<Arc<dyn SpeechSynthesisGateway>>,
    transcription: Option<Arc<dyn SpeechRecognitionGateway>>,
    grade_cooldown_days: u16,
    auth_key: Option<Vec<u8>>,
    /// Candidates with a profile review running, and whether a newer estimate awaits one.
    reviews: Arc<std::sync::Mutex<std::collections::HashMap<String, bool>>>,
}

impl PlatformService {
    pub fn new(
        repository: Arc<dyn PlatformRepository>,
        delivery: Arc<dyn VerificationDelivery>,
    ) -> Self {
        Self {
            repository,
            delivery,
            github: None,
            github_insights: None,
            llm: None,
            pdf: None,
            fsp: None,
            interviews: None,
            speech: None,
            transcription: None,
            grade_cooldown_days: 90,
            auth_key: None,
            reviews: Arc::default(),
        }
    }

    /// Rotating this key invalidates prior codes and sessions without deleting accounts.
    pub fn with_auth_key(mut self, hex_key: &str) -> Result<Self, &'static str> {
        if hex_key.len() != 64 || !hex_key.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err("FSP_AUTH_KEY must be 32 random bytes encoded as 64 hex characters");
        }
        self.auth_key = Some(
            (0..64)
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex_key[i..i + 2], 16).expect("validated hex"))
                .collect(),
        );
        Ok(self)
    }

    fn credential_hash(&self, kind: &str, context: &str, value: &str) -> String {
        let Some(key) = &self.auth_key else {
            return digest(value);
        };
        let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key size");
        for field in [kind, context, value] {
            mac.update(&(field.len() as u64).to_be_bytes());
            mac.update(field.as_bytes());
        }
        format!("{:x}", mac.finalize().into_bytes())
    }

    #[must_use]
    pub fn with_grade_cooldown_days(mut self, days: u16) -> Self {
        self.grade_cooldown_days = days;
        self
    }

    #[must_use]
    pub fn with_github(mut self, github: Arc<dyn GitHubGateway>) -> Self {
        self.github = Some(github);
        self
    }

    #[must_use]
    pub fn with_github_insights(mut self, insights: Arc<dyn crate::GitHubInsightsGateway>) -> Self {
        self.github_insights = Some(insights);
        self
    }

    #[must_use]
    pub fn with_llm(mut self, llm: Arc<dyn LlmGateway>) -> Self {
        self.llm = Some(llm);
        self
    }

    #[must_use]
    pub fn with_fsp(mut self, fsp: Arc<dyn FspGateway>) -> Self {
        self.fsp = Some(fsp);
        self
    }

    #[must_use]
    pub fn with_pdf(mut self, pdf: Arc<dyn crate::PdfGateway>) -> Self {
        self.pdf = Some(pdf);
        self
    }

    #[must_use]
    pub fn with_interviews(mut self, interviews: Arc<dyn InterviewGateway>) -> Self {
        self.interviews = Some(interviews);
        self
    }

    #[must_use]
    pub fn with_speech(mut self, speech: Arc<dyn SpeechSynthesisGateway>) -> Self {
        self.speech = Some(speech);
        self
    }

    #[must_use]
    pub fn with_transcription(mut self, transcription: Arc<dyn SpeechRecognitionGateway>) -> Self {
        self.transcription = Some(transcription);
        self
    }

    #[must_use]
    pub fn speech_configured(&self) -> bool {
        self.speech.is_some()
    }

    #[must_use]
    pub fn interviews_configured(&self) -> bool {
        self.interviews.is_some()
    }

    pub async fn my_interviews(
        &self,
        session: &SessionView,
    ) -> Result<InterviewsView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        self.interviews
            .as_ref()
            .ok_or_else(interviews_unavailable)?
            .list(candidate_id)
            .await
            .map_err(integration_error)
    }

    pub async fn start_interview(
        &self,
        session: &SessionView,
        input: InterviewStartInput,
    ) -> Result<InterviewView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        self.require_processing_consent(candidate_id).await?;
        self.interviews
            .as_ref()
            .ok_or_else(interviews_unavailable)?
            .start(candidate_id, &input)
            .await
            .map_err(integration_error)
    }

    pub async fn get_interview(
        &self,
        session: &SessionView,
        interview_id: &str,
    ) -> Result<InterviewView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        let interview = self
            .interviews
            .as_ref()
            .ok_or_else(interviews_unavailable)?
            .get(interview_id)
            .await
            .map_err(integration_error)?;
        if interview.candidate_id != candidate_id {
            return Err(ApplicationError::new(
                404,
                "interview_not_found",
                "Собеседование не найдено",
            ));
        }
        Ok(interview)
    }

    pub async fn answer_interview(
        &self,
        session: &SessionView,
        interview_id: &str,
        input: InterviewAnswerInput,
    ) -> Result<InterviewView, ApplicationError> {
        let interview = self.get_interview(session, interview_id).await?;
        self.require_processing_consent(&interview.candidate_id)
            .await?;
        self.interviews
            .as_ref()
            .ok_or_else(interviews_unavailable)?
            .answer(interview_id, &input)
            .await
            .map_err(integration_error)
    }

    pub async fn synthesize_interview_question(
        &self,
        session: &SessionView,
        interview_id: &str,
        input: InterviewSpeechInput,
    ) -> Result<SpeechAudioView, ApplicationError> {
        let interview = self.get_interview(session, interview_id).await?;
        let question = interview.current_question.ok_or_else(|| {
            ApplicationError::new(409, "interview_completed", "Собеседование уже завершено")
        })?;
        if question.id != input.question_id {
            return Err(ApplicationError::new(
                409,
                "interview_question_changed",
                "Вопрос собеседования уже изменился",
            ));
        }
        self.speech
            .as_ref()
            .ok_or_else(|| {
                ApplicationError::new(503, "speech_unavailable", "Озвучивание временно недоступно")
            })?
            .synthesize(&question.delivery.speech_text)
            .await
            .map_err(integration_error)
    }

    pub async fn transcribe_interview_answer(
        &self,
        session: &SessionView,
        interview_id: &str,
        input: InterviewTranscriptionInput,
    ) -> Result<SpeechTranscriptView, ApplicationError> {
        let interview = self.get_interview(session, interview_id).await?;
        let question = interview.current_question.ok_or_else(|| {
            ApplicationError::new(409, "interview_completed", "Собеседование уже завершено")
        })?;
        if question.id != input.question_id {
            return Err(ApplicationError::new(
                409,
                "interview_question_changed",
                "Вопрос собеседования уже изменился",
            ));
        }
        self.transcription
            .as_ref()
            .ok_or_else(|| {
                ApplicationError::new(
                    503,
                    "speech_unavailable",
                    "Распознавание речи временно недоступно",
                )
            })?
            .transcribe(&input.audio_base64, &input.mime_type)
            .await
            .map_err(integration_error)
    }

    #[must_use]
    pub fn pdf_import_configured(&self) -> bool {
        self.pdf.is_some() && self.llm.is_some()
    }

    pub async fn initialize(&self) -> Result<(), ApplicationError> {
        self.repository
            .seed_demo(&now())
            .await
            .map_err(ApplicationError::storage)
    }

    /// Readiness probe: the service is healthy only while its repository is.
    pub async fn healthy(&self) -> Result<(), ApplicationError> {
        self.repository
            .ping()
            .await
            .map_err(ApplicationError::storage)
    }

    #[must_use]
    pub fn grade_cooldown_days(&self) -> u16 {
        self.grade_cooldown_days
    }

    #[must_use]
    pub fn github_configured(&self) -> bool {
        self.github.is_some()
    }

    #[must_use]
    pub fn github_insights_configured(&self) -> bool {
        self.github_insights.is_some()
    }

    /// A stable per-candidate code: placing it in the GitHub bio proves account ownership.
    pub(crate) fn github_ownership_code(&self, candidate_id: &str) -> String {
        let hash = self.credential_hash("github-ownership", "v1", candidate_id);
        format!("fsp-{}", &hash[..10])
    }

    #[must_use]
    pub fn real_email_configured(&self) -> bool {
        self.delivery.contract_name() == "email"
    }

    #[must_use]
    pub fn llm_configured(&self) -> bool {
        self.llm.is_some()
    }

    pub async fn request_challenge(
        &self,
        input: EmailChallengeInput,
    ) -> Result<EmailChallengeView, ApplicationError> {
        let email = input.email.trim().to_lowercase();
        if !valid_email(&input.email) {
            return Err(ApplicationError::new(
                400,
                "validation_error",
                "Проверьте адрес электронной почты",
            ));
        }
        if let Some(account) = self
            .repository
            .account_by_email(&email)
            .await
            .map_err(ApplicationError::storage)?
            && account.role != input.role
        {
            return Err(ApplicationError::new(
                409,
                "role_mismatch",
                "Этот адрес зарегистрирован с другой ролью",
            ));
        }
        let delivery = self.delivery.contract_name_for_email(&email);
        let code = self.delivery.code_for_email(&email).map_err(|error| {
            ApplicationError::new(502, "verification_delivery_failed", error.message)
        })?;
        let challenge_id = format!("challenge-{}", Uuid::new_v4().simple());
        let expires_at = Utc::now() + Duration::minutes(15);
        let challenge = PendingChallenge {
            id: challenge_id.clone(),
            email: email.clone(),
            role: input.role,
            code_hash: self.credential_hash("code", &challenge_id, &code),
            expires_at: expires_at.to_rfc3339(),
            attempts: 0,
            used: false,
        };
        // Accounts are created lazily at verify time (mock parity), so issuing
        // a challenge never squats an email — and `issue_challenge` enforces
        // the cumulative five-attempt lockout across reissued codes.
        let issued = self
            .repository
            .issue_challenge(&challenge, delivery == "email")
            .await
            .map_err(ApplicationError::storage)?;
        match issued {
            crate::IssueChallenge::Issued => {}
            crate::IssueChallenge::Locked => {
                return Err(ApplicationError::new(
                    429,
                    "challenge_locked",
                    "Слишком много неверных попыток. Дождитесь истечения срока кода.",
                ));
            }
            crate::IssueChallenge::RateLimited => {
                return Err(ApplicationError::new(
                    429,
                    "rate_limited",
                    "Подождите минуту перед повторной отправкой. Можно запросить не более пяти писем за 15 минут.",
                ));
            }
        }
        self.delivery
            .deliver(&email, &code, &challenge.id)
            .await
            .map_err(|error| {
                ApplicationError::new(502, "verification_delivery_failed", error.message)
            })?;
        Ok(EmailChallengeView {
            challenge_id: challenge.id,
            expires_at: challenge.expires_at,
            delivery,
        })
    }

    pub async fn verify_email(
        &self,
        input: EmailVerifyInput,
    ) -> Result<(SessionView, String), ApplicationError> {
        if input.code.len() != 6 || !input.code.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(ApplicationError::new(400, "invalid_code", "Неверный код"));
        }
        // Challenge consumption, account resolution/registration and session
        // creation share one transaction: a failure anywhere rolls back the
        // used-mark, so a crashed verify never burns a code.
        let tx = self
            .repository
            .transaction()
            .await
            .map_err(ApplicationError::storage)?;
        let (outcome, challenge) = tx
            .consume_challenge(
                &input.challenge_id,
                &self.credential_hash("code", &input.challenge_id, &input.code),
                &now(),
            )
            .await
            .map_err(ApplicationError::storage)?;
        let challenge = match outcome {
            ConsumeChallenge::Consumed => challenge.ok_or_else(|| {
                ApplicationError::new(500, "internal_error", "Сервис временно недоступен")
            })?,
            ConsumeChallenge::Missing => {
                return Err(ApplicationError::new(
                    404,
                    "not_found",
                    "Запрос подтверждения не найден",
                ));
            }
            ConsumeChallenge::Used => {
                return Err(ApplicationError::new(
                    409,
                    "challenge_used",
                    "Код уже использован",
                ));
            }
            ConsumeChallenge::Expired => {
                return Err(ApplicationError::new(
                    400,
                    "challenge_expired",
                    "Срок действия кода истёк",
                ));
            }
            ConsumeChallenge::Locked => {
                return Err(ApplicationError::new(
                    429,
                    "challenge_locked",
                    "Создайте новый запрос подтверждения",
                ));
            }
            ConsumeChallenge::WrongCode => {
                // A rejected code is still a durable security event. Returning
                // without committing would restore the attempt budget.
                tx.commit().await.map_err(ApplicationError::storage)?;
                return Err(ApplicationError::new(400, "invalid_code", "Неверный код"));
            }
        };
        let mut account = match tx
            .account_by_email(&challenge.email)
            .await
            .map_err(ApplicationError::storage)?
        {
            Some(account) => {
                if account.role != challenge.role {
                    return Err(ApplicationError::new(
                        409,
                        "role_mismatch",
                        "Этот адрес зарегистрирован с другой ролью",
                    ));
                }
                account
            }
            None => {
                self.register_account(&*tx, challenge.email.clone(), challenge.role)
                    .await?
            }
        };
        tx.mark_email_verified(&account.id)
            .await
            .map_err(ApplicationError::storage)?;
        account.email_verified = true;
        let raw_token = Uuid::new_v4().to_string();
        tx.create_session(
            &self.credential_hash("session", "", &raw_token),
            &account.id,
            &(Utc::now() + Duration::days(30)).to_rfc3339(),
        )
        .await
        .map_err(ApplicationError::storage)?;
        tx.commit().await.map_err(ApplicationError::storage)?;
        Ok((self.session_view(&account).await?, raw_token))
    }

    pub async fn session(&self, raw_token: &str) -> Result<SessionView, ApplicationError> {
        let account = self
            .repository
            .account_for_session(&self.credential_hash("session", "", raw_token), &now())
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(401, "unauthenticated", "Войдите в аккаунт"))?;
        self.session_view(&account).await
    }

    pub async fn logout(&self, raw_token: &str) -> Result<(), ApplicationError> {
        self.repository
            .delete_session(&self.credential_hash("session", "", raw_token))
            .await
            .map_err(ApplicationError::storage)
    }

    pub async fn profile(&self, session: &SessionView) -> Result<ProfileView, ApplicationError> {
        let id = session.candidate_id.as_deref().ok_or_else(|| {
            ApplicationError::new(403, "role_forbidden", "Действие доступно кандидату")
        })?;
        let profile = self
            .repository
            .candidate(id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Профиль не найден"))?;
        let signals = self
            .freshness_signals(&profile.id, profile.grade_confirmed_at.as_deref())
            .await?;
        let assessment_running = self.has_active_assessment(&profile.id).await?;
        Ok(profile_view(
            profile,
            session.email_verified,
            &signals,
            assessment_running,
            self.grade_cooldown_days,
        ))
    }

    pub async fn update_profile(
        &self,
        session: &SessionView,
        patch: ProfilePatch,
    ) -> Result<ProfileView, ApplicationError> {
        validate_profile_patch(&patch)?;
        let id = session.candidate_id.as_deref().ok_or_else(|| {
            ApplicationError::new(403, "role_forbidden", "Действие доступно кандидату")
        })?;
        // Optimistic concurrency: re-read and reapply once if a concurrent
        // writer bumped the revision between our read and guarded write.
        let mut attempts = 0;
        let (profile, reestimated) = loop {
            let mut profile = self
                .repository
                .candidate(id)
                .await
                .map_err(ApplicationError::storage)?
                .ok_or_else(|| ApplicationError::new(404, "not_found", "Профиль не найден"))?;
            let stated = (
                profile.stated_experience_months(current_year()),
                profile.stated_team_role().map(str::to_owned),
            );
            apply_profile_patch(&mut profile, patch.clone())?;
            // The estimate's tenure and lead rules depend on the stated experience and role.
            let mut reestimated = false;
            if profile.estimate.is_some()
                && profile.consents.processing
                && (
                    profile.stated_experience_months(current_year()),
                    profile.stated_team_role().map(str::to_owned),
                ) != stated
                && let Some(estimate) = self.answer_estimate(&profile).await?
            {
                profile.apply_estimate(estimate);
                reestimated = true;
            }
            let base_revision = profile.revision;
            profile.revision += 1;
            profile.updated_at = now();
            match self
                .repository
                .save_candidate_guarded(&profile, base_revision)
                .await
            {
                Ok(()) => break (profile, reestimated),
                Err(RepositoryError::NotFound) if attempts == 0 => {
                    attempts += 1;
                }
                Err(error) => return Err(ApplicationError::storage(error)),
            }
        };
        if reestimated {
            self.spawn_estimate_review(&profile.id);
        }
        let signals = self
            .freshness_signals(&profile.id, profile.grade_confirmed_at.as_deref())
            .await?;
        let assessment_running = self.has_active_assessment(&profile.id).await?;
        Ok(profile_view(
            profile,
            session.email_verified,
            &signals,
            assessment_running,
            self.grade_cooldown_days,
        ))
    }

    pub async fn company(&self, session: &SessionView) -> Result<CompanyView, ApplicationError> {
        let id = session.company_id.as_deref().ok_or_else(|| {
            ApplicationError::new(403, "role_forbidden", "Действие доступно работодателю")
        })?;
        self.repository
            .company(id)
            .await
            .map_err(ApplicationError::storage)?
            .map(CompanyView::from)
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Компания не найдена"))
    }

    pub async fn update_company(
        &self,
        session: &SessionView,
        patch: CompanyPatch,
    ) -> Result<CompanyView, ApplicationError> {
        let tx = self
            .repository
            .transaction()
            .await
            .map_err(ApplicationError::storage)?;
        tx.lock_scope(&format!("company:{}", employer_company_id(session)?))
            .await
            .map_err(ApplicationError::storage)?;
        let scoped = Self {
            repository: tx.share(),
            ..self.clone()
        };
        validate_company_patch(&patch)?;
        let id = session.company_id.as_deref().ok_or_else(|| {
            ApplicationError::new(403, "role_forbidden", "Действие доступно работодателю")
        })?;
        let mut company = scoped
            .repository
            .company(id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Компания не найдена"))?;
        if let Some(value) = patch.display_name {
            company.display_name = value.trim().to_owned();
        }
        if let Some(value) = patch.business_area {
            company.business_area = value.trim().to_owned();
        }
        if let Some(value) = patch.description {
            company.description = value.trim().to_owned();
        }
        if let Some(value) = patch.contact_email {
            company.contact_email = value.trim().to_lowercase();
        }
        if let Some(value) = patch.website {
            company.website = value;
        }
        scoped
            .repository
            .save_company(&company)
            .await
            .map_err(ApplicationError::storage)?;
        tx.commit().await.map_err(ApplicationError::storage)?;
        Ok(company.into())
    }

    pub async fn needs(&self, session: &SessionView) -> Result<NeedsView, ApplicationError> {
        let company_id = employer_company_id(session)?;
        let items = self
            .repository
            .needs(company_id)
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .map(need_view)
            .collect();
        Ok(NeedsView { items })
    }

    /// The mock's `needFor`: employer needs are scoped to the caller's company,
    /// an out-of-scope id is indistinguishable from missing.
    async fn own_need(
        &self,
        session: &SessionView,
        need_id: &str,
    ) -> Result<HiringNeedRecord, ApplicationError> {
        let company_id = employer_company_id(session)?;
        self.repository
            .need(company_id, need_id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Ресурс не найден"))
    }

    pub async fn create_need(
        &self,
        session: &SessionView,
        input: NeedInput,
    ) -> Result<NeedView, ApplicationError> {
        let company_id = employer_company_id(session)?.to_owned();
        let requirements: Vec<fsp_domain::Requirement> = input
            .requirements
            .into_iter()
            .map(RequirementInput::into_domain)
            .collect();
        validate_need_schema(
            &input.title,
            &input.work_mode,
            &input.description,
            &requirements,
        )?;
        validate_need_rules(input.salary.from, input.salary.to, &requirements)?;
        let timestamp = now();
        let record = HiringNeedRecord {
            id: format!("need-{}", Uuid::new_v4().simple()),
            company_id,
            title: input.title,
            need: fsp_domain::HiringNeed {
                specialization: input.specialization,
                grade: input.grade,
                requirements,
            },
            work_mode: input.work_mode,
            salary_from: input.salary.from,
            salary_to: input.salary.to,
            description: input.description,
            published: false,
            published_at: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            revision: 1,
        };
        self.repository
            .save_need(&record)
            .await
            .map_err(ApplicationError::storage)?;
        Ok(need_view(record))
    }

    pub async fn get_need(
        &self,
        session: &SessionView,
        need_id: &str,
    ) -> Result<NeedView, ApplicationError> {
        self.own_need(session, need_id).await.map(need_view)
    }

    pub async fn update_need(
        &self,
        session: &SessionView,
        need_id: &str,
        patch: NeedPatch,
    ) -> Result<NeedView, ApplicationError> {
        let mut record = self.own_need(session, need_id).await?;
        if patch.is_empty() {
            return Err(validation("Запрос не соответствует контракту"));
        }
        if let Some(title) = &patch.title
            && !(1..=160).contains(&title.chars().count())
        {
            return Err(validation("Запрос не соответствует контракту"));
        }
        if let Some(description) = &patch.description
            && description.chars().count() > 6000
        {
            return Err(validation("Запрос не соответствует контракту"));
        }
        if let Some(work_mode) = &patch.work_mode
            && !WORK_MODES.contains(&work_mode.as_str())
        {
            return Err(validation("Запрос не соответствует контракту"));
        }
        if let Some(salary) = &patch.salary
            && (salary.from == 0
                || salary.to == 0
                || salary.from > 100_000_000
                || salary.to > 100_000_000)
        {
            return Err(validation("Запрос не соответствует контракту"));
        }
        if let Some(requirements) = &patch.requirements {
            if requirements.is_empty() || requirements.len() > 5 {
                return Err(validation("Запрос не соответствует контракту"));
            }
            let domain: Vec<fsp_domain::Requirement> = requirements
                .iter()
                .cloned()
                .map(RequirementInput::into_domain)
                .collect();
            validate_requirements_schema(&domain)?;
        }
        let (salary_from, salary_to) = patch
            .salary
            .map(|salary| (salary.from, salary.to))
            .unwrap_or((record.salary_from, record.salary_to));
        let requirements = patch
            .requirements
            .map(|list| {
                list.into_iter()
                    .map(RequirementInput::into_domain)
                    .collect::<Vec<_>>()
            })
            .unwrap_or(record.need.requirements);
        validate_need_rules(salary_from, salary_to, &requirements)?;
        if let Some(title) = patch.title {
            record.title = title;
        }
        if let Some(specialization) = patch.specialization {
            record.need.specialization = specialization;
        }
        if let Some(grade) = patch.grade {
            record.need.grade = grade;
        }
        if let Some(work_mode) = patch.work_mode {
            record.work_mode = work_mode;
        }
        if let Some(description) = patch.description {
            record.description = description;
        }
        record.salary_from = salary_from;
        record.salary_to = salary_to;
        record.need.requirements = requirements;
        let base_revision = record.revision;
        record.revision += 1;
        record.updated_at = now();
        self.repository
            .save_need_guarded(&record, base_revision)
            .await
            .map_err(|error| match error {
                RepositoryError::NotFound => {
                    ApplicationError::new(404, "not_found", "Потребность не найдена")
                }
                other => ApplicationError::storage(other),
            })?;
        Ok(need_view(record))
    }

    pub async fn delete_need(
        &self,
        session: &SessionView,
        need_id: &str,
    ) -> Result<(), ApplicationError> {
        let record = self.own_need(session, need_id).await?;
        self.repository
            .delete_need(&record.company_id, &record.id)
            .await
            .map_err(ApplicationError::storage)
    }

    pub async fn publish_need(
        &self,
        session: &SessionView,
        need_id: &str,
    ) -> Result<NeedView, ApplicationError> {
        let mut record = self.own_need(session, need_id).await?;
        if record.published {
            return Ok(need_view(record));
        }
        let company = self
            .repository
            .company(&record.company_id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| {
                ApplicationError::new(500, "internal_error", "Сервис временно недоступен")
            })?;
        if !company.ready_to_invite() {
            return Err(ApplicationError::new(
                422,
                "company_incomplete",
                "Заполните описание компании со сферой деятельности и контактную почту",
            ));
        }
        if !session.email_verified {
            return Err(ApplicationError::new(
                403,
                "email_not_verified",
                "Подтвердите почту работодателя",
            ));
        }
        validate_need_rules(
            record.salary_from,
            record.salary_to,
            &record.need.requirements,
        )?;
        record.published = true;
        record.published_at = Some(now());
        self.repository
            .save_need_guarded(&record, record.revision)
            .await
            .map_err(|error| match error {
                RepositoryError::NotFound => {
                    ApplicationError::new(404, "not_found", "Потребность не найдена")
                }
                other => ApplicationError::storage(other),
            })?;
        Ok(need_view(record))
    }

    pub async fn unpublish_need(
        &self,
        session: &SessionView,
        need_id: &str,
    ) -> Result<NeedView, ApplicationError> {
        let mut record = self.own_need(session, need_id).await?;
        record.published = false;
        record.published_at = None;
        self.repository
            .save_need_guarded(&record, record.revision)
            .await
            .map_err(|error| match error {
                RepositoryError::NotFound => {
                    ApplicationError::new(404, "not_found", "Потребность не найдена")
                }
                other => ApplicationError::storage(other),
            })?;
        Ok(need_view(record))
    }

    pub async fn need_graph(
        &self,
        session: &SessionView,
        need_id: &str,
    ) -> Result<GraphView, ApplicationError> {
        let record = self.own_need(session, need_id).await?;
        let mut nodes = vec![GraphNodeView {
            id: "need-root".into(),
            kind: "role".into(),
            label: record.title,
            skill_id: None,
            status: None,
            detail: None,
        }];
        let mut edges = Vec::new();
        for requirement in &record.need.requirements {
            let id = format!("skill-{}", requirement.skill_id);
            nodes.push(GraphNodeView {
                id: id.clone(),
                kind: "skill".into(),
                label: skill_label(&requirement.skill_id).into(),
                skill_id: Some(requirement.skill_id.clone()),
                status: None,
                detail: None,
            });
            edges.push(GraphEdgeView {
                id: format!("requires-{}", requirement.skill_id),
                from: "need-root".into(),
                to: id,
                relation: "requires".into(),
            });
        }
        Ok(GraphView {
            audience: "employer",
            nodes,
            edges,
            simulation: true,
        })
    }

    /// The mock's `applicationEligibility`: onboarding gates first, an existing
    /// active application last.
    async fn application_eligibility(
        &self,
        record: &CandidateIndexRecord,
        need_id: &str,
    ) -> Result<ApplicationEligibilityView, ApplicationError> {
        let profile = &record.profile;
        let reason = if !record.email_verified {
            Some("email")
        } else if !profile.questionnaire_complete() {
            Some("questionnaire")
        } else if profile.category().is_none() {
            Some("category")
        } else if !profile.consents.processing {
            Some("processing_consent")
        } else if self
            .active_application(&profile.id, need_id)
            .await?
            .is_some()
        {
            Some("application_exists")
        } else {
            None
        };
        Ok(ApplicationEligibilityView {
            can_apply: reason.is_none(),
            reason,
        })
    }

    /// The mock's `activeApplication`: the newest non-withdrawn application
    /// of this candidate to the need behind the vacancy.
    async fn active_application(
        &self,
        candidate_id: &str,
        need_id: &str,
    ) -> Result<Option<String>, ApplicationError> {
        Ok(self
            .repository
            .candidate_applications(candidate_id)
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .filter(|item| {
                item.vacancy.id == need_id && item.status != InteractionStatus::Withdrawn
            })
            .max_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)))
            .map(|item| item.id))
    }

    async fn vacancy_view(
        &self,
        need: HiringNeedRecord,
        candidate: &CandidateIndexRecord,
    ) -> Result<VacancyView, ApplicationError> {
        let company = self
            .repository
            .company(&need.company_id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| {
                ApplicationError::new(500, "internal_error", "Сервис временно недоступен")
            })?;
        let qualified =
            candidate.profile.category().is_some() && candidate.profile.consents.processing;
        let company_card = CompanyCardView {
            id: company.id,
            display_name: company.display_name,
            business_area: company.business_area,
            description: company.description,
            contact_email: company.contact_email,
        };
        let eligibility = self.application_eligibility(candidate, &need.id).await?;
        let application_id = self
            .active_application(&candidate.profile.id, &need.id)
            .await?;
        let r#match = candidate
            .rankable()
            .filter(|_| qualified)
            .map(|rankable| match_view(&rankable, &need));
        Ok(VacancyView {
            id: need.id.clone(),
            company: company_card,
            title: need.title,
            specialization: need.need.specialization,
            grade: need.need.grade,
            work_mode: need.work_mode,
            salary: SalaryView {
                from: need.salary_from,
                to: need.salary_to,
                currency: "RUB",
                period: "month",
                tax_basis: "gross",
            },
            description: need.description,
            requirements: need
                .need
                .requirements
                .into_iter()
                .map(|item| VacancyRequirementView {
                    skill_id: item.skill_id,
                    criticality: item.criticality,
                    minimum_grade: item.minimum_grade,
                })
                .collect(),
            published_at: need.published_at.unwrap_or_default(),
            updated_at: need.updated_at,
            r#match,
            application_id,
            eligibility,
        })
    }

    pub async fn search_vacancies(
        &self,
        session: &SessionView,
        search: &VacancySearch,
    ) -> Result<VacancyPageView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        let candidate = self.candidate_index(candidate_id).await?;
        let mut needs = self
            .repository
            .published_needs()
            .await
            .map_err(ApplicationError::storage)?;
        let rankable = candidate
            .rankable()
            .filter(|_| candidate.profile.consents.processing);
        // Score once per vacancy; filtering/pagination never changes the deterministic order.
        let scores: std::collections::HashMap<_, _> = rankable
            .as_ref()
            .map(|candidate| {
                needs
                    .iter()
                    .map(|need| (need.id.clone(), score(candidate, &need.need)))
                    .collect()
            })
            .unwrap_or_default();
        needs.sort_by(|left, right| {
            scores
                .get(&left.id)
                .zip(scores.get(&right.id))
                .map_or(std::cmp::Ordering::Equal, |(left, right)| {
                    fsp_domain::compare_vacancy_fit(left, right)
                })
                .then_with(|| right.published_at.cmp(&left.published_at))
                .then_with(|| left.id.cmp(&right.id))
        });
        let filtered = needs
            .into_iter()
            .filter(|need| {
                search
                    .specialization
                    .is_none_or(|value| need.need.specialization == value)
                    && search.grade.is_none_or(|value| need.need.grade == value)
                    && search
                        .work_mode
                        .as_deref()
                        .is_none_or(|value| need.work_mode == value)
                    && search.skill.as_deref().is_none_or(|skill| {
                        need.need
                            .requirements
                            .iter()
                            .any(|item| item.skill_id == skill)
                    })
            })
            .collect::<Vec<_>>();
        let total = filtered.len();
        let mut items = Vec::new();
        for need in filtered
            .into_iter()
            .skip((search.page - 1).saturating_mul(search.page_size))
            .take(search.page_size)
        {
            items.push(self.vacancy_view(need, &candidate).await?);
        }
        Ok(VacancyPageView {
            items,
            page: search.page,
            page_size: search.page_size,
            total,
            has_next_page: search.page * search.page_size < total,
        })
    }

    pub async fn get_vacancy(
        &self,
        session: &SessionView,
        vacancy_id: &str,
    ) -> Result<VacancyView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        let candidate = self.candidate_index(candidate_id).await?;
        let need = self
            .repository
            .published_needs()
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .find(|need| need.id == vacancy_id)
            .ok_or_else(|| {
                ApplicationError::new(
                    404,
                    "not_found",
                    "Вакансия не найдена или снята с публикации",
                )
            })?;
        self.vacancy_view(need, &candidate).await
    }

    /// The mock's `shortlistFor`: shortlists are scoped to the caller's
    /// company, an out-of-scope id is indistinguishable from missing.
    async fn own_shortlist(
        &self,
        session: &SessionView,
        shortlist_id: &str,
    ) -> Result<fsp_domain::ShortlistRecord, ApplicationError> {
        let company_id = employer_company_id(session)?;
        self.repository
            .shortlist(company_id, shortlist_id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Ресурс не найден"))
    }

    /// Candidate visibility for shortlist membership: alias falls back to the
    /// mock's placeholder and `available` reflects the public-bank rules.
    async fn shortlist_members(
        &self,
        record: &fsp_domain::ShortlistRecord,
    ) -> Result<Vec<ShortlistMemberView>, ApplicationError> {
        let candidates = self
            .repository
            .candidates()
            .await
            .map_err(ApplicationError::storage)?;
        Ok(record
            .member_ids
            .iter()
            .map(|member_id| {
                let candidate = candidates.iter().find(|item| item.profile.id == *member_id);
                ShortlistMemberView {
                    candidate_id: member_id.clone(),
                    alias: candidate
                        .map(|item| item.profile.alias.clone())
                        .unwrap_or_else(|| "Недоступный профиль".to_owned()),
                    available: candidate
                        .is_some_and(|item| item.profile.visible_to_employers(item.email_verified)),
                }
            })
            .collect())
    }

    async fn shortlist_detail(
        &self,
        record: fsp_domain::ShortlistRecord,
    ) -> Result<ShortlistView, ApplicationError> {
        let members = self.shortlist_members(&record).await?;
        Ok(ShortlistView {
            id: record.id,
            title: record.title,
            count: members.len(),
            created_at: record.created_at,
            updated_at: record.updated_at,
            members,
        })
    }

    fn validate_shortlist_title(title: &str) -> Result<(), ApplicationError> {
        if (1..=120).contains(&title.chars().count()) {
            Ok(())
        } else {
            Err(validation("Запрос не соответствует контракту"))
        }
    }

    pub async fn shortlists(
        &self,
        session: &SessionView,
    ) -> Result<ShortlistsView, ApplicationError> {
        let company_id = employer_company_id(session)?;
        let items = self
            .repository
            .shortlists(company_id)
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .map(|record| ShortlistSummaryView {
                id: record.id,
                title: record.title,
                count: record.member_ids.len(),
                created_at: record.created_at,
                updated_at: record.updated_at,
            })
            .collect();
        Ok(ShortlistsView { items })
    }

    pub async fn create_shortlist(
        &self,
        session: &SessionView,
        input: ShortlistInput,
    ) -> Result<ShortlistView, ApplicationError> {
        Self::validate_shortlist_title(&input.title)?;
        let company_id = employer_company_id(session)?.to_owned();
        let timestamp = now();
        let record = fsp_domain::ShortlistRecord {
            id: format!("shortlist-{}", Uuid::new_v4().simple()),
            company_id,
            title: input.title,
            member_ids: Vec::new(),
            created_at: timestamp.clone(),
            updated_at: timestamp,
        };
        self.repository
            .save_shortlist(&record)
            .await
            .map_err(ApplicationError::storage)?;
        self.shortlist_detail(record).await
    }

    pub async fn get_shortlist(
        &self,
        session: &SessionView,
        shortlist_id: &str,
    ) -> Result<ShortlistView, ApplicationError> {
        let record = self.own_shortlist(session, shortlist_id).await?;
        self.shortlist_detail(record).await
    }

    pub async fn update_shortlist(
        &self,
        session: &SessionView,
        shortlist_id: &str,
        input: ShortlistInput,
    ) -> Result<ShortlistView, ApplicationError> {
        let tx = self
            .repository
            .transaction()
            .await
            .map_err(ApplicationError::storage)?;
        tx.lock_scope(&format!(
            "shortlist:{}:{shortlist_id}",
            employer_company_id(session)?
        ))
        .await
        .map_err(ApplicationError::storage)?;
        let scoped = Self {
            repository: tx.share(),
            ..self.clone()
        };
        Self::validate_shortlist_title(&input.title)?;
        let mut record = scoped.own_shortlist(session, shortlist_id).await?;
        record.title = input.title;
        record.updated_at = now();
        scoped
            .repository
            .save_shortlist(&record)
            .await
            .map_err(ApplicationError::storage)?;
        tx.commit().await.map_err(ApplicationError::storage)?;
        self.shortlist_detail(record).await
    }

    pub async fn delete_shortlist(
        &self,
        session: &SessionView,
        shortlist_id: &str,
    ) -> Result<(), ApplicationError> {
        let tx = self
            .repository
            .transaction()
            .await
            .map_err(ApplicationError::storage)?;
        tx.lock_scope(&format!(
            "shortlist:{}:{shortlist_id}",
            employer_company_id(session)?
        ))
        .await
        .map_err(ApplicationError::storage)?;
        let scoped = Self {
            repository: tx.share(),
            ..self.clone()
        };
        let record = scoped.own_shortlist(session, shortlist_id).await?;
        scoped
            .repository
            .delete_shortlist(&record.company_id, &record.id)
            .await
            .map_err(ApplicationError::storage)?;
        tx.commit().await.map_err(ApplicationError::storage)
    }

    pub async fn add_shortlist_member(
        &self,
        session: &SessionView,
        shortlist_id: &str,
        candidate_id: &str,
    ) -> Result<ShortlistView, ApplicationError> {
        let tx = self
            .repository
            .transaction()
            .await
            .map_err(ApplicationError::storage)?;
        tx.lock_scope(&format!(
            "shortlist:{}:{shortlist_id}",
            employer_company_id(session)?
        ))
        .await
        .map_err(ApplicationError::storage)?;
        let scoped = Self {
            repository: tx.share(),
            ..self.clone()
        };
        let mut record = scoped.own_shortlist(session, shortlist_id).await?;
        // The mock's `visibleCandidate`: only candidates visible to employers
        // can be shortlisted.
        let visible = scoped
            .repository
            .candidate_index(candidate_id)
            .await
            .map_err(ApplicationError::storage)?
            .is_some_and(|item| item.profile.visible_to_employers(item.email_verified));
        if !visible {
            return Err(ApplicationError::new(404, "not_found", "Ресурс не найден"));
        }
        if !record.member_ids.iter().any(|id| id == candidate_id) {
            record.member_ids.push(candidate_id.to_owned());
        }
        record.updated_at = now();
        scoped
            .repository
            .save_shortlist(&record)
            .await
            .map_err(ApplicationError::storage)?;
        tx.commit().await.map_err(ApplicationError::storage)?;
        self.shortlist_detail(record).await
    }

    pub async fn remove_shortlist_member(
        &self,
        session: &SessionView,
        shortlist_id: &str,
        candidate_id: &str,
    ) -> Result<ShortlistView, ApplicationError> {
        let tx = self
            .repository
            .transaction()
            .await
            .map_err(ApplicationError::storage)?;
        tx.lock_scope(&format!(
            "shortlist:{}:{shortlist_id}",
            employer_company_id(session)?
        ))
        .await
        .map_err(ApplicationError::storage)?;
        let scoped = Self {
            repository: tx.share(),
            ..self.clone()
        };
        let mut record = scoped.own_shortlist(session, shortlist_id).await?;
        record.member_ids.retain(|id| id != candidate_id);
        record.updated_at = now();
        scoped
            .repository
            .save_shortlist(&record)
            .await
            .map_err(ApplicationError::storage)?;
        tx.commit().await.map_err(ApplicationError::storage)?;
        self.shortlist_detail(record).await
    }

    /// The mock's `invitationFor`: candidates reach their own invitations,
    /// employers reach their company's, everything else is missing.
    async fn invitation_for(
        &self,
        session: &SessionView,
        invitation_id: &str,
    ) -> Result<fsp_domain::InvitationRecord, ApplicationError> {
        let record = self
            .repository
            .invitation(invitation_id)
            .await
            .map_err(ApplicationError::storage)?;
        match record {
            Some(item)
                if match session.role {
                    Role::Candidate => {
                        session.candidate_id.as_deref() == Some(item.candidate_id.as_str())
                    }
                    Role::Employer => {
                        session.company_id.as_deref() == Some(item.company_id.as_str())
                    }
                } =>
            {
                Ok(item)
            }
            _ => Err(ApplicationError::new(404, "not_found", "Ресурс не найден")),
        }
    }

    /// The mock's `visibleCandidate`: a candidate reachable by employers.
    async fn visible_candidate(
        &self,
        candidate_id: &str,
    ) -> Result<CandidateIndexRecord, ApplicationError> {
        self.repository
            .candidate_index(candidate_id)
            .await
            .map_err(ApplicationError::storage)?
            .filter(|item| item.profile.visible_to_employers(item.email_verified))
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Ресурс не найден"))
    }

    async fn employer_company(&self, session: &SessionView) -> Result<Company, ApplicationError> {
        let company_id = employer_company_id(session)?;
        self.repository
            .company(company_id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Ресурс не найден"))
    }

    /// The shared publication preconditions: a complete company card and a
    /// verified employer e-mail, in the mock's check order.
    async fn require_invitable_employer(
        &self,
        session: &SessionView,
    ) -> Result<Company, ApplicationError> {
        let company = self.employer_company(session).await?;
        if !company.ready_to_invite() {
            return Err(ApplicationError::new(
                422,
                "company_incomplete",
                "Заполните описание компании со сферой деятельности и контактную почту",
            ));
        }
        if !session.email_verified {
            return Err(ApplicationError::new(
                403,
                "email_not_verified",
                "Подтвердите почту работодателя",
            ));
        }
        Ok(company)
    }

    pub async fn invitations(
        &self,
        session: &SessionView,
    ) -> Result<InvitationsView, ApplicationError> {
        let company_id = employer_company_id(session)?;
        let items = self
            .repository
            .invitations(company_id)
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .map(invitation_view)
            .collect();
        Ok(InvitationsView { items })
    }

    pub async fn get_invitation(
        &self,
        session: &SessionView,
        invitation_id: &str,
    ) -> Result<InvitationView, ApplicationError> {
        self.invitation_for(session, invitation_id)
            .await
            .map(invitation_view)
    }

    pub async fn withdraw_invitation(
        &self,
        session: &SessionView,
        invitation_id: &str,
    ) -> Result<InvitationView, ApplicationError> {
        let mut record = self.invitation_for(session, invitation_id).await?;
        if record.status == InteractionStatus::Withdrawn {
            return Ok(invitation_view(record));
        }
        if record.status.terminal() {
            return Err(ApplicationError::new(
                409,
                "invitation_terminal",
                "Решение по приглашению уже принято",
            ));
        }
        record.status = InteractionStatus::Withdrawn;
        record.responded_at = Some(now());
        self.repository
            .save_invitation_guarded(
                &record,
                &[InteractionStatus::Sent, InteractionStatus::Viewed],
            )
            .await
            .map_err(|error| match error {
                RepositoryError::NotFound => ApplicationError::new(
                    409,
                    "invitation_terminal",
                    "Решение по приглашению уже принято",
                ),
                other => ApplicationError::storage(other),
            })?;
        Ok(invitation_view(record))
    }

    pub async fn my_invitations(
        &self,
        session: &SessionView,
    ) -> Result<InvitationsView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        let items = self
            .repository
            .candidate_invitations(candidate_id)
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .map(invitation_view)
            .collect();
        Ok(InvitationsView { items })
    }

    pub async fn view_my_invitation(
        &self,
        session: &SessionView,
        invitation_id: &str,
    ) -> Result<InvitationView, ApplicationError> {
        let mut record = self.invitation_for(session, invitation_id).await?;
        if record.status == InteractionStatus::Sent {
            record.status = InteractionStatus::Viewed;
            record.viewed_at = Some(now());
            match self
                .repository
                .save_invitation_guarded(&record, &[InteractionStatus::Sent])
                .await
            {
                Ok(()) => {}
                Err(RepositoryError::NotFound) => {
                    record = self.invitation_for(session, invitation_id).await?;
                }
                Err(error) => return Err(ApplicationError::storage(error)),
            }
        }
        Ok(invitation_view(record))
    }

    pub async fn accept_my_invitation(
        &self,
        session: &SessionView,
        invitation_id: &str,
        input: AcceptInvitationInput,
    ) -> Result<InvitationView, ApplicationError> {
        let mut disclose = input.disclose;
        validate_disclose(&disclose)?;
        disclose.sort();
        let mut record = self.invitation_for(session, invitation_id).await?;
        if record.status == InteractionStatus::Accepted && record.disclosure_fields == disclose {
            return Ok(invitation_view(record));
        }
        if record.status.terminal() {
            return Err(ApplicationError::new(
                409,
                "invitation_terminal",
                "Решение по приглашению уже принято",
            ));
        }
        let candidate = self.candidate_index(candidate_id(session)?).await?;
        if !candidate.email_verified {
            return Err(ApplicationError::new(
                403,
                "email_not_verified",
                "Подтвердите почту",
            ));
        }
        record.status = InteractionStatus::Accepted;
        record.responded_at = Some(now());
        record.disclosure_fields = disclose;
        // The status flip and the disclosure recompute commit together: a
        // half-accepted invitation with a stale grant is not observable.
        let tx = self
            .repository
            .transaction()
            .await
            .map_err(ApplicationError::storage)?;
        tx.save_invitation_guarded(
            &record,
            &[InteractionStatus::Sent, InteractionStatus::Viewed],
        )
        .await
        .map_err(|error| match error {
            RepositoryError::NotFound => ApplicationError::new(
                409,
                "invitation_terminal",
                "Решение по приглашению уже принято",
            ),
            other => ApplicationError::storage(other),
        })?;
        self.refresh_disclosure(&*tx, &record.company_id.clone(), &candidate)
            .await?;
        tx.commit().await.map_err(ApplicationError::storage)?;
        Ok(invitation_view(record))
    }

    pub async fn decline_my_invitation(
        &self,
        session: &SessionView,
        invitation_id: &str,
    ) -> Result<InvitationView, ApplicationError> {
        let mut record = self.invitation_for(session, invitation_id).await?;
        if record.status == InteractionStatus::Declined {
            return Ok(invitation_view(record));
        }
        if record.status.terminal() {
            return Err(ApplicationError::new(
                409,
                "invitation_terminal",
                "Решение по приглашению уже принято",
            ));
        }
        record.status = InteractionStatus::Declined;
        record.responded_at = Some(now());
        self.repository
            .save_invitation_guarded(
                &record,
                &[InteractionStatus::Sent, InteractionStatus::Viewed],
            )
            .await
            .map_err(|error| match error {
                RepositoryError::NotFound => ApplicationError::new(
                    409,
                    "invitation_terminal",
                    "Решение по приглашению уже принято",
                ),
                other => ApplicationError::storage(other),
            })?;
        Ok(invitation_view(record))
    }

    /// The mock's `refreshDisclosure`: recomputes a company's disclosure grant
    /// from accepted invitations and non-withdrawn applications. Removing the
    /// last grant deletes the record. Runs on the caller's repository handle so
    /// the read-modify-write shares the surrounding transaction.
    async fn refresh_disclosure(
        &self,
        repo: &dyn PlatformRepository,
        company_id: &str,
        candidate: &CandidateIndexRecord,
    ) -> Result<(), ApplicationError> {
        repo.lock_scope(&format!("disclosure:{company_id}:{}", candidate.profile.id))
            .await
            .map_err(ApplicationError::storage)?;
        let invitations = repo
            .invitations(company_id)
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .filter(|item| {
                item.candidate_id == candidate.profile.id
                    && item.status == InteractionStatus::Accepted
            })
            .collect::<Vec<_>>();
        let applications = repo
            .company_applications(company_id)
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .filter(|item| {
                item.candidate_id == candidate.profile.id
                    && item.status != InteractionStatus::Withdrawn
            })
            .collect::<Vec<_>>();
        let granted = invitations
            .iter()
            .flat_map(|item| item.disclosure_fields.iter())
            .chain(
                applications
                    .iter()
                    .flat_map(|item| item.disclosure_fields.iter()),
            )
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        if granted.is_empty() {
            return repo
                .delete_disclosure(company_id, &candidate.profile.id)
                .await
                .map_err(ApplicationError::storage);
        }
        let invitation = invitations.iter().max_by(|a, b| {
            (a.responded_at.as_deref().unwrap_or(&a.created_at))
                .cmp(b.responded_at.as_deref().unwrap_or(&b.created_at))
        });
        let application = applications
            .iter()
            .max_by(|a, b| a.created_at.cmp(&b.created_at));
        let source_links = if granted.contains("sourceLinks") {
            let mut links = Vec::new();
            for source in repo
                .sources(&candidate.profile.id)
                .await
                .map_err(ApplicationError::storage)?
            {
                if let Some(analysis) = source.analysis {
                    for repository in analysis.repositories {
                        if !links.contains(&repository.url) {
                            links.push(repository.url);
                        }
                    }
                }
            }
            links
        } else {
            Vec::new()
        };
        let granted_at = [
            invitation.and_then(|item| item.responded_at.as_deref()),
            application.map(|item| item.created_at.as_str()),
        ]
        .into_iter()
        .flatten()
        .max();
        let record = fsp_domain::DisclosureRecord {
            candidate_id: candidate.profile.id.clone(),
            company_id: company_id.to_owned(),
            invitation_id: invitation.map(|item| item.id.clone()),
            application_id: application.map(|item| item.id.clone()),
            granted_at: granted_at.map(str::to_owned),
            name: granted
                .contains("name")
                .then(|| candidate.profile.name.clone()),
            email: granted
                .contains("email")
                .then(|| candidate.profile.email.clone()),
            source_links,
        };
        repo.save_disclosure(&record)
            .await
            .map_err(ApplicationError::storage)
    }

    /// The mock's `getCandidateDisclosure`: the grant row or the explicit
    /// not-granted shell when the company may see the candidate at all.
    pub async fn candidate_disclosure(
        &self,
        session: &SessionView,
        requested_candidate_id: &str,
    ) -> Result<DisclosureView, ApplicationError> {
        let company_id = employer_company_id(session)?;
        if let Some(grant) = self
            .repository
            .disclosure(company_id, requested_candidate_id)
            .await
            .map_err(ApplicationError::storage)?
        {
            return Ok(DisclosureView {
                candidate_id: grant.candidate_id,
                company_id: grant.company_id,
                granted: true,
                invitation_id: grant.invitation_id,
                application_id: grant.application_id,
                granted_at: grant.granted_at,
                fields: DisclosureFieldsView {
                    name: grant.name,
                    email: grant.email,
                    source_links: (!grant.source_links.is_empty()).then_some(grant.source_links),
                },
            });
        }
        // Without a grant the candidate must still be reachable: the mock's
        // `candidateForCompany` accepts a live application or public-bank
        // visibility, and always requires the candidate to exist.
        self.candidate_for_company(company_id, requested_candidate_id)
            .await?;
        Ok(DisclosureView {
            candidate_id: requested_candidate_id.to_owned(),
            company_id: company_id.to_owned(),
            granted: false,
            invitation_id: None,
            application_id: None,
            granted_at: None,
            fields: DisclosureFieldsView::default(),
        })
    }

    /// The mock's `applicationFor`: candidates reach their own applications,
    /// employers reach their company's, everything else is missing.
    async fn application_for(
        &self,
        session: &SessionView,
        application_id: &str,
    ) -> Result<fsp_domain::ApplicationRecord, ApplicationError> {
        let record = self
            .repository
            .application(application_id)
            .await
            .map_err(ApplicationError::storage)?;
        match record {
            Some(item)
                if match session.role {
                    Role::Candidate => {
                        session.candidate_id.as_deref() == Some(item.candidate_id.as_str())
                    }
                    Role::Employer => {
                        session.company_id.as_deref() == Some(item.company_id.as_str())
                    }
                } =>
            {
                Ok(item)
            }
            _ => Err(ApplicationError::new(404, "not_found", "Ресурс не найден")),
        }
    }

    /// The DTO includes a live match of the current candidate state against
    /// the need frozen at submission.
    async fn application_view(
        &self,
        record: fsp_domain::ApplicationRecord,
    ) -> Result<ApplicationView, ApplicationError> {
        let candidate = self
            .repository
            .candidate_index(&record.candidate_id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| {
                ApplicationError::new(500, "internal_error", "Сервис временно недоступен")
            })?;
        let rankable = candidate.rankable().ok_or_else(|| {
            ApplicationError::new(500, "internal_error", "Сервис временно недоступен")
        })?;
        Ok(ApplicationView {
            id: record.id,
            candidate_id: record.candidate_id,
            candidate_alias: record.candidate_alias,
            company: CompanyCardView {
                id: record.company.id,
                display_name: record.company.display_name,
                business_area: record.company.business_area,
                description: record.company.description,
                contact_email: record.company.contact_email,
            },
            vacancy: ApplicationVacancyView {
                id: record.vacancy.id,
                title: record.vacancy.title,
                specialization: record.vacancy.specialization,
                grade: record.vacancy.grade,
                work_mode: record.vacancy.work_mode,
                salary: SalaryView {
                    from: record.vacancy.salary_from,
                    to: record.vacancy.salary_to,
                    currency: "RUB",
                    period: "month",
                    tax_basis: "gross",
                },
                description: record.vacancy.description,
            },
            message: record.message,
            status: record.status,
            created_at: record.created_at,
            viewed_at: record.viewed_at,
            responded_at: record.responded_at,
            disclosure_fields: record.disclosure_fields,
            r#match: match_view(&rankable, &record.need_definition),
        })
    }

    /// The mock's `newestFirst`: newest `createdAt` first, id descending as
    /// the same-instant tie-breaker.
    fn newest_first(items: &mut [fsp_domain::ApplicationRecord]) {
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
    }

    /// The mock's `publishedNeed`.
    async fn published_need(&self, vacancy_id: &str) -> Result<HiringNeedRecord, ApplicationError> {
        self.repository
            .published_needs()
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .find(|need| need.id == vacancy_id)
            .ok_or_else(|| {
                ApplicationError::new(
                    404,
                    "not_found",
                    "Вакансия не найдена или снята с публикации",
                )
            })
    }

    pub async fn my_applications(
        &self,
        session: &SessionView,
    ) -> Result<ApplicationsView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        let mut records = self
            .repository
            .candidate_applications(candidate_id)
            .await
            .map_err(ApplicationError::storage)?;
        Self::newest_first(&mut records);
        let mut items = Vec::with_capacity(records.len());
        for record in records {
            items.push(self.application_view(record).await?);
        }
        Ok(ApplicationsView { items })
    }

    pub async fn get_my_application(
        &self,
        session: &SessionView,
        application_id: &str,
    ) -> Result<ApplicationView, ApplicationError> {
        let record = self.application_for(session, application_id).await?;
        self.application_view(record).await
    }

    pub async fn withdraw_my_application(
        &self,
        session: &SessionView,
        application_id: &str,
    ) -> Result<ApplicationView, ApplicationError> {
        let mut record = self.application_for(session, application_id).await?;
        if record.status == InteractionStatus::Withdrawn {
            return self.application_view(record).await;
        }
        if record.status.terminal() {
            return Err(ApplicationError::new(
                409,
                "application_terminal",
                "Решение по отклику уже принято",
            ));
        }
        record.status = InteractionStatus::Withdrawn;
        record.responded_at = Some(now());
        let candidate = self.candidate_index(&record.candidate_id.clone()).await?;
        let tx = self
            .repository
            .transaction()
            .await
            .map_err(ApplicationError::storage)?;
        tx.save_application_guarded(
            &record,
            &[InteractionStatus::Sent, InteractionStatus::Viewed],
        )
        .await
        .map_err(|error| match error {
            RepositoryError::NotFound => ApplicationError::new(
                409,
                "application_terminal",
                "Решение по отклику уже принято",
            ),
            other => ApplicationError::storage(other),
        })?;
        self.refresh_disclosure(&*tx, &record.company_id.clone(), &candidate)
            .await?;
        tx.commit().await.map_err(ApplicationError::storage)?;
        self.application_view(record).await
    }

    pub async fn company_applications(
        &self,
        session: &SessionView,
        need_id: Option<&str>,
    ) -> Result<ApplicationsView, ApplicationError> {
        let company_id = employer_company_id(session)?;
        let mut records = self
            .repository
            .company_applications(company_id)
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .filter(|item| need_id.is_none_or(|id| item.vacancy.id == id))
            .collect::<Vec<_>>();
        Self::newest_first(&mut records);
        let mut items = Vec::with_capacity(records.len());
        for record in records {
            items.push(self.application_view(record).await?);
        }
        Ok(ApplicationsView { items })
    }

    pub async fn hiring_flow_analytics(
        &self,
        session: &SessionView,
    ) -> Result<HiringFlowAnalyticsView, ApplicationError> {
        let applications = self.company_applications(session, None).await?;
        let facts = crate::analytics::hiring_flow(&applications.items);
        let primary = &facts.bottlenecks[0];
        let mut assistant = HiringAssistantView {
            mode: "metrics",
            summary: format!("{}: {}", primary.title, primary.evidence),
            actions: vec![primary.recommendation.clone()],
        };
        if let Some(llm) = &self.llm {
            match llm.analyze_hiring_flow(&facts).await {
                Ok(narrative)
                    if !narrative.summary.trim().is_empty()
                        && narrative.summary.chars().count() <= 600
                        && !narrative.actions.is_empty()
                        && narrative.actions.len() <= 3
                        && narrative.actions.iter().all(|action| {
                            !action.trim().is_empty() && action.chars().count() <= 180
                        }) =>
                {
                    assistant = HiringAssistantView {
                        mode: "bifrost",
                        summary: narrative.summary,
                        actions: narrative.actions,
                    };
                }
                Ok(_) => tracing::warn!("hiring-flow analyst returned an invalid empty narrative"),
                Err(error) => tracing::warn!(code = error.code, "hiring-flow analyst unavailable"),
            }
        }
        Ok(HiringFlowAnalyticsView {
            generated_at: now(),
            total: facts.total,
            stages: facts.stages,
            median_time_to_view_hours: facts.median_time_to_view_hours,
            median_time_to_decision_hours: facts.median_time_to_decision_hours,
            acceptance_rate_percent: facts.acceptance_rate_percent,
            vacancies: facts.vacancies,
            bottlenecks: facts.bottlenecks,
            assistant,
        })
    }

    pub async fn get_application(
        &self,
        session: &SessionView,
        application_id: &str,
    ) -> Result<ApplicationView, ApplicationError> {
        let record = self.application_for(session, application_id).await?;
        self.application_view(record).await
    }

    pub async fn view_application(
        &self,
        session: &SessionView,
        application_id: &str,
    ) -> Result<ApplicationView, ApplicationError> {
        let mut record = self.application_for(session, application_id).await?;
        if record.status == InteractionStatus::Sent {
            record.status = InteractionStatus::Viewed;
            record.viewed_at = Some(now());
            match self
                .repository
                .save_application_guarded(&record, &[InteractionStatus::Sent])
                .await
            {
                Ok(()) => {}
                Err(RepositoryError::NotFound) => {
                    record = self.application_for(session, application_id).await?;
                }
                Err(error) => return Err(ApplicationError::storage(error)),
            }
        }
        self.application_view(record).await
    }

    pub async fn accept_application(
        &self,
        session: &SessionView,
        application_id: &str,
    ) -> Result<ApplicationView, ApplicationError> {
        self.decide_application(session, application_id, InteractionStatus::Accepted)
            .await
    }

    pub async fn decline_application(
        &self,
        session: &SessionView,
        application_id: &str,
    ) -> Result<ApplicationView, ApplicationError> {
        self.decide_application(session, application_id, InteractionStatus::Declined)
            .await
    }

    /// The mock's accept/decline pair: the same terminal replay returns the
    /// current record, a different terminal outcome conflicts.
    async fn decide_application(
        &self,
        session: &SessionView,
        application_id: &str,
        outcome: InteractionStatus,
    ) -> Result<ApplicationView, ApplicationError> {
        let mut record = self.application_for(session, application_id).await?;
        if record.status == outcome {
            return self.application_view(record).await;
        }
        if record.status.terminal() {
            return Err(ApplicationError::new(
                409,
                "application_terminal",
                "Решение по отклику уже принято",
            ));
        }
        record.status = outcome;
        if record.viewed_at.is_none() {
            record.viewed_at = Some(now());
        }
        record.responded_at = Some(now());
        self.repository
            .save_application_guarded(
                &record,
                &[InteractionStatus::Sent, InteractionStatus::Viewed],
            )
            .await
            .map_err(|error| match error {
                RepositoryError::NotFound => ApplicationError::new(
                    409,
                    "application_terminal",
                    "Решение по отклику уже принято",
                ),
                other => ApplicationError::storage(other),
            })?;
        self.application_view(record).await
    }

    /// The mock's `getMyTasks`: current weekly offer, its response if the
    /// week is already answered, full history and freshness signals.
    pub async fn my_tasks(&self, session: &SessionView) -> Result<MyTasksView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        let candidate = self.candidate_index(candidate_id).await?;
        let responses = self.sorted_task_responses(candidate_id).await?;
        let offer = self.task_offer(&candidate, &responses).await?;
        let (_, from, to) = task_period(Utc::now());
        let signals = self
            .freshness_signals(
                candidate_id,
                candidate.profile.grade_confirmed_at.as_deref(),
            )
            .await?;
        Ok(MyTasksView {
            period: TaskPeriodView {
                from: from.to_rfc3339(),
                to: to.to_rfc3339(),
            },
            current: offer.current.as_ref().map(offered_task_view),
            current_response: offer.current_response.as_ref().map(task_response_view),
            reason: offer.reason,
            history: responses.iter().map(task_response_view).collect(),
            freshness: freshness_view(&signals),
        })
    }

    /// The mock's `taskOffer`: this week's answered response wins; otherwise
    /// the rotating pool of active tasks on evidenced skills at or below the
    /// verified grade, minus tasks already answered.
    async fn task_offer(
        &self,
        candidate: &CandidateIndexRecord,
        responses: &[fsp_domain::TaskResponseRecord],
    ) -> Result<TaskOffer, ApplicationError> {
        let (index, from, _) = task_period(Utc::now());
        if let Some(answered) = responses
            .iter()
            .find(|item| timestamp_at(&item.submitted_at).is_some_and(|at| at >= from))
        {
            return Ok(TaskOffer {
                current: Some(answered.task.clone()),
                current_response: Some(answered.clone()),
                reason: None,
            });
        }
        let Some(category) = candidate.profile.category() else {
            return Ok(TaskOffer {
                current: None,
                current_response: None,
                reason: Some("category"),
            });
        };
        if !candidate.profile.consents.processing {
            return Ok(TaskOffer {
                current: None,
                current_response: None,
                reason: Some("processing_consent"),
            });
        }
        let evidenced = candidate
            .skills
            .iter()
            .filter(|skill| skill.status != EvidenceStatus::Insufficient)
            .map(|skill| skill.skill_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let answered = responses
            .iter()
            .map(|item| item.task.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let verified = category.grade;
        let mut pool = self
            .repository
            .active_tasks()
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .filter(|task| {
                evidenced.contains(task.skill_id.as_str())
                    && task.grade <= verified
                    && !answered.contains(task.id.as_str())
            })
            .collect::<Vec<_>>();
        pool.sort_by(|a, b| a.id.cmp(&b.id));
        let Some(task) = pool.get(index as usize % pool.len().max(1)).cloned() else {
            return Ok(TaskOffer {
                current: None,
                current_response: None,
                reason: Some("no_tasks"),
            });
        };
        Ok(TaskOffer {
            current: Some(self.offered_task(&task).await?),
            current_response: None,
            reason: None,
        })
    }

    /// The mock's `offeredTask`: the task card plus the owning company's card.
    async fn offered_task(
        &self,
        task: &fsp_domain::TaskRecord,
    ) -> Result<fsp_domain::OfferedTaskSnapshot, ApplicationError> {
        let company = self
            .repository
            .company(&task.company_id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| {
                ApplicationError::new(500, "internal_error", "Сервис временно недоступен")
            })?;
        Ok(fsp_domain::OfferedTaskSnapshot {
            id: task.id.clone(),
            company_id: company.id,
            company_display_name: company.display_name,
            title: task.title.clone(),
            prompt: task.prompt.clone(),
            skill_id: task.skill_id.clone(),
            grade: task.grade,
            estimate_minutes: task.estimate_minutes,
        })
    }

    /// The mock's `responsesOf`: newest first, id as the tie-breaker.
    async fn sorted_task_responses(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::TaskResponseRecord>, ApplicationError> {
        let mut items = self
            .repository
            .candidate_task_responses(candidate_id)
            .await
            .map_err(ApplicationError::storage)?;
        items.sort_by(|a, b| {
            timestamp_at(&b.submitted_at)
                .cmp(&timestamp_at(&a.submitted_at))
                .then(b.id.cmp(&a.id))
        });
        Ok(items)
    }

    /// The mock's `listTasks`: the employer's tasks, newest first, with the
    /// number of responses each received.
    pub async fn tasks(&self, session: &SessionView) -> Result<TasksView, ApplicationError> {
        let company_id = employer_company_id(session)?;
        let mut records = self
            .repository
            .tasks(company_id)
            .await
            .map_err(ApplicationError::storage)?;
        records.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
        let responses = self
            .repository
            .company_task_responses(company_id)
            .await
            .map_err(ApplicationError::storage)?;
        let items = records
            .iter()
            .map(|task| {
                task_view(
                    task,
                    responses
                        .iter()
                        .filter(|item| item.task.id == task.id)
                        .count(),
                )
            })
            .collect();
        Ok(TasksView { items })
    }

    /// The mock's `createTask`: field validation first, then the company
    /// completeness and email gates.
    pub async fn create_task(
        &self,
        session: &SessionView,
        input: TaskInput,
    ) -> Result<TaskView, ApplicationError> {
        if !(1..=160).contains(&input.title.chars().count())
            || !(1..=4000).contains(&input.prompt.chars().count())
            || !(5..=120).contains(&input.estimate_minutes)
            || !fsp_domain::catalog::is_skill(&input.skill_id)
        {
            return Err(validation("Запрос не соответствует контракту"));
        }
        let company = self.require_invitable_employer(session).await?;
        let record = fsp_domain::TaskRecord {
            id: format!("task-{}", Uuid::new_v4().simple()),
            company_id: company.id,
            title: input.title,
            prompt: input.prompt,
            skill_id: input.skill_id,
            grade: input.grade,
            estimate_minutes: input.estimate_minutes,
            status: fsp_domain::TaskStatus::Active,
            created_at: now(),
        };
        self.repository
            .save_task(&record)
            .await
            .map_err(ApplicationError::storage)?;
        Ok(task_view(&record, 0))
    }

    /// The mock's `archiveTask`: unconditional, so a repeat is a no-op.
    pub async fn archive_task(
        &self,
        session: &SessionView,
        task_id: &str,
    ) -> Result<TaskView, ApplicationError> {
        let company_id = employer_company_id(session)?;
        let mut record = self.task_for(company_id, task_id).await?;
        record.status = fsp_domain::TaskStatus::Archived;
        self.repository
            .save_task(&record)
            .await
            .map_err(ApplicationError::storage)?;
        let count = self
            .repository
            .company_task_responses(company_id)
            .await
            .map_err(ApplicationError::storage)?
            .iter()
            .filter(|item| item.task.id == record.id)
            .count();
        Ok(task_view(&record, count))
    }

    async fn task_for(
        &self,
        company_id: &str,
        task_id: &str,
    ) -> Result<fsp_domain::TaskRecord, ApplicationError> {
        self.repository
            .task(task_id)
            .await
            .map_err(ApplicationError::storage)?
            .filter(|task| task.company_id == company_id)
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Ресурс не найден"))
    }

    /// The mock's `listTaskResponses`: the company's responses, newest first,
    /// optionally narrowed to one task.
    pub async fn task_responses(
        &self,
        session: &SessionView,
        task_id: Option<&str>,
    ) -> Result<TaskResponsesView, ApplicationError> {
        let company_id = employer_company_id(session)?;
        let mut items = self
            .repository
            .company_task_responses(company_id)
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .filter(|item| task_id.is_none_or(|id| item.task.id == id))
            .collect::<Vec<_>>();
        items.sort_by(|a, b| {
            timestamp_at(&b.submitted_at)
                .cmp(&timestamp_at(&a.submitted_at))
                .then(b.id.cmp(&a.id))
        });
        Ok(TaskResponsesView {
            items: items.iter().map(task_response_view).collect(),
        })
    }

    /// The mock's `reviewTaskResponse`: repeating the same outcome replays,
    /// a different outcome on a reviewed response conflicts.
    pub async fn review_task_response(
        &self,
        session: &SessionView,
        response_id: &str,
        input: TaskReviewInput,
    ) -> Result<TaskResponseView, ApplicationError> {
        if input.feedback.chars().count() > 2000 {
            return Err(validation("Запрос не соответствует контракту"));
        }
        let company_id = employer_company_id(session)?;
        let mut record = self
            .repository
            .task_response(response_id)
            .await
            .map_err(ApplicationError::storage)?
            .filter(|item| item.task.company_id == company_id)
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Ресурс не найден"))?;
        let trimmed = input.feedback.trim();
        let feedback = (!trimmed.is_empty()).then(|| trimmed.to_string());
        let outcome = match input.outcome {
            TaskReviewOutcome::Accepted => fsp_domain::TaskResponseStatus::Accepted,
            TaskReviewOutcome::NeedsWork => fsp_domain::TaskResponseStatus::NeedsWork,
        };
        if record.status != fsp_domain::TaskResponseStatus::Submitted {
            if record.status == outcome && record.feedback == feedback {
                return Ok(task_response_view(&record));
            }
            return Err(ApplicationError::new(
                409,
                "task_response_reviewed",
                "Ответ уже оценён",
            ));
        }
        record.status = outcome;
        record.feedback = feedback.clone();
        record.reviewed_at = Some(now());
        let result = self.repository.review_task_response(&record).await;
        match result {
            Ok(()) => Ok(task_response_view(&record)),
            // A concurrent review landed first: replay when the outcome matches,
            // conflict otherwise — same rule the pre-read check applied.
            Err(RepositoryError::NotFound) => match self
                .repository
                .task_response(response_id)
                .await
                .map_err(ApplicationError::storage)?
            {
                Some(current) if current.status == outcome && current.feedback == feedback => {
                    Ok(task_response_view(&current))
                }
                _ => Err(ApplicationError::new(
                    409,
                    "task_response_reviewed",
                    "Ответ уже оценён",
                )),
            },
            Err(other) => Err(ApplicationError::storage(other)),
        }
    }

    /// The mock's `listMyAssessments`, newest id first.
    pub async fn my_assessments(
        &self,
        session: &SessionView,
    ) -> Result<AssessmentsView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        let mut records = self
            .repository
            .candidate_assessments(candidate_id)
            .await
            .map_err(ApplicationError::storage)?;
        records.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(AssessmentsView {
            items: records.iter().map(assessment_view).collect(),
        })
    }

    pub async fn get_assessment(
        &self,
        session: &SessionView,
        assessment_id: &str,
    ) -> Result<AssessmentView, ApplicationError> {
        Ok(assessment_view(
            &self.assessment_for(session, assessment_id).await?,
        ))
    }

    /// The mock's `saveAssessmentAnswer`: stores the answer and moves the
    /// draft into `in_progress`.
    pub async fn save_assessment_answer(
        &self,
        session: &SessionView,
        assessment_id: &str,
        input: fsp_domain::AssessmentAnswer,
    ) -> Result<AssessmentView, ApplicationError> {
        if input.code.chars().count() > 100_000 || input.explanation.chars().count() > 20_000 {
            return Err(validation("Запрос не соответствует контракту"));
        }
        let tx = self
            .repository
            .transaction()
            .await
            .map_err(ApplicationError::storage)?;
        let scoped = Self {
            repository: tx.share(),
            ..self.clone()
        };
        tx.lock_candidate(candidate_id(session)?)
            .await
            .map_err(ApplicationError::storage)?;
        let mut record = scoped.assessment_for(session, assessment_id).await?;
        scoped
            .require_processing_consent(&record.candidate_id)
            .await?;
        mutable_assessment(&record)?;
        if !fsp_domain::valid_knowledge_answer(&record.task, &input) {
            return Err(validation(
                "Ответ содержит неизвестный вопрос, вариант или повтор",
            ));
        }
        record.answer = input;
        record.checks = fsp_domain::knowledge_completion(&record.task, &record.answer);
        record.status = fsp_domain::AssessmentStatus::InProgress;
        let base_revision = record.revision;
        record.revision += 1;
        record.updated_at = now();
        tx.save_assessment_guarded(
            &record,
            base_revision,
            &[
                fsp_domain::AssessmentStatus::Draft,
                fsp_domain::AssessmentStatus::InProgress,
            ],
        )
        .await
        .map_err(|error| match error {
            RepositoryError::NotFound => ApplicationError::new(
                409,
                "assessment_not_editable",
                "Проверка уже отправлена или завершена",
            ),
            other => ApplicationError::storage(other),
        })?;
        tx.commit().await.map_err(ApplicationError::storage)?;
        Ok(assessment_view(&record))
    }

    /// The mock's `runAssessmentChecks` job, run inline: the demo checks
    /// complete synchronously, so the returned job is already terminal.
    pub async fn run_assessment_checks(
        &self,
        session: &SessionView,
        assessment_id: &str,
    ) -> Result<JobView, ApplicationError> {
        let tx = self
            .repository
            .transaction()
            .await
            .map_err(ApplicationError::storage)?;
        let scoped = Self {
            repository: tx.share(),
            ..self.clone()
        };
        tx.lock_candidate(candidate_id(session)?)
            .await
            .map_err(ApplicationError::storage)?;
        let mut record = scoped.assessment_for(session, assessment_id).await?;
        scoped
            .require_processing_consent(&record.candidate_id)
            .await?;
        mutable_assessment(&record)?;
        let mut job = self.new_job(
            &record.candidate_id,
            "checks",
            Some(&record.id),
            Some(record.revision),
        );
        record.checks = fsp_domain::knowledge_completion(&record.task, &record.answer);
        record.updated_at = now();
        // Checks write and the job row commit together.
        tx.save_assessment_guarded(
            &record,
            record.revision,
            &[
                fsp_domain::AssessmentStatus::Draft,
                fsp_domain::AssessmentStatus::InProgress,
            ],
        )
        .await
        .map_err(|error| match error {
            RepositoryError::NotFound => ApplicationError::new(
                409,
                "assessment_not_editable",
                "Проверка уже отправлена или завершена",
            ),
            other => ApplicationError::storage(other),
        })?;
        job.status = "succeeded".into();
        job.updated_at = now();
        job.result_json = Some(self.job_result(&JobResultView::Checks {
            assessment_id: record.id.clone(),
            checks: record.checks.clone(),
            simulation: false,
        })?);
        tx.save_job(&job).await.map_err(ApplicationError::storage)?;
        tx.commit().await.map_err(ApplicationError::storage)?;
        Ok(job_view(job, None))
    }

    /// Assistance is advisory; only an evaluator can issue assessment results.
    pub async fn ask_assessment_assistant(
        &self,
        session: &SessionView,
        assessment_id: &str,
        input: AssistantInput,
    ) -> Result<AssistantReplyView, ApplicationError> {
        let record = self.assessment_for(session, assessment_id).await?;
        self.require_processing_consent(&record.candidate_id)
            .await?;
        mutable_assessment(&record)?;
        let question = input.message.trim();
        if question.is_empty() || question.chars().count() > 10_000 {
            return Err(validation("Введите вопрос длиной до 10000 символов"));
        }
        let llm = self.llm.as_ref().ok_or_else(|| {
            ApplicationError::new(503, "assistant_unavailable", "ИИ-помощник не настроен")
        })?;
        let reply = llm
            .assessment_help(&fsp_domain::AssessmentHelpRequest {
                task: record.task,
                answer: record.answer,
                question: question.into(),
            })
            .await
            .map_err(integration_error)?;
        if reply.message.trim().is_empty()
            || reply.message.chars().count() > 6000
            || reply
                .message
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t' && c != '\r')
        {
            return Err(ApplicationError::new(
                502,
                "assistant_invalid_response",
                "Помощник вернул некорректный ответ. Повторите вопрос",
            ));
        }
        self.require_processing_consent(&record.candidate_id)
            .await?;
        mutable_assessment(&self.assessment_for(session, assessment_id).await?)?;
        Ok(AssistantReplyView {
            message: reply.message.trim().into(),
            simulation: false,
            ai_allowed: true,
            external_ai_allowed: true,
        })
    }

    /// Score the immutable knowledge test, preserve grades on failure and
    /// attach successful results as evidence for the tested skills.
    /// Writes go through the caller's repository handle so the grade change and
    /// the assessment result commit in one transaction.
    async fn complete_assessment(
        &self,
        repo: &dyn PlatformRepository,
        record: &mut fsp_domain::AssessmentRecord,
    ) -> Result<(), ApplicationError> {
        repo.lock_candidate(&record.candidate_id)
            .await
            .map_err(ApplicationError::storage)?;
        let mut candidate = repo
            .candidate_index(&record.candidate_id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Кандидат не найден"))?;
        let score = fsp_domain::score_knowledge(
            record.specialization,
            record.grade,
            &record.task,
            &record.answer,
        )
        .ok_or_else(|| {
            ApplicationError::new(422, "answer_incomplete", "Ответьте на все вопросы теста")
        })?;
        let previous_grade = candidate.profile.verified_grade;
        let grade_changed = score.passed
            && (candidate.profile.verified_grade != Some(record.grade)
                || candidate.profile.verified_specialization != Some(record.specialization));
        if score.passed
            && (candidate.profile.verified_specialization != Some(record.specialization)
                || candidate.profile.verified_grade.is_none()
                || candidate
                    .profile
                    .verified_grade
                    .is_some_and(|grade| record.grade >= grade))
        {
            candidate.profile.verified_grade = Some(record.grade);
            candidate.profile.verified_specialization = Some(record.specialization);
            if grade_changed {
                candidate.profile.grade_confirmed_at = Some(now());
                candidate.last_signal_at = candidate.profile.grade_confirmed_at.clone();
            }
            candidate.last_signal_at = Some(now());
            // A passed test confirms the two topics it covers.
            let confirmed_skills =
                &fsp_domain::catalog::specialization(record.specialization).test_topics;
            for skill_id in confirmed_skills {
                let skill = candidate
                    .skills
                    .iter_mut()
                    .find(|item| &item.skill_id == skill_id);
                match skill {
                    Some(item) => {
                        item.status = EvidenceStatus::Confirmed;
                        item.evidence_grade = Some(record.grade);
                        if !item
                            .sources
                            .contains(&fsp_domain::EvidenceSource::Assessment)
                        {
                            item.sources.push(fsp_domain::EvidenceSource::Assessment);
                        }
                    }
                    None => candidate.skills.push(fsp_domain::SkillEvidence {
                        skill_id: skill_id.clone(),
                        status: EvidenceStatus::Confirmed,
                        evidence_grade: Some(record.grade),
                        sources: vec![fsp_domain::EvidenceSource::Assessment],
                    }),
                }
            }
            candidate.profile.revision += 1;
            candidate.profile.updated_at = now();
            repo.save_candidate(&candidate.profile)
                .await
                .map_err(ApplicationError::storage)?;
            repo.save_candidate_index(&candidate)
                .await
                .map_err(ApplicationError::storage)?;
        }
        record.checks = score.checks;
        record.status = if score.passed {
            fsp_domain::AssessmentStatus::Passed
        } else {
            fsp_domain::AssessmentStatus::Failed
        };
        record.updated_at = now();
        record.revision += 1;
        record.result = Some(fsp_domain::AssessmentResultData {
            assessment_id: record.id.clone(),
            outcome: if score.passed { "passed" } else { "failed" }.into(),
            confirmed_grade: candidate.profile.verified_grade,
            previous_grade,
            grade_changed,
            rubric: score.rubric,
            checks: record.checks.clone(),
            feedback: score.feedback,
            next_eligible_at: candidate
                .profile
                .grade_confirmed_at
                .as_deref()
                .and_then(timestamp_at)
                .map(|at| (at + Duration::days(i64::from(self.grade_cooldown_days))).to_rfc3339()),
            simulation: false,
        });
        Ok(())
    }

    async fn assessment_for(
        &self,
        session: &SessionView,
        assessment_id: &str,
    ) -> Result<fsp_domain::AssessmentRecord, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        self.repository
            .assessment(assessment_id)
            .await
            .map_err(ApplicationError::storage)?
            .filter(|item| item.candidate_id == candidate_id)
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Ресурс не найден"))
    }

    /// Whether the candidate has an assessment in the mock's active set —
    /// feeds the `active_assessment` eligibility reason.
    async fn has_active_assessment(&self, candidate_id: &str) -> Result<bool, ApplicationError> {
        Ok(self
            .repository
            .candidate_assessments(candidate_id)
            .await
            .map_err(ApplicationError::storage)?
            .iter()
            .any(|item| item.status.active() && item.task.execution == "questionnaire"))
    }

    fn new_job(
        &self,
        candidate_id: &str,
        kind: &str,
        assessment_id: Option<&str>,
        input_revision: Option<u64>,
    ) -> fsp_domain::JobRecord {
        let timestamp = now();
        fsp_domain::JobRecord {
            id: format!("job-{}", Uuid::new_v4().simple()),
            owner_id: candidate_id.to_string(),
            kind: kind.into(),
            status: "running".into(),
            candidate_id: Some(candidate_id.to_string()),
            assessment_id: assessment_id.map(str::to_string),
            input_revision,
            result_json: None,
            confirmed: false,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            result_source_id: None,
            error_code: None,
            error_message: None,
        }
    }

    fn job_result(&self, result: &JobResultView) -> Result<String, ApplicationError> {
        serde_json::to_string(result)
            .map_err(|error| ApplicationError::new(500, "internal_error", error.to_string()))
    }

    /// The mock's `getMyGapPlan`: the need is reachable only through an
    /// invitation addressed to this candidate.
    pub async fn my_gap_plan(
        &self,
        session: &SessionView,
        need_id: Option<&str>,
    ) -> Result<GapPlanView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        let candidate = self.candidate_index(candidate_id).await?;
        let invitation = self
            .repository
            .candidate_invitations(candidate_id)
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .find(|item| Some(item.need_definition.id.as_str()) == need_id)
            .ok_or_else(|| {
                ApplicationError::new(404, "not_found", "Потребность недоступна этому кандидату")
            })?;
        Ok(gap_plan(&candidate, &invitation.need_definition))
    }

    /// The mock's `getCandidateGapPlan`: the candidate must be reachable by
    /// the company and the need must belong to it.
    pub async fn candidate_gap_plan(
        &self,
        session: &SessionView,
        candidate_id: &str,
        need_id: Option<&str>,
    ) -> Result<GapPlanView, ApplicationError> {
        let company_id = employer_company_id(session)?;
        let candidate = self.candidate_for_company(company_id, candidate_id).await?;
        let need_id =
            need_id.ok_or_else(|| ApplicationError::new(404, "not_found", "Ресурс не найден"))?;
        let need = self
            .repository
            .need(company_id, need_id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Ресурс не найден"))?;
        Ok(gap_plan(&candidate, &need))
    }

    /// The mock's `candidateForCompany`: a live application or public-bank
    /// visibility makes the candidate reachable for the company.
    async fn candidate_for_company(
        &self,
        company_id: &str,
        candidate_id: &str,
    ) -> Result<CandidateIndexRecord, ApplicationError> {
        let applied = self
            .repository
            .company_applications(company_id)
            .await
            .map_err(ApplicationError::storage)?
            .iter()
            .any(|item| {
                item.candidate_id == candidate_id && item.status != InteractionStatus::Withdrawn
            });
        self.repository
            .candidate_index(candidate_id)
            .await
            .map_err(ApplicationError::storage)?
            .filter(|item| applied || item.profile.visible_to_employers(item.email_verified))
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Ресурс не найден"))
    }

    /// The mock's `confirmProfileImport`: a succeeded, unconfirmed
    /// `profile_import` job applies the submitted profile patch once.
    pub async fn confirm_profile_import(
        &self,
        session: &SessionView,
        job_id: &str,
        input: ImportConfirmInput,
    ) -> Result<ProfileView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        let mut job = self
            .repository
            .job(candidate_id, job_id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Ресурс не найден"))?;
        if job.kind != "profile_import" {
            return Err(ApplicationError::new(404, "not_found", "Импорт не найден"));
        }
        if job.status != "succeeded" {
            return Err(ApplicationError::new(
                409,
                "import_not_ready",
                "Дождитесь завершения импорта",
            ));
        }
        if job.confirmed {
            return Err(ApplicationError::new(
                409,
                "import_already_confirmed",
                "Импорт уже применён",
            ));
        }
        self.require_processing_consent(candidate_id).await?;
        validate_profile_patch(&input.profile)?;
        let tx = self
            .repository
            .transaction()
            .await
            .map_err(ApplicationError::storage)?;
        tx.lock_candidate(candidate_id)
            .await
            .map_err(ApplicationError::storage)?;
        if tx
            .job(candidate_id, job_id)
            .await
            .map_err(ApplicationError::storage)?
            .is_some_and(|current| current.confirmed)
        {
            return Err(ApplicationError::new(
                409,
                "import_already_confirmed",
                "Импорт уже применён",
            ));
        }
        let mut profile = tx
            .candidate(candidate_id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Профиль не найден"))?;
        imports::require_current_profile(&profile, job.input_revision.unwrap_or(profile.revision))?;
        apply_profile_patch(&mut profile, input.profile)?;
        profile.revision += 1;
        profile.updated_at = now();
        tx.save_candidate(&profile)
            .await
            .map_err(ApplicationError::storage)?;
        job.confirmed = true;
        job.updated_at = now();
        tx.save_job_unconfirmed(&job)
            .await
            .map_err(|error| match error {
                RepositoryError::NotFound => {
                    ApplicationError::new(409, "import_already_confirmed", "Импорт уже применён")
                }
                other => ApplicationError::storage(other),
            })?;
        tx.commit().await.map_err(ApplicationError::storage)?;
        let signals = self
            .freshness_signals(&profile.id, profile.grade_confirmed_at.as_deref())
            .await?;
        let assessment_running = self.has_active_assessment(&profile.id).await?;
        Ok(profile_view(
            profile,
            session.email_verified,
            &signals,
            assessment_running,
            self.grade_cooldown_days,
        ))
    }

    /// The mock's `exportMyProfile`: the profile DTO the PDF is rendered
    /// from; rendering lives in the transport layer.
    pub async fn export_profile(
        &self,
        session: &SessionView,
    ) -> Result<ProfileView, ApplicationError> {
        self.profile(session).await
    }

    pub async fn candidate_search(
        &self,
        session: &SessionView,
        search: CandidateSearch,
    ) -> Result<CandidatePageView, ApplicationError> {
        let company_id = employer_company_id(session)?;
        let need = match search.need_id.as_deref() {
            Some(id) => Some(
                self.repository
                    .need(company_id, id)
                    .await
                    .map_err(ApplicationError::storage)?
                    .ok_or_else(|| {
                        ApplicationError::new(404, "not_found", "Потребность не найдена")
                    })?,
            ),
            None if search.recommended => {
                return Err(ApplicationError::new(
                    422,
                    "need_required",
                    "Для рекомендаций выберите потребность",
                ));
            }
            None => None,
        };
        let mut records = self
            .repository
            .candidates()
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .filter(|item| item.profile.visible_to_employers(item.email_verified))
            .filter(|item| {
                item.profile.category().is_some_and(|category| {
                    search
                        .specialization
                        .is_none_or(|filter| filter == category.specialization)
                        && search.grade.is_none_or(|filter| filter == category.grade)
                        && search
                            .category_source
                            .is_none_or(|filter| filter == category.source)
                }) && search.skill.as_ref().is_none_or(|filter| {
                    item.skills.iter().any(|skill| {
                        skill.skill_id == *filter && skill.status != EvidenceStatus::Insufficient
                    })
                }) && (!search.fsp_verified || item.fsp_verified)
            })
            .collect::<Vec<_>>();
        if search.recommended {
            let selected = need.as_ref().expect("recommended search has a need");
            records.retain(|item| {
                item.profile.category().is_some_and(|category| {
                    category.specialization == selected.need.specialization
                        && category.grade >= selected.need.grade
                })
            });
        }
        if let Some(selected) = &need {
            let rankable = records
                .iter()
                .filter_map(CandidateIndexRecord::rankable)
                .collect::<Vec<_>>();
            let order = rank_candidates(&rankable, &selected.need)
                .into_iter()
                .enumerate()
                .map(|(position, item)| (item.0, position))
                .collect::<std::collections::HashMap<_, _>>();
            records.sort_by_key(|item| order.get(&item.profile.id).copied().unwrap_or(usize::MAX));
        } else {
            // Visible candidates all have a category; test-confirmed ones lead within a grade.
            records.sort_by_cached_key(|item| {
                let category = item.profile.category();
                (
                    category.as_ref().map(|category| category.specialization),
                    std::cmp::Reverse(category.as_ref().map(|category| category.grade)),
                    std::cmp::Reverse(category.as_ref().map_or(0, fsp_domain::Category::strength)),
                    !item.fsp_verified,
                    item.profile.id.clone(),
                )
            });
        }
        let total = records.len();
        let start = (search.page - 1).saturating_mul(search.page_size);
        let mut items = Vec::new();
        for candidate in records.into_iter().skip(start).take(search.page_size) {
            let signals = self
                .freshness_signals(
                    &candidate.profile.id,
                    candidate.profile.grade_confirmed_at.as_deref(),
                )
                .await?;
            let (github, fsp) = self
                .source_signals(&candidate.profile.id, need.as_ref())
                .await?;
            items.push(anonymous_candidate(
                candidate,
                need.as_ref(),
                &signals,
                github,
                fsp,
            ));
        }
        Ok(CandidatePageView {
            items,
            page: search.page,
            page_size: search.page_size,
            total,
            has_next_page: start.saturating_add(search.page_size) < total,
        })
    }

    pub async fn candidate(
        &self,
        session: &SessionView,
        candidate_id: &str,
        need_id: Option<&str>,
    ) -> Result<AnonymousCandidateView, ApplicationError> {
        let company_id = employer_company_id(session)?;
        let need = match need_id {
            Some(id) => self
                .repository
                .need(company_id, id)
                .await
                .map_err(ApplicationError::storage)?,
            None => None,
        };
        let candidate = self.candidate_for_company(company_id, candidate_id).await?;
        let signals = self
            .freshness_signals(
                &candidate.profile.id,
                candidate.profile.grade_confirmed_at.as_deref(),
            )
            .await?;
        let (github, fsp) = self
            .source_signals(&candidate.profile.id, need.as_ref())
            .await?;
        Ok(anonymous_candidate(
            candidate,
            need.as_ref(),
            &signals,
            github,
            fsp,
        ))
    }

    pub async fn github_authorization_url(
        &self,
        session: &SessionView,
        redirect_uri: &str,
    ) -> Result<String, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        let github = self.github()?;
        let state = Uuid::new_v4().simple().to_string();
        self.repository
            .save_oauth_state(
                candidate_id,
                &state,
                &(Utc::now() + Duration::minutes(10)).to_rfc3339(),
            )
            .await
            .map_err(ApplicationError::storage)?;
        github
            .authorization_url(&state, redirect_uri)
            .map_err(integration_error)
    }

    pub async fn complete_github_authorization(
        &self,
        session: &SessionView,
        state: &str,
        code: &str,
    ) -> Result<(), ApplicationError> {
        let candidate_id = candidate_id(session)?;
        let valid = self
            .repository
            .consume_oauth_state(candidate_id, state, &now())
            .await
            .map_err(ApplicationError::storage)?;
        if !valid {
            return Err(ApplicationError::new(
                400,
                "oauth_state_invalid",
                "Ссылка авторизации устарела или уже использована",
            ));
        }
        let token = self
            .github()?
            .exchange_code(code)
            .await
            .map_err(integration_error)?;
        self.repository
            .save_github_token(candidate_id, &token)
            .await
            .map_err(ApplicationError::storage)
    }

    pub async fn sources(&self, session: &SessionView) -> Result<SourcesView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        let mut records = self
            .repository
            .sources(candidate_id)
            .await
            .map_err(ApplicationError::storage)?;
        for provider in [
            fsp_domain::SourceProvider::Github,
            fsp_domain::SourceProvider::Fsp,
        ] {
            if !records.iter().any(|record| record.provider == provider) {
                records.push(fsp_domain::SourceRecord {
                    id: format!("source-{}-{candidate_id}", source_provider(provider)),
                    candidate_id: candidate_id.into(),
                    provider,
                    synthetic: provider == fsp_domain::SourceProvider::Fsp,
                    active_job_id: None,
                    account_label: None,
                    last_synced_at: None,
                    analysis: None,
                    verified_participation: false,
                    competition_ids: Vec::new(),
                    fsp_results: Vec::new(),
                });
            }
        }
        records.sort_by_key(|record| source_provider(record.provider));
        let simulation = records.iter().any(|record| record.synthetic);
        let code = self.github_ownership_code(candidate_id);
        let mut items = Vec::with_capacity(records.len());
        for record in records {
            let progress = self.github_progress(&record).await;
            items.push(source_view(record, &code, progress));
        }
        Ok(SourcesView { simulation, items })
    }

    pub async fn skills(&self, session: &SessionView) -> Result<SkillsView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        let record = self.candidate_index(candidate_id).await?;
        let assessments = self
            .repository
            .candidate_assessments(candidate_id)
            .await
            .map_err(ApplicationError::storage)?;
        let observed_at = record.profile.updated_at.clone();
        let items = record
            .skills
            .into_iter()
            .map(|skill| {
                let evidence = skill
                    .sources
                    .iter()
                    .enumerate()
                    .map(|(index, source)| {
                        let assessment = (*source == fsp_domain::EvidenceSource::Assessment)
                            .then(|| {
                                assessments.iter().find(|attempt| {
                                    attempt.status == fsp_domain::AssessmentStatus::Passed
                                        && Some(attempt.grade) == skill.evidence_grade
                                        && matches!(skill.skill_id.as_str(), "rust" | "postgresql")
                                })
                            })
                            .flatten();
                        EvidenceView {
                            id: format!("evidence-{}-{index}", skill.skill_id),
                            source: *source,
                            title: match assessment {
                                Some(item) if item.task.execution == "questionnaire" => format!(
                                    "Тест знаний: {} · {}",
                                    skill_label(&skill.skill_id),
                                    item.task.version
                                ),
                                _ => format!("Свидетельство: {}", skill_label(&skill.skill_id)),
                            },
                            url: None,
                            reference: Some(assessment.map_or_else(
                                || format!("{}:{}", source_name(*source), skill.skill_id),
                                |item| item.id.clone(),
                            )),
                            observed_at: assessment.map_or_else(
                                || observed_at.clone(),
                                |item| item.updated_at.clone(),
                            ),
                            scope: "individual",
                            synthetic: *source == fsp_domain::EvidenceSource::Fsp
                                || assessment.is_some_and(|a| a.task.execution != "questionnaire"),
                        }
                    })
                    .collect::<Vec<_>>();
                SelfSkillView {
                    skill_id: skill.skill_id.clone(),
                    label: skill_label(&skill.skill_id).into(),
                    status: skill.status,
                    evidence_grade: skill.evidence_grade,
                    evidence_sources: skill.sources,
                    summary: skill_summary(skill.status).into(),
                    evidence_count: evidence.len(),
                    confirmed_at: (skill.status == EvidenceStatus::Confirmed)
                        .then(|| observed_at.clone()),
                    evidence,
                }
            })
            .collect();
        Ok(SkillsView {
            items,
            simulation: true,
        })
    }

    async fn candidate_index(&self, id: &str) -> Result<CandidateIndexRecord, ApplicationError> {
        self.repository
            .candidate_index(id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Кандидат не найден"))
    }

    /// The mock's `freshness()` inputs: the verified-grade signal plus every
    /// counted task response (not marked `needs_work`), assessment first.
    /// The anonymous GitHub projection of a candidate's latest report, if any.
    async fn source_signals(
        &self,
        candidate_id: &str,
        need: Option<&HiringNeedRecord>,
    ) -> Result<
        (
            Option<fsp_domain::GitHubSignal>,
            Option<crate::EmployerFspView>,
        ),
        ApplicationError,
    > {
        let sources = self
            .repository
            .sources(candidate_id)
            .await
            .map_err(ApplicationError::storage)?;
        let github = sources
            .iter()
            .filter(|source| source.provider == fsp_domain::SourceProvider::Github)
            .find_map(|source| {
                let analysis = source.analysis.as_ref()?;
                analysis
                    .report
                    .as_ref()
                    .map(|report| report.employer_signal(analysis.ownership.is_some()))
            });
        let fsp = sources
            .iter()
            .find(|source| source.provider == fsp_domain::SourceProvider::Fsp)
            .map(|source| {
                let published = source
                    .fsp_results
                    .iter()
                    .filter(|result| result.status == "published")
                    .collect::<Vec<_>>();
                crate::EmployerFspView {
                    verified_participation: source.verified_participation,
                    published_results: published.len(),
                    podium_results: published
                        .iter()
                        .filter(|result| result.place.is_some_and(|place| (1..=3).contains(&place)))
                        .count(),
                    relevant_results: published
                        .iter()
                        .filter(|result| {
                            need.is_some_and(|need| {
                                need.need.requirements.iter().any(|requirement| {
                                    let policy = &requirement.evidence_policy;
                                    policy.accept_fsp
                                        && (policy.competition_selection == "system"
                                            || policy
                                                .competition_ids
                                                .contains(&result.competition_id))
                                        && COMPETITIONS.iter().any(|(id, skills)| {
                                            *id == result.competition_id
                                                && skills.contains(&requirement.skill_id.as_str())
                                        })
                                })
                            })
                        })
                        .count(),
                    last_synced_at: source.last_synced_at.clone(),
                    synthetic: source.synthetic,
                }
            });
        Ok((github, fsp))
    }

    async fn freshness_signals(
        &self,
        candidate_id: &str,
        grade_confirmed_at: Option<&str>,
    ) -> Result<Vec<(String, &'static str)>, ApplicationError> {
        let mut signals: Vec<(String, &'static str)> = Vec::new();
        if let Some(at) = grade_confirmed_at {
            signals.push((at.to_string(), "assessment"));
        }
        for attempt in self
            .repository
            .candidate_assessments(candidate_id)
            .await
            .map_err(ApplicationError::storage)?
        {
            if attempt.status == fsp_domain::AssessmentStatus::Passed
                && attempt.task.execution == "questionnaire"
            {
                signals.push((attempt.updated_at, "assessment"));
            }
        }
        for item in self
            .repository
            .candidate_task_responses(candidate_id)
            .await
            .map_err(ApplicationError::storage)?
        {
            if item.status != fsp_domain::TaskResponseStatus::NeedsWork {
                signals.push((item.submitted_at, "task"));
            }
        }
        Ok(signals)
    }

    /// Mirrors the mock's `processing(candidate)` guard on data-processing
    /// operations (source connect/sync, assessments, task responses, …).
    async fn require_processing_consent(&self, candidate_id: &str) -> Result<(), ApplicationError> {
        let profile = self
            .repository
            .candidate(candidate_id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Кандидат не найден"))?;
        if !profile.consents.processing {
            return Err(ApplicationError::new(
                403,
                "processing_consent_required",
                "Для обработки данных необходимо согласие",
            ));
        }
        Ok(())
    }

    pub async fn job(
        &self,
        session: &SessionView,
        job_id: &str,
    ) -> Result<JobView, ApplicationError> {
        let owner_id = session
            .candidate_id
            .as_deref()
            .or(session.company_id.as_deref())
            .ok_or_else(|| ApplicationError::new(403, "role_forbidden", "Недоступная операция"))?;
        let record = self
            .repository
            .job(owner_id, job_id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Задача не найдена"))?;
        // Only a GitHub analysis outlives the request that started it; polling advances it.
        if record.kind == "source_sync"
            && record.status == "running"
            && let Some(candidate_id) = session.candidate_id.as_deref()
        {
            return self.advance_github_job(candidate_id, record).await;
        }
        let source = if let Some(source_id) = record.result_source_id.as_ref() {
            self.repository
                .sources(owner_id)
                .await
                .map_err(ApplicationError::storage)?
                .into_iter()
                .find(|source| &source.id == source_id)
                .map(|source| source_view(source, &self.github_ownership_code(owner_id), None))
        } else {
            None
        };
        Ok(job_view(record, source))
    }

    fn github_insights(&self) -> Result<&Arc<dyn crate::GitHubInsightsGateway>, ApplicationError> {
        self.github_insights.as_ref().ok_or_else(|| {
            ApplicationError::new(
                503,
                "github_not_configured",
                "Подключение GitHub временно недоступно",
            )
        })
    }

    fn github(&self) -> Result<&Arc<dyn GitHubGateway>, ApplicationError> {
        self.github.as_ref().ok_or_else(|| {
            ApplicationError::new(
                503,
                "github_not_configured",
                "Подключение GitHub временно недоступно",
            )
        })
    }

    /// Account + role profile creation; takes the unit-of-work handle so the
    /// caller's transaction owns the write. `email` is stored normalized.
    async fn register_account(
        &self,
        repo: &dyn PlatformRepository,
        email: String,
        role: Role,
    ) -> Result<Account, ApplicationError> {
        let suffix = Uuid::new_v4().simple().to_string();
        let account = Account {
            id: format!("actor-{}", &suffix[..12]),
            email: email.clone(),
            role,
            candidate_id: (role == Role::Candidate).then(|| format!("candidate-{}", &suffix[..12])),
            company_id: (role == Role::Employer).then(|| format!("company-{}", &suffix[..12])),
            email_verified: false,
        };
        // Referenced rows first: accounts carry FKs to candidates/companies.
        if let Some(candidate_id) = &account.candidate_id {
            repo.create_candidate(&CandidateProfile {
                id: candidate_id.clone(),
                alias: format!("Кандидат №{}", suffix[..3].to_uppercase()),
                email: email.clone(),
                name: fsp_domain::DEFAULT_CANDIDATE_NAME.into(),
                specialization: Specialization::Backend,
                selected_grade: Grade::Junior,
                stack: Vec::new(),
                team_role: String::new(),
                experience_months: None,
                experience_since: None,
                experience: String::new(),
                soft_skills: Vec::new(),
                consents: Consents {
                    processing: false,
                    publication: false,
                },
                verified_grade: None,
                verified_specialization: None,
                grade_confirmed_at: None,
                estimate: None,
                auto_fields: Vec::new(),
                revision: 1,
                updated_at: now(),
            })
            .await
            .map_err(ApplicationError::storage)?;
        }
        if let Some(company_id) = &account.company_id {
            repo.create_company(&Company {
                id: company_id.clone(),
                display_name: "Новая компания".into(),
                business_area: String::new(),
                description: String::new(),
                contact_email: email.clone(),
                website: None,
            })
            .await
            .map_err(ApplicationError::storage)?;
        }
        repo.create_account(&account, &now())
            .await
            .map_err(ApplicationError::storage)?;
        Ok(account)
    }

    async fn session_view(&self, account: &Account) -> Result<SessionView, ApplicationError> {
        let display_name = if let Some(id) = &account.candidate_id {
            self.repository
                .candidate(id)
                .await
                .map_err(ApplicationError::storage)?
                .map(|profile| profile.name)
                .unwrap_or_else(|| "Кандидат".into())
        } else if let Some(id) = &account.company_id {
            self.repository
                .company(id)
                .await
                .map_err(ApplicationError::storage)?
                .map(|company| company.display_name)
                .unwrap_or_else(|| "Работодатель".into())
        } else {
            String::new()
        };
        Ok(SessionView {
            actor_id: account.id.clone(),
            role: account.role,
            email_verified: account.email_verified,
            candidate_id: account.candidate_id.clone(),
            company_id: account.company_id.clone(),
            display_name,
        })
    }
}

fn employer_company_id(session: &SessionView) -> Result<&str, ApplicationError> {
    session.company_id.as_deref().ok_or_else(|| {
        ApplicationError::new(403, "role_forbidden", "Действие доступно работодателю")
    })
}

fn candidate_id(session: &SessionView) -> Result<&str, ApplicationError> {
    session
        .candidate_id
        .as_deref()
        .ok_or_else(|| ApplicationError::new(403, "role_forbidden", "Действие доступно кандидату"))
}

fn integration_error(error: IntegrationError) -> ApplicationError {
    let status = match error.code {
        "github_login_invalid" | "github_account_not_found" => 422,
        "github_rate_limited" | "github_not_configured" => 503,
        "interview_not_found" => 404,
        "interview_conflict" => 409,
        "interview_input_invalid" | "speech_input_invalid" => 422,
        "interview_model_unavailable" | "interview_service_unavailable" => 503,
        "speech_unavailable" | "speech_not_configured" => 503,
        _ => 502,
    };
    ApplicationError::new(status, error.code, error.message)
}

fn interviews_unavailable() -> ApplicationError {
    ApplicationError::new(
        503,
        "interview_service_unavailable",
        "Сервис собеседований временно недоступен",
    )
}

fn source_provider(provider: fsp_domain::SourceProvider) -> &'static str {
    match provider {
        fsp_domain::SourceProvider::Github => "github",
        fsp_domain::SourceProvider::Fsp => "fsp",
    }
}

fn source_name(source: fsp_domain::EvidenceSource) -> &'static str {
    match source {
        fsp_domain::EvidenceSource::Assessment => "assessment",
        fsp_domain::EvidenceSource::Github => "github",
        fsp_domain::EvidenceSource::Fsp => "fsp",
    }
}

fn evidence_status_name(status: EvidenceStatus) -> &'static str {
    match status {
        EvidenceStatus::Confirmed => "confirmed",
        EvidenceStatus::Evidence => "evidence",
        EvidenceStatus::Observed => "observed",
        EvidenceStatus::Insufficient => "insufficient",
    }
}

fn need_view(record: HiringNeedRecord) -> NeedView {
    NeedView {
        id: record.id,
        company_id: record.company_id,
        title: record.title,
        specialization: record.need.specialization,
        grade: record.need.grade,
        work_mode: record.work_mode,
        salary: SalaryView {
            from: record.salary_from,
            to: record.salary_to,
            currency: "RUB",
            period: "month",
            tax_basis: "gross",
        },
        description: record.description,
        requirements: record
            .need
            .requirements
            .into_iter()
            .map(|item| RequirementView {
                skill_id: item.skill_id,
                criticality: item.criticality,
                minimum_grade: item.minimum_grade,
                evidence_policy: EvidencePolicyView::from(&item.evidence_policy),
            })
            .collect(),
        published: record.published,
        published_at: record.published_at,
        created_at: record.created_at,
        updated_at: record.updated_at,
        revision: record.revision,
    }
}

fn invitation_view(record: fsp_domain::InvitationRecord) -> InvitationView {
    InvitationView {
        id: record.id,
        candidate_id: record.candidate_id,
        candidate_alias: record.candidate_alias,
        company: CompanyCardView {
            id: record.company.id,
            display_name: record.company.display_name,
            business_area: record.company.business_area,
            description: record.company.description,
            contact_email: record.company.contact_email,
        },
        need: InvitationNeedView {
            id: record.need.id,
            title: record.need.title,
            work_mode: record.need.work_mode,
            salary: SalaryView {
                from: record.need.salary_from,
                to: record.need.salary_to,
                currency: "RUB",
                period: "month",
                tax_basis: "gross",
            },
            description: record.need.description,
        },
        message: record.message,
        status: record.status,
        created_at: record.created_at,
        viewed_at: record.viewed_at,
        responded_at: record.responded_at,
        disclosure_fields: record.disclosure_fields,
    }
}

/// The contract's `disclose` array: at least one field, unique, from the
/// disclosure enum.
fn validate_disclose(disclose: &[String]) -> Result<(), ApplicationError> {
    const FIELDS: [&str; 3] = ["name", "email", "sourceLinks"];
    let unique = disclose.iter().collect::<std::collections::BTreeSet<_>>();
    if disclose.is_empty()
        || unique.len() != disclose.len()
        || disclose
            .iter()
            .any(|field| !FIELDS.contains(&field.as_str()))
    {
        return Err(validation("Запрос не соответствует контракту"));
    }
    Ok(())
}

fn anonymous_candidate(
    record: CandidateIndexRecord,
    need: Option<&HiringNeedRecord>,
    signals: &[(String, &'static str)],
    github: Option<fsp_domain::GitHubSignal>,
    fsp: Option<crate::EmployerFspView>,
) -> AnonymousCandidateView {
    let rankable = record.rankable().expect("visible candidate is rankable");
    let category = record
        .profile
        .category()
        .expect("visible candidate has a category");
    let candidate_match = need.map(|selected| match_view(&rankable, selected));
    let estimate = (category.source == fsp_domain::CategorySource::Estimate)
        .then_some(record.profile.estimate.as_ref())
        .flatten()
        .map(|estimate| EmployerEstimateView {
            score: estimate.score,
            computed_at: estimate.computed_at.clone(),
            factors: estimate
                .factors
                .iter()
                .map(|factor| EmployerEstimateFactorView {
                    id: factor.id.clone(),
                    label: factor.label.clone(),
                    points: factor.points,
                    max: factor.max,
                })
                .collect(),
        });
    let review = record.profile.estimate.as_ref().and_then(|estimate| {
        estimate
            .review
            .as_ref()
            .map(|review| crate::EmployerReviewView {
                specialization: review.specialization,
                grade: review.grade,
                agrees: review.agrees,
                computed_at: estimate.computed_at.clone(),
            })
    });
    AnonymousCandidateView {
        id: record.profile.id,
        alias: record.profile.alias,
        specialization: rankable.specialization,
        grade: rankable.grade,
        category_source: category.source,
        category_confidence: category.confidence,
        verified_grade: record.profile.verified_grade,
        grade_confirmed_at: record.profile.grade_confirmed_at,
        estimate,
        work_modes: record.work_modes,
        available_for_offers: record.available_for_offers,
        skills: record
            .skills
            .into_iter()
            .map(|skill| SkillView {
                label: skill_label(&skill.skill_id).into(),
                summary: skill_summary(skill.status).into(),
                evidence_count: skill.sources.len(),
                confirmed_at: (skill.status == EvidenceStatus::Confirmed)
                    .then(|| record.profile.updated_at.clone()),
                skill_id: skill.skill_id,
                status: skill.status,
                evidence_grade: skill.evidence_grade,
                evidence_sources: skill.sources,
            })
            .collect(),
        source_kinds: record.source_kinds,
        fsp_verified: record.fsp_verified,
        freshness: freshness_view(signals),
        r#match: candidate_match,
        github,
        fsp,
        review,
        simulation: true,
    }
}

fn match_view(
    candidate: &fsp_domain::RankableCandidate,
    need: &HiringNeedRecord,
) -> CandidateMatchView {
    let result = score(candidate, &need.need);
    let requirements = need
        .need
        .requirements
        .iter()
        .map(|requirement| match_requirement(&candidate.skills, requirement))
        .collect::<Vec<_>>();
    let status = match_status(&result, &requirements);
    CandidateMatchView {
        need_id: need.id.clone(),
        status,
        grade_matches: result.grade_matches,
        required_met: result.required_met,
        required_total: result.required_total,
        desired_met: result.desired_met,
        desired_total: result.desired_total,
        required_evidenced: result.required_evidenced,
        desired_evidenced: result.desired_evidenced,
        requirements,
        explanation: if result.grade_matches {
            format!(
                "Подтверждено обязательных требований: {} из {}.",
                result.required_met, result.required_total
            )
        } else {
            "Специализация или грейд категории пока не соответствуют потребности.".into()
        },
        algorithm_version: result.algorithm_version,
    }
}

fn match_requirement(
    skills: &[fsp_domain::SkillEvidence],
    requirement: &Requirement,
) -> MatchRequirementView {
    let evidence = fsp_domain::requirement_evidence(skills, requirement);
    let status = if evidence.meets {
        "meets"
    } else if evidence.status == EvidenceStatus::Insufficient {
        "not_evidenced"
    } else {
        "needs_verification"
    };
    MatchRequirementView {
        skill_id: requirement.skill_id.clone(),
        criticality: requirement.criticality,
        status,
        evidence_status: evidence.status,
        minimum_grade: requirement.minimum_grade,
        evidence_grade: evidence.grade,
        reason: if evidence.meets {
            "Навык подтверждён проверкой нужного уровня, которую принимает работодатель."
        } else if status == "not_evidenced" {
            "Пока нет достаточных свидетельств по этому навыку."
        } else if evidence.accepted_sources.is_empty() {
            "Имеющиеся источники не соответствуют политике этой потребности."
        } else {
            "Есть основания, но уровень по этому требованию ещё следует подтвердить."
        },
        evidence_sources: evidence.sources,
        accepted_sources: evidence.accepted_sources,
    }
}

fn skill_label(skill_id: &str) -> &str {
    fsp_domain::catalog::skill_label(skill_id)
}

fn skill_summary(status: EvidenceStatus) -> &'static str {
    match status {
        EvidenceStatus::Confirmed => "Навык подтверждён проверяемым свидетельством.",
        EvidenceStatus::Evidence => "Есть свидетельства, требуется подтверждение уровня.",
        EvidenceStatus::Observed => "Навык обнаружен в подключённых источниках.",
        EvidenceStatus::Insufficient => "Недостаточно данных для подтверждения.",
    }
}

/// The mock's `FRESHNESS_DAYS`: a verified grade or a counted task answer keeps a
/// profile fresh for this window.
const FRESHNESS_DAYS: i64 = 90;

/// The mock's `taskPeriod`: ISO weeks anchored to Monday 1970-01-05 UTC.
const EPOCH_MONDAY_MS: i64 = 345_600_000;
const WEEK_MS: i64 = 7 * 86_400_000;

fn task_period(now: chrono::DateTime<Utc>) -> (i64, chrono::DateTime<Utc>, chrono::DateTime<Utc>) {
    let index = (now.timestamp_millis() - EPOCH_MONDAY_MS).div_euclid(WEEK_MS);
    let from = chrono::DateTime::from_timestamp_millis(EPOCH_MONDAY_MS + index * WEEK_MS)
        .unwrap_or_default();
    (index, from, from + Duration::milliseconds(WEEK_MS))
}

/// RFC3339 parsing tolerant of both `Z` and `+00:00` suffixes (seeds use
/// `Z`, runtime stamps use `to_rfc3339`).
fn timestamp_at(value: &str) -> Option<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|at| at.with_timezone(&Utc))
}

/// Internal result of `task_offer`: the task card, this week's response if
/// the week is already answered, or the reason nothing is offered.
struct TaskOffer {
    current: Option<fsp_domain::OfferedTaskSnapshot>,
    current_response: Option<fsp_domain::TaskResponseRecord>,
    reason: Option<&'static str>,
}

fn offered_task_view(snapshot: &fsp_domain::OfferedTaskSnapshot) -> OfferedTaskView {
    OfferedTaskView {
        id: snapshot.id.clone(),
        company: OfferedCompanyView {
            id: snapshot.company_id.clone(),
            display_name: snapshot.company_display_name.clone(),
        },
        title: snapshot.title.clone(),
        prompt: snapshot.prompt.clone(),
        skill_id: snapshot.skill_id.clone(),
        grade: snapshot.grade,
        estimate_minutes: snapshot.estimate_minutes,
    }
}

fn task_response_view(record: &fsp_domain::TaskResponseRecord) -> TaskResponseView {
    TaskResponseView {
        id: record.id.clone(),
        candidate_id: record.candidate_id.clone(),
        candidate_alias: record.candidate_alias.clone(),
        task: offered_task_view(&record.task),
        mode: record.mode,
        answer: record.answer.clone(),
        status: record.status,
        feedback: record.feedback.clone(),
        submitted_at: record.submitted_at.clone(),
        reviewed_at: record.reviewed_at.clone(),
    }
}

fn task_view(record: &fsp_domain::TaskRecord, response_count: usize) -> TaskView {
    TaskView {
        id: record.id.clone(),
        title: record.title.clone(),
        prompt: record.prompt.clone(),
        skill_id: record.skill_id.clone(),
        grade: record.grade,
        estimate_minutes: record.estimate_minutes,
        status: record.status,
        created_at: record.created_at.clone(),
        response_count,
    }
}

/// The mock's `mutableAssessment`: only drafts and in-progress attempts
/// accept edits and new runs.
fn mutable_assessment(record: &fsp_domain::AssessmentRecord) -> Result<(), ApplicationError> {
    if record.task.execution != "questionnaire" {
        return Err(ApplicationError::new(
            422,
            "assessment_format_retired",
            "Эта демонстрационная задача архивирована. Начните новый тест знаний",
        ));
    }
    if !matches!(
        record.status,
        fsp_domain::AssessmentStatus::Draft | fsp_domain::AssessmentStatus::InProgress
    ) {
        return Err(ApplicationError::new(
            409,
            "assessment_not_editable",
            "Проверка уже отправлена или завершена",
        ));
    }
    Ok(())
}

fn assessment_view(record: &fsp_domain::AssessmentRecord) -> AssessmentView {
    AssessmentView {
        id: record.id.clone(),
        specialization: record.specialization,
        grade: record.grade,
        status: record.status,
        task: record.task.clone(),
        answer: record.answer.clone(),
        checks: record.checks.clone(),
        result: record.result.clone(),
        created_at: record.created_at.clone(),
        updated_at: record.updated_at.clone(),
        revision: record.revision,
        simulation: record.task.execution != "questionnaire",
    }
}

/// The mock's `COMPETITIONS` fixture — id and accepted skills.
const COMPETITIONS: [(&str, &[&str]); 2] = [
    ("competition-demo-api", &["rust", "postgresql"]),
    ("competition-demo-systems", &["docker", "redis"]),
];

/// The mock's `gapPlan`: every unmet requirement gets an action, the FSP
/// competitions the policy accepts and whether an assessment can close it.
fn gap_plan(candidate: &CandidateIndexRecord, need: &HiringNeedRecord) -> GapPlanView {
    let items = need
        .need
        .requirements
        .iter()
        .filter_map(|requirement| {
            let item = match_requirement(&candidate.skills, requirement);
            if item.status == "meets" {
                return None;
            }
            let policy = &requirement.evidence_policy;
            let accepted_competition_ids = if policy.accept_fsp {
                COMPETITIONS
                    .iter()
                    .filter(|(_, skills)| skills.contains(&item.skill_id.as_str()))
                    .filter(|(id, _)| {
                        policy.competition_selection == "system"
                            || policy.competition_ids.iter().any(|listed| listed == id)
                    })
                    .map(|(id, _)| (*id).to_owned())
                    .collect()
            } else {
                Vec::new()
            };
            Some(GapItemView {
                skill_id: item.skill_id,
                status: item.status,
                reason: item.reason,
                action: if policy.accept_assessments {
                    "Пройдите тест знаний по навыку нужного уровня."
                } else {
                    "Предоставьте свидетельство из источников, которые принимает работодатель."
                },
                accepted_competition_ids,
                assessment_available: policy.accept_assessments
                    && fsp_domain::knowledge_available(need.need.specialization, need.need.grade),
            })
        })
        .collect();
    GapPlanView {
        candidate_id: candidate.profile.id.clone(),
        need_id: need.id.clone(),
        items,
        simulation: true,
    }
}

/// The mock's `freshness()`: the latest verified-grade or counted task
/// signal decides `lastSignal`; `assessment` sorts first on ties because the
/// mock pushes it before task signals and uses a stable sort.
fn freshness_view(signals: &[(String, &'static str)]) -> FreshnessView {
    let mut ordered = signals.to_vec();
    ordered.sort_by(|a, b| b.0.cmp(&a.0));
    let latest = ordered.first();
    let fresh = latest
        .and_then(|(at, _)| chrono::DateTime::parse_from_rfc3339(at).ok())
        .is_some_and(|at| Utc::now() - at.with_timezone(&Utc) <= Duration::days(FRESHNESS_DAYS));
    FreshnessView {
        last_signal: latest.map(|(_, kind)| *kind),
        last_signal_at: latest.map(|(at, _)| at.clone()),
        fresh,
        window_days: FRESHNESS_DAYS as u16,
    }
}

fn profile_view(
    profile: CandidateProfile,
    email_verified: bool,
    signals: &[(String, &'static str)],
    assessment_running: bool,
    cooldown_days: u16,
) -> ProfileView {
    let questionnaire_complete = profile.questionnaire_complete();
    let assessment_passed =
        profile.verified_grade.is_some() && profile.verified_specialization.is_some();
    let category = profile.category();
    let visible = profile.visible_to_employers(email_verified);
    let mut blockers = Vec::new();
    if !email_verified {
        blockers.push("email");
    }
    if !questionnaire_complete {
        blockers.push("questionnaire");
    }
    if category.is_none() {
        blockers.push("category");
    }
    if !profile.consents.processing {
        blockers.push("processing_consent");
    }
    if !profile.consents.publication {
        blockers.push("publication_consent");
    }
    let processing_consent = profile.consents.processing;
    // Mirrors the mock's eligibility(): onboarding gates starting any check; a
    // successful verification freezes upgrades for the cooldown window; the
    // The selected scope additionally rejects unavailable assessment banks and downgrades.
    let onboarding_incomplete = !(email_verified && questionnaire_complete && processing_consent);
    let next_eligible_at = profile
        .grade_confirmed_at
        .as_deref()
        .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
        .map(|at| (at + Duration::days(i64::from(cooldown_days))).to_rfc3339());
    let cooling = next_eligible_at
        .as_deref()
        .is_some_and(|at| at > Utc::now().to_rfc3339().as_str());
    let same_grade = profile.verified_grade == Some(profile.selected_grade)
        && profile.verified_specialization == Some(profile.specialization);
    let reason = if onboarding_incomplete {
        Some("onboarding_incomplete")
    } else if assessment_running {
        Some("active_assessment")
    } else if cooling {
        Some("cooldown")
    } else {
        None
    };
    let selected_reason = if onboarding_incomplete || assessment_running {
        Some(if onboarding_incomplete {
            "onboarding_incomplete"
        } else {
            "active_assessment"
        })
    } else if !fsp_domain::knowledge_available(profile.specialization, profile.selected_grade) {
        Some("unsupported_scope")
    } else if profile.verified_specialization == Some(profile.specialization)
        && profile
            .verified_grade
            .is_some_and(|grade| profile.selected_grade < grade)
    {
        Some("grade_downgrade")
    } else if cooling && !same_grade {
        Some("cooldown")
    } else {
        None
    };
    ProfileView {
        id: profile.id,
        alias: profile.alias,
        email: profile.email,
        email_verified,
        name: profile.name,
        industry: "it",
        specialization: profile.specialization,
        selected_grade: profile.selected_grade,
        stack: profile.stack,
        team_role: profile.team_role,
        // A start year counts up to the current year whenever the profile is read.
        experience_months: profile
            .experience_since
            .map(|since| fsp_domain::experience_since_months(since, current_year()))
            .or(profile.experience_months),
        experience_since: profile.experience_since,
        experience: profile.experience,
        soft_skills: profile.soft_skills,
        consents: profile.consents,
        verified_grade: profile.verified_grade,
        verified_specialization: profile.verified_specialization,
        grade_confirmed_at: profile.grade_confirmed_at.clone(),
        category: CategoryView {
            specialization: category.map(|item| item.specialization),
            grade: category.map(|item| item.grade),
            eligible: visible,
            source: category.map(|item| item.source),
            confidence: category.and_then(|item| item.confidence),
        },
        estimate: profile.estimate,
        auto_fields: profile.auto_fields,
        freshness: freshness_view(signals),
        onboarding: OnboardingView {
            email_verified,
            questionnaire_complete,
            assessment_passed,
            ready_for_publication: email_verified
                && questionnaire_complete
                && category.is_some()
                && processing_consent,
            visible_to_employers: visible,
            blockers,
        },
        assessment_eligibility: GradeEligibilityView {
            selected_specialization: profile.specialization,
            selected_grade: profile.selected_grade,
            can_start_selected: selected_reason.is_none(),
            selected_reason,
            can_start_upgrade: reason.is_none(),
            next_eligible_at,
            cooldown_days,
            reason,
        },
        revision: profile.revision,
        updated_at: profile.updated_at,
    }
}

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

/// The current calendar year: experience stated as a start year counts up to it.
fn current_year() -> i32 {
    Utc::now().year()
}

fn unique(values: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        if !result.contains(&value) {
            result.push(value);
        }
    }
    result
}

/// Applies a validated `ProfilePatch`, like the mock's `patchProfile`. The
/// publication consent cannot outlive the processing consent. An answer the
/// candidate changes is theirs from then on: estimates no longer overwrite it.
fn apply_profile_patch(
    profile: &mut CandidateProfile,
    patch: ProfilePatch,
) -> Result<(), ApplicationError> {
    if let Some(name) = patch.name {
        let name = name.trim().to_owned();
        claim_field(profile, "name", profile.name != name);
        profile.name = name;
    }
    if let Some(value) = patch.specialization {
        claim_field(profile, "specialization", profile.specialization != value);
        profile.specialization = value;
    }
    if let Some(value) = patch.selected_grade {
        claim_field(profile, "selectedGrade", profile.selected_grade != value);
        profile.selected_grade = value;
    }
    if let Some(value) = patch.stack {
        let value = unique(value);
        claim_field(profile, "stack", profile.stack != value);
        profile.stack = value;
    }
    if let Some(value) = patch.team_role {
        let value = value.trim().to_owned();
        claim_field(profile, "teamRole", profile.team_role != value);
        profile.team_role = value;
    }
    // A start year is the candidate's statement and the months follow from it; without a year,
    // explicit months (typed or from a résumé) count; clearing the year clears the experience.
    match (patch.experience_since, patch.experience_months) {
        (Some(Some(year)), _) => {
            claim_field(profile, "experienceMonths", true);
            profile.experience_since = Some(year);
            profile.experience_months =
                Some(fsp_domain::experience_since_months(year, current_year()));
        }
        (_, Some(value)) => {
            claim_field(
                profile,
                "experienceMonths",
                profile.experience_months != value || profile.experience_since.is_some(),
            );
            profile.experience_since = None;
            profile.experience_months = value;
        }
        (Some(None), None) => {
            claim_field(profile, "experienceMonths", true);
            profile.experience_since = None;
            profile.experience_months = None;
        }
        (None, None) => {}
    }
    if let Some(value) = patch.experience {
        claim_field(profile, "experience", profile.experience != value);
        profile.experience = value;
    }
    if let Some(value) = patch.soft_skills {
        let value = unique(value);
        claim_field(profile, "softSkills", profile.soft_skills != value);
        profile.soft_skills = value;
    }
    if let Some(value) = patch.consents {
        profile.consents = value;
    }
    if profile.consents.publication && !profile.consents.processing {
        return Err(ApplicationError::new(
            403,
            "processing_consent_required",
            "Для обработки данных необходимо согласие",
        ));
    }
    Ok(())
}

fn claim_field(profile: &mut CandidateProfile, field: &str, changed: bool) {
    if changed {
        profile.auto_fields.retain(|item| item != field);
    }
}

fn validate_profile_patch(patch: &ProfilePatch) -> Result<(), ApplicationError> {
    let has_property = patch.name.is_some()
        || patch.industry.is_some()
        || patch.specialization.is_some()
        || patch.selected_grade.is_some()
        || patch.stack.is_some()
        || patch.team_role.is_some()
        || patch.experience_months.is_some()
        || patch.experience_since.is_some()
        || patch.experience.is_some()
        || patch.soft_skills.is_some()
        || patch.consents.is_some();
    if !has_property {
        return Err(validation("Передайте хотя бы одно поле профиля"));
    }
    if patch
        .name
        .as_ref()
        .is_some_and(|value| value.trim().is_empty() || value.chars().count() > 120)
    {
        return Err(validation("Имя должно содержать от 1 до 120 символов"));
    }
    if patch.industry.as_deref().is_some_and(|value| value != "it") {
        return Err(validation("Поддерживается только ИТ-специализация"));
    }
    if patch
        .experience_months
        .is_some_and(|value| value.is_some_and(|months| months > 1200))
    {
        return Err(validation("Опыт не может превышать 1200 месяцев"));
    }
    if patch.experience_since.is_some_and(|value| {
        value.is_some_and(|year| !(1950..=current_year()).contains(&i32::from(year)))
    }) {
        return Err(validation("Год начала в IT — от 1950 до текущего года"));
    }
    if patch
        .team_role
        .as_ref()
        .is_some_and(|value| value.chars().count() > 120)
        || patch
            .experience
            .as_ref()
            .is_some_and(|value| value.chars().count() > 4000)
    {
        return Err(validation("Текстовое поле превышает допустимую длину"));
    }
    if let Some(stack) = &patch.stack
        && (stack.len() > MAX_STACK
            || stack
                .iter()
                .any(|value| !fsp_domain::catalog::is_skill(value))
            || unique_count(stack) != stack.len())
    {
        return Err(validation("Проверьте технологии в стеке"));
    }
    if patch.soft_skills.as_ref().is_some_and(|values| {
        values.len() > 12
            || values.iter().any(|value| value.chars().count() > 100)
            || unique_count(values) != values.len()
    }) {
        return Err(validation("Проверьте список гибких навыков"));
    }
    Ok(())
}

fn validate_company_patch(patch: &CompanyPatch) -> Result<(), ApplicationError> {
    let has_property = patch.display_name.is_some()
        || patch.business_area.is_some()
        || patch.description.is_some()
        || patch.contact_email.is_some()
        || patch.website.is_some();
    if !has_property {
        return Err(validation("Передайте хотя бы одно поле компании"));
    }
    if patch
        .display_name
        .as_ref()
        .is_some_and(|value| value.is_empty() || value.chars().count() > 120)
        || patch
            .business_area
            .as_ref()
            .is_some_and(|value| value.chars().count() > 200)
        || patch
            .description
            .as_ref()
            .is_some_and(|value| value.chars().count() > 4000)
    {
        return Err(validation("Проверьте длину полей компании"));
    }
    if patch
        .contact_email
        .as_deref()
        .is_some_and(|value| !valid_email(value))
    {
        return Err(validation("Проверьте адрес электронной почты"));
    }
    if let Some(Some(value)) = &patch.website {
        let valid = value.len() <= 2048
            && (value.starts_with("https://") || value.starts_with("http://"))
            && !value
                .chars()
                .any(|ch| ch.is_whitespace() || ch.is_control())
            && value
                .split_once("://")
                .is_some_and(|(_, authority)| !authority.starts_with('/'))
            && url::Url::parse(value).is_ok_and(|url| url.has_host());
        if !valid {
            return Err(validation("Укажите корректный HTTP(S)-адрес сайта"));
        }
    }
    Ok(())
}

// Matches the ASCII dot-atom address format used by the contract's Ajv validator.
fn valid_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    value.len() <= 254
        && local.split('.').all(|atom| {
            !atom.is_empty()
                && atom
                    .bytes()
                    .all(|ch| ch.is_ascii_alphanumeric() || b"!#$%&'*+/=?^_`{|}~-".contains(&ch))
        })
        && domain.contains('.')
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                && label
                    .bytes()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == b'-')
        })
}

fn unique_count(values: &[String]) -> usize {
    let mut deduplicated = values.to_vec();
    deduplicated.sort();
    deduplicated.dedup();
    deduplicated.len()
}

fn validation(message: &'static str) -> ApplicationError {
    ApplicationError::new(400, "validation_error", message)
}

/// Technologies a profile lists; evidence fills them automatically, so a broad stack fits.
const MAX_STACK: usize = 12;
const WORK_MODES: &[&str] = &["remote", "hybrid", "office"];
/// Competition ids the meta catalog advertises.
const COMPETITION_IDS: &[&str] = &["competition-demo-api", "competition-demo-systems"];

/// Contract schema checks the mock performs through ajv: lengths, ranges and
/// enumerated values map to `validation_error`.
fn validate_need_schema(
    title: &str,
    work_mode: &str,
    description: &str,
    requirements: &[fsp_domain::Requirement],
) -> Result<(), ApplicationError> {
    if !(1..=160).contains(&title.chars().count())
        || description.chars().count() > 6000
        || requirements.is_empty()
        || requirements.len() > 5
        || !WORK_MODES.contains(&work_mode)
    {
        return Err(validation("Запрос не соответствует контракту"));
    }
    validate_requirements_schema(requirements)
}

fn validate_requirements_schema(
    requirements: &[fsp_domain::Requirement],
) -> Result<(), ApplicationError> {
    let invalid = requirements.iter().any(|item| {
        !fsp_domain::catalog::is_skill(&item.skill_id)
            || !matches!(
                item.evidence_policy.competition_selection.as_str(),
                "system" | "selected"
            )
            || item
                .evidence_policy
                .competition_ids
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != item.evidence_policy.competition_ids.len()
    });
    if invalid {
        return Err(validation("Запрос не соответствует контракту"));
    }
    Ok(())
}

/// The mock's `validateNeed` business rules, in the same order.
fn validate_need_rules(
    salary_from: u64,
    salary_to: u64,
    requirements: &[fsp_domain::Requirement],
) -> Result<(), ApplicationError> {
    if salary_from == 0 || salary_to == 0 || salary_from > 100_000_000 || salary_to > 100_000_000 {
        return Err(validation("Запрос не соответствует контракту"));
    }
    if salary_from > salary_to {
        return Err(ApplicationError::new(
            422,
            "salary_range_invalid",
            "Минимальная зарплата не может превышать максимальную",
        ));
    }
    let unique = requirements
        .iter()
        .map(|item| &item.skill_id)
        .collect::<std::collections::HashSet<_>>();
    if unique.len() != requirements.len() {
        return Err(ApplicationError::new(
            422,
            "duplicate_skill",
            "Навык нельзя повторять в требованиях",
        ));
    }
    if !requirements
        .iter()
        .any(|item| item.criticality == fsp_domain::Criticality::Required)
    {
        return Err(ApplicationError::new(
            422,
            "required_skill_missing",
            "Укажите хотя бы одно обязательное требование",
        ));
    }
    for requirement in requirements {
        let policy = &requirement.evidence_policy;
        if !policy.accept_assessments && !policy.accept_github && !policy.accept_fsp {
            return Err(ApplicationError::new(
                422,
                "empty_evidence_policy",
                "Выберите хотя бы один источник свидетельств",
            ));
        }
        if policy
            .competition_ids
            .iter()
            .any(|id| !COMPETITION_IDS.contains(&id.as_str()))
        {
            return Err(ApplicationError::new(
                422,
                "unknown_competition",
                "Неизвестное соревнование",
            ));
        }
        if policy.competition_selection == "system" && !policy.competition_ids.is_empty() {
            return Err(ApplicationError::new(
                422,
                "ambiguous_competition_policy",
                "При системном выборе список соревнований должен быть пуст",
            ));
        }
        if policy.accept_fsp
            && policy.competition_selection == "selected"
            && policy.competition_ids.is_empty()
        {
            return Err(ApplicationError::new(
                422,
                "competition_required",
                "Укажите принимаемые соревнования",
            ));
        }
    }
    Ok(())
}
