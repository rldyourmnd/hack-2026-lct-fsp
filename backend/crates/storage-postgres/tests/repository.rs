//! Repository tests against a real PostgreSQL database. Each test gets an
//! isolated `fsp_test_*` database; `FSP_TEST_DATABASE_URL` must point at an
//! admin connection (the `just db-up` service or the CI service container).

use std::sync::Arc;

use fsp_application::{
    CandidateSearch, EmailChallengeInput, EmailVerifyInput, IdempotencyClaim, PlatformRepository,
    PlatformService, ProfilePatch, VerificationDelivery,
};
use fsp_domain::{
    Criticality, EvidencePolicy, Grade, HiringNeed, HiringNeedRecord, Requirement, Role,
    Specialization,
};
use fsp_storage_postgres::PostgresRepository;

fn admin_url() -> String {
    std::env::var("FSP_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://fsp:fsp@127.0.0.1:5434/postgres".into())
}

struct FixedDelivery;

#[async_trait::async_trait]
impl VerificationDelivery for FixedDelivery {
    fn code_for(&self) -> Result<String, fsp_application::IntegrationError> {
        Ok("123456".into())
    }

    async fn deliver(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<(), fsp_application::IntegrationError> {
        Ok(())
    }

    fn contract_name(&self) -> &'static str {
        "mock_mailbox"
    }
}

async fn repository() -> (PostgresRepository, fsp_storage_postgres::TestDatabase) {
    PostgresRepository::connect_test_database(&admin_url()).await
}

#[tokio::test(flavor = "current_thread")]
async fn login_session_and_profile_survive_reconnect() {
    let (repository, database) = repository().await;
    let service = PlatformService::new(Arc::new(repository), Arc::new(FixedDelivery));
    let challenge = service
        .request_challenge(EmailChallengeInput {
            email: "New.Candidate@example.com".into(),
            role: Role::Candidate,
        })
        .await
        .expect("challenge is created");
    let wrong = service
        .verify_email(EmailVerifyInput {
            challenge_id: challenge.challenge_id.clone(),
            code: "000000".into(),
        })
        .await
        .expect_err("wrong code is rejected");
    assert_eq!(wrong.code, "invalid_code");

    let (session, token) = service
        .verify_email(EmailVerifyInput {
            challenge_id: challenge.challenge_id.clone(),
            code: "123456".into(),
        })
        .await
        .expect("correct code creates a session");
    assert_eq!(session.role, Role::Candidate);
    assert!(session.email_verified);
    let reused = service
        .verify_email(EmailVerifyInput {
            challenge_id: challenge.challenge_id,
            code: "123456".into(),
        })
        .await
        .expect_err("a challenge is one-time");
    assert_eq!(reused.code, "challenge_used");
    drop(service);

    let reopened = PlatformService::new(
        Arc::new(
            PostgresRepository::connect(&database.url)
                .await
                .expect("database reconnects"),
        ),
        Arc::new(FixedDelivery),
    );
    let restored = reopened.session(&token).await.expect("session persists");
    let profile = reopened.profile(&restored).await.expect("profile persists");
    assert_eq!(profile.email, "new.candidate@example.com");
    let updated = reopened
        .update_profile(
            &restored,
            ProfilePatch {
                name: Some("Мария Волкова".into()),
                industry: Some("it".into()),
                ..ProfilePatch::default()
            },
        )
        .await
        .expect("frontend profile payload is accepted");
    assert_eq!(updated.name, "Мария Волкова");
    let empty_patch = reopened
        .update_profile(&restored, ProfilePatch::default())
        .await
        .expect_err("empty profile patches violate the contract");
    assert_eq!(empty_patch.code, "validation_error");

    let mismatch = reopened
        .request_challenge(EmailChallengeInput {
            email: "new.candidate@example.com".into(),
            role: Role::Employer,
        })
        .await
        .expect_err("an account cannot change role during login");
    assert_eq!(mismatch.code, "role_mismatch");
}

#[tokio::test(flavor = "current_thread")]
async fn seeded_recommendations_use_the_recorded_deterministic_order() {
    let (repository, _database) = repository().await;
    let service = PlatformService::new(Arc::new(repository), Arc::new(FixedDelivery));
    service.initialize().await.expect("demo data is seeded");
    let challenge = service
        .request_challenge(EmailChallengeInput {
            email: "hr@sever-tech.example".into(),
            role: Role::Employer,
        })
        .await
        .expect("employer challenge is created");
    let (session, _) = service
        .verify_email(EmailVerifyInput {
            challenge_id: challenge.challenge_id,
            code: "123456".into(),
        })
        .await
        .expect("employer session is created");
    let page = service
        .candidate_search(
            &session,
            CandidateSearch {
                need_id: Some("need-001".into()),
                recommended: true,
                page: 1,
                page_size: 10,
                ..CandidateSearch::default()
            },
        )
        .await
        .expect("recommendations are available");
    let ids = page
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["candidate031", "candidate018"]);
    assert_eq!(
        page.items[0]
            .r#match
            .as_ref()
            .expect("match exists")
            .required_met,
        2
    );
}

#[tokio::test(flavor = "current_thread")]
async fn idempotency_keys_round_trip_and_keep_the_first_response() {
    let (repository, _database) = repository().await;

    let tx = repository.transaction().await.unwrap();
    let claim = tx
        .idempotency_claim("actor:op:/path:key-0001", "{\"a\":1}")
        .await
        .unwrap();
    assert!(matches!(claim, IdempotencyClaim::Execute));
    let second = repository.transaction().await.unwrap();
    // The competing transaction waits until both the effect and response commit.
    let pending = second.idempotency_claim("actor:op:/path:key-0001", "{\"a\":1}");
    tokio::pin!(pending);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut pending)
            .await
            .is_err()
    );
    tx.idempotency_finish("actor:op:/path:key-0001", 201, "{\"id\":\"one\"}")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let IdempotencyClaim::Stored(stored) = pending.await.unwrap() else {
        panic!("finished scope replays the stored response");
    };
    assert_eq!(stored.fingerprint, "{\"a\":1}");
    assert_eq!(stored.status, 201);
    assert_eq!(stored.body, "{\"id\":\"one\"}");
}

#[tokio::test(flavor = "current_thread")]
async fn idempotent_command_and_claim_roll_back_if_response_cannot_be_stored() {
    struct Unserializable;
    impl serde::Serialize for Unserializable {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom(
                "injected response serialization failure",
            ))
        }
    }
    let (repository, _database) = repository().await;
    let repository = Arc::new(repository);
    let service = PlatformService::new(repository.clone(), Arc::new(FixedDelivery));
    service.initialize().await.unwrap();
    let challenge = service
        .request_challenge(EmailChallengeInput {
            email: "hr@sever-tech.example".into(),
            role: Role::Employer,
        })
        .await
        .unwrap();
    let (session, _) = service
        .verify_email(EmailVerifyInput {
            challenge_id: challenge.challenge_id,
            code: "123456".into(),
        })
        .await
        .unwrap();
    let body = serde_json::json!({"candidateId":"candidate018", "needId":"need-001", "message":"Предложение"});
    let context = || fsp_application::IdempotencyContext {
        operation: "createInvitation",
        path: "/api/v1/invitations",
        key: "rollback-test",
        body: &body,
        status: 201,
    };
    let before = repository
        .invitations(session.company_id.as_deref().unwrap())
        .await
        .unwrap()
        .len();
    let command_session = &session;
    let input = serde_json::from_value(body.clone()).unwrap();
    let result = service
        .idempotent(&session, context(), |commands| async move {
            commands.create_invitation(command_session, input).await?;
            Ok(Unserializable)
        })
        .await;
    assert!(matches!(result, Err(error) if error.code == "internal_error"));
    assert_eq!(
        repository
            .invitations(session.company_id.as_deref().unwrap())
            .await
            .unwrap()
            .len(),
        before
    );
    let claims: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM idempotency_keys")
        .fetch_one(repository.pool())
        .await
        .unwrap();
    assert_eq!(claims, 0);
    let input = serde_json::from_value(body.clone()).unwrap();
    let result = service
        .idempotent(&session, context(), |commands| async move {
            commands.create_invitation(command_session, input).await
        })
        .await
        .unwrap();
    assert!(matches!(result, fsp_application::Idempotent::Fresh(_)));
    assert_eq!(
        repository
            .invitations(session.company_id.as_deref().unwrap())
            .await
            .unwrap()
            .len(),
        before + 1
    );
}

/// The synthetic report of `contracts/fixtures` (an owned Rust/React project, a led organization
/// project and two commits in someone else's Redis library), under the canonical login.
fn github_report(login: &str) -> fsp_domain::GitHubReport {
    let mut report: fsp_domain::GitHubReport = serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/github-report.json"
    ))
    .unwrap();
    report.login = login.into();
    report
}

/// A fake analysis service. Analyses finish after `pending` polls that carry the ownership code
/// (progress views without the code do not advance them); the bio holds the code only for
/// logins listed in `verified`.
struct Insights {
    verified: &'static [&'static str],
    pending: std::sync::atomic::AtomicU32,
}

impl Insights {
    fn finished(verified: &'static [&'static str]) -> Self {
        Self::after(0, verified)
    }

    fn after(pending: u32, verified: &'static [&'static str]) -> Self {
        Self {
            verified,
            pending: pending.into(),
        }
    }

    fn state(&self, login: &str, ownership_code: Option<&str>) -> fsp_application::InsightsStatus {
        use std::sync::atomic::Ordering;
        let pending = self.pending.load(Ordering::SeqCst);
        if pending > 0 {
            if ownership_code.is_some() {
                self.pending.store(pending - 1, Ordering::SeqCst);
            }
            return fsp_application::InsightsStatus::Pending {
                queued: false,
                stage: "Клонирование и разбор истории".into(),
                progress: 40,
            };
        }
        // GitHub answers with the canonical login whatever case was typed.
        let canonical = login.to_ascii_lowercase();
        fsp_application::InsightsStatus::Done(Box::new(fsp_application::GitHubInsights {
            ownership_code_found: Some(self.verified.contains(&canonical.as_str())),
            report: github_report(&canonical),
        }))
    }
}

#[async_trait::async_trait]
impl fsp_application::GitHubInsightsGateway for Insights {
    async fn submit(
        &self,
        login: &str,
        ownership_code: Option<&str>,
        _: bool,
    ) -> Result<fsp_application::InsightsStatus, fsp_application::IntegrationError> {
        assert!(ownership_code.is_some_and(|code| code.starts_with("fsp-")));
        Ok(self.state(login, ownership_code))
    }

    async fn status(
        &self,
        login: &str,
        ownership_code: Option<&str>,
    ) -> Result<Option<fsp_application::InsightsStatus>, fsp_application::IntegrationError> {
        Ok(Some(self.state(login, ownership_code)))
    }
}

async fn signed_in_candidate(
    service: &PlatformService,
    repository: &PostgresRepository,
    email: &str,
) -> fsp_application::SessionView {
    let challenge = service
        .request_challenge(EmailChallengeInput {
            email: email.into(),
            role: Role::Candidate,
        })
        .await
        .unwrap();
    let session = service
        .verify_email(EmailVerifyInput {
            challenge_id: challenge.challenge_id,
            code: "123456".into(),
        })
        .await
        .unwrap()
        .0;
    let id = session.candidate_id.as_deref().unwrap();
    let mut profile = repository.candidate(id).await.unwrap().unwrap();
    profile.consents.processing = true;
    repository.save_candidate(&profile).await.unwrap();
    session
}

/// An LLM that keeps the specialization, rates one grade lower and rewrites the team role.
#[derive(Default)]
struct Reviewer {
    control: Option<(
        tokio::sync::mpsc::UnboundedSender<()>,
        Arc<tokio::sync::Semaphore>,
    )>,
}

#[async_trait::async_trait]
impl fsp_application::LlmGateway for Reviewer {
    async fn extract_resume(
        &self,
        _: &[fsp_domain::PdfPage],
    ) -> Result<fsp_domain::ResumeExtraction, fsp_application::IntegrationError> {
        unreachable!()
    }
    async fn assessment_help(
        &self,
        _: &fsp_domain::AssessmentHelpRequest,
    ) -> Result<fsp_domain::AssessmentHelpReply, fsp_application::IntegrationError> {
        unreachable!()
    }
    async fn match_repository(
        &self,
        _: &fsp_domain::RepositoryMatchRequest,
    ) -> Result<fsp_domain::RepositoryCompetitionMatch, fsp_application::IntegrationError> {
        unreachable!()
    }
    async fn review_profile(
        &self,
        request: &fsp_domain::ProfileReviewRequest,
    ) -> Result<fsp_domain::ProfileReviewReply, fsp_application::IntegrationError> {
        // The review sees anonymized evidence only.
        let serialized = serde_json::to_string(request).unwrap();
        assert!(!serialized.contains("Иван") && !serialized.contains("auto-dev"));
        if let Some((started, proceed)) = &self.control {
            started.send(()).unwrap();
            proceed.acquire().await.unwrap().forget();
        }
        Ok(fsp_domain::ProfileReviewReply {
            specialization: Specialization::Backend,
            grade: Grade::Junior,
            rationale: "Стабильная работа над собственными проектами".into(),
            experience: "Разрабатывает API на Rust и веб-интерфейсы на TypeScript.".into(),
            team_role: "Ведёт бэкенд-проекты".into(),
            recommendations: vec!["Добавьте ревью чужого кода".into()],
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn withdrawing_consent_skips_a_queued_profile_review() {
    let (repository, _database) = repository().await;
    let repository = Arc::new(repository);
    let (started, mut calls) = tokio::sync::mpsc::unbounded_channel();
    let proceed = Arc::new(tokio::sync::Semaphore::new(0));
    let service = PlatformService::new(repository.clone(), Arc::new(FixedDelivery))
        .with_github_insights(Arc::new(Insights::finished(&[])))
        .with_llm(Arc::new(Reviewer {
            control: Some((started, proceed.clone())),
        }));
    service.initialize().await.unwrap();
    let session = signed_in_candidate(&service, &repository, "review-consent@example.com").await;
    service
        .connect_source(
            &session,
            "github",
            fsp_application::SourceConnectInput {
                account: "auto-dev".into(),
            },
        )
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), calls.recv())
        .await
        .unwrap()
        .unwrap();
    // The first request is in flight; changing an estimate queues one replacement.
    service
        .update_profile(
            &session,
            ProfilePatch {
                team_role: Some("Тимлид".into()),
                ..ProfilePatch::default()
            },
        )
        .await
        .unwrap();
    service
        .update_profile(
            &session,
            ProfilePatch {
                consents: Some(fsp_domain::Consents {
                    processing: false,
                    publication: false,
                }),
                ..ProfilePatch::default()
            },
        )
        .await
        .unwrap();
    proceed.add_permits(1);
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), calls.recv())
            .await
            .is_err(),
        "a queued review must not start after processing consent was withdrawn"
    );
    let profile = repository
        .candidate(session.candidate_id.as_deref().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert!(!profile.consents.processing);
    assert!(profile.estimate.unwrap().review.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn connecting_github_builds_the_profile_and_an_estimated_category() {
    let (repository, _database) = repository().await;
    let repository = Arc::new(repository);
    let service = PlatformService::new(repository.clone(), Arc::new(FixedDelivery))
        .with_github_insights(Arc::new(Insights::finished(&[])))
        .with_llm(Arc::new(Reviewer::default()));
    service.initialize().await.unwrap();
    let session = signed_in_candidate(&service, &repository, "auto-profile@example.com").await;
    let profile = service.profile(&session).await.unwrap();
    assert_eq!(profile.name, fsp_domain::DEFAULT_CANDIDATE_NAME);
    assert!(profile.estimate.is_none());
    assert!(profile.onboarding.blockers.contains(&"category"));
    let error = service.refresh_estimate(&session, false).await.unwrap_err();
    assert_eq!(error.code, "source_not_connected");

    service
        .connect_source(
            &session,
            "github",
            fsp_application::SourceConnectInput {
                account: "auto-dev".into(),
            },
        )
        .await
        .unwrap();
    // The rules apply with the source; the LLM review joins them in the background.
    let mut profile = service.profile(&session).await.unwrap();
    for _ in 0..100 {
        if profile
            .estimate
            .as_ref()
            .is_some_and(|estimate| estimate.review.is_some())
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        profile = service.profile(&session).await.unwrap();
    }
    assert_eq!(profile.name, "Иван Петров");
    assert_eq!(profile.specialization, Specialization::Backend);
    assert_eq!(profile.selected_grade, Grade::Middle);
    assert_eq!(profile.stack.len(), 12);
    assert_eq!(profile.auto_fields.len(), fsp_domain::AUTO_FIELDS.len());
    assert_eq!(profile.team_role, "Ведёт бэкенд-проекты");
    let estimate = profile.estimate.as_ref().unwrap();
    assert_eq!(
        estimate.grade,
        Some(Grade::Middle),
        "the grade stays rule-based"
    );
    assert!(!estimate.review.as_ref().unwrap().agrees);
    assert_eq!(estimate.confidence, fsp_domain::SignalConfidence::Medium);
    assert_eq!(
        profile.category.source,
        Some(fsp_domain::CategorySource::Estimate)
    );
    assert_eq!(profile.category.grade, Some(Grade::Middle));
    assert!(!profile.onboarding.blockers.contains(&"category"));
    assert!(!profile.onboarding.blockers.contains(&"questionnaire"));

    // An edited answer stays the candidate's; publication makes the estimated category searchable.
    service
        .update_profile(
            &session,
            ProfilePatch {
                team_role: Some("Тимлид".into()),
                consents: Some(fsp_domain::Consents {
                    processing: true,
                    publication: true,
                }),
                ..ProfilePatch::default()
            },
        )
        .await
        .unwrap();
    let mut refreshed = service.refresh_estimate(&session, false).await.unwrap();
    for _ in 0..100 {
        if refreshed
            .estimate
            .as_ref()
            .is_some_and(|estimate| estimate.review.is_some())
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        refreshed = service.profile(&session).await.unwrap();
    }
    assert_eq!(refreshed.team_role, "Тимлид");
    assert!(
        !refreshed
            .auto_fields
            .iter()
            .any(|field| field == "teamRole")
    );
    assert!(refreshed.onboarding.visible_to_employers);

    let challenge = service
        .request_challenge(EmailChallengeInput {
            email: "hr@sever-tech.example".into(),
            role: Role::Employer,
        })
        .await
        .unwrap();
    let (employer, _) = service
        .verify_email(EmailVerifyInput {
            challenge_id: challenge.challenge_id,
            code: "123456".into(),
        })
        .await
        .unwrap();
    let search = |source| CandidateSearch {
        specialization: Some(Specialization::Backend),
        category_source: Some(source),
        page: 1,
        page_size: 50,
        ..CandidateSearch::default()
    };
    let estimated = service
        .candidate_search(&employer, search(fsp_domain::CategorySource::Estimate))
        .await
        .unwrap();
    let card = estimated
        .items
        .iter()
        .find(|item| Some(item.id.as_str()) == session.candidate_id.as_deref())
        .expect("the estimated candidate is in the bank");
    assert_eq!(card.grade, Grade::Middle);
    assert_eq!(card.verified_grade, None);
    assert_eq!(
        card.category_confidence,
        Some(fsp_domain::SignalConfidence::Medium)
    );
    assert_eq!(card.estimate.as_ref().unwrap().factors.len(), 8);
    let confirmed = service
        .candidate_search(&employer, search(fsp_domain::CategorySource::Assessment))
        .await
        .unwrap();
    assert!(!confirmed.items.is_empty());
    assert!(
        confirmed
            .items
            .iter()
            .all(|item| item.category_source == fsp_domain::CategorySource::Assessment)
    );

    // Typed experience is re-estimated at once and never costs the public evidence points.
    let score = refreshed.estimate.as_ref().unwrap().score;
    let typed = service
        .update_profile(
            &session,
            ProfilePatch {
                experience_months: Some(Some(5)),
                experience_since: None,
                ..ProfilePatch::default()
            },
        )
        .await
        .unwrap();
    let estimate = typed.estimate.as_ref().unwrap();
    assert_eq!(estimate.score, score);
    assert_eq!(estimate.review, None, "a new estimate waits for its review");
    assert_eq!(typed.experience_months, Some(5));

    // A start year in IT is the candidate's statement: experience counts from it, up to now.
    let year = chrono::Datelike::year(&chrono::Utc::now());
    let started = service
        .update_profile(
            &session,
            ProfilePatch {
                experience_since: Some(Some(2016)),
                ..ProfilePatch::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(started.experience_since, Some(2016));
    let since_months = u32::try_from(year - 2016).unwrap() * 12;
    assert_eq!(started.experience_months, Some(since_months));
    let tenure = started
        .estimate
        .as_ref()
        .unwrap()
        .factors
        .iter()
        .find(|factor| factor.id == "github_longevity")
        .unwrap();
    assert_eq!(tenure.points, tenure.max);

    // Rebuilding hands the evidence-based answers back; the stated experience stays the candidate's.
    let rebuilt = service.refresh_estimate(&session, true).await.unwrap();
    assert_ne!(rebuilt.team_role, "Тимлид");
    assert_eq!(rebuilt.name, "Иван Петров");
    for field in ["teamRole", "softSkills", "stack"] {
        assert!(
            rebuilt.auto_fields.iter().any(|item| item == field),
            "{field}"
        );
    }
    assert!(
        !rebuilt
            .auto_fields
            .iter()
            .any(|item| item == "experienceMonths")
    );
    assert_eq!(rebuilt.experience_since, Some(2016));
    assert_eq!(rebuilt.experience_months, Some(since_months));

    // An estimate of earlier rules updates itself the next time the candidate opens the profile.
    let id = session.candidate_id.as_deref().unwrap();
    let mut stored = repository.candidate(id).await.unwrap().unwrap();
    if let Some(estimate) = stored.estimate.as_mut() {
        estimate.version = "estimate-v3".into();
        estimate.growth.clear();
    }
    repository.save_candidate(&stored).await.unwrap();
    let opened = service.my_profile(&session).await.unwrap();
    let estimate = opened.estimate.as_ref().unwrap();
    assert_eq!(estimate.version, fsp_domain::ESTIMATE_VERSION);
    assert!(!estimate.growth.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn source_ingestion_updates_evidence_without_inventing_or_overwriting_grades() {
    let (repository, _database) = repository().await;
    let repository = Arc::new(repository);
    let service = PlatformService::new(repository.clone(), Arc::new(FixedDelivery))
        .with_github_insights(Arc::new(Insights::finished(&[])));
    service.initialize().await.unwrap();
    let session = signed_in_candidate(&service, &repository, "alexey@example.com").await;
    let id = session.candidate_id.as_deref().unwrap();
    let before = repository.candidate_index(id).await.unwrap().unwrap();
    let source = service
        .connect_source(
            &session,
            "github",
            fsp_application::SourceConnectInput {
                account: "https://github.com/owner".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(source.account_label.as_deref(), Some("owner"));
    let github = source.github.as_ref().unwrap();
    assert!(!github.ownership.verified);
    assert!(github.ownership.code.starts_with("fsp-"));
    assert_eq!(github.report.as_ref().unwrap().login, "owner");
    assert!(github.analysis.is_none());
    assert!(source.active_job_id.is_none());
    assert!(!github.employer_preview.as_ref().unwrap().ownership_verified);
    let connected = repository.candidate_index(id).await.unwrap().unwrap();
    assert_eq!(
        connected.profile.verified_grade,
        before.profile.verified_grade
    );
    assert!(
        connected
            .source_kinds
            .contains(&fsp_domain::EvidenceSource::Github)
    );
    let rust = connected
        .skills
        .iter()
        .find(|skill| skill.skill_id == "rust")
        .unwrap();
    assert!(rust.sources.contains(&fsp_domain::EvidenceSource::Github));
    assert_eq!(
        rust.evidence_grade,
        before
            .skills
            .iter()
            .find(|skill| skill.skill_id == "rust")
            .unwrap()
            .evidence_grade
    );
    let react = connected
        .skills
        .iter()
        .find(|skill| skill.skill_id == "react")
        .unwrap();
    assert_eq!(react.status, fsp_domain::EvidenceStatus::Evidence);
    assert_eq!(react.evidence_grade, None);
    // Two commits to someone else's repository are not evidence of the candidate's Redis skill.
    assert!(
        connected
            .skills
            .iter()
            .find(|skill| skill.skill_id == "redis")
            .is_none_or(|skill| !skill.sources.contains(&fsp_domain::EvidenceSource::Github))
    );
    let job = service.sync_source(&session, &source.id).await.unwrap();
    assert_eq!(job.status, "succeeded");
    let synced = repository.candidate_index(id).await.unwrap().unwrap();
    assert_eq!(synced.skills, connected.skills);
    assert_eq!(synced.profile.verified_grade, before.profile.verified_grade);

    let invalid = service
        .connect_source(
            &session,
            "github",
            fsp_application::SourceConnectInput {
                account: "https://github.com/owner/project".into(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(
        (invalid.status, invalid.code),
        (422, "github_login_invalid")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_running_github_analysis_is_applied_when_its_job_is_polled() {
    let (repository, _database) = repository().await;
    let repository = Arc::new(repository);
    let service = PlatformService::new(repository.clone(), Arc::new(FixedDelivery))
        .with_github_insights(Arc::new(Insights::after(3, &[])));
    service.initialize().await.unwrap();
    let session = signed_in_candidate(&service, &repository, "async@example.com").await;
    let id = session.candidate_id.as_deref().unwrap();
    let source = service
        .connect_source(
            &session,
            "github",
            fsp_application::SourceConnectInput {
                account: "owner".into(),
            },
        )
        .await
        .unwrap();
    let job_id = source.active_job_id.clone().unwrap();
    let github = source.github.as_ref().unwrap();
    assert!(github.report.is_none());
    assert_eq!(github.analysis.as_ref().unwrap().state, "running");
    let listed = service.sources(&session).await.unwrap();
    let listed = listed
        .items
        .iter()
        .find(|item| item.provider == fsp_domain::SourceProvider::Github)
        .unwrap();
    assert_eq!(listed.active_job_id.as_deref(), Some(job_id.as_str()));
    assert_eq!(
        listed
            .github
            .as_ref()
            .unwrap()
            .analysis
            .as_ref()
            .unwrap()
            .progress,
        40
    );
    // A sync while the analysis runs follows the same job.
    let again = service.sync_source(&session, &source.id).await.unwrap();
    assert_eq!(again.id, job_id);
    // Connecting again replaces the running analysis; the replaced job fails as stale.
    let replaced = service
        .connect_source(
            &session,
            "github",
            fsp_application::SourceConnectInput {
                account: "@owner".into(),
            },
        )
        .await
        .unwrap();
    let stale = service.job(&session, &job_id).await.unwrap();
    assert_eq!(stale.status, "failed");
    assert_eq!(stale.error.unwrap().code, "stale_input");
    let job_id = replaced.active_job_id.unwrap();
    let running = service.job(&session, &job_id).await.unwrap();
    assert_eq!(
        (running.status.as_str(), running.poll_after_ms),
        ("running", 2_000)
    );
    assert!(
        !repository
            .candidate_index(id)
            .await
            .unwrap()
            .unwrap()
            .skills
            .iter()
            .any(|skill| skill.sources.contains(&fsp_domain::EvidenceSource::Github))
    );
    let done = service.job(&session, &job_id).await.unwrap();
    assert_eq!(done.status, "succeeded");
    // Polling a finished job replays it.
    assert_eq!(
        service.job(&session, &job_id).await.unwrap().status,
        "succeeded"
    );
    let stored = repository.sources(id).await.unwrap();
    let stored = stored
        .iter()
        .find(|item| item.provider == fsp_domain::SourceProvider::Github)
        .unwrap();
    assert!(stored.active_job_id.is_none());
    assert!(
        stored
            .analysis
            .as_ref()
            .and_then(|analysis| analysis.report.as_ref())
            .is_some()
    );
    let rust = repository
        .candidate_index(id)
        .await
        .unwrap()
        .unwrap()
        .skills
        .into_iter()
        .find(|skill| skill.skill_id == "rust")
        .unwrap();
    assert!(rust.sources.contains(&fsp_domain::EvidenceSource::Github));
}

#[tokio::test(flavor = "current_thread")]
async fn a_github_account_can_be_proven_by_only_one_candidate() {
    let (repository, _database) = repository().await;
    let repository = Arc::new(repository);
    let service = PlatformService::new(repository.clone(), Arc::new(FixedDelivery))
        .with_github_insights(Arc::new(Insights::finished(&["owner"])));
    service.initialize().await.unwrap();
    let first = signed_in_candidate(&service, &repository, "first@example.com").await;
    let source = service
        .connect_source(
            &first,
            "github",
            fsp_application::SourceConnectInput {
                account: "@owner".into(),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("{} {}", error.code, error.message));
    let ownership = &source.github.as_ref().unwrap().ownership;
    assert!(ownership.verified);
    assert_eq!(
        ownership.method,
        Some(fsp_domain::OwnershipMethod::ProfileCode)
    );
    let second = signed_in_candidate(&service, &repository, "second@example.com").await;
    let taken = service
        .connect_source(
            &second,
            "github",
            fsp_application::SourceConnectInput {
                account: "OWNER".into(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!((taken.status, taken.code), (409, "github_account_taken"));
}

#[tokio::test(flavor = "current_thread")]
async fn corrupt_profile_json_is_reported_instead_of_silently_erased() {
    let (repository, _database) = repository().await;
    repository.seed_demo("2026-10-05T00:00:00Z").await.unwrap();
    sqlx::query("UPDATE candidates SET soft_skills_json='not-json' WHERE id='candidate024'")
        .execute(repository.pool())
        .await
        .unwrap();
    assert!(repository.candidate("candidate024").await.is_err());
    assert!(repository.candidate_index("candidate024").await.is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn shortlists_round_trip_and_stay_scoped_to_the_company() {
    let (repository, _database) = repository().await;
    sqlx::query(
        "INSERT INTO companies(id,display_name,business_area,description,contact_email,website) VALUES('company-sever','Север Тех','Разработка','','hr@sever-tech.example',NULL)",
    )
    .execute(repository.pool())
    .await
    .expect("company is stored");
    repository
        .save_shortlist(&fsp_domain::ShortlistRecord {
            id: "shortlist-test".into(),
            company_id: "company-sever".into(),
            title: "Кандидаты".into(),
            member_ids: vec!["candidate018".into()],
            created_at: "2026-10-06T00:00:00Z".into(),
            updated_at: "2026-10-06T00:00:00Z".into(),
        })
        .await
        .expect("shortlist is stored");

    let stored = repository
        .shortlist("company-sever", "shortlist-test")
        .await
        .expect("lookup works")
        .expect("shortlist exists");
    assert_eq!(stored.member_ids, ["candidate018"]);
    assert!(
        repository
            .shortlist("company-other", "shortlist-test")
            .await
            .expect("lookup works")
            .is_none()
    );
    assert_eq!(
        repository
            .shortlists("company-sever")
            .await
            .expect("list works")
            .len(),
        1
    );

    repository
        .delete_shortlist("company-sever", "shortlist-test")
        .await
        .expect("shortlist is removed");
    assert!(
        repository
            .shortlists("company-sever")
            .await
            .expect("list works")
            .is_empty()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn invitations_and_disclosures_round_trip() {
    let (repository, _database) = repository().await;
    repository
        .seed_demo("2026-10-06T00:00:00Z")
        .await
        .expect("demo data is seeded");
    sqlx::query(
        "INSERT INTO companies(id,display_name,business_area,description,contact_email,website) VALUES('company-sever','Север Тех','Разработка','','hr@sever-tech.example',NULL)",
    )
    .execute(repository.pool())
    .await
    .expect("company is stored");
    let need = HiringNeedRecord {
        id: "need-test".into(),
        company_id: "company-sever".into(),
        title: "Backend-разработчик".into(),
        need: demo_need(),
        work_mode: "remote".into(),
        salary_from: 200_000,
        salary_to: 300_000,
        description: "Описание вакансии".into(),
        published: true,
        published_at: Some("2026-10-06T00:00:00Z".into()),
        created_at: "2026-10-06T00:00:00Z".into(),
        updated_at: "2026-10-06T00:00:00Z".into(),
        revision: 1,
    };
    let record = fsp_domain::InvitationRecord {
        id: "invitation-test".into(),
        company_id: "company-sever".into(),
        candidate_id: "candidate018".into(),
        candidate_alias: "Кандидат №018".into(),
        company: fsp_domain::CompanyCard {
            id: "company-sever".into(),
            display_name: "Север Тех".into(),
            business_area: "Разработка".into(),
            description: String::new(),
            contact_email: "hr@sever-tech.example".into(),
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
        message: "Приглашаем на интервью".into(),
        status: fsp_domain::InteractionStatus::Sent,
        created_at: "2026-10-06T00:00:00Z".into(),
        viewed_at: None,
        responded_at: None,
        disclosure_fields: Vec::new(),
    };
    repository
        .save_invitation(&record)
        .await
        .expect("invitation is stored");

    let stored = repository
        .invitation("invitation-test")
        .await
        .expect("lookup works")
        .expect("invitation exists");
    assert_eq!(stored.status, fsp_domain::InteractionStatus::Sent);
    assert_eq!(stored.need.id, "need-test");
    assert_eq!(stored.need_definition.id, "need-test");
    assert_eq!(
        repository.invitations("company-sever").await.unwrap().len(),
        1
    );
    assert_eq!(
        repository
            .candidate_invitations("candidate018")
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        repository
            .candidate_invitations("candidate031")
            .await
            .unwrap()
            .is_empty()
    );

    repository
        .save_disclosure(&fsp_domain::DisclosureRecord {
            candidate_id: "candidate018".into(),
            company_id: "company-sever".into(),
            invitation_id: Some("invitation-test".into()),
            application_id: None,
            granted_at: Some("2026-10-06T00:00:00Z".into()),
            name: Some("Илья Соколов".into()),
            email: None,
            source_links: vec!["https://github.com/example".into()],
        })
        .await
        .expect("disclosure is stored");
    let grant = repository
        .disclosure("company-sever", "candidate018")
        .await
        .expect("lookup works")
        .expect("grant exists");
    assert_eq!(grant.name.as_deref(), Some("Илья Соколов"));
    assert!(grant.email.is_none());
    assert_eq!(grant.source_links, ["https://github.com/example"]);

    repository
        .delete_disclosure("company-sever", "candidate018")
        .await
        .expect("disclosure is removed");
    assert!(
        repository
            .disclosure("company-sever", "candidate018")
            .await
            .expect("lookup works")
            .is_none()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn tasks_and_responses_round_trip_and_stay_scoped() {
    let (repository, _database) = repository().await;
    repository
        .seed_demo("2026-10-06T00:00:00Z")
        .await
        .expect("demo data is seeded");
    sqlx::query(
        "INSERT INTO companies(id,display_name,business_area,description,contact_email,website) VALUES('company-sever','Север Тех','Разработка','','hr@sever-tech.example',NULL)",
    )
    .execute(repository.pool())
    .await
    .expect("company is stored");

    repository
        .save_task(&fsp_domain::TaskRecord {
            id: "task-test".into(),
            company_id: "company-sever".into(),
            title: "Задание".into(),
            prompt: "Опишите подход".into(),
            skill_id: "rust".into(),
            grade: Grade::Middle,
            estimate_minutes: 45,
            status: fsp_domain::TaskStatus::Active,
            created_at: "2026-10-06T00:00:00Z".into(),
        })
        .await
        .expect("task is stored");
    let stored = repository
        .task("task-test")
        .await
        .expect("lookup works")
        .expect("task exists");
    assert_eq!(stored.status, fsp_domain::TaskStatus::Active);
    assert_eq!(stored.grade, Grade::Middle);
    assert!(
        repository
            .active_tasks()
            .await
            .expect("active list works")
            .iter()
            .any(|task| task.id == "task-test")
    );

    repository
        .save_task_response(&fsp_domain::TaskResponseRecord {
            id: "task-response-test".into(),
            candidate_id: "candidate018".into(),
            candidate_alias: "Кандидат №018".into(),
            task: fsp_domain::OfferedTaskSnapshot {
                id: "task-test".into(),
                company_id: "company-sever".into(),
                company_display_name: "Север Тех".into(),
                title: "Задание".into(),
                prompt: "Опишите подход".into(),
                skill_id: "rust".into(),
                grade: Grade::Middle,
                estimate_minutes: 45,
            },
            mode: fsp_domain::TaskMode::Approach,
            answer: "Ответ".into(),
            status: fsp_domain::TaskResponseStatus::Submitted,
            feedback: None,
            submitted_at: "2026-10-06T12:00:00Z".into(),
            reviewed_at: None,
        })
        .await
        .expect("response is stored");
    let response = repository
        .task_response("task-response-test")
        .await
        .expect("lookup works")
        .expect("response exists");
    assert_eq!(response.task.id, "task-test");
    assert_eq!(response.task.company_display_name, "Север Тех");
    assert_eq!(
        repository
            .candidate_task_responses("candidate018")
            .await
            .expect("candidate list works")
            .len(),
        1
    );
    assert_eq!(
        repository
            .company_task_responses("company-sever")
            .await
            .expect("company list works")
            .len(),
        1
    );
    assert!(
        repository
            .company_task_responses("company-other")
            .await
            .expect("other company list works")
            .is_empty()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn assessments_and_jobs_round_trip_with_persisted_results() {
    let (repository, _database) = repository().await;
    repository
        .seed_demo("2026-10-06T00:00:00Z")
        .await
        .expect("demo data is seeded");

    repository
        .save_assessment(&fsp_domain::AssessmentRecord {
            id: "assessment-test".into(),
            candidate_id: "candidate024".into(),
            specialization: Specialization::Backend,
            grade: Grade::Middle,
            status: fsp_domain::AssessmentStatus::InProgress,
            task: fsp_domain::AssessmentTask {
                id: "task-idempotency-v1".into(),
                family: "idempotent-api".into(),
                version: "1".into(),
                variant_id: "orders-middle-a".into(),
                title: "Задание".into(),
                prompt: "Опишите подход".into(),
                language: "rust".into(),
                starter_code: "todo!()".into(),
                acceptance_criteria: vec!["Первый запрос создаёт заказ".into()],
                rubric: vec![fsp_domain::RubricCriterion {
                    id: "correctness".into(),
                    label: "Корректность".into(),
                    weight: 4,
                }],
                ai_allowed: true,
                external_ai_allowed: true,
                execution: "simulated".into(),
                questions: vec![],
            },
            answer: fsp_domain::AssessmentAnswer {
                code: "todo!()".into(),
                explanation: "Ответ".into(),
                external_ai_used: false,
                responses: vec![],
            },
            checks: vec![fsp_domain::AssessmentCheck {
                id: "check-1".into(),
                label: "Проверка".into(),
                status: "passed".into(),
                message: "Успешно".into(),
            }],
            result: None,
            created_at: "2026-10-06T00:00:00Z".into(),
            updated_at: "2026-10-06T01:00:00Z".into(),
            revision: 3,
        })
        .await
        .expect("assessment is stored");
    let stored = repository
        .assessment("assessment-test")
        .await
        .expect("lookup works")
        .expect("assessment exists");
    assert_eq!(stored.status, fsp_domain::AssessmentStatus::InProgress);
    assert_eq!(stored.revision, 3);
    assert_eq!(stored.task.variant_id, "orders-middle-a");
    assert_eq!(stored.checks.len(), 1);
    assert_eq!(
        repository
            .candidate_assessments("candidate024")
            .await
            .expect("candidate list works")
            .len(),
        1
    );
    assert!(
        repository
            .candidate_assessments("candidate-999")
            .await
            .expect("other candidate list works")
            .is_empty()
    );

    repository
        .save_job(&fsp_domain::JobRecord {
            id: "job-assessment".into(),
            owner_id: "candidate024".into(),
            kind: "assessment".into(),
            status: "succeeded".into(),
            candidate_id: Some("candidate024".into()),
            assessment_id: Some("assessment-test".into()),
            input_revision: Some(3),
            result_json: Some("{\"kind\":\"assessment\",\"ok\":true}".into()),
            confirmed: false,
            created_at: "2026-10-06T00:00:00Z".into(),
            updated_at: "2026-10-06T01:00:00Z".into(),
            result_source_id: None,
            error_code: None,
            error_message: None,
        })
        .await
        .expect("job is stored");
    let job = repository
        .job("candidate024", "job-assessment")
        .await
        .expect("lookup works")
        .expect("job exists");
    assert_eq!(job.kind, "assessment");
    assert_eq!(job.input_revision, Some(3));
    assert_eq!(
        job.result_json.as_deref(),
        Some("{\"kind\":\"assessment\",\"ok\":true}")
    );
}

fn demo_need() -> HiringNeed {
    let policy = EvidencePolicy {
        accept_assessments: true,
        accept_github: true,
        accept_fsp: true,
        competition_selection: "system".into(),
        competition_ids: Vec::new(),
    };
    HiringNeed {
        specialization: Specialization::Backend,
        grade: Grade::Middle,
        requirements: vec![
            Requirement {
                skill_id: "rust".into(),
                criticality: Criticality::Required,
                minimum_grade: Grade::Middle,
                evidence_policy: policy.clone(),
            },
            Requirement {
                skill_id: "postgresql".into(),
                criticality: Criticality::Required,
                minimum_grade: Grade::Junior,
                evidence_policy: policy.clone(),
            },
            Requirement {
                skill_id: "docker".into(),
                criticality: Criticality::Desired,
                minimum_grade: Grade::Junior,
                evidence_policy: policy,
            },
        ],
    }
}

#[tokio::test]
async fn rollback_accepts_future_additive_schema_but_rejects_corrupt_history() {
    use sqlx::migrate::{Migrate, Migration, MigrationType};
    use std::borrow::Cow;

    let (repository, database) = repository().await;
    let mut connection = repository.pool().acquire().await.unwrap();
    let future = Migration::new(
        9000,
        Cow::Borrowed("future additive fixture"),
        MigrationType::Simple,
        Cow::Borrowed(
            "CREATE TABLE future_release_data (value text NOT NULL); INSERT INTO future_release_data VALUES ('preserved');",
        ),
        false,
    );
    connection.apply(&future).await.unwrap();
    drop(connection);
    let reopened = PostgresRepository::connect(&database.url).await.unwrap();
    let value: String = sqlx::query_scalar("SELECT value FROM future_release_data")
        .fetch_one(reopened.pool())
        .await
        .unwrap();
    assert_eq!(value, "preserved");
    reopened.pool().close().await;

    sqlx::query("UPDATE _sqlx_migrations SET success = false WHERE version = 9000")
        .execute(repository.pool())
        .await
        .unwrap();
    let error = PostgresRepository::connect(&database.url)
        .await
        .err()
        .unwrap();
    assert!(format!("{error:?}").contains("partially applied"));
    sqlx::query("UPDATE _sqlx_migrations SET success = true WHERE version = 9000")
        .execute(repository.pool())
        .await
        .unwrap();

    // An unknown version within the binary's history must not be silently ignored.
    sqlx::query("INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (0, 'unknown old migration', true, '\\x00', 0)")
        .execute(repository.pool()).await.unwrap();
    let error = PostgresRepository::connect(&database.url)
        .await
        .err()
        .unwrap();
    assert!(format!("{error:?}").contains("migration 0 was previously applied but is missing"));
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 0")
        .execute(repository.pool())
        .await
        .unwrap();

    sqlx::query("UPDATE _sqlx_migrations SET checksum = '\\x00' WHERE version = 1")
        .execute(repository.pool())
        .await
        .unwrap();
    let error = PostgresRepository::connect(&database.url)
        .await
        .err()
        .unwrap();
    assert!(format!("{error:?}").contains("modified"));
    repository.pool().close().await;
}
