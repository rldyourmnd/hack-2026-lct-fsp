//! PostgreSQL storage adapter for the platform repository port.
//!
//! One `PgPool` backs every operation; multi-statement use cases share a
//! `RepositoryTransaction` so partial commits are impossible. JSON fields stay
//! as text payloads because the SQL layer never inspects them; timestamps are
//! real `timestamptz` columns exchanged with the domain as RFC3339 strings.

mod migrations;
mod seed;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use fsp_application::{
    ConsumeChallenge, IdempotencyClaim, PendingChallenge, PlatformRepository, RepositoryError,
    RepositoryTransaction, StoredIdempotentResponse,
};
use fsp_domain::{
    Account, CandidateIndexRecord, CandidateProfile, Company, Consents, Grade, HiringNeedRecord,
    JobRecord, Role, SourceRecord, Specialization,
};
use serde::de::DeserializeOwned;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions, PgRow};
use sqlx::{Connection, PgConnection, Postgres, Row, Transaction};
use tokio::sync::Mutex;

/// Where repository calls run: either pooled one-shot connections or a shared
/// unit-of-work transaction. `transaction()` hands out the `Tx` variant.
#[derive(Clone)]
enum Conn {
    Pool(PgPool),
    Tx(Arc<Mutex<Option<Transaction<'static, Postgres>>>>),
}

/// PostgreSQL implementation of [`PlatformRepository`].
#[derive(Clone)]
pub struct PostgresRepository {
    conn: Conn,
}

/// A live transaction borrowed from the pool; exposes the full repository
/// surface. `commit()` persists; dropping rolls the transaction back.
struct TxRepository {
    repo: PostgresRepository,
    tx: Arc<Mutex<Option<Transaction<'static, Postgres>>>>,
}

/// Connection checkout backing one repository call: either a pooled
/// connection or the lock guard of a unit-of-work transaction.
enum ConnGuard<'a> {
    Owned(sqlx::pool::PoolConnection<Postgres>),
    Tx(tokio::sync::MutexGuard<'a, Option<Transaction<'static, Postgres>>>),
}

impl ConnGuard<'_> {
    fn get(&mut self) -> Result<&mut PgConnection, RepositoryError> {
        match self {
            ConnGuard::Owned(conn) => Ok(conn),
            ConnGuard::Tx(guard) => Ok(guard
                .as_mut()
                .ok_or_else(|| RepositoryError::Storage("transaction is finished".into()))?),
        }
    }
}

impl PostgresRepository {
    /// Connects to `url`, retries briefly so the service survives starting
    /// alongside PostgreSQL, then applies every pending migration.
    pub async fn connect(url: &str) -> Result<Self, RepositoryError> {
        let mut options = PgConnectOptions::from_str(url).map_err(storage)?;
        options = options.options([
            ("statement_timeout", "15s"),
            ("idle_in_transaction_session_timeout", "30s"),
        ]);
        let mut attempt = 0;
        let pool = loop {
            attempt += 1;
            match PgPoolOptions::new()
                .max_connections(8)
                .min_connections(1)
                .acquire_timeout(Duration::from_secs(5))
                .idle_timeout(Duration::from_secs(300))
                .max_lifetime(Duration::from_secs(1800))
                .connect_with(options.clone())
                .await
            {
                Ok(pool) => break pool,
                Err(error) if attempt < 10 => {
                    tracing::warn!(?error, attempt, "postgres connect failed; retrying");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
                Err(error) => return Err(storage(error)),
            }
        };
        migrations::run(&pool)
            .await
            .map_err(|error| RepositoryError::Storage(error.to_string()))?;
        Ok(Self {
            conn: Conn::Pool(pool),
        })
    }

    /// The connection this repository view is bound to: a pooled checkout or
    /// the shared unit-of-work transaction. `ConnGuard` derefs to
    /// `&mut PgConnection` for the ops functions.
    async fn conn(&self) -> Result<ConnGuard<'_>, RepositoryError> {
        match &self.conn {
            Conn::Pool(pool) => Ok(ConnGuard::Owned(pool.acquire().await.map_err(storage)?)),
            Conn::Tx(state) => Ok(ConnGuard::Tx(state.lock().await)),
        }
    }

    /// The underlying pool — used by tests for fixture inserts.
    #[cfg(feature = "testing")]
    pub fn pool(&self) -> &PgPool {
        match &self.conn {
            Conn::Pool(pool) => pool,
            Conn::Tx(_) => panic!("pool() is unavailable inside a transaction"),
        }
    }

    /// Creates an isolated `fsp_test_<ts>_<uuid>` database against the admin
    /// connection string `admin_url` (`FSP_TEST_DATABASE_URL`; the compose
    /// `db` service default is documented in `.env.example`). Idle test
    /// databases left behind by earlier runs are reaped first; live tests keep
    /// at least one pooled connection so their database is never reaped.
    #[cfg(feature = "testing")]
    pub async fn connect_test_database(admin_url: &str) -> (Self, TestDatabase) {
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(admin_url)
            .await
            .expect("test admin database accepts connections");
        let stale: Vec<String> = sqlx::query_scalar(
            "SELECT datname FROM pg_database d \
             WHERE datname ~ '^fsp_test_[0-9]+_[0-9a-f]{32}$' \
             AND NOT EXISTS (SELECT 1 FROM pg_stat_activity a WHERE a.datname = d.datname)",
        )
        .fetch_all(&admin)
        .await
        .expect("stale test databases are listed");
        // Names carry their creation timestamp: a database without sessions is only
        // reaped once it is older than STALE_SECS, so the create→first-connect window
        // of a parallel test does not look orphaned. Never force another test's
        // connection closed if it appears between listing and cleanup.
        const STALE_SECS: i64 = 300;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_secs() as i64;
        for stale_name in stale {
            let Some(rest) = stale_name.strip_prefix("fsp_test_") else {
                continue;
            };
            let Ok(created_at) = rest.split('_').next().unwrap_or_default().parse::<i64>() else {
                continue;
            };
            if now - created_at < STALE_SECS {
                continue;
            }
            let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{stale_name}\""))
                .execute(&admin)
                .await;
        }
        let name = format!("fsp_test_{}_{}", now, uuid::Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE DATABASE \"{name}\""))
            .execute(&admin)
            .await
            .expect("test database is created");
        admin.close().await;
        let mut url = admin_url.to_string();
        if let Some(slash) = url.rfind('/') {
            url.replace_range(slash + 1.., &name);
        }
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .min_connections(1)
            .connect(&url)
            .await
            .expect("test database accepts connections");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("test database runs migrations");
        (
            Self {
                conn: Conn::Pool(pool),
            },
            TestDatabase {
                name,
                url,
                admin_url: admin_url.to_string(),
            },
        )
    }
}

/// Handle to a per-test database created by [`PostgresRepository::connect_test_database`].
#[cfg(feature = "testing")]
pub struct TestDatabase {
    pub name: String,
    pub url: String,
    pub admin_url: String,
}

const ACCOUNT_COLUMNS: &str = "id,email,role,candidate_id,company_id,email_verified";
const CANDIDATE_COLUMNS: &str = "id,alias,email,name,specialization,selected_grade,stack_json,team_role,experience_months,experience,soft_skills_json,processing_consent,publication_consent,verified_grade,verified_specialization,grade_confirmed_at,revision,updated_at,estimate_json,auto_fields_json,experience_since";
const NEED_COLUMNS: &str = "id,company_id,title,specialization,grade,work_mode,salary_from,salary_to,description,requirements_json,published,published_at,created_at,updated_at,revision";
const SOURCE_COLUMNS: &str = "id,candidate_id,provider,synthetic,active_job_id,account_label,last_synced_at,analysis_json,verified_participation,competition_ids_json,fsp_results_json";
const JOB_COLUMNS: &str = "id,owner_id,kind,status,created_at,updated_at,candidate_id,assessment_id,input_revision,result_json,confirmed,result_source_id,error_code,error_message";
const INVITATION_COLUMNS: &str = "id,company_id,candidate_id,candidate_alias,company_json,need_json,need_definition_json,message,status,created_at,viewed_at,responded_at,disclosure_json";
const APPLICATION_COLUMNS: &str = "id,company_id,candidate_id,candidate_alias,company_json,vacancy_json,need_definition_json,message,status,created_at,viewed_at,responded_at,disclosure_json";
const TASK_COLUMNS: &str =
    "id,company_id,title,prompt,skill_id,grade,estimate_minutes,status,created_at";
const TASK_RESPONSE_COLUMNS: &str = "id,candidate_id,company_id,candidate_alias,task_json,mode,answer,status,feedback,submitted_at,reviewed_at";
const ASSESSMENT_COLUMNS: &str = "id,candidate_id,specialization,grade,status,task_json,answer_json,checks_json,result_json,created_at,updated_at,revision";
const SHORTLIST_COLUMNS: &str = "id,company_id,title,members_json,created_at,updated_at";
const DISCLOSURE_COLUMNS: &str =
    "candidate_id,company_id,invitation_id,application_id,granted_at,name,email,source_links_json";
const CANDIDATE_INDEX_SELECT: &str = "SELECT c.id,c.alias,c.email,c.name,c.specialization,c.selected_grade,c.stack_json,\
     c.team_role,c.experience_months,c.experience,c.soft_skills_json,c.processing_consent,\
     c.publication_consent,c.verified_grade,c.verified_specialization,c.grade_confirmed_at,c.estimate_json,c.auto_fields_json,c.experience_since,\
     c.revision,c.updated_at,\
     i.work_modes_json,i.available_for_offers,i.skills_json,i.source_kinds_json,\
     i.fsp_verified,i.last_signal_at,\
     (SELECT a.email_verified FROM accounts a WHERE a.candidate_id=c.id LIMIT 1) \
     AS account_email_verified \
     FROM candidates c LEFT JOIN candidate_index i ON i.candidate_id=c.id";

#[async_trait::async_trait]
impl PlatformRepository for PostgresRepository {
    async fn transaction(&self) -> Result<Box<dyn RepositoryTransaction>, RepositoryError> {
        let Conn::Pool(pool) = &self.conn else {
            return Err(RepositoryError::Storage(
                "nested transactions are not supported".into(),
            ));
        };
        let tx = pool.begin().await.map_err(storage)?;
        let shared = Arc::new(Mutex::new(Some(tx)));
        Ok(Box::new(TxRepository {
            repo: PostgresRepository {
                conn: Conn::Tx(Arc::clone(&shared)),
            },
            tx: shared,
        }))
    }

    async fn lock_scope(&self, scope: &str) -> Result<(), RepositoryError> {
        if !matches!(self.conn, Conn::Tx(_)) {
            return Err(RepositoryError::Storage(
                "aggregate lock requires a transaction".into(),
            ));
        }
        let mut conn = self.conn().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('aggregate:' || $1, 0))")
            .bind(scope)
            .execute(conn.get()?)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn lock_candidate(&self, candidate_id: &str) -> Result<(), RepositoryError> {
        if !matches!(self.conn, Conn::Tx(_)) {
            return Err(RepositoryError::Storage(
                "candidate lock requires a transaction".into(),
            ));
        }
        let mut conn = self.conn().await?;
        sqlx::query("SELECT id FROM candidates WHERE id=$1 FOR UPDATE")
            .bind(candidate_id)
            .fetch_optional(conn.get()?)
            .await
            .map_err(storage)?
            .ok_or(RepositoryError::NotFound)?;
        Ok(())
    }

    async fn candidate_index(
        &self,
        candidate_id: &str,
    ) -> Result<Option<CandidateIndexRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::load_candidate_index(conn.get()?, candidate_id).await
    }

    async fn task_response_for(
        &self,
        candidate_id: &str,
        task_id: &str,
    ) -> Result<Option<fsp_domain::TaskResponseRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::task_response_for(conn.get()?, candidate_id, task_id).await
    }

    async fn review_task_response(
        &self,
        response: &fsp_domain::TaskResponseRecord,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::review_task_response(conn.get()?, response).await
    }

    async fn save_candidate_guarded(
        &self,
        profile: &CandidateProfile,
        expected_revision: u64,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_candidate_guarded(conn.get()?, profile, expected_revision).await
    }

    async fn save_need_guarded(
        &self,
        need: &HiringNeedRecord,
        expected_revision: u64,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_need_guarded(conn.get()?, need, expected_revision).await
    }

    async fn save_invitation_guarded(
        &self,
        invitation: &fsp_domain::InvitationRecord,
        expected_statuses: &[fsp_domain::InteractionStatus],
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_invitation_guarded(conn.get()?, invitation, expected_statuses).await
    }

    async fn save_application_guarded(
        &self,
        application: &fsp_domain::ApplicationRecord,
        expected_statuses: &[fsp_domain::InteractionStatus],
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_application_guarded(conn.get()?, application, expected_statuses).await
    }

    async fn save_assessment_guarded(
        &self,
        assessment: &fsp_domain::AssessmentRecord,
        expected_revision: u64,
        expected_statuses: &[fsp_domain::AssessmentStatus],
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_assessment_guarded(
            conn.get()?,
            assessment,
            expected_revision,
            expected_statuses,
        )
        .await
    }

    async fn save_job_unconfirmed(&self, job: &JobRecord) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_job_unconfirmed(conn.get()?, job).await
    }

    async fn idempotency_claim(
        &self,
        scope: &str,
        fingerprint: &str,
    ) -> Result<IdempotencyClaim, RepositoryError> {
        if !matches!(self.conn, Conn::Tx(_)) {
            return Err(RepositoryError::Storage(
                "idempotency requires a transaction".into(),
            ));
        }
        let mut conn = self.conn().await?;
        ops::idempotency_claim(conn.get()?, scope, fingerprint).await
    }

    async fn idempotency_finish(
        &self,
        scope: &str,
        status: u16,
        body: &str,
    ) -> Result<(), RepositoryError> {
        if !matches!(self.conn, Conn::Tx(_)) {
            return Err(RepositoryError::Storage(
                "idempotency requires a transaction".into(),
            ));
        }
        let mut conn = self.conn().await?;
        ops::idempotency_finish(conn.get()?, scope, status, body).await
    }

    async fn sweep_expired(&self, now: &str) -> Result<u64, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::sweep_expired(conn.get()?, now).await
    }

    async fn seed_demo(&self, now: &str) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::seed_demo(conn.get()?, now).await
    }

    async fn account_by_email(&self, email: &str) -> Result<Option<Account>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::account_by_email(conn.get()?, email).await
    }

    async fn create_account(&self, value: &Account, now: &str) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::create_account(conn.get()?, value, now).await
    }

    async fn create_candidate(&self, value: &CandidateProfile) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::create_candidate(conn.get()?, value).await
    }

    async fn create_company(&self, value: &Company) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::create_company(conn.get()?, value).await
    }

    async fn issue_challenge(
        &self,
        challenge: &PendingChallenge,
        limit_delivery: bool,
    ) -> Result<fsp_application::IssueChallenge, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::issue_challenge(conn.get()?, challenge, limit_delivery).await
    }

    async fn mark_email_verified(&self, account_id: &str) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::mark_email_verified(conn.get()?, account_id).await
    }

    async fn ping(&self) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::ping(conn.get()?).await
    }

    async fn consume_challenge(
        &self,
        id: &str,
        code_hash: &str,
        now: &str,
    ) -> Result<(ConsumeChallenge, Option<PendingChallenge>), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::consume_challenge(conn.get()?, id, code_hash, now).await
    }

    async fn create_session(
        &self,
        token_hash: &str,
        account_id: &str,
        expires_at: &str,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::create_session(conn.get()?, token_hash, account_id, expires_at).await
    }

    async fn account_for_session(
        &self,
        token_hash: &str,
        now: &str,
    ) -> Result<Option<Account>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::account_for_session(conn.get()?, token_hash, now).await
    }

    async fn delete_session(&self, token_hash: &str) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::delete_session(conn.get()?, token_hash).await
    }

    async fn candidate(&self, id: &str) -> Result<Option<CandidateProfile>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::load_candidate(conn.get()?, id).await
    }

    async fn save_candidate(&self, value: &CandidateProfile) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_candidate(conn.get()?, value).await
    }

    async fn company(&self, id: &str) -> Result<Option<Company>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::company(conn.get()?, id).await
    }

    async fn save_company(&self, value: &Company) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_company(conn.get()?, value).await
    }

    async fn candidates(&self) -> Result<Vec<CandidateIndexRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::candidates(conn.get()?).await
    }

    async fn needs(&self, company_id: &str) -> Result<Vec<HiringNeedRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::needs(conn.get()?, company_id).await
    }

    async fn need(
        &self,
        company_id: &str,
        need_id: &str,
    ) -> Result<Option<HiringNeedRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::need(conn.get()?, company_id, need_id).await
    }

    async fn save_need(&self, value: &HiringNeedRecord) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_need(conn.get()?, value).await
    }

    async fn delete_need(&self, company_id: &str, need_id: &str) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::delete_need(conn.get()?, company_id, need_id).await
    }

    async fn published_needs(&self) -> Result<Vec<HiringNeedRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::published_needs(conn.get()?).await
    }

    async fn shortlists(
        &self,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::ShortlistRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::shortlists(conn.get()?, company_id).await
    }

    async fn shortlist(
        &self,
        company_id: &str,
        shortlist_id: &str,
    ) -> Result<Option<fsp_domain::ShortlistRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::shortlist(conn.get()?, company_id, shortlist_id).await
    }

    async fn save_shortlist(
        &self,
        value: &fsp_domain::ShortlistRecord,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_shortlist(conn.get()?, value).await
    }

    async fn delete_shortlist(
        &self,
        company_id: &str,
        shortlist_id: &str,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::delete_shortlist(conn.get()?, company_id, shortlist_id).await
    }

    async fn invitations(
        &self,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::InvitationRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::invitations(conn.get()?, company_id).await
    }

    async fn candidate_invitations(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::InvitationRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::candidate_invitations(conn.get()?, candidate_id).await
    }

    async fn invitation(
        &self,
        invitation_id: &str,
    ) -> Result<Option<fsp_domain::InvitationRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::invitation(conn.get()?, invitation_id).await
    }

    async fn save_invitation(
        &self,
        value: &fsp_domain::InvitationRecord,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_invitation(conn.get()?, value).await
    }

    async fn disclosure(
        &self,
        company_id: &str,
        candidate_id: &str,
    ) -> Result<Option<fsp_domain::DisclosureRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::disclosure(conn.get()?, company_id, candidate_id).await
    }

    async fn save_disclosure(
        &self,
        value: &fsp_domain::DisclosureRecord,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_disclosure(conn.get()?, value).await
    }

    async fn delete_disclosure(
        &self,
        company_id: &str,
        candidate_id: &str,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::delete_disclosure(conn.get()?, company_id, candidate_id).await
    }

    async fn company_applications(
        &self,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::ApplicationRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::company_applications(conn.get()?, company_id).await
    }

    async fn candidate_applications(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::ApplicationRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::candidate_applications(conn.get()?, candidate_id).await
    }

    async fn application(
        &self,
        application_id: &str,
    ) -> Result<Option<fsp_domain::ApplicationRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::application(conn.get()?, application_id).await
    }

    async fn save_application(
        &self,
        value: &fsp_domain::ApplicationRecord,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_application(conn.get()?, value).await
    }

    async fn tasks(
        &self,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::TaskRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::tasks(conn.get()?, company_id).await
    }

    async fn active_tasks(&self) -> Result<Vec<fsp_domain::TaskRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::active_tasks(conn.get()?).await
    }

    async fn task(&self, task_id: &str) -> Result<Option<fsp_domain::TaskRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::task(conn.get()?, task_id).await
    }

    async fn save_task(&self, value: &fsp_domain::TaskRecord) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_task(conn.get()?, value).await
    }

    async fn candidate_task_responses(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::TaskResponseRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::candidate_task_responses(conn.get()?, candidate_id).await
    }

    async fn company_task_responses(
        &self,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::TaskResponseRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::company_task_responses(conn.get()?, company_id).await
    }

    async fn task_response(
        &self,
        response_id: &str,
    ) -> Result<Option<fsp_domain::TaskResponseRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::task_response(conn.get()?, response_id).await
    }

    async fn save_task_response(
        &self,
        value: &fsp_domain::TaskResponseRecord,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_task_response(conn.get()?, value).await
    }

    async fn save_candidate_index(
        &self,
        record: &CandidateIndexRecord,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_candidate_index(conn.get()?, record).await
    }

    async fn candidate_assessments(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::AssessmentRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::candidate_assessments(conn.get()?, candidate_id).await
    }

    async fn assessment(
        &self,
        assessment_id: &str,
    ) -> Result<Option<fsp_domain::AssessmentRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::assessment(conn.get()?, assessment_id).await
    }

    async fn save_assessment(
        &self,
        value: &fsp_domain::AssessmentRecord,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_assessment(conn.get()?, value).await
    }

    async fn save_oauth_state(
        &self,
        candidate_id: &str,
        state: &str,
        expires_at: &str,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_oauth_state(conn.get()?, candidate_id, state, expires_at).await
    }

    async fn consume_oauth_state(
        &self,
        candidate_id: &str,
        state: &str,
        now: &str,
    ) -> Result<bool, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::consume_oauth_state(conn.get()?, candidate_id, state, now).await
    }

    async fn save_github_token(
        &self,
        candidate_id: &str,
        token: &str,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_github_token(conn.get()?, candidate_id, token).await
    }

    async fn github_token(&self, candidate_id: &str) -> Result<Option<String>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::github_token(conn.get()?, candidate_id).await
    }

    async fn sources(&self, candidate_id: &str) -> Result<Vec<SourceRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::sources(conn.get()?, candidate_id).await
    }

    async fn github_sources(&self, login: &str) -> Result<Vec<SourceRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::github_sources(conn.get()?, login).await
    }

    async fn save_source(&self, source: &SourceRecord) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_source(conn.get()?, source).await
    }

    async fn save_job(&self, job: &JobRecord) -> Result<(), RepositoryError> {
        let mut conn = self.conn().await?;
        ops::save_job(conn.get()?, job).await
    }

    async fn job(
        &self,
        owner_id: &str,
        job_id: &str,
    ) -> Result<Option<JobRecord>, RepositoryError> {
        let mut conn = self.conn().await?;
        ops::job(conn.get()?, owner_id, job_id).await
    }
}

#[async_trait::async_trait]
impl PlatformRepository for TxRepository {
    async fn transaction(&self) -> Result<Box<dyn RepositoryTransaction>, RepositoryError> {
        Err(RepositoryError::Storage(
            "nested transactions are not supported".into(),
        ))
    }

    async fn lock_scope(&self, scope: &str) -> Result<(), RepositoryError> {
        self.repo.lock_scope(scope).await
    }

    async fn lock_candidate(&self, candidate_id: &str) -> Result<(), RepositoryError> {
        self.repo.lock_candidate(candidate_id).await
    }

    async fn candidate_index(
        &self,
        candidate_id: &str,
    ) -> Result<Option<CandidateIndexRecord>, RepositoryError> {
        self.repo.candidate_index(candidate_id).await
    }

    async fn task_response_for(
        &self,
        candidate_id: &str,
        task_id: &str,
    ) -> Result<Option<fsp_domain::TaskResponseRecord>, RepositoryError> {
        self.repo.task_response_for(candidate_id, task_id).await
    }

    async fn review_task_response(
        &self,
        response: &fsp_domain::TaskResponseRecord,
    ) -> Result<(), RepositoryError> {
        self.repo.review_task_response(response).await
    }

    async fn save_candidate_guarded(
        &self,
        profile: &CandidateProfile,
        expected_revision: u64,
    ) -> Result<(), RepositoryError> {
        self.repo
            .save_candidate_guarded(profile, expected_revision)
            .await
    }

    async fn save_need_guarded(
        &self,
        need: &HiringNeedRecord,
        expected_revision: u64,
    ) -> Result<(), RepositoryError> {
        self.repo.save_need_guarded(need, expected_revision).await
    }

    async fn save_invitation_guarded(
        &self,
        invitation: &fsp_domain::InvitationRecord,
        expected_statuses: &[fsp_domain::InteractionStatus],
    ) -> Result<(), RepositoryError> {
        self.repo
            .save_invitation_guarded(invitation, expected_statuses)
            .await
    }

    async fn save_application_guarded(
        &self,
        application: &fsp_domain::ApplicationRecord,
        expected_statuses: &[fsp_domain::InteractionStatus],
    ) -> Result<(), RepositoryError> {
        self.repo
            .save_application_guarded(application, expected_statuses)
            .await
    }

    async fn save_assessment_guarded(
        &self,
        assessment: &fsp_domain::AssessmentRecord,
        expected_revision: u64,
        expected_statuses: &[fsp_domain::AssessmentStatus],
    ) -> Result<(), RepositoryError> {
        self.repo
            .save_assessment_guarded(assessment, expected_revision, expected_statuses)
            .await
    }

    async fn save_job_unconfirmed(&self, job: &JobRecord) -> Result<(), RepositoryError> {
        self.repo.save_job_unconfirmed(job).await
    }

    async fn idempotency_claim(
        &self,
        scope: &str,
        fingerprint: &str,
    ) -> Result<IdempotencyClaim, RepositoryError> {
        self.repo.idempotency_claim(scope, fingerprint).await
    }

    async fn idempotency_finish(
        &self,
        scope: &str,
        status: u16,
        body: &str,
    ) -> Result<(), RepositoryError> {
        self.repo.idempotency_finish(scope, status, body).await
    }

    async fn sweep_expired(&self, now: &str) -> Result<u64, RepositoryError> {
        self.repo.sweep_expired(now).await
    }

    async fn seed_demo(&self, now: &str) -> Result<(), RepositoryError> {
        self.repo.seed_demo(now).await
    }

    async fn account_by_email(&self, email: &str) -> Result<Option<Account>, RepositoryError> {
        self.repo.account_by_email(email).await
    }

    async fn create_account(&self, value: &Account, now: &str) -> Result<(), RepositoryError> {
        self.repo.create_account(value, now).await
    }

    async fn create_candidate(&self, value: &CandidateProfile) -> Result<(), RepositoryError> {
        self.repo.create_candidate(value).await
    }

    async fn create_company(&self, value: &Company) -> Result<(), RepositoryError> {
        self.repo.create_company(value).await
    }

    async fn issue_challenge(
        &self,
        challenge: &PendingChallenge,
        limit_delivery: bool,
    ) -> Result<fsp_application::IssueChallenge, RepositoryError> {
        self.repo.issue_challenge(challenge, limit_delivery).await
    }

    async fn mark_email_verified(&self, account_id: &str) -> Result<(), RepositoryError> {
        self.repo.mark_email_verified(account_id).await
    }

    async fn ping(&self) -> Result<(), RepositoryError> {
        self.repo.ping().await
    }

    async fn consume_challenge(
        &self,
        id: &str,
        code_hash: &str,
        now: &str,
    ) -> Result<(ConsumeChallenge, Option<PendingChallenge>), RepositoryError> {
        self.repo.consume_challenge(id, code_hash, now).await
    }

    async fn create_session(
        &self,
        token_hash: &str,
        account_id: &str,
        expires_at: &str,
    ) -> Result<(), RepositoryError> {
        self.repo
            .create_session(token_hash, account_id, expires_at)
            .await
    }

    async fn account_for_session(
        &self,
        token_hash: &str,
        now: &str,
    ) -> Result<Option<Account>, RepositoryError> {
        self.repo.account_for_session(token_hash, now).await
    }

    async fn delete_session(&self, token_hash: &str) -> Result<(), RepositoryError> {
        self.repo.delete_session(token_hash).await
    }

    async fn candidate(&self, id: &str) -> Result<Option<CandidateProfile>, RepositoryError> {
        self.repo.candidate(id).await
    }

    async fn save_candidate(&self, value: &CandidateProfile) -> Result<(), RepositoryError> {
        self.repo.save_candidate(value).await
    }

    async fn company(&self, id: &str) -> Result<Option<Company>, RepositoryError> {
        self.repo.company(id).await
    }

    async fn save_company(&self, value: &Company) -> Result<(), RepositoryError> {
        self.repo.save_company(value).await
    }

    async fn candidates(&self) -> Result<Vec<CandidateIndexRecord>, RepositoryError> {
        self.repo.candidates().await
    }

    async fn needs(&self, company_id: &str) -> Result<Vec<HiringNeedRecord>, RepositoryError> {
        self.repo.needs(company_id).await
    }

    async fn need(
        &self,
        company_id: &str,
        need_id: &str,
    ) -> Result<Option<HiringNeedRecord>, RepositoryError> {
        self.repo.need(company_id, need_id).await
    }

    async fn save_need(&self, value: &HiringNeedRecord) -> Result<(), RepositoryError> {
        self.repo.save_need(value).await
    }

    async fn delete_need(&self, company_id: &str, need_id: &str) -> Result<(), RepositoryError> {
        self.repo.delete_need(company_id, need_id).await
    }

    async fn published_needs(&self) -> Result<Vec<HiringNeedRecord>, RepositoryError> {
        self.repo.published_needs().await
    }

    async fn shortlists(
        &self,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::ShortlistRecord>, RepositoryError> {
        self.repo.shortlists(company_id).await
    }

    async fn shortlist(
        &self,
        company_id: &str,
        shortlist_id: &str,
    ) -> Result<Option<fsp_domain::ShortlistRecord>, RepositoryError> {
        self.repo.shortlist(company_id, shortlist_id).await
    }

    async fn save_shortlist(
        &self,
        value: &fsp_domain::ShortlistRecord,
    ) -> Result<(), RepositoryError> {
        self.repo.save_shortlist(value).await
    }

    async fn delete_shortlist(
        &self,
        company_id: &str,
        shortlist_id: &str,
    ) -> Result<(), RepositoryError> {
        self.repo.delete_shortlist(company_id, shortlist_id).await
    }

    async fn invitations(
        &self,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::InvitationRecord>, RepositoryError> {
        self.repo.invitations(company_id).await
    }

    async fn candidate_invitations(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::InvitationRecord>, RepositoryError> {
        self.repo.candidate_invitations(candidate_id).await
    }

    async fn invitation(
        &self,
        invitation_id: &str,
    ) -> Result<Option<fsp_domain::InvitationRecord>, RepositoryError> {
        self.repo.invitation(invitation_id).await
    }

    async fn save_invitation(
        &self,
        value: &fsp_domain::InvitationRecord,
    ) -> Result<(), RepositoryError> {
        self.repo.save_invitation(value).await
    }

    async fn disclosure(
        &self,
        company_id: &str,
        candidate_id: &str,
    ) -> Result<Option<fsp_domain::DisclosureRecord>, RepositoryError> {
        self.repo.disclosure(company_id, candidate_id).await
    }

    async fn save_disclosure(
        &self,
        value: &fsp_domain::DisclosureRecord,
    ) -> Result<(), RepositoryError> {
        self.repo.save_disclosure(value).await
    }

    async fn delete_disclosure(
        &self,
        company_id: &str,
        candidate_id: &str,
    ) -> Result<(), RepositoryError> {
        self.repo.delete_disclosure(company_id, candidate_id).await
    }

    async fn company_applications(
        &self,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::ApplicationRecord>, RepositoryError> {
        self.repo.company_applications(company_id).await
    }

    async fn candidate_applications(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::ApplicationRecord>, RepositoryError> {
        self.repo.candidate_applications(candidate_id).await
    }

    async fn application(
        &self,
        application_id: &str,
    ) -> Result<Option<fsp_domain::ApplicationRecord>, RepositoryError> {
        self.repo.application(application_id).await
    }

    async fn save_application(
        &self,
        value: &fsp_domain::ApplicationRecord,
    ) -> Result<(), RepositoryError> {
        self.repo.save_application(value).await
    }

    async fn tasks(
        &self,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::TaskRecord>, RepositoryError> {
        self.repo.tasks(company_id).await
    }

    async fn active_tasks(&self) -> Result<Vec<fsp_domain::TaskRecord>, RepositoryError> {
        self.repo.active_tasks().await
    }

    async fn task(&self, task_id: &str) -> Result<Option<fsp_domain::TaskRecord>, RepositoryError> {
        self.repo.task(task_id).await
    }

    async fn save_task(&self, value: &fsp_domain::TaskRecord) -> Result<(), RepositoryError> {
        self.repo.save_task(value).await
    }

    async fn candidate_task_responses(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::TaskResponseRecord>, RepositoryError> {
        self.repo.candidate_task_responses(candidate_id).await
    }

    async fn company_task_responses(
        &self,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::TaskResponseRecord>, RepositoryError> {
        self.repo.company_task_responses(company_id).await
    }

    async fn task_response(
        &self,
        response_id: &str,
    ) -> Result<Option<fsp_domain::TaskResponseRecord>, RepositoryError> {
        self.repo.task_response(response_id).await
    }

    async fn save_task_response(
        &self,
        value: &fsp_domain::TaskResponseRecord,
    ) -> Result<(), RepositoryError> {
        self.repo.save_task_response(value).await
    }

    async fn save_candidate_index(
        &self,
        record: &CandidateIndexRecord,
    ) -> Result<(), RepositoryError> {
        self.repo.save_candidate_index(record).await
    }

    async fn candidate_assessments(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::AssessmentRecord>, RepositoryError> {
        self.repo.candidate_assessments(candidate_id).await
    }

    async fn assessment(
        &self,
        assessment_id: &str,
    ) -> Result<Option<fsp_domain::AssessmentRecord>, RepositoryError> {
        self.repo.assessment(assessment_id).await
    }

    async fn save_assessment(
        &self,
        value: &fsp_domain::AssessmentRecord,
    ) -> Result<(), RepositoryError> {
        self.repo.save_assessment(value).await
    }

    async fn save_oauth_state(
        &self,
        candidate_id: &str,
        state: &str,
        expires_at: &str,
    ) -> Result<(), RepositoryError> {
        self.repo
            .save_oauth_state(candidate_id, state, expires_at)
            .await
    }

    async fn consume_oauth_state(
        &self,
        candidate_id: &str,
        state: &str,
        now: &str,
    ) -> Result<bool, RepositoryError> {
        self.repo
            .consume_oauth_state(candidate_id, state, now)
            .await
    }

    async fn save_github_token(
        &self,
        candidate_id: &str,
        token: &str,
    ) -> Result<(), RepositoryError> {
        self.repo.save_github_token(candidate_id, token).await
    }

    async fn github_token(&self, candidate_id: &str) -> Result<Option<String>, RepositoryError> {
        self.repo.github_token(candidate_id).await
    }

    async fn sources(&self, candidate_id: &str) -> Result<Vec<SourceRecord>, RepositoryError> {
        self.repo.sources(candidate_id).await
    }

    async fn github_sources(&self, login: &str) -> Result<Vec<SourceRecord>, RepositoryError> {
        self.repo.github_sources(login).await
    }

    async fn save_source(&self, source: &SourceRecord) -> Result<(), RepositoryError> {
        self.repo.save_source(source).await
    }

    async fn save_job(&self, job: &JobRecord) -> Result<(), RepositoryError> {
        self.repo.save_job(job).await
    }

    async fn job(
        &self,
        owner_id: &str,
        job_id: &str,
    ) -> Result<Option<JobRecord>, RepositoryError> {
        self.repo.job(owner_id, job_id).await
    }
}

#[async_trait::async_trait]
impl RepositoryTransaction for TxRepository {
    fn share(&self) -> Arc<dyn PlatformRepository> {
        Arc::new(self.repo.clone())
    }

    async fn commit(self: Box<Self>) -> Result<(), RepositoryError> {
        let tx = self
            .tx
            .lock()
            .await
            .take()
            .ok_or_else(|| RepositoryError::Storage("transaction is finished".into()))?;
        tx.commit().await.map_err(storage)
    }
}

mod ops {
    use super::*;

    /// Claims and completes inside the caller's transaction. A process crash
    /// rolls back the claim and the command together. The lock also serializes
    /// the absent-row case without stealing an active owner's work.
    pub(super) async fn idempotency_claim(
        conn: &mut PgConnection,
        scope: &str,
        fingerprint: &str,
    ) -> Result<IdempotencyClaim, RepositoryError> {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('idempotency:' || $1, 0))")
            .bind(scope)
            .execute(&mut *conn)
            .await
            .map_err(storage)?;
        let row = sqlx::query(
            "SELECT fingerprint,status,body_json,state FROM idempotency_keys WHERE scope=$1 FOR UPDATE",
        ).bind(scope).fetch_optional(&mut *conn).await.map_err(storage)?;
        if let Some(row) = row {
            if row.try_get::<String, _>("state").map_err(storage)? != "done" {
                // A visible running row can only come from the old release.
                // Its business effect is unknown: never replay it by guessing.
                return Ok(IdempotencyClaim::InFlight);
            }
            return Ok(IdempotencyClaim::Stored(Box::new(
                StoredIdempotentResponse {
                    fingerprint: row.try_get("fingerprint").map_err(storage)?,
                    status: row
                        .try_get::<i32, _>("status")
                        .map_err(storage)?
                        .try_into()
                        .map_err(|_| RepositoryError::Storage("invalid status".into()))?,
                    body: row.try_get("body_json").map_err(storage)?,
                },
            )));
        }
        sqlx::query("INSERT INTO idempotency_keys(scope,fingerprint,state,created_at) VALUES($1,$2,'running',now())")
            .bind(scope).bind(fingerprint).execute(conn).await.map_err(storage)?;
        Ok(IdempotencyClaim::Execute)
    }

    pub(super) async fn idempotency_finish(
        conn: &mut PgConnection,
        scope: &str,
        status: u16,
        body: &str,
    ) -> Result<(), RepositoryError> {
        let changed = sqlx::query(
            "UPDATE idempotency_keys SET state='done',status=$2,body_json=$3 WHERE scope=$1 AND state='running'",
        )
        .bind(scope)
        .bind(i32::from(status))
        .bind(body)
        .execute(&mut *conn)
        .await
        .map_err(storage)?.rows_affected();
        if changed != 1 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    /// Seeds the demo fixture once. A transaction-scoped advisory lock serializes
    /// concurrent startups: the second caller waits, then sees the committed
    /// accounts and skips seeding.
    pub(super) async fn seed_demo(
        conn: &mut PgConnection,
        now: &str,
    ) -> Result<(), RepositoryError> {
        let mut tx = conn.begin().await.map_err(storage)?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('fsp_demo_seed'))")
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts)")
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        if exists {
            tx.commit().await.map_err(storage)?;
            return Ok(());
        }
        seed::seed(&mut tx, now).await?;
        tx.commit().await.map_err(storage)
    }

    /// Single-candidate index read sharing `candidates()`' projection.
    pub(super) async fn load_candidate_index(
        conn: &mut PgConnection,
        candidate_id: &str,
    ) -> Result<Option<CandidateIndexRecord>, RepositoryError> {
        sqlx::query(&format!("{CANDIDATE_INDEX_SELECT} WHERE c.id=$1"))
            .bind(candidate_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(storage)?
            .map(|row| candidate_index(&row))
            .transpose()
    }

    /// The submit-time dedup read: the candidate's response for one task.
    pub(super) async fn task_response_for(
        conn: &mut PgConnection,
        candidate_id: &str,
        task_id: &str,
    ) -> Result<Option<fsp_domain::TaskResponseRecord>, RepositoryError> {
        sqlx::query(&format!(
        "SELECT {TASK_RESPONSE_COLUMNS} FROM task_responses WHERE candidate_id=$1 AND task_id=$2"
    ))
    .bind(candidate_id)
    .bind(task_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(storage)?
    .map(|row| task_response_record(&row))
    .transpose()
    }

    /// Review transition guarded on the row still awaiting review.
    pub(super) async fn review_task_response(
        conn: &mut PgConnection,
        response: &fsp_domain::TaskResponseRecord,
    ) -> Result<(), RepositoryError> {
        let changed = sqlx::query(
            "UPDATE task_responses SET status=$1,feedback=$2,reviewed_at=$3 \
         WHERE id=$4 AND status='submitted'",
        )
        .bind(response.status.as_str())
        .bind(&response.feedback)
        .bind(ts_opt(&response.reviewed_at)?)
        .bind(&response.id)
        .execute(&mut *conn)
        .await
        .map_err(storage)?
        .rows_affected();
        if changed == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    /// Candidate write guarded by the expected revision (optimistic concurrency).
    pub(super) async fn save_candidate_guarded(
        conn: &mut PgConnection,
        value: &CandidateProfile,
        expected_revision: u64,
    ) -> Result<(), RepositoryError> {
        let changed = sqlx::query(
        "UPDATE candidates SET alias=$1,email=$2,name=$3,specialization=$4,selected_grade=$5,stack_json=$6,team_role=$7,experience_months=$8,experience=$9,soft_skills_json=$10,processing_consent=$11,publication_consent=$12,verified_grade=$13,verified_specialization=$14,grade_confirmed_at=$15,revision=$16,updated_at=$17,estimate_json=$20,auto_fields_json=$21,experience_since=$22 WHERE id=$18 AND revision=$19",
    )
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
    .bind(&value.id)
    .bind(expected_revision as i64)
    .bind(estimate_json(value)?)
    .bind(serde_json::to_string(&value.auto_fields).map_err(json_storage)?)
    .bind(value.experience_since.map(i32::from))
    .execute(&mut *conn)
    .await
    .map_err(storage)?
    .rows_affected();
        if changed == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    /// Need write guarded by the expected revision and the owning company.
    pub(super) async fn save_need_guarded(
        conn: &mut PgConnection,
        value: &HiringNeedRecord,
        expected_revision: u64,
    ) -> Result<(), RepositoryError> {
        let changed = sqlx::query(
        "UPDATE needs SET title=$1,specialization=$2,grade=$3,work_mode=$4,salary_from=$5,salary_to=$6,description=$7,requirements_json=$8,published=$9,published_at=$10,updated_at=$11,revision=$12 WHERE id=$13 AND company_id=$14 AND revision=$15",
    )
    .bind(&value.title)
    .bind(specialization(value.need.specialization))
    .bind(grade(value.need.grade))
    .bind(&value.work_mode)
    .bind(value.salary_from as i64)
    .bind(value.salary_to as i64)
    .bind(&value.description)
    .bind(serde_json::to_string(&value.need.requirements).map_err(json_storage)?)
    .bind(value.published)
    .bind(ts_opt(&value.published_at)?)
    .bind(ts(&value.updated_at)?)
    .bind(value.revision as i64)
    .bind(&value.id)
    .bind(&value.company_id)
    .bind(expected_revision as i64)
    .execute(&mut *conn)
    .await
    .map_err(storage)?
    .rows_affected();
        if changed == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    /// Invitation transition guarded on the stored status.
    pub(super) async fn save_invitation_guarded(
        conn: &mut PgConnection,
        value: &fsp_domain::InvitationRecord,
        expected_statuses: &[fsp_domain::InteractionStatus],
    ) -> Result<(), RepositoryError> {
        let statuses: Vec<&str> = expected_statuses.iter().map(|s| s.as_str()).collect();
        let changed = sqlx::query(
            "UPDATE invitations SET status=$1,viewed_at=COALESCE($2,viewed_at),responded_at=$3,disclosure_json=$4 \
         WHERE id=$5 AND status = ANY($6)",
        )
        .bind(value.status.as_str())
        .bind(ts_opt(&value.viewed_at)?)
        .bind(ts_opt(&value.responded_at)?)
        .bind(serde_json::to_string(&value.disclosure_fields).map_err(json_storage)?)
        .bind(&value.id)
        .bind(statuses)
        .execute(&mut *conn)
        .await
        .map_err(storage)?
        .rows_affected();
        if changed == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    /// Application transition guarded on the stored status.
    pub(super) async fn save_application_guarded(
        conn: &mut PgConnection,
        value: &fsp_domain::ApplicationRecord,
        expected_statuses: &[fsp_domain::InteractionStatus],
    ) -> Result<(), RepositoryError> {
        let statuses: Vec<&str> = expected_statuses.iter().map(|s| s.as_str()).collect();
        let changed = sqlx::query(
            "UPDATE applications SET status=$1,viewed_at=COALESCE($2,viewed_at),responded_at=$3,disclosure_json=$4 \
         WHERE id=$5 AND status = ANY($6)",
        )
        .bind(value.status.as_str())
        .bind(ts_opt(&value.viewed_at)?)
        .bind(ts_opt(&value.responded_at)?)
        .bind(serde_json::to_string(&value.disclosure_fields).map_err(json_storage)?)
        .bind(&value.id)
        .bind(statuses)
        .execute(&mut *conn)
        .await
        .map_err(storage)?
        .rows_affected();
        if changed == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    /// Assessment write guarded on both revision and the editable status set.
    pub(super) async fn save_assessment_guarded(
        conn: &mut PgConnection,
        value: &fsp_domain::AssessmentRecord,
        expected_revision: u64,
        expected_statuses: &[fsp_domain::AssessmentStatus],
    ) -> Result<(), RepositoryError> {
        let statuses: Vec<&str> = expected_statuses.iter().map(|s| s.as_str()).collect();
        let changed = sqlx::query(
        "UPDATE assessments SET specialization=$1,grade=$2,status=$3,task_json=$4,answer_json=$5,checks_json=$6,result_json=$7,updated_at=$8,revision=$9 \
         WHERE id=$10 AND revision=$11 AND status = ANY($12)",
    )
    .bind(specialization(value.specialization))
    .bind(grade(value.grade))
    .bind(value.status.as_str())
    .bind(serde_json::to_string(&value.task).map_err(json_storage)?)
    .bind(serde_json::to_string(&value.answer).map_err(json_storage)?)
    .bind(serde_json::to_string(&value.checks).map_err(json_storage)?)
    .bind(value.result.as_ref().map(serde_json::to_string).transpose().map_err(json_storage)?)
    .bind(ts(&value.updated_at)?)
    .bind(value.revision as i64)
    .bind(&value.id)
    .bind(expected_revision as i64)
    .bind(statuses)
    .execute(&mut *conn)
    .await
    .map_err(storage)?
    .rows_affected();
        if changed == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    /// Job write that lands only while the import is still unconfirmed.
    pub(super) async fn save_job_unconfirmed(
        conn: &mut PgConnection,
        job: &JobRecord,
    ) -> Result<(), RepositoryError> {
        let changed = sqlx::query(
        "INSERT INTO jobs(id,owner_id,kind,status,created_at,updated_at,candidate_id,assessment_id,input_revision,result_json,confirmed,result_source_id,error_code,error_message) \
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14) \
         ON CONFLICT(id) DO UPDATE SET kind=excluded.kind,status=excluded.status,updated_at=excluded.updated_at,candidate_id=excluded.candidate_id,assessment_id=excluded.assessment_id,input_revision=excluded.input_revision,result_json=excluded.result_json,confirmed=excluded.confirmed,result_source_id=excluded.result_source_id,error_code=excluded.error_code,error_message=excluded.error_message \
         WHERE jobs.confirmed=FALSE",
    )
    .bind(&job.id)
    .bind(&job.owner_id)
    .bind(&job.kind)
    .bind(&job.status)
    .bind(ts(&job.created_at)?)
    .bind(ts(&job.updated_at)?)
    .bind(&job.candidate_id)
    .bind(&job.assessment_id)
    .bind(job.input_revision.map(|value| value as i64))
    .bind(&job.result_json)
    .bind(job.confirmed)
    .bind(&job.result_source_id)
    .bind(&job.error_code)
    .bind(&job.error_message)
    .execute(&mut *conn)
    .await
    .map_err(storage)?
    .rows_affected();
        if changed == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    /// Deletes expired sessions, challenges and OAuth states plus finished
    /// idempotency rows; legacy unresolved claims require investigation.
    pub(super) async fn sweep_expired(
        conn: &mut PgConnection,
        now: &str,
    ) -> Result<u64, RepositoryError> {
        let now_ts = ts(now)?;
        let mut removed = 0u64;
        for statement in [
            "DELETE FROM sessions WHERE expires_at<=$1",
            "DELETE FROM challenges WHERE expires_at<=$1",
            "DELETE FROM oauth_states WHERE expires_at<=$1",
            "DELETE FROM idempotency_keys WHERE state='done' AND created_at < $1 - interval '1 day'",
        ] {
            removed += sqlx::query(statement)
                .bind(now_ts)
                .execute(&mut *conn)
                .await
                .map_err(storage)?
                .rows_affected();
        }
        Ok(removed)
    }

    pub(super) async fn ping(conn: &mut PgConnection) -> Result<(), RepositoryError> {
        sqlx::query("SELECT 1")
            .execute(&mut *conn)
            .await
            .map_err(storage)?;
        Ok(())
    }
    pub(super) async fn account_by_email(
        conn: &mut PgConnection,
        email: &str,
    ) -> Result<Option<Account>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {ACCOUNT_COLUMNS} FROM accounts WHERE email=$1"
        ))
        .bind(email)
        .fetch_optional(&mut *conn)
        .await
        .map_err(storage)?
        .map(|row| account(&row))
        .transpose()
    }

    pub(super) async fn create_account(
        conn: &mut PgConnection,
        value: &Account,
        now: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO accounts(id,email,role,candidate_id,company_id,email_verified,created_at) VALUES($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(&value.id)
        .bind(&value.email)
        .bind(role(value.role))
        .bind(&value.candidate_id)
        .bind(&value.company_id)
        .bind(value.email_verified)
        .bind(ts(now)?)
        .execute(&mut *conn)
        .await
        .map_err(conflict)?;
        Ok(())
    }

    pub(super) async fn create_candidate(
        conn: &mut PgConnection,
        value: &CandidateProfile,
    ) -> Result<(), RepositoryError> {
        insert_candidate(conn, value).await
    }

    pub(super) async fn create_company(
        conn: &mut PgConnection,
        value: &Company,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO companies(id,display_name,business_area,description,contact_email,website) VALUES($1,$2,$3,$4,$5,$6)",
        )
        .bind(&value.id)
        .bind(&value.display_name)
        .bind(&value.business_area)
        .bind(&value.description)
        .bind(&value.contact_email)
        .bind(&value.website)
        .execute(&mut *conn)
        .await
        .map_err(conflict)?;
        Ok(())
    }

    /// Consumes a challenge under `FOR UPDATE`: the mock's verify order —
    /// expired, used, locked, then code — with the wrong-code attempt counted
    /// inside the same transaction. `Locked` only applies to a challenge that was
    /// already at five attempts on entry; the fifth wrong code still reports
    /// `WrongCode` and locks on the next try.
    pub(super) async fn consume_challenge(
        conn: &mut PgConnection,
        id: &str,
        code_hash: &str,
        now: &str,
    ) -> Result<(ConsumeChallenge, Option<PendingChallenge>), RepositoryError> {
        let mut transaction = conn.begin().await.map_err(storage)?;
        let email: Option<String> = sqlx::query_scalar("SELECT email FROM challenges WHERE id=$1")
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(storage)?;
        let Some(email) = email else {
            return Ok((ConsumeChallenge::Missing, None));
        };
        lock_email(&mut transaction, &email).await?;
        let challenge = sqlx::query(
        "SELECT email,role,code_hash,expires_at,attempts,used FROM challenges WHERE id=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(storage)?;
        let Some(row) = challenge else {
            return Ok((ConsumeChallenge::Missing, None));
        };
        let email: String = row.try_get("email").map_err(storage)?;
        let role = parse_role(&row.try_get::<String, _>("role").map_err(storage)?)?;
        let expected: String = row.try_get("code_hash").map_err(storage)?;
        let expires_at: DateTime<Utc> = row.try_get("expires_at").map_err(storage)?;
        let attempts: i32 = row.try_get("attempts").map_err(storage)?;
        let used: bool = row.try_get("used").map_err(storage)?;
        if expires_at <= ts(now)? {
            return Ok((ConsumeChallenge::Expired, None));
        }
        if used {
            return Ok((ConsumeChallenge::Used, None));
        }
        if challenge_attempts(&mut transaction, &email).await? >= 5 {
            return Ok((ConsumeChallenge::Locked, None));
        }
        if expected != code_hash {
            sqlx::query("UPDATE challenges SET attempts=attempts+1 WHERE id=$1")
                .bind(id)
                .execute(&mut *transaction)
                .await
                .map_err(storage)?;
            transaction.commit().await.map_err(storage)?;
            return Ok((ConsumeChallenge::WrongCode, None));
        }
        sqlx::query("UPDATE challenges SET used=TRUE WHERE id=$1")
            .bind(id)
            .execute(&mut *transaction)
            .await
            .map_err(storage)?;
        transaction.commit().await.map_err(storage)?;
        Ok((
            ConsumeChallenge::Consumed,
            Some(PendingChallenge {
                id: id.to_string(),
                email,
                role,
                code_hash: expected,
                expires_at: expires_at.to_rfc3339_opts(SecondsFormat::Millis, true),
                attempts: attempts as u8,
                used: true,
            }),
        ))
    }

    /// Issues a challenge under a cumulative lockout: the running sum of attempts
    /// across the email's live challenges must stay under five, so reissuing a
    /// code never resets the brute-force budget.
    pub(super) async fn issue_challenge(
        conn: &mut PgConnection,
        value: &PendingChallenge,
        limit_delivery: bool,
    ) -> Result<fsp_application::IssueChallenge, RepositoryError> {
        let mut transaction = conn.begin().await.map_err(storage)?;
        lock_email(&mut transaction, &value.email).await?;
        let spent = challenge_attempts(&mut transaction, &value.email).await?;
        if spent >= 5 {
            transaction.commit().await.map_err(storage)?;
            return Ok(fsp_application::IssueChallenge::Locked);
        }
        if limit_delivery {
            let limited: bool = sqlx::query_scalar(
                "SELECT count(*) >= 5 OR COALESCE(bool_or(created_at > now() - interval '1 minute'), false) FROM challenges WHERE email=$1 AND created_at > now() - interval '15 minutes'",
            ).bind(&value.email).fetch_one(&mut *transaction).await.map_err(storage)?;
            if limited {
                return Ok(fsp_application::IssueChallenge::RateLimited);
            }
        }
        sqlx::query(
        "INSERT INTO challenges(id,email,role,code_hash,expires_at,attempts,used) VALUES($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(&value.id)
    .bind(&value.email)
    .bind(crate::role(value.role))
    .bind(&value.code_hash)
    .bind(ts(&value.expires_at)?)
    .bind(i32::from(value.attempts))
    .bind(value.used)
    .execute(&mut *transaction)
    .await
    .map_err(storage)?;
        transaction.commit().await.map_err(storage)?;
        Ok(fsp_application::IssueChallenge::Issued)
    }

    // Issuance and verification use the same transaction-scoped lock. On a
    // successful verification it survives the savepoint through registration
    // and session creation in the outer unit of work.
    async fn lock_email(conn: &mut PgConnection, email: &str) -> Result<(), RepositoryError> {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('auth-email:' || $1, 0))")
            .bind(email)
            .execute(conn)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn challenge_attempts(
        conn: &mut PgConnection,
        email: &str,
    ) -> Result<i64, RepositoryError> {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(attempts),0) FROM challenges \
             WHERE email=$1 AND used=FALSE AND expires_at > now()",
        )
        .bind(email)
        .fetch_one(conn)
        .await
        .map_err(storage)
    }

    pub(super) async fn mark_email_verified(
        conn: &mut PgConnection,
        account_id: &str,
    ) -> Result<(), RepositoryError> {
        let changed = sqlx::query("UPDATE accounts SET email_verified=TRUE WHERE id=$1")
            .bind(account_id)
            .execute(&mut *conn)
            .await
            .map_err(storage)?
            .rows_affected();
        if changed == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    pub(super) async fn create_session(
        conn: &mut PgConnection,
        token_hash: &str,
        account_id: &str,
        expires_at: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query("INSERT INTO sessions(token_hash,account_id,expires_at) VALUES($1,$2,$3)")
            .bind(token_hash)
            .bind(account_id)
            .bind(ts(expires_at)?)
            .execute(&mut *conn)
            .await
            .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn account_for_session(
        conn: &mut PgConnection,
        token_hash: &str,
        now: &str,
    ) -> Result<Option<Account>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {ACCOUNT_COLUMNS} FROM sessions s JOIN accounts a ON a.id=s.account_id WHERE s.token_hash=$1 AND s.expires_at>$2"
        ))
        .bind(token_hash)
        .bind(ts(now)?)
        .fetch_optional(&mut *conn)
        .await
        .map_err(storage)?
        .map(|row| account(&row))
        .transpose()
    }

    pub(super) async fn delete_session(
        conn: &mut PgConnection,
        token_hash: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
            .bind(token_hash)
            .execute(&mut *conn)
            .await
            .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn load_candidate(
        conn: &mut PgConnection,
        id: &str,
    ) -> Result<Option<CandidateProfile>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {CANDIDATE_COLUMNS} FROM candidates WHERE id=$1"
        ))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(storage)?
        .map(|row| candidate(&row))
        .transpose()
    }

    pub(super) async fn save_candidate(
        conn: &mut PgConnection,
        value: &CandidateProfile,
    ) -> Result<(), RepositoryError> {
        update_candidate(conn, value).await
    }

    pub(super) async fn company(
        conn: &mut PgConnection,
        id: &str,
    ) -> Result<Option<Company>, RepositoryError> {
        sqlx::query(
            "SELECT id,display_name,business_area,description,contact_email,website FROM companies WHERE id=$1",
        )
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(storage)?
        .map(|row| {
            Ok(Company {
                id: row.try_get(0).map_err(storage)?,
                display_name: row.try_get(1).map_err(storage)?,
                business_area: row.try_get(2).map_err(storage)?,
                description: row.try_get(3).map_err(storage)?,
                contact_email: row.try_get(4).map_err(storage)?,
                website: row.try_get(5).map_err(storage)?,
            })
        })
        .transpose()
    }

    pub(super) async fn save_company(
        conn: &mut PgConnection,
        value: &Company,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "UPDATE companies SET display_name=$1,business_area=$2,description=$3,contact_email=$4,website=$5 WHERE id=$6",
        )
        .bind(&value.display_name)
        .bind(&value.business_area)
        .bind(&value.description)
        .bind(&value.contact_email)
        .bind(&value.website)
        .bind(&value.id)
        .execute(&mut *conn)
        .await
        .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn candidates(
        conn: &mut PgConnection,
    ) -> Result<Vec<CandidateIndexRecord>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT c.id,c.alias,c.email,c.name,c.specialization,c.selected_grade,c.stack_json,\
             c.team_role,c.experience_months,c.experience,c.soft_skills_json,c.processing_consent,\
             c.publication_consent,c.verified_grade,c.verified_specialization,c.grade_confirmed_at,c.estimate_json,c.auto_fields_json,c.experience_since,\
             c.revision,c.updated_at,\
             i.work_modes_json,i.available_for_offers,i.skills_json,i.source_kinds_json,\
             i.fsp_verified,i.last_signal_at,\
             (SELECT a.email_verified FROM accounts a WHERE a.candidate_id=c.id LIMIT 1) \
             AS account_email_verified \
             FROM candidates c LEFT JOIN candidate_index i ON i.candidate_id=c.id",
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?;
        rows.iter().map(candidate_index).collect()
    }

    pub(super) async fn needs(
        conn: &mut PgConnection,
        company_id: &str,
    ) -> Result<Vec<HiringNeedRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {NEED_COLUMNS} FROM needs WHERE company_id=$1 ORDER BY created_at,id"
        ))
        .bind(company_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?
        .iter()
        .map(need_record)
        .collect()
    }

    pub(super) async fn need(
        conn: &mut PgConnection,
        company_id: &str,
        need_id: &str,
    ) -> Result<Option<HiringNeedRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {NEED_COLUMNS} FROM needs WHERE company_id=$1 AND id=$2"
        ))
        .bind(company_id)
        .bind(need_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(storage)?
        .map(|row| need_record(&row))
        .transpose()
    }

    pub(super) async fn save_need(
        conn: &mut PgConnection,
        value: &HiringNeedRecord,
    ) -> Result<(), RepositoryError> {
        let requirements = serde_json::to_string(&value.need.requirements).map_err(json_storage)?;
        sqlx::query(
            "INSERT INTO needs(id,company_id,title,specialization,grade,work_mode,salary_from,salary_to,description,requirements_json,published,published_at,created_at,updated_at,revision) \
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15) \
             ON CONFLICT(id) DO UPDATE SET company_id=excluded.company_id,title=excluded.title,specialization=excluded.specialization,grade=excluded.grade,work_mode=excluded.work_mode,salary_from=excluded.salary_from,salary_to=excluded.salary_to,description=excluded.description,requirements_json=excluded.requirements_json,published=excluded.published,published_at=excluded.published_at,created_at=excluded.created_at,updated_at=excluded.updated_at,revision=excluded.revision",
        )
        .bind(&value.id)
        .bind(&value.company_id)
        .bind(&value.title)
        .bind(specialization(value.need.specialization))
        .bind(grade(value.need.grade))
        .bind(&value.work_mode)
        .bind(value.salary_from as i64)
        .bind(value.salary_to as i64)
        .bind(&value.description)
        .bind(requirements)
        .bind(value.published)
        .bind(ts_opt(&value.published_at)?)
        .bind(ts(&value.created_at)?)
        .bind(ts(&value.updated_at)?)
        .bind(value.revision as i64)
        .execute(&mut *conn)
        .await
        .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn delete_need(
        conn: &mut PgConnection,
        company_id: &str,
        need_id: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM needs WHERE id=$1 AND company_id=$2")
            .bind(need_id)
            .bind(company_id)
            .execute(&mut *conn)
            .await
            .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn published_needs(
        conn: &mut PgConnection,
    ) -> Result<Vec<HiringNeedRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {NEED_COLUMNS} FROM needs WHERE published=TRUE ORDER BY created_at,id"
        ))
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?
        .iter()
        .map(need_record)
        .collect()
    }

    pub(super) async fn shortlists(
        conn: &mut PgConnection,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::ShortlistRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {SHORTLIST_COLUMNS} FROM shortlists WHERE company_id=$1 ORDER BY seq"
        ))
        .bind(company_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?
        .iter()
        .map(shortlist_record)
        .collect()
    }

    pub(super) async fn shortlist(
        conn: &mut PgConnection,
        company_id: &str,
        shortlist_id: &str,
    ) -> Result<Option<fsp_domain::ShortlistRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {SHORTLIST_COLUMNS} FROM shortlists WHERE company_id=$1 AND id=$2"
        ))
        .bind(company_id)
        .bind(shortlist_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(storage)?
        .map(|row| shortlist_record(&row))
        .transpose()
    }

    pub(super) async fn save_shortlist(
        conn: &mut PgConnection,
        value: &fsp_domain::ShortlistRecord,
    ) -> Result<(), RepositoryError> {
        let members = serde_json::to_string(&value.member_ids).map_err(json_storage)?;
        sqlx::query(
            "INSERT INTO shortlists(id,company_id,title,members_json,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6) \
             ON CONFLICT(id) DO UPDATE SET company_id=excluded.company_id,title=excluded.title,members_json=excluded.members_json,created_at=excluded.created_at,updated_at=excluded.updated_at",
        )
        .bind(&value.id)
        .bind(&value.company_id)
        .bind(&value.title)
        .bind(members)
        .bind(ts(&value.created_at)?)
        .bind(ts(&value.updated_at)?)
        .execute(&mut *conn)
        .await
        .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn delete_shortlist(
        conn: &mut PgConnection,
        company_id: &str,
        shortlist_id: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM shortlists WHERE id=$1 AND company_id=$2")
            .bind(shortlist_id)
            .bind(company_id)
            .execute(&mut *conn)
            .await
            .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn invitations(
        conn: &mut PgConnection,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::InvitationRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {INVITATION_COLUMNS} FROM invitations WHERE company_id=$1 ORDER BY seq"
        ))
        .bind(company_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?
        .iter()
        .map(invitation_record)
        .collect()
    }

    pub(super) async fn candidate_invitations(
        conn: &mut PgConnection,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::InvitationRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {INVITATION_COLUMNS} FROM invitations WHERE candidate_id=$1 ORDER BY seq"
        ))
        .bind(candidate_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?
        .iter()
        .map(invitation_record)
        .collect()
    }

    pub(super) async fn invitation(
        conn: &mut PgConnection,
        invitation_id: &str,
    ) -> Result<Option<fsp_domain::InvitationRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {INVITATION_COLUMNS} FROM invitations WHERE id=$1"
        ))
        .bind(invitation_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(storage)?
        .map(|row| invitation_record(&row))
        .transpose()
    }

    pub(super) async fn save_invitation(
        conn: &mut PgConnection,
        value: &fsp_domain::InvitationRecord,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO invitations(id,company_id,candidate_id,need_id,candidate_alias,company_json,need_json,need_definition_json,message,status,created_at,viewed_at,responded_at,disclosure_json) \
             VALUES($1,$2,$3,$14,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) \
             ON CONFLICT(id) DO UPDATE SET company_id=excluded.company_id,candidate_id=excluded.candidate_id,need_id=excluded.need_id,candidate_alias=excluded.candidate_alias,company_json=excluded.company_json,need_json=excluded.need_json,need_definition_json=excluded.need_definition_json,message=excluded.message,status=excluded.status,created_at=excluded.created_at,viewed_at=excluded.viewed_at,responded_at=excluded.responded_at,disclosure_json=excluded.disclosure_json",
        )
        .bind(&value.id)
        .bind(&value.company_id)
        .bind(&value.candidate_id)
        .bind(&value.candidate_alias)
        .bind(serde_json::to_string(&value.company).map_err(json_storage)?)
        .bind(serde_json::to_string(&value.need).map_err(json_storage)?)
        .bind(serde_json::to_string(&value.need_definition).map_err(json_storage)?)
        .bind(&value.message)
        .bind(value.status.as_str())
        .bind(ts(&value.created_at)?)
        .bind(ts_opt(&value.viewed_at)?)
        .bind(ts_opt(&value.responded_at)?)
        .bind(serde_json::to_string(&value.disclosure_fields).map_err(json_storage)?)
        .bind(&value.need.id)
        .execute(&mut *conn)
        .await
        .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn disclosure(
        conn: &mut PgConnection,
        company_id: &str,
        candidate_id: &str,
    ) -> Result<Option<fsp_domain::DisclosureRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {DISCLOSURE_COLUMNS} FROM disclosures WHERE company_id=$1 AND candidate_id=$2"
        ))
        .bind(company_id)
        .bind(candidate_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(storage)?
        .map(|row| disclosure_record(&row))
        .transpose()
    }

    pub(super) async fn save_disclosure(
        conn: &mut PgConnection,
        value: &fsp_domain::DisclosureRecord,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO disclosures(company_id,candidate_id,invitation_id,application_id,granted_at,name,email,source_links_json) \
             VALUES($1,$2,$3,$4,$5,$6,$7,$8) \
             ON CONFLICT(company_id,candidate_id) DO UPDATE SET invitation_id=excluded.invitation_id,application_id=excluded.application_id,granted_at=excluded.granted_at,name=excluded.name,email=excluded.email,source_links_json=excluded.source_links_json",
        )
        .bind(&value.company_id)
        .bind(&value.candidate_id)
        .bind(&value.invitation_id)
        .bind(&value.application_id)
        .bind(ts_opt(&value.granted_at)?)
        .bind(&value.name)
        .bind(&value.email)
        .bind(serde_json::to_string(&value.source_links).map_err(json_storage)?)
        .execute(&mut *conn)
        .await
        .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn delete_disclosure(
        conn: &mut PgConnection,
        company_id: &str,
        candidate_id: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM disclosures WHERE company_id=$1 AND candidate_id=$2")
            .bind(company_id)
            .bind(candidate_id)
            .execute(&mut *conn)
            .await
            .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn company_applications(
        conn: &mut PgConnection,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::ApplicationRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {APPLICATION_COLUMNS} FROM applications WHERE company_id=$1 ORDER BY seq"
        ))
        .bind(company_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?
        .iter()
        .map(application_record)
        .collect()
    }

    pub(super) async fn candidate_applications(
        conn: &mut PgConnection,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::ApplicationRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {APPLICATION_COLUMNS} FROM applications WHERE candidate_id=$1 ORDER BY seq"
        ))
        .bind(candidate_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?
        .iter()
        .map(application_record)
        .collect()
    }

    pub(super) async fn application(
        conn: &mut PgConnection,
        application_id: &str,
    ) -> Result<Option<fsp_domain::ApplicationRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {APPLICATION_COLUMNS} FROM applications WHERE id=$1"
        ))
        .bind(application_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(storage)?
        .map(|row| application_record(&row))
        .transpose()
    }

    pub(super) async fn save_application(
        conn: &mut PgConnection,
        value: &fsp_domain::ApplicationRecord,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO applications(id,company_id,candidate_id,need_id,candidate_alias,company_json,vacancy_json,need_definition_json,message,status,created_at,viewed_at,responded_at,disclosure_json) \
             VALUES($1,$2,$3,$14,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) \
             ON CONFLICT(id) DO UPDATE SET company_id=excluded.company_id,candidate_id=excluded.candidate_id,need_id=excluded.need_id,candidate_alias=excluded.candidate_alias,company_json=excluded.company_json,vacancy_json=excluded.vacancy_json,need_definition_json=excluded.need_definition_json,message=excluded.message,status=excluded.status,created_at=excluded.created_at,viewed_at=excluded.viewed_at,responded_at=excluded.responded_at,disclosure_json=excluded.disclosure_json",
        )
        .bind(&value.id)
        .bind(&value.company_id)
        .bind(&value.candidate_id)
        .bind(&value.candidate_alias)
        .bind(serde_json::to_string(&value.company).map_err(json_storage)?)
        .bind(serde_json::to_string(&value.vacancy).map_err(json_storage)?)
        .bind(serde_json::to_string(&value.need_definition).map_err(json_storage)?)
        .bind(&value.message)
        .bind(value.status.as_str())
        .bind(ts(&value.created_at)?)
        .bind(ts_opt(&value.viewed_at)?)
        .bind(ts_opt(&value.responded_at)?)
        .bind(serde_json::to_string(&value.disclosure_fields).map_err(json_storage)?)
        .bind(&value.vacancy.id)
        .execute(&mut *conn)
        .await
        .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn tasks(
        conn: &mut PgConnection,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::TaskRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {TASK_COLUMNS} FROM tasks WHERE company_id=$1 ORDER BY created_at,id"
        ))
        .bind(company_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?
        .iter()
        .map(task_record)
        .collect()
    }

    pub(super) async fn active_tasks(
        conn: &mut PgConnection,
    ) -> Result<Vec<fsp_domain::TaskRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {TASK_COLUMNS} FROM tasks WHERE status='active' ORDER BY created_at,id"
        ))
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?
        .iter()
        .map(task_record)
        .collect()
    }

    pub(super) async fn task(
        conn: &mut PgConnection,
        task_id: &str,
    ) -> Result<Option<fsp_domain::TaskRecord>, RepositoryError> {
        sqlx::query(&format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id=$1"))
            .bind(task_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(storage)?
            .map(|row| task_record(&row))
            .transpose()
    }

    pub(super) async fn save_task(
        conn: &mut PgConnection,
        value: &fsp_domain::TaskRecord,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO tasks(id,company_id,title,prompt,skill_id,grade,estimate_minutes,status,created_at) \
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) \
             ON CONFLICT(id) DO UPDATE SET company_id=excluded.company_id,title=excluded.title,prompt=excluded.prompt,skill_id=excluded.skill_id,grade=excluded.grade,estimate_minutes=excluded.estimate_minutes,status=excluded.status,created_at=excluded.created_at",
        )
        .bind(&value.id)
        .bind(&value.company_id)
        .bind(&value.title)
        .bind(&value.prompt)
        .bind(&value.skill_id)
        .bind(grade(value.grade))
        .bind(value.estimate_minutes as i32)
        .bind(value.status.as_str())
        .bind(ts(&value.created_at)?)
        .execute(&mut *conn)
        .await
        .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn candidate_task_responses(
        conn: &mut PgConnection,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::TaskResponseRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {TASK_RESPONSE_COLUMNS} FROM task_responses WHERE candidate_id=$1 ORDER BY seq"
        ))
        .bind(candidate_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?
        .iter()
        .map(task_response_record)
        .collect()
    }

    pub(super) async fn company_task_responses(
        conn: &mut PgConnection,
        company_id: &str,
    ) -> Result<Vec<fsp_domain::TaskResponseRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {TASK_RESPONSE_COLUMNS} FROM task_responses WHERE company_id=$1 ORDER BY seq"
        ))
        .bind(company_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?
        .iter()
        .map(task_response_record)
        .collect()
    }

    pub(super) async fn task_response(
        conn: &mut PgConnection,
        response_id: &str,
    ) -> Result<Option<fsp_domain::TaskResponseRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {TASK_RESPONSE_COLUMNS} FROM task_responses WHERE id=$1"
        ))
        .bind(response_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(storage)?
        .map(|row| task_response_record(&row))
        .transpose()
    }

    pub(super) async fn save_task_response(
        conn: &mut PgConnection,
        value: &fsp_domain::TaskResponseRecord,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO task_responses(id,candidate_id,company_id,task_id,candidate_alias,task_json,mode,answer,status,feedback,submitted_at,reviewed_at) \
             VALUES($1,$2,$3,$12,$4,$5,$6,$7,$8,$9,$10,$11) \
             ON CONFLICT(id) DO UPDATE SET candidate_id=excluded.candidate_id,company_id=excluded.company_id,task_id=excluded.task_id,candidate_alias=excluded.candidate_alias,task_json=excluded.task_json,mode=excluded.mode,answer=excluded.answer,status=excluded.status,feedback=excluded.feedback,submitted_at=excluded.submitted_at,reviewed_at=excluded.reviewed_at",
        )
        .bind(&value.id)
        .bind(&value.candidate_id)
        .bind(&value.task.company_id)
        .bind(&value.candidate_alias)
        .bind(serde_json::to_string(&value.task).map_err(json_storage)?)
        .bind(value.mode.as_str())
        .bind(&value.answer)
        .bind(value.status.as_str())
        .bind(&value.feedback)
        .bind(ts(&value.submitted_at)?)
        .bind(ts_opt(&value.reviewed_at)?)
        .bind(&value.task.id)
        .execute(&mut *conn)
        .await
        .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn save_candidate_index(
        conn: &mut PgConnection,
        record: &CandidateIndexRecord,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO candidate_index(candidate_id,work_modes_json,available_for_offers,skills_json,source_kinds_json,fsp_verified,last_signal_at) \
             VALUES($1,$2,$3,$4,$5,$6,$7) \
             ON CONFLICT(candidate_id) DO UPDATE SET work_modes_json=excluded.work_modes_json,available_for_offers=excluded.available_for_offers,skills_json=excluded.skills_json,source_kinds_json=excluded.source_kinds_json,fsp_verified=excluded.fsp_verified,last_signal_at=excluded.last_signal_at",
        )
        .bind(&record.profile.id)
        .bind(serde_json::to_string(&record.work_modes).map_err(json_storage)?)
        .bind(record.available_for_offers)
        .bind(serde_json::to_string(&record.skills).map_err(json_storage)?)
        .bind(serde_json::to_string(&record.source_kinds).map_err(json_storage)?)
        .bind(record.fsp_verified)
        .bind(ts_opt(&record.last_signal_at)?)
        .execute(&mut *conn)
        .await
        .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn candidate_assessments(
        conn: &mut PgConnection,
        candidate_id: &str,
    ) -> Result<Vec<fsp_domain::AssessmentRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {ASSESSMENT_COLUMNS} FROM assessments WHERE candidate_id=$1 ORDER BY created_at,id"
        ))
        .bind(candidate_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?
        .iter()
        .map(assessment_record)
        .collect()
    }

    pub(super) async fn assessment(
        conn: &mut PgConnection,
        assessment_id: &str,
    ) -> Result<Option<fsp_domain::AssessmentRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {ASSESSMENT_COLUMNS} FROM assessments WHERE id=$1"
        ))
        .bind(assessment_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(storage)?
        .map(|row| assessment_record(&row))
        .transpose()
    }

    pub(super) async fn save_assessment(
        conn: &mut PgConnection,
        value: &fsp_domain::AssessmentRecord,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO assessments(id,candidate_id,specialization,grade,status,task_json,answer_json,checks_json,result_json,created_at,updated_at,revision) \
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) \
             ON CONFLICT(id) DO UPDATE SET candidate_id=excluded.candidate_id,specialization=excluded.specialization,grade=excluded.grade,status=excluded.status,task_json=excluded.task_json,answer_json=excluded.answer_json,checks_json=excluded.checks_json,result_json=excluded.result_json,created_at=excluded.created_at,updated_at=excluded.updated_at,revision=excluded.revision",
        )
        .bind(&value.id)
        .bind(&value.candidate_id)
        .bind(specialization(value.specialization))
        .bind(grade(value.grade))
        .bind(value.status.as_str())
        .bind(serde_json::to_string(&value.task).map_err(json_storage)?)
        .bind(serde_json::to_string(&value.answer).map_err(json_storage)?)
        .bind(serde_json::to_string(&value.checks).map_err(json_storage)?)
        .bind(value.result.as_ref().map(serde_json::to_string).transpose().map_err(json_storage)?)
        .bind(ts(&value.created_at)?)
        .bind(ts(&value.updated_at)?)
        .bind(value.revision as i64)
        .execute(&mut *conn)
        .await
        .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn save_oauth_state(
        conn: &mut PgConnection,
        candidate_id: &str,
        state: &str,
        expires_at: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO oauth_states(state,candidate_id,expires_at,used) VALUES($1,$2,$3,FALSE)",
        )
        .bind(state)
        .bind(candidate_id)
        .bind(ts(expires_at)?)
        .execute(&mut *conn)
        .await
        .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn consume_oauth_state(
        conn: &mut PgConnection,
        candidate_id: &str,
        state: &str,
        now: &str,
    ) -> Result<bool, RepositoryError> {
        let changed = sqlx::query(
            "UPDATE oauth_states SET used=TRUE WHERE state=$1 AND candidate_id=$2 AND expires_at>$3 AND used=FALSE",
        )
        .bind(state)
        .bind(candidate_id)
        .bind(ts(now)?)
        .execute(&mut *conn)
        .await
        .map_err(storage)?
        .rows_affected();
        Ok(changed == 1)
    }

    pub(super) async fn save_github_token(
        conn: &mut PgConnection,
        candidate_id: &str,
        token: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO github_credentials(candidate_id,access_token) VALUES($1,$2) \
             ON CONFLICT(candidate_id) DO UPDATE SET access_token=excluded.access_token",
        )
        .bind(candidate_id)
        .bind(token)
        .execute(&mut *conn)
        .await
        .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn github_token(
        conn: &mut PgConnection,
        candidate_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        sqlx::query("SELECT access_token FROM github_credentials WHERE candidate_id=$1")
            .bind(candidate_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(storage)?
            .map(|row| row.try_get::<String, _>(0).map_err(storage))
            .transpose()
    }

    pub(super) async fn sources(
        conn: &mut PgConnection,
        candidate_id: &str,
    ) -> Result<Vec<SourceRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {SOURCE_COLUMNS} FROM sources WHERE candidate_id=$1 ORDER BY id"
        ))
        .bind(candidate_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?
        .iter()
        .map(source_record)
        .collect()
    }

    pub(super) async fn github_sources(
        conn: &mut PgConnection,
        login: &str,
    ) -> Result<Vec<SourceRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {SOURCE_COLUMNS} FROM sources WHERE provider='github' AND lower(account_label)=lower($1) ORDER BY id"
        ))
        .bind(login)
        .fetch_all(&mut *conn)
        .await
        .map_err(storage)?
        .iter()
        .map(source_record)
        .collect()
    }

    pub(super) async fn save_source(
        conn: &mut PgConnection,
        source: &SourceRecord,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO sources(id,candidate_id,provider,synthetic,active_job_id,account_label,last_synced_at,analysis_json,verified_participation,competition_ids_json,fsp_results_json) \
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) \
             ON CONFLICT(id) DO UPDATE SET active_job_id=excluded.active_job_id,account_label=excluded.account_label,last_synced_at=excluded.last_synced_at,analysis_json=excluded.analysis_json,verified_participation=excluded.verified_participation,competition_ids_json=excluded.competition_ids_json,fsp_results_json=excluded.fsp_results_json",
        )
        .bind(&source.id)
        .bind(&source.candidate_id)
        .bind(source_provider(source.provider))
        .bind(source.synthetic)
        .bind(&source.active_job_id)
        .bind(&source.account_label)
        .bind(ts_opt(&source.last_synced_at)?)
        .bind(source.analysis.as_ref().map(serde_json::to_string).transpose().map_err(json_storage)?)
        .bind(source.verified_participation)
        .bind(serde_json::to_string(&source.competition_ids).map_err(json_storage)?)
        .bind(serde_json::to_string(&source.fsp_results).map_err(json_storage)?)
        .execute(&mut *conn)
        .await
        .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn save_job(
        conn: &mut PgConnection,
        job: &JobRecord,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO jobs(id,owner_id,kind,status,created_at,updated_at,candidate_id,assessment_id,input_revision,result_json,confirmed,result_source_id,error_code,error_message) \
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14) \
             ON CONFLICT(id) DO UPDATE SET kind=excluded.kind,status=excluded.status,updated_at=excluded.updated_at,candidate_id=excluded.candidate_id,assessment_id=excluded.assessment_id,input_revision=excluded.input_revision,result_json=excluded.result_json,confirmed=excluded.confirmed,result_source_id=excluded.result_source_id,error_code=excluded.error_code,error_message=excluded.error_message",
        )
        .bind(&job.id)
        .bind(&job.owner_id)
        .bind(&job.kind)
        .bind(&job.status)
        .bind(ts(&job.created_at)?)
        .bind(ts(&job.updated_at)?)
        .bind(&job.candidate_id)
        .bind(&job.assessment_id)
        .bind(job.input_revision.map(|value| value as i64))
        .bind(&job.result_json)
        .bind(job.confirmed)
        .bind(&job.result_source_id)
        .bind(&job.error_code)
        .bind(&job.error_message)
        .execute(&mut *conn)
        .await
        .map_err(storage)?;
        Ok(())
    }

    pub(super) async fn job(
        conn: &mut PgConnection,
        owner_id: &str,
        job_id: &str,
    ) -> Result<Option<JobRecord>, RepositoryError> {
        sqlx::query(&format!(
            "SELECT {JOB_COLUMNS} FROM jobs WHERE owner_id=$1 AND id=$2"
        ))
        .bind(owner_id)
        .bind(job_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(storage)?
        .map(|row| {
            Ok(JobRecord {
                id: row.try_get("id").map_err(storage)?,
                owner_id: row.try_get("owner_id").map_err(storage)?,
                kind: row.try_get("kind").map_err(storage)?,
                status: row.try_get("status").map_err(storage)?,
                created_at: row_ts(&row, "created_at")?,
                updated_at: row_ts(&row, "updated_at")?,
                candidate_id: row.try_get("candidate_id").map_err(storage)?,
                assessment_id: row.try_get("assessment_id").map_err(storage)?,
                input_revision: row
                    .try_get::<Option<i64>, _>("input_revision")
                    .map_err(storage)?
                    .map(|value| value as u64),
                result_json: row.try_get("result_json").map_err(storage)?,
                confirmed: row.try_get("confirmed").map_err(storage)?,
                result_source_id: row.try_get("result_source_id").map_err(storage)?,
                error_code: row.try_get("error_code").map_err(storage)?,
                error_message: row.try_get("error_message").map_err(storage)?,
            })
        })
        .transpose()
    }
}

pub(crate) async fn insert_candidate(
    conn: &mut PgConnection,
    value: &CandidateProfile,
) -> Result<(), RepositoryError> {
    sqlx::query(
        "INSERT INTO candidates(id,alias,email,name,specialization,selected_grade,stack_json,team_role,experience_months,experience,soft_skills_json,processing_consent,publication_consent,verified_grade,verified_specialization,grade_confirmed_at,revision,updated_at,estimate_json,auto_fields_json,experience_since) \
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21)",
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
    .bind(estimate_json(value)?)
    .bind(serde_json::to_string(&value.auto_fields).map_err(json_storage)?)
    .bind(value.experience_since.map(i32::from))
    .execute(&mut *conn)
    .await
    .map_err(conflict)?;
    Ok(())
}

pub(crate) async fn update_candidate(
    conn: &mut PgConnection,
    value: &CandidateProfile,
) -> Result<(), RepositoryError> {
    let changed = sqlx::query(
        "UPDATE candidates SET alias=$1,email=$2,name=$3,specialization=$4,selected_grade=$5,stack_json=$6,team_role=$7,experience_months=$8,experience=$9,soft_skills_json=$10,processing_consent=$11,publication_consent=$12,verified_grade=$13,verified_specialization=$14,grade_confirmed_at=$15,revision=$16,updated_at=$17,estimate_json=$19,auto_fields_json=$20,experience_since=$21 WHERE id=$18",
    )
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
    .bind(&value.id)
    .bind(estimate_json(value)?)
    .bind(serde_json::to_string(&value.auto_fields).map_err(json_storage)?)
    .bind(value.experience_since.map(i32::from))
    .execute(&mut *conn)
    .await
    .map_err(storage)?
    .rows_affected();
    if changed == 0 {
        return Err(RepositoryError::NotFound);
    }
    Ok(())
}

fn candidate_index(row: &PgRow) -> Result<CandidateIndexRecord, RepositoryError> {
    let profile = candidate_prefixed(row)?;
    let stored_skills: Option<String> = row.try_get("skills_json").map_err(storage)?;
    let skills = match stored_skills {
        Some(json) => parse_json(&json)?,
        None => profile
            .stack
            .iter()
            .map(|skill_id| fsp_domain::SkillEvidence {
                skill_id: skill_id.clone(),
                status: fsp_domain::EvidenceStatus::Observed,
                evidence_grade: None,
                sources: Vec::new(),
            })
            .collect(),
    };
    let work_modes = match row
        .try_get::<Option<String>, _>("work_modes_json")
        .map_err(storage)?
    {
        Some(json) => parse_json(&json)?,
        None => vec!["remote".to_string()],
    };
    let source_kinds = match row
        .try_get::<Option<String>, _>("source_kinds_json")
        .map_err(storage)?
    {
        Some(json) => parse_json(&json)?,
        None => Vec::new(),
    };
    Ok(CandidateIndexRecord {
        email_verified: row
            .try_get::<Option<bool>, _>("account_email_verified")
            .map_err(storage)?
            .unwrap_or(false),
        work_modes,
        available_for_offers: row
            .try_get::<Option<bool>, _>("available_for_offers")
            .map_err(storage)?
            .unwrap_or(false),
        skills,
        source_kinds,
        fsp_verified: row
            .try_get::<Option<bool>, _>("fsp_verified")
            .map_err(storage)?
            .unwrap_or(false),
        last_signal_at: row_ts_opt(row, "last_signal_at")?,
        profile,
    })
}

fn estimate_json(value: &CandidateProfile) -> Result<Option<String>, RepositoryError> {
    value
        .estimate
        .as_ref()
        .map(|estimate| serde_json::to_string(estimate).map_err(json_storage))
        .transpose()
}

fn candidate_prefixed(row: &PgRow) -> Result<CandidateProfile, RepositoryError> {
    Ok(CandidateProfile {
        estimate: row
            .try_get::<Option<String>, _>("estimate_json")
            .map_err(storage)?
            .as_deref()
            .map(parse_json)
            .transpose()?,
        auto_fields: parse_json(
            &row.try_get::<String, _>("auto_fields_json")
                .map_err(storage)?,
        )?,
        id: row.try_get("id").map_err(storage)?,
        alias: row.try_get("alias").map_err(storage)?,
        email: row.try_get("email").map_err(storage)?,
        name: row.try_get("name").map_err(storage)?,
        specialization: parse_specialization(
            &row.try_get::<String, _>("specialization")
                .map_err(storage)?,
        )?,
        selected_grade: parse_grade(
            &row.try_get::<String, _>("selected_grade")
                .map_err(storage)?,
        )?,
        stack: parse_json(&row.try_get::<String, _>("stack_json").map_err(storage)?)?,
        team_role: row.try_get("team_role").map_err(storage)?,
        experience_months: row
            .try_get::<Option<i32>, _>("experience_months")
            .map_err(storage)?
            .map(|v| v as u32),
        experience_since: row
            .try_get::<Option<i32>, _>("experience_since")
            .map_err(storage)?
            .and_then(|v| u16::try_from(v).ok()),
        experience: row.try_get("experience").map_err(storage)?,
        soft_skills: parse_json(
            &row.try_get::<String, _>("soft_skills_json")
                .map_err(storage)?,
        )?,
        consents: Consents {
            processing: row.try_get("processing_consent").map_err(storage)?,
            publication: row.try_get("publication_consent").map_err(storage)?,
        },
        verified_grade: row
            .try_get::<Option<String>, _>("verified_grade")
            .map_err(storage)?
            .as_deref()
            .map(parse_grade)
            .transpose()?,
        verified_specialization: row
            .try_get::<Option<String>, _>("verified_specialization")
            .map_err(storage)?
            .as_deref()
            .map(parse_specialization)
            .transpose()?,
        grade_confirmed_at: row_ts_opt(row, "grade_confirmed_at")?,
        revision: row.try_get::<i64, _>("revision").map_err(storage)? as u64,
        updated_at: row_ts(row, "updated_at")?,
    })
}

fn candidate(row: &PgRow) -> Result<CandidateProfile, RepositoryError> {
    candidate_prefixed(row)
}

fn account(row: &PgRow) -> Result<Account, RepositoryError> {
    Ok(Account {
        id: row.try_get("id").map_err(storage)?,
        email: row.try_get("email").map_err(storage)?,
        role: parse_role(row.try_get::<String, _>("role").map_err(storage)?.as_str())?,
        candidate_id: row.try_get("candidate_id").map_err(storage)?,
        company_id: row.try_get("company_id").map_err(storage)?,
        email_verified: row.try_get("email_verified").map_err(storage)?,
    })
}

fn need_record(row: &PgRow) -> Result<HiringNeedRecord, RepositoryError> {
    Ok(HiringNeedRecord {
        id: row.try_get("id").map_err(storage)?,
        company_id: row.try_get("company_id").map_err(storage)?,
        title: row.try_get("title").map_err(storage)?,
        need: fsp_domain::HiringNeed {
            specialization: parse_specialization(
                &row.try_get::<String, _>("specialization")
                    .map_err(storage)?,
            )?,
            grade: parse_grade(&row.try_get::<String, _>("grade").map_err(storage)?)?,
            requirements: parse_json(
                &row.try_get::<String, _>("requirements_json")
                    .map_err(storage)?,
            )?,
        },
        work_mode: row.try_get("work_mode").map_err(storage)?,
        salary_from: row.try_get::<i64, _>("salary_from").map_err(storage)? as u64,
        salary_to: row.try_get::<i64, _>("salary_to").map_err(storage)? as u64,
        description: row.try_get("description").map_err(storage)?,
        published: row.try_get("published").map_err(storage)?,
        published_at: row_ts_opt(row, "published_at")?,
        created_at: row_ts(row, "created_at")?,
        updated_at: row_ts(row, "updated_at")?,
        revision: row.try_get::<i64, _>("revision").map_err(storage)? as u64,
    })
}

fn shortlist_record(row: &PgRow) -> Result<fsp_domain::ShortlistRecord, RepositoryError> {
    Ok(fsp_domain::ShortlistRecord {
        id: row.try_get("id").map_err(storage)?,
        company_id: row.try_get("company_id").map_err(storage)?,
        title: row.try_get("title").map_err(storage)?,
        member_ids: parse_json(&row.try_get::<String, _>("members_json").map_err(storage)?)?,
        created_at: row_ts(row, "created_at")?,
        updated_at: row_ts(row, "updated_at")?,
    })
}

fn invitation_record(row: &PgRow) -> Result<fsp_domain::InvitationRecord, RepositoryError> {
    let status: String = row.try_get("status").map_err(storage)?;
    Ok(fsp_domain::InvitationRecord {
        id: row.try_get("id").map_err(storage)?,
        company_id: row.try_get("company_id").map_err(storage)?,
        candidate_id: row.try_get("candidate_id").map_err(storage)?,
        candidate_alias: row.try_get("candidate_alias").map_err(storage)?,
        company: parse_json(&row.try_get::<String, _>("company_json").map_err(storage)?)?,
        need: parse_json(&row.try_get::<String, _>("need_json").map_err(storage)?)?,
        need_definition: parse_json(
            &row.try_get::<String, _>("need_definition_json")
                .map_err(storage)?,
        )?,
        message: row.try_get("message").map_err(storage)?,
        status: fsp_domain::InteractionStatus::parse(&status).ok_or_else(|| {
            RepositoryError::Storage(format!("unknown invitation status {status}"))
        })?,
        created_at: row_ts(row, "created_at")?,
        viewed_at: row_ts_opt(row, "viewed_at")?,
        responded_at: row_ts_opt(row, "responded_at")?,
        disclosure_fields: parse_json(
            &row.try_get::<String, _>("disclosure_json")
                .map_err(storage)?,
        )?,
    })
}

fn application_record(row: &PgRow) -> Result<fsp_domain::ApplicationRecord, RepositoryError> {
    let status: String = row.try_get("status").map_err(storage)?;
    Ok(fsp_domain::ApplicationRecord {
        id: row.try_get("id").map_err(storage)?,
        company_id: row.try_get("company_id").map_err(storage)?,
        candidate_id: row.try_get("candidate_id").map_err(storage)?,
        candidate_alias: row.try_get("candidate_alias").map_err(storage)?,
        company: parse_json(&row.try_get::<String, _>("company_json").map_err(storage)?)?,
        vacancy: parse_json(&row.try_get::<String, _>("vacancy_json").map_err(storage)?)?,
        need_definition: parse_json(
            &row.try_get::<String, _>("need_definition_json")
                .map_err(storage)?,
        )?,
        message: row.try_get("message").map_err(storage)?,
        status: fsp_domain::InteractionStatus::parse(&status).ok_or_else(|| {
            RepositoryError::Storage(format!("unknown application status {status}"))
        })?,
        created_at: row_ts(row, "created_at")?,
        viewed_at: row_ts_opt(row, "viewed_at")?,
        responded_at: row_ts_opt(row, "responded_at")?,
        disclosure_fields: parse_json(
            &row.try_get::<String, _>("disclosure_json")
                .map_err(storage)?,
        )?,
    })
}

fn disclosure_record(row: &PgRow) -> Result<fsp_domain::DisclosureRecord, RepositoryError> {
    Ok(fsp_domain::DisclosureRecord {
        candidate_id: row.try_get("candidate_id").map_err(storage)?,
        company_id: row.try_get("company_id").map_err(storage)?,
        invitation_id: row.try_get("invitation_id").map_err(storage)?,
        application_id: row.try_get("application_id").map_err(storage)?,
        granted_at: row_ts_opt(row, "granted_at")?,
        name: row.try_get("name").map_err(storage)?,
        email: row.try_get("email").map_err(storage)?,
        source_links: parse_json(
            &row.try_get::<String, _>("source_links_json")
                .map_err(storage)?,
        )?,
    })
}

fn task_record(row: &PgRow) -> Result<fsp_domain::TaskRecord, RepositoryError> {
    let status: String = row.try_get("status").map_err(storage)?;
    Ok(fsp_domain::TaskRecord {
        id: row.try_get("id").map_err(storage)?,
        company_id: row.try_get("company_id").map_err(storage)?,
        title: row.try_get("title").map_err(storage)?,
        prompt: row.try_get("prompt").map_err(storage)?,
        skill_id: row.try_get("skill_id").map_err(storage)?,
        grade: parse_grade(&row.try_get::<String, _>("grade").map_err(storage)?)?,
        estimate_minutes: row.try_get::<i32, _>("estimate_minutes").map_err(storage)? as u32,
        status: fsp_domain::TaskStatus::parse(&status)
            .ok_or_else(|| RepositoryError::Storage(format!("unknown task status {status}")))?,
        created_at: row_ts(row, "created_at")?,
    })
}

fn task_response_record(row: &PgRow) -> Result<fsp_domain::TaskResponseRecord, RepositoryError> {
    let mode: String = row.try_get("mode").map_err(storage)?;
    let status: String = row.try_get("status").map_err(storage)?;
    Ok(fsp_domain::TaskResponseRecord {
        id: row.try_get("id").map_err(storage)?,
        candidate_id: row.try_get("candidate_id").map_err(storage)?,
        candidate_alias: row.try_get("candidate_alias").map_err(storage)?,
        task: parse_json(&row.try_get::<String, _>("task_json").map_err(storage)?)?,
        mode: fsp_domain::TaskMode::parse(&mode)
            .ok_or_else(|| RepositoryError::Storage(format!("unknown task mode {mode}")))?,
        answer: row.try_get("answer").map_err(storage)?,
        status: fsp_domain::TaskResponseStatus::parse(&status).ok_or_else(|| {
            RepositoryError::Storage(format!("unknown task response status {status}"))
        })?,
        feedback: row.try_get("feedback").map_err(storage)?,
        submitted_at: row_ts(row, "submitted_at")?,
        reviewed_at: row_ts_opt(row, "reviewed_at")?,
    })
}

fn assessment_record(row: &PgRow) -> Result<fsp_domain::AssessmentRecord, RepositoryError> {
    let status: String = row.try_get("status").map_err(storage)?;
    Ok(fsp_domain::AssessmentRecord {
        id: row.try_get("id").map_err(storage)?,
        candidate_id: row.try_get("candidate_id").map_err(storage)?,
        specialization: parse_specialization(
            &row.try_get::<String, _>("specialization")
                .map_err(storage)?,
        )?,
        grade: parse_grade(&row.try_get::<String, _>("grade").map_err(storage)?)?,
        status: fsp_domain::AssessmentStatus::parse(&status).ok_or_else(|| {
            RepositoryError::Storage(format!("unknown assessment status {status}"))
        })?,
        task: parse_json(&row.try_get::<String, _>("task_json").map_err(storage)?)?,
        answer: parse_json(&row.try_get::<String, _>("answer_json").map_err(storage)?)?,
        checks: parse_json(&row.try_get::<String, _>("checks_json").map_err(storage)?)?,
        result: row
            .try_get::<Option<String>, _>("result_json")
            .map_err(storage)?
            .map(|json| parse_json(&json))
            .transpose()?,
        created_at: row_ts(row, "created_at")?,
        updated_at: row_ts(row, "updated_at")?,
        revision: row.try_get::<i64, _>("revision").map_err(storage)? as u64,
    })
}

fn source_record(row: &PgRow) -> Result<SourceRecord, RepositoryError> {
    Ok(SourceRecord {
        fsp_results: parse_json(
            &row.try_get::<String, _>("fsp_results_json")
                .map_err(storage)?,
        )?,
        id: row.try_get("id").map_err(storage)?,
        candidate_id: row.try_get("candidate_id").map_err(storage)?,
        provider: parse_source_provider(&row.try_get::<String, _>("provider").map_err(storage)?)?,
        synthetic: row.try_get("synthetic").map_err(storage)?,
        active_job_id: row.try_get("active_job_id").map_err(storage)?,
        account_label: row.try_get("account_label").map_err(storage)?,
        last_synced_at: row_ts_opt(row, "last_synced_at")?,
        analysis: row
            .try_get::<Option<String>, _>("analysis_json")
            .map_err(storage)?
            .map(|json| parse_json(&json))
            .transpose()?,
        verified_participation: row.try_get("verified_participation").map_err(storage)?,
        competition_ids: parse_json(
            &row.try_get::<String, _>("competition_ids_json")
                .map_err(storage)?,
        )?,
    })
}

fn parse_json<T: DeserializeOwned>(value: &str) -> Result<T, RepositoryError> {
    serde_json::from_str(value)
        .map_err(|error| RepositoryError::Storage(format!("stored json is invalid: {error}")))
}

fn json_storage(error: serde_json::Error) -> RepositoryError {
    RepositoryError::Storage(error.to_string())
}

fn role(value: Role) -> &'static str {
    match value {
        Role::Candidate => "candidate",
        Role::Employer => "employer",
    }
}
fn parse_role(value: &str) -> Result<Role, RepositoryError> {
    match value {
        "candidate" => Ok(Role::Candidate),
        "employer" => Ok(Role::Employer),
        _ => Err(RepositoryError::Storage(format!("unknown role: {value}"))),
    }
}
fn grade(value: Grade) -> &'static str {
    value.as_str()
}
fn parse_grade(value: &str) -> Result<Grade, RepositoryError> {
    value
        .parse()
        .map_err(|_| RepositoryError::Storage(format!("unknown grade: {value}")))
}
fn specialization(value: Specialization) -> &'static str {
    value.as_str()
}
fn parse_specialization(value: &str) -> Result<Specialization, RepositoryError> {
    value
        .parse()
        .map_err(|_| RepositoryError::Storage(format!("unknown specialization: {value}")))
}
fn source_provider(value: fsp_domain::SourceProvider) -> &'static str {
    match value {
        fsp_domain::SourceProvider::Github => "github",
        fsp_domain::SourceProvider::Fsp => "fsp",
    }
}
fn parse_source_provider(value: &str) -> Result<fsp_domain::SourceProvider, RepositoryError> {
    match value {
        "github" => Ok(fsp_domain::SourceProvider::Github),
        "fsp" => Ok(fsp_domain::SourceProvider::Fsp),
        _ => Err(RepositoryError::Storage(format!(
            "unknown source provider: {value}"
        ))),
    }
}

/// Binds an RFC3339 domain timestamp into a `timestamptz` parameter.
fn ts(value: &str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(value)
        .map(|parsed| parsed.with_timezone(&Utc))
        .map_err(|error| {
            RepositoryError::Storage(format!("timestamp is not RFC3339: {value}: {error}"))
        })
}

fn ts_opt(value: &Option<String>) -> Result<Option<DateTime<Utc>>, RepositoryError> {
    value.as_deref().map(ts).transpose()
}

/// Reads a `timestamptz` column back into the domain's RFC3339 string form.
fn row_ts(row: &PgRow, column: &str) -> Result<String, RepositoryError> {
    Ok(row
        .try_get::<DateTime<Utc>, _>(column)
        .map_err(storage)?
        .to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn row_ts_opt(row: &PgRow, column: &str) -> Result<Option<String>, RepositoryError> {
    row.try_get::<Option<DateTime<Utc>>, _>(column)
        .map_err(storage)?
        .map(|value| value.to_rfc3339_opts(SecondsFormat::Millis, true))
        .map_or(Ok(None), |value| Ok(Some(value)))
}

fn storage(error: sqlx::Error) -> RepositoryError {
    match error {
        sqlx::Error::Database(ref inner) => match inner.code().as_deref() {
            Some("23505") => RepositoryError::Conflict,
            Some("23503") | Some("23502") => RepositoryError::NotFound,
            // Serialization failure / deadlock: safe to retry.
            Some("40001") | Some("40P01") => RepositoryError::Unavailable,
            _ => RepositoryError::Storage(inner.to_string()),
        },
        sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed | sqlx::Error::Io(_) => {
            RepositoryError::Unavailable
        }
        other => RepositoryError::Storage(other.to_string()),
    }
}

fn conflict(error: sqlx::Error) -> RepositoryError {
    storage(error)
}
