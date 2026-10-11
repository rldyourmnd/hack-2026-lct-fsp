//! Demo seed mirroring the mock fixture. Runs inside one transaction so a
//! fresh database either receives the complete fixture or nothing.

use fsp_application::RepositoryError;
use fsp_domain::{
    Account, CandidateProfile, Consents, Criticality, EvidencePolicy, EvidenceSource,
    EvidenceStatus, Grade, HiringNeed, Requirement, Role, SkillEvidence, Specialization,
};
use sqlx::{Connection, PgConnection};

use crate::{grade, json_storage, specialization, storage, ts, ts_opt};

pub(crate) async fn seed(conn: &mut PgConnection, now: &str) -> Result<(), RepositoryError> {
    let mut tx = conn.begin().await.map_err(storage)?;
    let candidate = Account {
        id: "candidate024".into(),
        email: "alexey@example.com".into(),
        role: Role::Candidate,
        candidate_id: Some("candidate024".into()),
        company_id: None,
        email_verified: true,
    };
    let employer = Account {
        id: "employer-a".into(),
        email: "hr@sever-tech.example".into(),
        role: Role::Employer,
        candidate_id: None,
        company_id: Some("company-a".into()),
        email_verified: true,
    };
    let employer_b = Account {
        id: "employer-b".into(),
        email: "hr@second-company.example".into(),
        role: Role::Employer,
        candidate_id: None,
        company_id: Some("company-b".into()),
        email_verified: true,
    };
    // Referenced rows first: accounts carry FKs to candidates and companies.
    sqlx::query(
        "INSERT INTO candidates(id,alias,email,name,specialization,selected_grade,stack_json,team_role,experience_months,experience,soft_skills_json,processing_consent,publication_consent,verified_grade,verified_specialization,grade_confirmed_at,revision,updated_at) \
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)",
    )
    .bind("candidate024")
    .bind("Кандидат №024")
    .bind(&candidate.email)
    .bind("Алексей Морозов")
    .bind("backend")
    .bind("middle")
    .bind(serde_json::to_string(&vec!["rust", "postgresql", "docker"]).map_err(json_storage)?)
    .bind("Backend-разработчик")
    .bind(30_i32)
    .bind("Разрабатывает надёжные API и фоновые обработчики.")
    .bind(serde_json::to_string(&vec!["Командная работа"]).map_err(json_storage)?)
    .bind(true)
    .bind(true)
    .bind("junior")
    .bind("backend")
    .bind(Some(ts("2026-06-01T12:00:00Z")?))
    .bind(1_i64)
    .bind(ts(now)?)
    .execute(&mut *tx)
    .await
    .map_err(storage)?;
    for profile in demo_candidates(now) {
        insert_candidate_tx(&mut tx, &profile).await?;
    }
    // Companies and needs mirror the mock fixtures so the demo stand serves the
    // same dataset whichever implementation is active.
    for company in [
        (
            "company-a",
            "Север Тех",
            "Разработка программного обеспечения",
            "Демонстрационная технологическая компания. Разрабатываем сервисы для управления заказами и логистикой.",
            "hr@sever-tech.example",
            Some("https://example.com/sever-tech"),
        ),
        (
            "company-b",
            "Вторая компания",
            "Информационные технологии",
            "Вторая демонстрационная компания для проверки разграничения доступа.",
            "hr@second-company.example",
            None,
        ),
    ] {
        sqlx::query(
            "INSERT INTO companies(id,display_name,business_area,description,contact_email,website) VALUES($1,$2,$3,$4,$5,$6)",
        )
        .bind(company.0)
        .bind(company.1)
        .bind(company.2)
        .bind(company.3)
        .bind(company.4)
        .bind(company.5)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
    }
    for account in [&candidate, &employer, &employer_b] {
        sqlx::query(
            "INSERT INTO accounts(id,email,role,candidate_id,company_id,email_verified,created_at) VALUES($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(&account.id)
        .bind(&account.email)
        .bind(crate::role(account.role))
        .bind(&account.candidate_id)
        .bind(&account.company_id)
        .bind(account.email_verified)
        .bind(ts(now)?)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
    }
    for profile in demo_candidates(now) {
        // Demo candidates are pre-verified fixtures: give each one a verified
        // account so employer visibility checks see the same state as the mock.
        sqlx::query(
            "INSERT INTO accounts(id,email,role,candidate_id,company_id,email_verified,created_at) VALUES($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(format!("account-{}", profile.id))
        .bind(&profile.email)
        .bind("candidate")
        .bind(&profile.id)
        .bind(Option::<String>::None)
        .bind(true)
        .bind(ts(now)?)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
    }
    for (id, skills, sources, fsp_verified) in demo_candidate_indexes() {
        sqlx::query(
            "INSERT INTO candidate_index(candidate_id,work_modes_json,available_for_offers,skills_json,source_kinds_json,fsp_verified,last_signal_at) VALUES($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(id)
        .bind("[\"remote\",\"hybrid\"]")
        .bind(true)
        .bind(serde_json::to_string(&skills).map_err(json_storage)?)
        .bind(serde_json::to_string(&sources).map_err(json_storage)?)
        .bind(fsp_verified)
        .bind(ts(now)?)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
    }
    sqlx::query(
        "INSERT INTO sources(id,candidate_id,provider,synthetic,active_job_id,account_label,last_synced_at,analysis_json,verified_participation,competition_ids_json) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
    )
    .bind("source-fsp-candidate024")
    .bind("candidate024")
    .bind("fsp")
    .bind(true)
    .bind(Option::<String>::None)
    .bind("FSP-DEMO-024")
    .bind(ts(now)?)
    .bind(Option::<String>::None)
    .bind(true)
    .bind("[\"competition-demo-api\"]")
    .execute(&mut *tx)
    .await
    .map_err(storage)?;
    let default_requirements = demo_need().requirements;
    let junior_requirements = vec![
        requirement("rust", Criticality::Required, Grade::Junior),
        requirement("postgresql", Criticality::Desired, Grade::Junior),
        requirement("docker", Criticality::Desired, Grade::Junior),
    ];
    // (id, company, title, specialization, grade, work_mode, from, to, description, requirements, published)
    for item in [
        (
            "need-001",
            "company-a",
            "Бэкенд-разработчик",
            Specialization::Backend,
            Grade::Middle,
            "remote",
            200000_i64,
            260000_i64,
            "Разработка надёжных API на Rust и PostgreSQL. Удалённая работа, задачи по надёжности и качеству сервисов.",
            default_requirements.clone(),
            false,
        ),
        (
            "need-002",
            "company-a",
            "Бэкенд-разработчик сервиса уведомлений",
            Specialization::Backend,
            Grade::Junior,
            "hybrid",
            120000_i64,
            160000_i64,
            "Сервис доставки уведомлений на Rust: очереди, повторные попытки и наблюдаемость. Гибрид, офис в Москве два дня в неделю, наставник в команде.",
            junior_requirements,
            true,
        ),
        (
            "need-b-001",
            "company-b",
            "Бэкенд-разработчик",
            Specialization::Backend,
            Grade::Middle,
            "remote",
            200000_i64,
            260000_i64,
            "Разработка надёжных API на Rust и PostgreSQL. Удалённая работа, задачи по надёжности и качеству сервисов.",
            default_requirements.clone(),
            true,
        ),
        (
            "need-b-002",
            "company-b",
            "Фронтенд-разработчик (React)",
            Specialization::Frontend,
            Grade::Middle,
            "office",
            180000_i64,
            230000_i64,
            "Интерфейсы внутренних сервисов на React и TypeScript: формы, таблицы, доступность. Офис в Санкт-Петербурге.",
            vec![requirement("react", Criticality::Required, Grade::Middle)],
            true,
        ),
    ] {
        sqlx::query(
            "INSERT INTO needs(id,company_id,title,specialization,grade,work_mode,salary_from,salary_to,description,requirements_json,published,published_at,created_at,updated_at,revision) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
        )
        .bind(item.0)
        .bind(item.1)
        .bind(item.2)
        .bind(specialization(item.3))
        .bind(grade(item.4))
        .bind(item.5)
        .bind(item.6)
        .bind(item.7)
        .bind(item.8)
        .bind(serde_json::to_string(&item.9).map_err(json_storage)?)
        .bind(item.10)
        .bind(if item.10 { Some(ts(now)?) } else { None })
        .bind(ts(now)?)
        .bind(ts(now)?)
        .bind(1_i64)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
    }
    // Mirror of the mock's `shortlist-001` fixture for the demo employer.
    sqlx::query(
        "INSERT INTO shortlists(id,company_id,title,members_json,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6)",
    )
    .bind("shortlist-001")
    .bind("company-a")
    .bind("Бэкенд · Мидл")
    .bind(serde_json::to_string(&vec!["candidate024", "candidate018", "candidate041"]).map_err(json_storage)?)
    .bind(ts(now)?)
    .bind(ts(now)?)
    .execute(&mut *tx)
    .await
    .map_err(storage)?;
    // Mirror of the mock's task fixtures: task-003 belongs to the second company.
    for (id, company_id, title, prompt, skill_id, task_grade, minutes) in [
        (
            "task-001",
            "company-a",
            "Повтор платежа без двойного списания",
            "Платёжный сервис получает повторный запрос после таймаута клиента. Как сделать, чтобы деньги не списались дважды? Решите или опишите подход: таблицы и ограничения PostgreSQL, порядок операций, что вернуть повторному запросу.",
            "postgresql",
            Grade::Junior,
            30_i32,
        ),
        (
            "task-002",
            "company-a",
            "Повторные попытки отправки уведомлений",
            "Сервис уведомлений на Rust вызывает внешний API, который иногда отвечает ошибкой или не отвечает. Предложите, как организовать повторные попытки: где хранить состояние, как не перегрузить внешний сервис и когда прекращать попытки.",
            "rust",
            Grade::Junior,
            45_i32,
        ),
        (
            "task-003",
            "company-b",
            "Форма регистрации с проверкой полей",
            "Нужна форма регистрации на React: почта, пароль и согласие с условиями. Опишите, как проверять поля и показывать ошибки так, чтобы форма была доступна с клавиатуры и для экранного диктора.",
            "react",
            Grade::Middle,
            40_i32,
        ),
    ] {
        sqlx::query(
            "INSERT INTO tasks(id,company_id,title,prompt,skill_id,grade,estimate_minutes,status,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(id)
        .bind(company_id)
        .bind(title)
        .bind(prompt)
        .bind(skill_id)
        .bind(grade(task_grade))
        .bind(minutes)
        .bind("active")
        .bind(ts(now)?)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
    }
    // Mirror of the mock's accepted task responses, so history and freshness
    // start with real task signals.
    for (id, candidate_id, alias, task_id, answer, feedback, submitted, reviewed) in [
        (
            "task-response-seed-031",
            "candidate031",
            "Кандидат №031",
            "task-002",
            "Очередь задач в PostgreSQL со статусом и временем следующей попытки, экспоненциальная задержка с ограничением, ключ идемпотентности на стороне получателя, после пяти неудач — в очередь разбора.",
            "Хороший разбор задержек и идемпотентности.",
            "2026-09-20T12:00:00Z",
            "2026-09-21T12:00:00Z",
        ),
        (
            "task-response-seed-041",
            "candidate041",
            "Кандидат №041",
            "task-003",
            "Проверка при потере фокуса и при отправке, ошибки связаны с полями через aria-describedby, фокус переходит к первому ошибочному полю, состояние кнопки не блокирует отправку без объяснения.",
            "Учтены доступность и порядок фокуса.",
            "2026-09-13T12:00:00Z",
            "2026-09-14T12:00:00Z",
        ),
    ] {
        let row = sqlx::query(
            "SELECT company_id,title,prompt,skill_id,grade,estimate_minutes FROM tasks WHERE id=$1",
        )
        .bind(task_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        let task_company: String = sqlx::Row::try_get(&row, "company_id").map_err(storage)?;
        let company_name = if task_company == "company-b" {
            "Вторая компания"
        } else {
            "Север Тех"
        };
        let task_json = serde_json::json!({
            "id": task_id,
            "companyId": task_company,
            "companyDisplayName": company_name,
            "title": sqlx::Row::try_get::<String, _>(&row, "title").map_err(storage)?,
            "prompt": sqlx::Row::try_get::<String, _>(&row, "prompt").map_err(storage)?,
            "skillId": sqlx::Row::try_get::<String, _>(&row, "skill_id").map_err(storage)?,
            "grade": sqlx::Row::try_get::<String, _>(&row, "grade").map_err(storage)?,
            "estimateMinutes": sqlx::Row::try_get::<i32, _>(&row, "estimate_minutes").map_err(storage)?,
        });
        sqlx::query(
            "INSERT INTO task_responses(id,candidate_id,company_id,task_id,candidate_alias,task_json,mode,answer,status,feedback,submitted_at,reviewed_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        )
        .bind(id)
        .bind(candidate_id)
        .bind(&task_company)
        .bind(task_id)
        .bind(alias)
        .bind(task_json.to_string())
        .bind("approach")
        .bind(answer)
        .bind("accepted")
        .bind(feedback)
        .bind(ts(submitted)?)
        .bind(ts(reviewed)?)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
    }
    tx.commit().await.map_err(storage)?;
    Ok(())
}

async fn insert_candidate_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    value: &CandidateProfile,
) -> Result<(), RepositoryError> {
    sqlx::query(
        "INSERT INTO candidates(id,alias,email,name,specialization,selected_grade,stack_json,team_role,experience_months,experience,soft_skills_json,processing_consent,publication_consent,verified_grade,verified_specialization,grade_confirmed_at,revision,updated_at) \
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)",
    )
    .bind(&value.id)
    .bind(&value.alias)
    .bind(&value.email)
    .bind(&value.name)
    .bind(specialization(value.specialization))
    .bind(grade(value.selected_grade))
    .bind(serde_json::to_string(&value.stack).map_err(json_storage)?)
    .bind(&value.team_role)
    .bind(value.experience_months.map(|v| v as i32))
    .bind(&value.experience)
    .bind(serde_json::to_string(&value.soft_skills).map_err(json_storage)?)
    .bind(value.consents.processing)
    .bind(value.consents.publication)
    .bind(value.verified_grade.map(grade))
    .bind(value.verified_specialization.map(specialization))
    .bind(ts_opt(&value.grade_confirmed_at)?)
    .bind(value.revision as i64)
    .bind(ts(&value.updated_at)?)
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    Ok(())
}

fn demo_candidates(now: &str) -> Vec<CandidateProfile> {
    [
        (
            "candidate018",
            "Кандидат №018",
            "Илья Соколов",
            Grade::Middle,
            vec!["rust", "docker"],
        ),
        (
            "candidate031",
            "Кандидат №031",
            "Мария Волкова",
            Grade::Senior,
            vec!["rust", "postgresql", "docker", "redis"],
        ),
        (
            "candidate041",
            "Кандидат №041",
            "Никита Орлов",
            Grade::Middle,
            vec!["react"],
        ),
    ]
    .into_iter()
    .map(
        |(id, alias, name, verified_grade, stack)| CandidateProfile {
            id: id.into(),
            alias: alias.into(),
            email: format!("{id}@example.com"),
            name: name.into(),
            specialization: if id == "candidate041" {
                Specialization::Frontend
            } else {
                Specialization::Backend
            },
            selected_grade: verified_grade,
            stack: stack.into_iter().map(String::from).collect(),
            team_role: "Разработчик".into(),
            experience_months: Some(36),
            experience_since: None,
            experience: "Коммерческая разработка и командные проекты.".into(),
            soft_skills: vec!["Командная работа".into()],
            consents: Consents {
                processing: true,
                publication: true,
            },
            verified_grade: Some(verified_grade),
            verified_specialization: Some(if id == "candidate041" {
                Specialization::Frontend
            } else {
                Specialization::Backend
            }),
            grade_confirmed_at: Some(now.into()),
            estimate: None,
            auto_fields: Vec::new(),
            revision: 1,
            updated_at: now.into(),
        },
    )
    .collect()
}

fn demo_candidate_indexes() -> Vec<(&'static str, Vec<SkillEvidence>, Vec<EvidenceSource>, bool)> {
    let skills = |items: &[(&str, Grade)]| {
        items
            .iter()
            .map(|(id, grade)| SkillEvidence {
                skill_id: (*id).into(),
                status: EvidenceStatus::Confirmed,
                evidence_grade: Some(*grade),
                sources: vec![EvidenceSource::Assessment],
            })
            .collect()
    };
    vec![
        (
            "candidate024",
            skills(&[
                ("rust", Grade::Junior),
                ("postgresql", Grade::Junior),
                ("docker", Grade::Junior),
            ]),
            vec![EvidenceSource::Assessment],
            false,
        ),
        (
            "candidate018",
            skills(&[("rust", Grade::Middle), ("docker", Grade::Middle)]),
            vec![EvidenceSource::Assessment, EvidenceSource::Fsp],
            true,
        ),
        (
            "candidate031",
            skills(&[
                ("rust", Grade::Senior),
                ("postgresql", Grade::Senior),
                ("docker", Grade::Middle),
                ("redis", Grade::Middle),
            ]),
            vec![EvidenceSource::Assessment, EvidenceSource::Fsp],
            true,
        ),
        (
            "candidate041",
            skills(&[("react", Grade::Middle)]),
            vec![EvidenceSource::Assessment],
            false,
        ),
    ]
}

fn requirement(skill_id: &str, criticality: Criticality, minimum_grade: Grade) -> Requirement {
    Requirement {
        skill_id: skill_id.into(),
        criticality,
        minimum_grade,
        evidence_policy: EvidencePolicy {
            accept_assessments: true,
            accept_github: true,
            accept_fsp: true,
            competition_selection: "system".into(),
            competition_ids: Vec::new(),
        },
    }
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
