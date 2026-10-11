#!/usr/bin/env bash
# No GitHub, deployment or email credentials are passed to build/test processes.
set -euo pipefail
export HOME=/var/lib/fsp-ci/home
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
export CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup"
export CARGO_TARGET_DIR=/var/lib/fsp-ci/cache/target
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export CI=1
cd "${2:?checkout required}"
case "${1:?gate required}" in
  platform)
    for module in contracts/codegen mock-server frontend; do bun install --cwd "$module" --frozen-lockfile; done
    for module in contracts/codegen mock-server; do
      bun run --cwd "$module" typecheck
      bun run --cwd "$module" lint
    done
    bun run --cwd mock-server test
    bun run --cwd frontend verify
    # Collect live tests without sending mail; catch runtime/module errors before deployment.
    (cd frontend && bun run e2e:public --list)
    (cd frontend && bunx playwright install chromium && bun run e2e)
    ;;
  backend)
    database="fsp-ci-backend-$$"
    trap 'docker rm -f "$database" >/dev/null 2>&1 || true' EXIT
    export POSTGRES_PASSWORD
    POSTGRES_PASSWORD="$(python3 -c 'import secrets; print(secrets.token_hex(24))')"
    docker run --detach --name "$database" --label fsp.ci=true --memory=1g \
      -e POSTGRES_USER=postgres -e POSTGRES_PASSWORD -p 127.0.0.1::5432 postgres:17-alpine
    for _ in {1..60}; do docker exec "$database" pg_isready -U postgres && break; sleep 1; done
    port="$(docker port "$database" 5432/tcp)"
    export FSP_TEST_DATABASE_URL="postgres://postgres:${POSTGRES_PASSWORD}@${port}/postgres"
    cd backend
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo test --workspace --locked
    cargo build --locked -p fsp-server
    ;;
  compose)
    # Compose resources are project-scoped; never prune the shared Docker host.
    export COMPOSE_PROJECT_NAME=fsp-ci FSP_PORT=18080 FSP_BIND_IP=127.0.0.1 FSP_DB_PORT=15434
    trap 'docker compose logs --tail=100; docker compose down --volumes --remove-orphans' EXIT
    docker compose up --build --detach --wait
    base=http://127.0.0.1:18080
    status() { curl -sS -o /dev/null -w '%{http_code}' "$@"; }
    test "$(status "$base/candidate/vacancies")" = 200
    test "$(status "$base/api/v1/meta")" = 200
    test "$(status "$base/__mock__/reset")" = 404
    body='{"email":"ci@example.com","role":"candidate"}'
    test "$(status -X POST -H "Origin: $base" -H 'Content-Type: application/json' -d "$body" "$base/api/v1/auth/email/challenges")" = 201
    test "$(status -X POST -H 'Origin: https://evil.example' -H 'Content-Type: application/json' -d "$body" "$base/api/v1/auth/email/challenges")" = 403
    interview=http://127.0.0.1:4040
    started="$(curl -fsS -H 'Authorization: Bearer demo-interview-token' -H 'Content-Type: application/json' \
      -d '{"candidateId":"ci-candidate","blueprintId":"backend-middle-v1","maxTurns":10}' \
      "$interview/v1/interviews")"
    interview_id="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])' <<<"$started")"
    docker compose restart interview
    for _ in {1..30}; do curl -fsS "$interview/health" >/dev/null 2>&1 && break; sleep 1; done
    curl -fsS -H 'Authorization: Bearer demo-interview-token' \
      "$interview/v1/interviews/$interview_id" >/dev/null
    bash infra/tests/nginx.sh "$base"
    ;;
  release-safety)
    bash infra/tests/deploy.sh
    bash infra/tests/release.sh
    bash infra/tests/nginx-config.sh
    python3 -m unittest discover -s infra/ci -p 'test_*.py'
    shellcheck infra/*.sh infra/tests/*.sh infra/ci/*.sh
    ;;
  *) echo 'Unknown gate' >&2; exit 1 ;;
esac
