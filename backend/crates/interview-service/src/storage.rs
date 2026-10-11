use std::collections::BTreeMap;

use async_trait::async_trait;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tokio::sync::RwLock;

use crate::domain::InterviewSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryError {
    Conflict,
    Unavailable,
}

#[async_trait]
pub trait InterviewRepository: Send + Sync {
    async fn insert(&self, session: &InterviewSession) -> Result<(), RepositoryError>;
    async fn get(&self, id: &str) -> Result<Option<InterviewSession>, RepositoryError>;
    async fn list(&self, candidate_id: &str) -> Result<Vec<InterviewSession>, RepositoryError>;
    async fn replace(
        &self,
        session: &InterviewSession,
        expected_revision: u32,
    ) -> Result<(), RepositoryError>;
    async fn healthy(&self) -> Result<(), RepositoryError>;
}

#[derive(Default)]
pub struct MemoryInterviewRepository {
    sessions: RwLock<BTreeMap<String, InterviewSession>>,
}

#[async_trait]
impl InterviewRepository for MemoryInterviewRepository {
    async fn insert(&self, session: &InterviewSession) -> Result<(), RepositoryError> {
        let mut sessions = self.sessions.write().await;
        if sessions.contains_key(&session.id)
            || sessions.values().any(|current| {
                current.candidate_id == session.candidate_id
                    && current.status == crate::domain::InterviewStatus::Active
            })
        {
            return Err(RepositoryError::Conflict);
        }
        sessions.insert(session.id.clone(), session.clone());
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<InterviewSession>, RepositoryError> {
        Ok(self.sessions.read().await.get(id).cloned())
    }

    async fn list(&self, candidate_id: &str) -> Result<Vec<InterviewSession>, RepositoryError> {
        let mut items = self
            .sessions
            .read()
            .await
            .values()
            .filter(|session| session.candidate_id == candidate_id)
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| right.id.cmp(&left.id));
        Ok(items)
    }

    async fn replace(
        &self,
        session: &InterviewSession,
        expected_revision: u32,
    ) -> Result<(), RepositoryError> {
        let mut sessions = self.sessions.write().await;
        let current = sessions.get(&session.id).ok_or(RepositoryError::Conflict)?;
        if current.revision != expected_revision {
            return Err(RepositoryError::Conflict);
        }
        sessions.insert(session.id.clone(), session.clone());
        Ok(())
    }

    async fn healthy(&self) -> Result<(), RepositoryError> {
        Ok(())
    }
}

pub struct PostgresInterviewRepository {
    pool: PgPool,
}

impl PostgresInterviewRepository {
    pub async fn connect(database_url: &str) -> Result<Self, RepositoryError> {
        let options = database_url
            .parse()
            .map_err(|_| RepositoryError::Unavailable)?;
        Self::connect_with(options).await
    }

    pub async fn connect_with(options: PgConnectOptions) -> Result<Self, RepositoryError> {
        let pool = PgPoolOptions::new()
            .connect_with(options)
            .await
            .map_err(|_| RepositoryError::Unavailable)?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS graph_interviews (\
                id text PRIMARY KEY, candidate_id text NOT NULL, \
                status text NOT NULL CHECK (status IN ('active', 'completed')), \
                revision bigint NOT NULL CHECK (revision >= 0), state jsonb NOT NULL, \
                created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now())",
        )
            .execute(&pool)
            .await
            .map_err(|_| RepositoryError::Unavailable)?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS graph_interviews_candidate_created_idx \
                ON graph_interviews (candidate_id, created_at DESC)",
        )
        .execute(&pool)
        .await
        .map_err(|_| RepositoryError::Unavailable)?;
        sqlx::query(
            "CREATE UNIQUE INDEX IF NOT EXISTS graph_interviews_one_active_candidate_idx \
                ON graph_interviews (candidate_id) WHERE status = 'active'",
        )
        .execute(&pool)
        .await
        .map_err(|_| RepositoryError::Unavailable)?;
        Ok(Self { pool })
    }
}

#[async_trait]
impl InterviewRepository for PostgresInterviewRepository {
    async fn insert(&self, session: &InterviewSession) -> Result<(), RepositoryError> {
        let state = serde_json::to_value(session).map_err(|_| RepositoryError::Unavailable)?;
        sqlx::query(
            "INSERT INTO graph_interviews (id, candidate_id, status, revision, state) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(&session.id)
        .bind(&session.candidate_id)
        .bind(match session.status {
            crate::domain::InterviewStatus::Active => "active",
            crate::domain::InterviewStatus::Completed => "completed",
        })
        .bind(i64::from(session.revision))
        .bind(state)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            if error.as_database_error().is_some_and(|error| error.is_unique_violation()) {
                RepositoryError::Conflict
            } else {
                RepositoryError::Unavailable
            }
        })?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<InterviewSession>, RepositoryError> {
        let state = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT state FROM graph_interviews WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| RepositoryError::Unavailable)?;
        state
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| RepositoryError::Unavailable)
    }

    async fn list(&self, candidate_id: &str) -> Result<Vec<InterviewSession>, RepositoryError> {
        sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT state FROM graph_interviews WHERE candidate_id = $1 ORDER BY created_at DESC",
        )
        .bind(candidate_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| RepositoryError::Unavailable)?
        .into_iter()
        .map(|state| serde_json::from_value(state).map_err(|_| RepositoryError::Unavailable))
        .collect()
    }

    async fn replace(
        &self,
        session: &InterviewSession,
        expected_revision: u32,
    ) -> Result<(), RepositoryError> {
        let state = serde_json::to_value(session).map_err(|_| RepositoryError::Unavailable)?;
        let result = sqlx::query(
            "UPDATE graph_interviews SET status = $1, revision = $2, state = $3, updated_at = now() WHERE id = $4 AND revision = $5",
        )
        .bind(match session.status {
            crate::domain::InterviewStatus::Active => "active",
            crate::domain::InterviewStatus::Completed => "completed",
        })
        .bind(i64::from(session.revision))
        .bind(state)
        .bind(&session.id)
        .bind(i64::from(expected_revision))
        .execute(&self.pool)
        .await
        .map_err(|_| RepositoryError::Unavailable)?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(RepositoryError::Conflict)
        }
    }

    async fn healthy(&self) -> Result<(), RepositoryError> {
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map(|_| ())
            .map_err(|_| RepositoryError::Unavailable)
    }
}
