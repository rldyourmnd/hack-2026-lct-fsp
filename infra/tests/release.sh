#!/usr/bin/env bash
# Exercise actual release control flow with isolated paths and failing host-command substitutes.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT
export FSP_RELEASE_ROOT="$fixture/fsp" FSP_SYSTEM_ROOT="$fixture/system" FSP_RUSTUP_ROOT="$fixture/rustup"
export FSP_TEST_COMMANDS="$fixture/commands"
mkdir -p "$fixture/bin" "$FSP_RELEASE_ROOT/releases" "$FSP_SYSTEM_ROOT/etc/"{fsp,systemd/system,nginx/sites-available,nginx/snippets}
export PATH="$fixture/bin:$PATH"
printf 'rust\n' > "$FSP_SYSTEM_ROOT/etc/fsp/api-backend"
printf 'FSP_PUBLIC_WEB_URL=https://192.0.2.1\n' > "$FSP_SYSTEM_ROOT/etc/fsp/rust-api.env"
printf 'INTERVIEW_API_TOKEN=test-token\n' > "$FSP_SYSTEM_ROOT/etc/fsp/interview.env"
old=1111111111111111111111111111111111111111
new=2222222222222222222222222222222222222222
export FSP_TEST_NEW="$new"

cat > "$fixture/bin/systemctl" <<'STUB'
#!/usr/bin/env bash
set -eu
printf '%s\n' "$*" >> "$FSP_TEST_COMMANDS"
if [[ "$1" == is-active ]]; then exit 0; fi
if [[ "${FSP_TEST_FAIL:-}" == restart && "$1" == restart && ! -f "$FSP_RELEASE_ROOT/failed-once" ]]; then
  touch "$FSP_RELEASE_ROOT/failed-once"
  exit 1
fi
STUB
cat > "$fixture/bin/curl" <<'STUB'
#!/usr/bin/env bash
set -eu
if [[ "${FSP_TEST_FAIL:-}" == health && "$(readlink -f "$FSP_RELEASE_ROOT/current")" == "$FSP_RELEASE_ROOT/releases/$FSP_TEST_NEW" ]]; then exit 1; fi
if [[ "${FSP_TEST_FAIL:-}" == fsp_health && "$*" == *':4020/health'* && "$(readlink -f "$FSP_RELEASE_ROOT/current")" == "$FSP_RELEASE_ROOT/releases/$FSP_TEST_NEW" ]]; then exit 1; fi
if [[ "${FSP_TEST_FAIL:-}" == insights_health && "$*" == *':4030/health'* && "$(readlink -f "$FSP_RELEASE_ROOT/current")" == "$FSP_RELEASE_ROOT/releases/$FSP_TEST_NEW" ]]; then exit 1; fi
if [[ "${FSP_TEST_FAIL:-}" == interview_health && "$*" == *':4040/health'* && "$(readlink -f "$FSP_RELEASE_ROOT/current")" == "$FSP_RELEASE_ROOT/releases/$FSP_TEST_NEW" ]]; then exit 1; fi
if [[ "${FSP_TEST_FAIL:-}" == silero_health && "$*" == *':4050/health'* && "$(readlink -f "$FSP_RELEASE_ROOT/current")" == "$FSP_RELEASE_ROOT/releases/$FSP_TEST_NEW" ]]; then exit 1; fi
STUB
cat > "$fixture/bin/nginx" <<'STUB'
#!/usr/bin/env bash
set -eu
if [[ "${FSP_TEST_FAIL:-}" == nginx && ! -f "$FSP_RELEASE_ROOT/failed-once" ]]; then
  touch "$FSP_RELEASE_ROOT/failed-once"
  exit 1
fi
STUB
cat > "$fixture/bin/bun" <<'STUB'
#!/usr/bin/env bash
set -eu
if [[ "${FSP_TEST_FAIL:-}" == build ]]; then exit 1; fi
if [[ "$*" == 'run build' ]]; then mkdir -p dist; echo built > dist/index.html; fi
STUB
cat > "$fixture/bin/sleep" <<'STUB'
#!/usr/bin/env bash
exit 0
STUB
cat > "$fixture/bin/systemd-run" <<'STUB'
#!/usr/bin/env bash
set -eu
[[ "${FSP_TEST_FAIL:-}" != pdf ]]
STUB
mkdir -p "$FSP_RUSTUP_ROOT/toolchains/1.99.0-x86_64-unknown-linux-gnu/bin"
cat > "$FSP_RUSTUP_ROOT/toolchains/1.99.0-x86_64-unknown-linux-gnu/bin/cargo" <<'STUB'
#!/usr/bin/env bash
set -eu
# Concurrent builds would collide here if the host lock did not protect the whole build.
mkdir "$FSP_RELEASE_ROOT/building"
/bin/sleep 0.1
mkdir -p "$CARGO_TARGET_DIR/release"
printf '#!/bin/sh\nexit 0\n' > "$CARGO_TARGET_DIR/release/fsp-career-api"
printf '#!/bin/sh\nexit 0\n' > "$CARGO_TARGET_DIR/release/fsp-simulator"
printf '#!/bin/sh\nexit 0\n' > "$CARGO_TARGET_DIR/release/fsp-github-insights"
printf '#!/bin/sh\nexit 0\n' > "$CARGO_TARGET_DIR/release/fsp-interview-service"
rmdir "$FSP_RELEASE_ROOT/building"
STUB
chmod +x "$fixture/bin/"* "$FSP_RUSTUP_ROOT/toolchains/1.99.0-x86_64-unknown-linux-gnu/bin/cargo"

make_release() {
  local dir="$1"
  mkdir -p "$dir/"{frontend/dist,mock-server,backend,bin}
  cp -r "$HERE" "$dir/infra"
  echo '<main>fixture</main>' > "$dir/frontend/dist/index.html"
  echo 'channel = "1.99.0"' > "$dir/backend/rust-toolchain.toml"
  mkdir -p "$dir/backend/crates/storage-postgres/migrations"
  echo 'SELECT 1;' > "$dir/backend/crates/storage-postgres/migrations/0001_initial.sql"
  printf '#!/bin/sh\nexit 0\n' > "$dir/bin/fsp-career-api"
  chmod +x "$dir/bin/fsp-career-api"
}
reset_previous() {
  ln -sfn "$FSP_RELEASE_ROOT/releases/$old" "$FSP_RELEASE_ROOT/current"
  echo old-nginx > "$FSP_SYSTEM_ROOT/etc/nginx/sites-available/fsp-demo.conf"
  echo old-unit > "$FSP_SYSTEM_ROOT/etc/systemd/system/fsp-api.service"
  rm -f "$FSP_RELEASE_ROOT/failed-once" "$FSP_TEST_COMMANDS"
}
make_release "$FSP_RELEASE_ROOT/releases/$old"
rm "$FSP_RELEASE_ROOT/releases/$old/infra/fsp-simulator.service"
make_release "$FSP_RELEASE_ROOT/releases/$new"
cp "$FSP_RELEASE_ROOT/releases/$new/bin/fsp-career-api" "$FSP_RELEASE_ROOT/releases/$new/bin/fsp-simulator"
cp "$FSP_RELEASE_ROOT/releases/$new/bin/fsp-career-api" "$FSP_RELEASE_ROOT/releases/$new/bin/fsp-github-insights"
cp "$FSP_RELEASE_ROOT/releases/$new/bin/fsp-career-api" "$FSP_RELEASE_ROOT/releases/$new/bin/fsp-interview-service"
reset_previous

bash "$HERE/release.sh" "$old"
[[ ! -f "$FSP_TEST_COMMANDS" ]]
[[ -f "$FSP_RELEASE_ROOT/releases/$old/frontend/dist/index.html" ]]
echo 'PASS: active SHA is not rebuilt or restarted'

for failure in restart health fsp_health insights_health interview_health nginx; do
  reset_previous
  if FSP_TEST_FAIL="$failure" bash "$HERE/release.sh" "$new" > "$fixture/output" 2>&1; then
    echo "Expected $failure failure" >&2; exit 1
  fi
  [[ "$(readlink -f "$FSP_RELEASE_ROOT/current")" == "$FSP_RELEASE_ROOT/releases/$old" ]]
  [[ "$(cat "$FSP_SYSTEM_ROOT/etc/nginx/sites-available/fsp-demo.conf")" == old-nginx ]]
  [[ "$(cat "$FSP_SYSTEM_ROOT/etc/systemd/system/fsp-api.service")" == old-unit ]]
  echo "PASS: $failure restores release, nginx and unit"
done

reset_previous
mkdir -p "$FSP_RELEASE_ROOT/releases/$new/voice-runtime" "$FSP_RELEASE_ROOT/voice-venv/bin" "$FSP_RELEASE_ROOT/models"
touch "$FSP_RELEASE_ROOT/releases/$new/voice-runtime/server.py" "$FSP_RELEASE_ROOT/models/v5_5_ru.pt"
printf '#!/bin/sh\n' > "$FSP_RELEASE_ROOT/voice-venv/bin/python"
chmod +x "$FSP_RELEASE_ROOT/voice-venv/bin/python"
printf 'FSP_TTS_PROVIDER=silero\n' > "$FSP_SYSTEM_ROOT/etc/fsp/speech.env"
if FSP_TEST_FAIL=silero_health bash "$HERE/release.sh" "$new" > "$fixture/output" 2>&1; then
  echo 'Expected Silero health failure' >&2; exit 1
fi
[[ "$(readlink -f "$FSP_RELEASE_ROOT/current")" == "$FSP_RELEASE_ROOT/releases/$old" ]]
echo 'PASS: Silero health failure restores the previous release'
rm -f "$FSP_SYSTEM_ROOT/etc/fsp/speech.env"

stage="$FSP_RELEASE_ROOT/releases/.staging-test"
make_release "$stage"
third=3333333333333333333333333333333333333333
if FSP_TEST_FAIL=build bash "$HERE/release.sh" "$third" "$stage" > "$fixture/output" 2>&1; then exit 1; fi
[[ ! -d "$FSP_RELEASE_ROOT/releases/$third" ]]
[[ "$(readlink -f "$FSP_RELEASE_ROOT/current")" == "$FSP_RELEASE_ROOT/releases/$old" ]]
echo 'PASS: failed build leaves active release intact'

bash "$stage/infra/release.sh" "$third" "$stage" > "$fixture/first.log" 2>&1 &
first=$!
stage_two="$FSP_RELEASE_ROOT/releases/.staging-two"
make_release "$stage_two"
fourth=4444444444444444444444444444444444444444
bash "$stage_two/infra/release.sh" "$fourth" "$stage_two" > "$fixture/second.log" 2>&1 &
second=$!
wait "$first"
wait "$second"
[[ -f "$FSP_RELEASE_ROOT/current/frontend/dist/index.html" ]]
[[ ! -d "$FSP_RELEASE_ROOT/building" ]]
echo 'PASS: overlapping builds serialize and activate complete releases'

# Older releases remain usable during pruning.
for n in {5..9}; do
  printf -v sha '%040d' "$n"
  make_release "$FSP_RELEASE_ROOT/releases/$sha"
done
bash "$HERE/release.sh" "$old" >/dev/null
[[ -d "$FSP_RELEASE_ROOT/releases/$old" ]]
[[ -d "$FSP_RELEASE_ROOT/releases/$fourth" || -d "$FSP_RELEASE_ROOT/releases/$third" ]]
rg_command='disable --now fsp-simulator'
[[ "$(cat "$FSP_TEST_COMMANDS")" == *"$rg_command"* ]]
echo 'PASS: legacy rollback stops the simulator and retains both releases'

# Release manifests must be compatible before any host/service mutation.
make_release "$FSP_RELEASE_ROOT/releases/$new"
schema=backend/crates/storage-postgres/migrations
for failure in changed added removed divergent strict_target; do
  reset_previous
  old_dir="$FSP_RELEASE_ROOT/releases/$old"
  new_dir="$FSP_RELEASE_ROOT/releases/$new"
  rm -f "$old_dir/$schema/"*.sql "$new_dir/$schema/"*.sql
  rm -f "$old_dir/backend/ROLLBACK_COMPATIBLE" "$new_dir/backend/ROLLBACK_COMPATIBLE"
  echo 'SELECT 1;' > "$old_dir/$schema/0001_initial.sql"
  echo 'SELECT 1;' > "$new_dir/$schema/0001_initial.sql"
  case "$failure" in
    changed) echo 'SELECT 2;' > "$new_dir/$schema/0001_initial.sql" ;;
    added) echo 'SELECT 2;' > "$new_dir/$schema/0002_extra.sql" ;;
    removed) echo 'SELECT 2;' > "$old_dir/$schema/0002_extra.sql" ;;
    divergent)
      echo 'SELECT 2;' > "$old_dir/$schema/0002_extra.sql"
      echo 'SELECT 3;' > "$new_dir/$schema/0003_other.sql"
      ;;
    strict_target) echo sqlx-forward-additive-v1 > "$old_dir/backend/ROLLBACK_COMPATIBLE" ;;
  esac
  if bash "$HERE/release.sh" "$new" > "$fixture/output" 2>&1; then
    echo "Expected schema rejection: $failure" >&2; exit 1
  fi
  [[ ! -f "$FSP_TEST_COMMANDS" ]]
  [[ "$(readlink -f "$FSP_RELEASE_ROOT/current")" == "$old_dir" ]]
  [[ "$(cat "$FSP_SYSTEM_ROOT/etc/systemd/system/fsp-api.service")" == old-unit ]]
  echo "PASS: $failure schema leaves the active release untouched"
done

echo sqlx-forward-additive-v1 > "$new_dir/backend/ROLLBACK_COMPATIBLE"
echo 'SELECT 2;' > "$new_dir/$schema/0002_extra.sql"
bash "$HERE/release.sh" "$new" >/dev/null
bash "$HERE/release.sh" "$old" >/dev/null
[[ "$(readlink -f "$FSP_RELEASE_ROOT/current")" == "$old_dir" ]]
echo 'PASS: compatible binaries can upgrade and roll back an additive schema'

reset_previous
if FSP_TEST_FAIL=health bash "$HERE/release.sh" "$new" > "$fixture/output" 2>&1; then exit 1; fi
[[ "$(readlink -f "$FSP_RELEASE_ROOT/current")" == "$old_dir" ]]
echo 'PASS: additive release health failure restores a compatible binary'

reset_previous
echo 'FSP_PDF_IMPORT=1' >> "$FSP_SYSTEM_ROOT/etc/fsp/rust-api.env"
if FSP_TEST_FAIL=pdf bash "$HERE/release.sh" "$new" > "$fixture/output" 2>&1; then exit 1; fi
[[ ! -f "$FSP_TEST_COMMANDS" ]]
[[ "$(readlink -f "$FSP_RELEASE_ROOT/current")" == "$old_dir" ]]
[[ "$(cat "$FSP_SYSTEM_ROOT/etc/systemd/system/fsp-api.service")" == old-unit ]]
echo 'PASS: PDF runtime failure stops before activation and migrations'
