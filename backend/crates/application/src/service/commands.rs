//! Commands whose effects and replay response share one database transaction.
use super::*;

/// Available only inside `PlatformService::idempotent`. Its repository is
/// bound to the caller's unit of work; commands cannot independently commit.
pub struct Commands {
    service: PlatformService,
}

impl PlatformService {
    /// Execute the command and store its replay response in one unit of work.
    /// Failed commands and serialization/storage errors roll back every effect.
    pub async fn idempotent<T, F, Fut>(
        &self,
        session: &SessionView,
        context: IdempotencyContext<'_>,
        run: F,
    ) -> Result<Idempotent<T>, ApplicationError>
    where
        T: serde::Serialize,
        F: FnOnce(Commands) -> Fut,
        Fut: std::future::Future<Output = Result<T, ApplicationError>>,
    {
        let IdempotencyContext {
            operation,
            path,
            key,
            body,
            status,
        } = context;
        let scope = format!("{}:{}:{}:{}", session.actor_id, operation, path, key);
        let fingerprint = serde_json::to_string(body).map_err(|_| {
            ApplicationError::new(500, "internal_error", "Сервис временно недоступен")
        })?;
        let tx = self
            .repository
            .transaction()
            .await
            .map_err(ApplicationError::storage)?;
        match tx
            .idempotency_claim(&scope, &fingerprint)
            .await
            .map_err(ApplicationError::storage)?
        {
            IdempotencyClaim::Execute => {
                let commands = Commands {
                    service: PlatformService {
                        repository: tx.share(),
                        ..self.clone()
                    },
                };
                let value = run(commands).await?;
                let stored_body = serde_json::to_string(&value).map_err(|_| {
                    ApplicationError::new(500, "internal_error", "Сервис временно недоступен")
                })?;
                tx.idempotency_finish(&scope, status, &stored_body)
                    .await
                    .map_err(ApplicationError::storage)?;
                tx.commit().await.map_err(ApplicationError::storage)?;
                Ok(Idempotent::Fresh(value))
            }
            IdempotencyClaim::Stored(stored) => {
                if stored.fingerprint != fingerprint {
                    return Err(ApplicationError::new(
                        409,
                        "idempotency_conflict",
                        "Ключ уже использован с другим запросом",
                    ));
                }
                Ok(Idempotent::Replayed {
                    status: stored.status,
                    body: stored.body,
                })
            }
            IdempotencyClaim::InFlight => Err(ApplicationError::new(
                500,
                "internal_error",
                "Сервис временно недоступен",
            )),
        }
    }
}

impl Commands {
    /// The mock's `createInvitation`, run inside `idempotent` by the caller.
    pub async fn create_invitation(
        &self,
        session: &SessionView,
        input: InvitationInput,
    ) -> Result<InvitationView, ApplicationError> {
        if !(1..=4000).contains(&input.message.chars().count()) {
            return Err(validation("Запрос не соответствует контракту"));
        }
        let company = self.service.require_invitable_employer(session).await?;
        let need = self.service.own_need(session, &input.need_id).await?;
        validate_need_rules(need.salary_from, need.salary_to, &need.need.requirements)?;
        let candidate = self.service.visible_candidate(&input.candidate_id).await?;
        let duplicate = self
            .service
            .repository
            .invitations(&company.id)
            .await
            .map_err(ApplicationError::storage)?
            .into_iter()
            .any(|item| {
                item.candidate_id == candidate.profile.id
                    && item.need.id == need.id
                    && matches!(
                        item.status,
                        InteractionStatus::Sent
                            | InteractionStatus::Viewed
                            | InteractionStatus::Accepted
                    )
            });
        if duplicate {
            return Err(ApplicationError::new(
                409,
                "invitation_exists",
                "Приглашение по этой потребности уже существует",
            ));
        }
        let timestamp = now();
        let record = fsp_domain::InvitationRecord {
            id: format!("invitation-{}", Uuid::new_v4().simple()),
            company_id: company.id.clone(),
            candidate_id: candidate.profile.id.clone(),
            candidate_alias: candidate.profile.alias.clone(),
            company: fsp_domain::CompanyCard {
                id: company.id,
                display_name: company.display_name,
                business_area: company.business_area,
                description: company.description,
                contact_email: company.contact_email,
            },
            need: fsp_domain::NeedSnapshot {
                id: need.id.clone(),
                title: need.title.clone(),
                work_mode: need.work_mode.clone(),
                salary_from: need.salary_from,
                salary_to: need.salary_to,
                description: need.description.clone(),
            },
            need_definition: need,
            message: input.message,
            status: InteractionStatus::Sent,
            created_at: timestamp,
            viewed_at: None,
            responded_at: None,
            disclosure_fields: Vec::new(),
        };
        self.service
            .repository
            .save_invitation(&record)
            .await
            .map_err(|error| match error {
                // The partial unique index fires when the duplicate check above
                // lost a race with a concurrent create.
                RepositoryError::Conflict => ApplicationError::new(
                    409,
                    "invitation_exists",
                    "Приглашение по этой потребности уже существует",
                ),
                other => ApplicationError::storage(other),
            })?;
        Ok(invitation_view(record))
    }

    /// The mock's `createApplication`, run inside `idempotent` by the caller.
    pub async fn create_application(
        &self,
        session: &SessionView,
        input: ApplicationInput,
    ) -> Result<ApplicationView, ApplicationError> {
        if !(1..=4000).contains(&input.message.chars().count()) {
            return Err(validation("Запрос не соответствует контракту"));
        }
        validate_disclose(&input.disclose)?;
        let candidate = self.service.candidate_index(candidate_id(session)?).await?;
        self.service
            .require_processing_consent(&candidate.profile.id)
            .await?;
        let need = self.service.published_need(&input.vacancy_id).await?;
        let eligibility = self
            .service
            .application_eligibility(&candidate, &need.id)
            .await?;
        if let Some(reason) = eligibility.reason {
            let (code, message) = match reason {
                "application_exists" => {
                    ("application_exists", "Вы уже откликнулись на эту вакансию")
                }
                "email" => (
                    "application_not_eligible",
                    "Подтвердите почту, чтобы откликаться на вакансии",
                ),
                "questionnaire" => (
                    "application_not_eligible",
                    "Заполните анкету, чтобы откликаться на вакансии",
                ),
                "category" => (
                    "application_not_eligible",
                    "Подключите GitHub или ФСП либо пройдите тест знаний, чтобы откликаться на вакансии",
                ),
                _ => (
                    "application_not_eligible",
                    "Для отклика необходимо согласие на обработку данных",
                ),
            };
            return Err(ApplicationError::new(409, code, message));
        }
        let company = self
            .service
            .repository
            .company(&need.company_id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| {
                ApplicationError::new(500, "internal_error", "Сервис временно недоступен")
            })?;
        let mut disclose = input.disclose;
        disclose.sort();
        let record = fsp_domain::ApplicationRecord {
            id: format!("application-{}", Uuid::new_v4().simple()),
            company_id: company.id.clone(),
            candidate_id: candidate.profile.id.clone(),
            candidate_alias: candidate.profile.alias.clone(),
            company: fsp_domain::CompanyCard {
                id: company.id,
                display_name: company.display_name,
                business_area: company.business_area,
                description: company.description,
                contact_email: company.contact_email,
            },
            vacancy: fsp_domain::VacancySnapshot {
                id: need.id.clone(),
                title: need.title.clone(),
                specialization: need.need.specialization,
                grade: need.need.grade,
                work_mode: need.work_mode.clone(),
                salary_from: need.salary_from,
                salary_to: need.salary_to,
                description: need.description.clone(),
            },
            need_definition: need,
            message: input.message,
            status: InteractionStatus::Sent,
            created_at: now(),
            viewed_at: None,
            responded_at: None,
            disclosure_fields: disclose,
        };
        let tx = &*self.service.repository;
        tx.save_application(&record)
            .await
            .map_err(|error| match error {
                // Partial unique index: a concurrent create on the same
                // vacancy won the race after the eligibility check.
                RepositoryError::Conflict => ApplicationError::new(
                    409,
                    "application_exists",
                    "Вы уже откликнулись на эту вакансию",
                ),
                other => ApplicationError::storage(other),
            })?;
        self.service
            .refresh_disclosure(tx, &record.company_id.clone(), &candidate)
            .await?;
        self.service.application_view(record).await
    }

    /// The mock's `respondToTask`, run inside `idempotent` by the caller.
    pub async fn respond_to_task(
        &self,
        session: &SessionView,
        task_id: &str,
        input: TaskResponseInput,
    ) -> Result<TaskResponseView, ApplicationError> {
        if !(1..=10000).contains(&input.answer.chars().count()) {
            return Err(validation("Запрос не соответствует контракту"));
        }
        let candidate_id = candidate_id(session)?;
        let candidate = self.service.candidate_index(candidate_id).await?;
        self.service
            .require_processing_consent(candidate_id)
            .await?;
        let responses = self.service.sorted_task_responses(candidate_id).await?;
        let offer = self.service.task_offer(&candidate, &responses).await?;
        if offer.current_response.is_some() {
            return Err(ApplicationError::new(
                409,
                "task_response_exists",
                "Ответ на задание этой недели уже отправлен",
            ));
        }
        let current = offer
            .current
            .filter(|task| task.id == task_id)
            .ok_or_else(|| {
                ApplicationError::new(409, "task_not_offered", "Это задание сейчас не предложено")
            })?;
        let record = fsp_domain::TaskResponseRecord {
            id: format!("task-response-{}", Uuid::new_v4().simple()),
            candidate_id: candidate.profile.id.clone(),
            candidate_alias: candidate.profile.alias.clone(),
            task: current,
            mode: input.mode,
            answer: input.answer,
            status: fsp_domain::TaskResponseStatus::Submitted,
            feedback: None,
            submitted_at: now(),
            reviewed_at: None,
        };
        self.service
            .repository
            .save_task_response(&record)
            .await
            .map_err(|error| match error {
                // One response per (candidate, task): the offer check above lost
                // the race with a concurrent submission.
                RepositoryError::Conflict => ApplicationError::new(
                    409,
                    "task_response_exists",
                    "Ответ на задание этой недели уже отправлен",
                ),
                other => ApplicationError::storage(other),
            })?;
        Ok(task_response_view(&record))
    }

    /// The mock's `startAssessment`, run inside `idempotent` by the caller:
    /// consent, onboarding, supported scope, one active attempt,
    /// no same-scope downgrade and the grade-change cooldown, in this order.
    pub async fn start_assessment(
        &self,
        session: &SessionView,
        input: AssessmentStartInput,
    ) -> Result<AssessmentView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        // Different command keys still compete for the same active-attempt slot.
        // Read eligibility only after joining the candidate's write order.
        self.service
            .repository
            .lock_candidate(candidate_id)
            .await
            .map_err(ApplicationError::storage)?;
        let candidate = self.service.candidate_index(candidate_id).await?;
        self.service
            .require_processing_consent(candidate_id)
            .await?;
        if !candidate.email_verified || !candidate.profile.questionnaire_complete() {
            return Err(ApplicationError::new(
                409,
                "onboarding_incomplete",
                "Подтвердите почту и заполните анкету",
            ));
        }
        if !fsp_domain::knowledge_available(input.specialization, input.grade) {
            return Err(ApplicationError::new(
                422,
                "assessment_scope_unavailable",
                "Для выбранной специализации и грейда тест пока недоступен",
            ));
        }
        let attempts = self
            .service
            .repository
            .candidate_assessments(candidate_id)
            .await
            .map_err(ApplicationError::storage)?;
        let running = attempts
            .iter()
            .any(|item| item.status.active() && item.task.execution == "questionnaire");
        if running {
            return Err(ApplicationError::new(
                409,
                "assessment_active",
                "Сначала завершите текущую проверку",
            ));
        }
        if candidate.profile.verified_specialization == Some(input.specialization)
            && candidate
                .profile
                .verified_grade
                .is_some_and(|grade| input.grade < grade)
        {
            return Err(ApplicationError::new(
                422,
                "grade_downgrade_forbidden",
                "Проверка не должна понижать подтверждённый грейд",
            ));
        }
        let changing = candidate.profile.verified_grade.is_some()
            && (candidate.profile.verified_grade != Some(input.grade)
                || candidate.profile.verified_specialization != Some(input.specialization));
        let cooling_off = candidate
            .profile
            .grade_confirmed_at
            .as_deref()
            .and_then(timestamp_at)
            .is_some_and(|at| {
                at + Duration::days(i64::from(self.service.grade_cooldown_days)) > Utc::now()
            });
        if changing && cooling_off {
            return Err(ApplicationError::new(
                409,
                "grade_change_cooldown",
                "Изменить грейд можно после nextEligibleAt в профиле",
            ));
        }
        // Retire only unfinished legacy simulations. History and confirmed grades remain intact.
        // This write shares the candidate lock and transaction with the new attempt.
        for mut legacy in attempts
            .into_iter()
            .filter(|a| a.status.active() && a.task.execution != "questionnaire")
        {
            legacy.status = fsp_domain::AssessmentStatus::ServiceError;
            legacy.updated_at = now();
            legacy.revision += 1;
            legacy.result = Some(fsp_domain::AssessmentResultData {
                assessment_id: legacy.id.clone(), outcome: "service_error".into(),
                confirmed_grade: candidate.profile.verified_grade, previous_grade: candidate.profile.verified_grade,
                grade_changed: false, rubric: vec![], checks: vec![],
                feedback: "Демонстрационная задача архивирована при переходе на тесты знаний. Оценка не выставлялась.".into(),
                next_eligible_at: None, simulation: true,
            });
            self.service
                .repository
                .save_assessment(&legacy)
                .await
                .map_err(ApplicationError::storage)?;
        }
        let assessment_id = format!("assessment-{}", Uuid::new_v4().simple());
        let task = fsp_domain::knowledge_task(input.specialization, input.grade, &assessment_id)
            .ok_or_else(|| {
                ApplicationError::new(
                    422,
                    "assessment_scope_unavailable",
                    "Для выбранной специализации и грейда тест пока недоступен",
                )
            })?;
        let timestamp = now();
        let record = fsp_domain::AssessmentRecord {
            id: assessment_id,
            candidate_id: candidate_id.to_string(),
            specialization: input.specialization,
            grade: input.grade,
            status: fsp_domain::AssessmentStatus::Draft,
            answer: fsp_domain::AssessmentAnswer {
                code: task.starter_code.clone(),
                explanation: String::new(),
                external_ai_used: false,
                responses: vec![],
            },
            checks: vec![],
            task,
            result: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            revision: 1,
        };
        self.service
            .repository
            .save_assessment(&record)
            .await
            .map_err(ApplicationError::storage)?;
        Ok(assessment_view(&record))
    }

    /// The mock's `submitAssessment`, run inside `idempotent` by the caller:
    /// guards, then the submitted attempt completes inline.
    pub async fn submit_assessment(
        &self,
        session: &SessionView,
        assessment_id: &str,
    ) -> Result<JobView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        self.service
            .repository
            .lock_candidate(candidate_id)
            .await
            .map_err(ApplicationError::storage)?;
        self.service
            .require_processing_consent(candidate_id)
            .await?;
        let mut record = self.service.assessment_for(session, assessment_id).await?;
        mutable_assessment(&record)?;
        if fsp_domain::score_knowledge(
            record.specialization,
            record.grade,
            &record.task,
            &record.answer,
        )
        .is_none()
        {
            return Err(ApplicationError::new(
                422,
                "answer_incomplete",
                "Ответьте на все вопросы теста",
            ));
        }
        record.status = fsp_domain::AssessmentStatus::Submitted;
        record.updated_at = now();
        let base_revision = record.revision;
        record.revision += 1;
        // Submission, the grade/profile write inside `complete_assessment`, the
        // result and the job row share one transaction.
        let tx = &*self.service.repository;
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
        let mut job =
            self.service
                .new_job(&record.candidate_id, "assessment", Some(&record.id), None);
        self.service.complete_assessment(tx, &mut record).await?;
        tx.save_assessment(&record)
            .await
            .map_err(ApplicationError::storage)?;
        job.status = if record.status == fsp_domain::AssessmentStatus::ServiceError {
            job.error_code = Some("assessment_service_unavailable".into());
            job.error_message = Some("Симулированная ошибка оценивания".into());
            "failed"
        } else {
            "succeeded"
        }
        .into();
        job.updated_at = now();
        job.result_json = Some(self.service.job_result(&JobResultView::Assessment {
            assessment: assessment_view(&record),
        })?);
        tx.save_job(&job).await.map_err(ApplicationError::storage)?;
        Ok(job_view(job, None))
    }
}
