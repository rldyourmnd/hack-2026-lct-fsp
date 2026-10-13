//! Exercise supervisor signals against the real binary while a model request is in flight.
#![cfg(unix)]

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use axum::{Json, Router, extract::State, routing::post};
use fsp_storage_postgres::PostgresRepository;
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::time::{sleep, timeout};

struct ControlledModel {
    evaluating: Semaphore,
    release: Semaphore,
}

async fn completion(
    State(model): State<Arc<ControlledModel>>,
    Json(request): Json<Value>,
) -> Json<Value> {
    let input: Value =
        serde_json::from_str(request["messages"][1]["content"].as_str().unwrap()).unwrap();
    let output = if let Some(answer) = input["answer"].as_str() {
        model.evaluating.add_permits(1);
        model.release.acquire().await.unwrap().forget();
        json!({"state": "confirmed", "rationale": "Тестовая оценка", "evidenceQuote": answer})
    } else {
        json!({"question": "Как обеспечить идемпотентность операции?"})
    };
    Json(json!({"choices": [{"message": {"content": output.to_string()}}]}))
}

async fn drains_active_answer(signal: &str) {
    let admin_url = std::env::var("FSP_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://fsp:fsp@127.0.0.1:5434/postgres".into());
    let (repository, database) = PostgresRepository::connect_test_database(&admin_url).await;
    let model = Arc::new(ControlledModel {
        evaluating: Semaphore::new(0),
        release: Semaphore::new(0),
    });
    let model_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let model_address = model_listener.local_addr().unwrap();
    let model_task = tokio::spawn({
        let model = model.clone();
        async move {
            axum::serve(
                model_listener,
                Router::new()
                    .route("/chat", post(completion))
                    .with_state(model),
            )
            .await
            .unwrap();
        }
    });
    let reserved = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = reserved.local_addr().unwrap();
    drop(reserved);
    let mut child = Command::new(env!("CARGO_BIN_EXE_fsp-interview-service"))
        .env_clear()
        .env("INTERVIEW_HOST", "127.0.0.1")
        .env("INTERVIEW_PORT", address.port().to_string())
        .env("INTERVIEW_API_TOKEN", "shutdown-test-only")
        .env("DATABASE_URL", &database.url)
        .env("BIFROST_CHAT_URL", format!("http://{model_address}/chat"))
        .env("INTERVIEW_BIFROST_MODEL", "controlled-test-model")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let base = format!("http://{address}");
    timeout(Duration::from_secs(10), async {
        loop {
            assert!(
                child.try_wait().unwrap().is_none(),
                "service exited before health"
            );
            if client
                .get(format!("{base}/health"))
                .send()
                .await
                .is_ok_and(|response| response.status().is_success())
            {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("service becomes healthy");
    let started: Value = client
        .post(format!("{base}/v1/interviews"))
        .bearer_auth("shutdown-test-only")
        .json(&json!({"candidateId": "shutdown-candidate", "blueprintId": "backend-middle-v1"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = started["id"].as_str().unwrap().to_owned();
    let request = client
        .post(format!("{base}/v1/interviews/{id}/answers"))
        .bearer_auth("shutdown-test-only")
        .json(&json!({
            "questionId": started["currentQuestion"]["id"],
            "answer": "Использую уникальный ключ операции.",
            "responseMode": "text",
            "revision": started["revision"]
        }));
    let answer = tokio::spawn(async move { request.send().await });
    timeout(Duration::from_secs(5), model.evaluating.acquire())
        .await
        .expect("answer reaches controlled model")
        .unwrap()
        .forget();
    assert!(
        Command::new("kill")
            .args([signal, &child.id().unwrap().to_string()])
            .status()
            .await
            .unwrap()
            .success()
    );
    timeout(Duration::from_secs(5), async {
        loop {
            assert!(
                child.try_wait().unwrap().is_none(),
                "shutdown aborted an active answer"
            );
            if TcpStream::connect(address).await.is_err() {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("shutdown stops accepting new connections");
    model.release.add_permits(1);
    let response: Value = answer
        .await
        .unwrap()
        .expect("active connection drains")
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(response["revision"], 1);
    assert!(
        timeout(Duration::from_secs(5), child.wait())
            .await
            .expect("process exits after draining")
            .unwrap()
            .success()
    );
    let stored: (i64, i32) = sqlx::query_as(
        "SELECT revision, jsonb_array_length(state->'evidence') FROM graph_interviews WHERE id = $1"
    ).bind(&id).fetch_one(repository.pool()).await.unwrap();
    assert_eq!(
        stored,
        (1, 1),
        "the answer is persisted exactly once before exit"
    );
    model_task.abort();
    repository.pool().close().await;
    let admin = sqlx::PgPool::connect(&admin_url).await.unwrap();
    sqlx::query(&format!("DROP DATABASE \"{}\"", database.name))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

#[tokio::test]
async fn sigterm_drains_and_persists_active_answer() {
    drains_active_answer("-TERM").await;
}

#[tokio::test]
async fn sigint_still_drains_and_persists_active_answer() {
    drains_active_answer("-INT").await;
}
