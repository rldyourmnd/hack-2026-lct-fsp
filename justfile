set dotenv-load := true

default:
    @just --list

# Install the TypeScript modules: mock server, contract generator, frontend
install:
    bun install --cwd mock-server --frozen-lockfile
    bun install --cwd contracts/codegen --frozen-lockfile
    bun install --cwd frontend --frozen-lockfile

# Mock API on 127.0.0.1:4010
mock:
    bun run --cwd mock-server start

# Frontend on 127.0.0.1:5173; FSP_API_TARGET selects the API (default: the mock)
dev:
    bun run --cwd frontend dev

# Rebuild contracts/openapi.json, the frontend types and the operation index
contract:
    bun run --cwd mock-server contract
    bun run --cwd contracts/codegen generate

# Types, lint and tests of the contract generator, the mock and the frontend
verify:
    bun run --cwd contracts/codegen typecheck
    bun run --cwd contracts/codegen lint
    bun run --cwd mock-server typecheck
    bun run --cwd mock-server lint
    bun run --cwd mock-server test
    bun run --cwd frontend verify

# Production frontend bundle for the public stand with real email
stand-frontend:
    cd frontend && VITE_DEMO_STAND=1 bun run build
    cd frontend && VITE_DEMO_STAND=1 bun scripts/check-bundle.ts

# Browser acceptance tests against the mock
e2e:
    bun run --cwd frontend e2e

# Rust backend on 127.0.0.1:8080 (needs `just db-up` for a local PostgreSQL)
backend:
    cd backend && cargo run -p fsp-server

# Fictional FSP provider on 127.0.0.1:4020; start alongside the local Rust backend.
fsp-simulator:
    cd backend && cargo run -p fsp-integration-fsp-synthetic --bin fsp-simulator

# Graph-guided interview service on 127.0.0.1:4040.
interview:
    cd backend && INTERVIEW_API_TOKEN=${INTERVIEW_API_TOKEN:-demo-interview-token} DATABASE_URL=${DATABASE_URL:-postgres://fsp:fsp@127.0.0.1:5434/fsp} cargo run -p fsp-interview-service

# Public GitHub analysis on 127.0.0.1:4030; tokenless; set FSP_GITHUB_INSIGHTS_URL for the API.
github-insights:
    cd backend && cargo run -p fsp-github-insights --bin fsp-github-insights

# Local PostgreSQL for development and tests (127.0.0.1:5434, user/db "fsp")
db-up:
    docker compose up --detach --wait db

# Stop the local PostgreSQL (keeps the pg-data volume)
db-down:
    docker compose stop db

# Remove the local PostgreSQL and its data volume
db-reset:
    docker compose down --volumes

# Rust backend: format check, clippy and tests
backend-check:
    cd backend && cargo fmt --all --check
    cd backend && cargo clippy --workspace --all-targets --locked -- -D warnings
    cd backend && cargo test --workspace --locked

# The whole demo in containers on http://localhost:8080 (FSP_PORT overrides the port)
docker-up:
    docker compose up --build --detach --wait

# Stop the containers
docker-down:
    docker compose down

# Build a git ref on the demo stand and switch to it
deploy ref="main":
    infra/deploy.sh {{ref}}
