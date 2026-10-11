#!/usr/bin/env bash
# Idempotent provisioning of the FSP demo stand on Ubuntu 24.04. Run as root:
#   provision.sh <public-ipv4>
# Installs nginx, a pinned Bun runtime, certbot, the mock API unit and an
# IP-address TLS certificate (Let's Encrypt "shortlived" profile, 160 hours).
set -euo pipefail

PUBLIC_IP="${1:?usage: provision.sh <public-ipv4>}"
BUN_VERSION=1.4.2
BUN_SHA256=36368faef7527875d5ffa52e53cd48021741f2a83eb6208a8dd64068d422a913
HERE="$(cd "$(dirname "$0")" && pwd)"

export DEBIAN_FRONTEND=noninteractive
apt-get update -q
apt-get upgrade -yq
# git: the GitHub insights service reads public repositories through blobless clones.
apt-get install -yq nginx ufw curl unzip build-essential postgresql bubblewrap poppler-utils git
install -D -m 644 "$HERE/apparmor/fsp-pdf-bwrap" /etc/apparmor.d/fsp-pdf-bwrap
apparmor_parser -r /etc/apparmor.d/fsp-pdf-bwrap

if [ "$(/opt/bun/bin/bun --version 2>/dev/null)" != "$BUN_VERSION" ]; then
  tmp="$(mktemp -d)"
  curl -fsSLo "$tmp/bun.zip" "https://github.com/oven-sh/bun/releases/download/bun-v$BUN_VERSION/bun-linux-x64.zip"
  echo "$BUN_SHA256  $tmp/bun.zip" | sha256sum -c -
  unzip -q "$tmp/bun.zip" -d "$tmp"
  install -D -m 755 "$tmp/bun-linux-x64/bun" "/opt/bun-$BUN_VERSION/bin/bun"
  ln -sfn "/opt/bun-$BUN_VERSION" /opt/bun
  rm -rf "$tmp"
fi
ln -sf /opt/bun/bin/bun /usr/local/bin/bun
# The stand builds and runs on Bun only; drop the Node/pnpm toolchain of earlier provisioning.
rm -rf /opt/node /opt/node-v* /opt/pnpm
rm -f /usr/local/bin/node /usr/local/bin/npm /usr/local/bin/npx /usr/local/bin/pnpm

# Pinned rustup for the on-host backend build: a checksum-verified installer binary, never a
# piped shell script. RUST_TOOLCHAIN must stay in sync with backend/rust-toolchain.toml.
RUSTUP_INIT_VERSION=1.29.1
RUSTUP_INIT_SHA256=dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71
RUST_TOOLCHAIN=1.99.0
if [ ! -x "/opt/rustup/toolchains/$RUST_TOOLCHAIN-x86_64-unknown-linux-gnu/bin/cargo" ]; then
  tmp="$(mktemp -d)"
  curl -fsSLo "$tmp/rustup-init" \
    "https://static.rust-lang.org/rustup/archive/$RUSTUP_INIT_VERSION/x86_64-unknown-linux-gnu/rustup-init"
  echo "$RUSTUP_INIT_SHA256  $tmp/rustup-init" | sha256sum -c -
  chmod +x "$tmp/rustup-init"
  RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo \
    "$tmp/rustup-init" -y --profile minimal --default-toolchain "$RUST_TOOLCHAIN" --no-modify-path
  rm -rf "$tmp"
fi

snap install core >/dev/null 2>&1 || true
snap list certbot >/dev/null 2>&1 || snap install --classic certbot
snap refresh certbot >/dev/null 2>&1 || true
ln -sf /snap/bin/certbot /usr/local/bin/certbot

id fsp >/dev/null 2>&1 || useradd --system --home-dir /opt/fsp --shell /usr/sbin/nologin fsp
install -d -m 755 /opt/fsp /opt/fsp/releases /var/www/acme /etc/fsp

# PostgreSQL is the only datastore. Loopback-only listen, a dedicated fsp role and database,
# credentials generated once and kept in root-managed /etc/fsp/db.env (never re-generated on
# re-runs: an existing file means the role already holds that password).
systemctl enable --now postgresql
for pg_main in /etc/postgresql/*/main; do
  [ -d "$pg_main" ] || continue
  install -d -m 755 "$pg_main/conf.d"
  printf "listen_addresses = 'localhost'\n" > "$pg_main/conf.d/fsp-localhost.conf"
done
systemctl restart postgresql
if [ ! -f /etc/fsp/db.env ]; then
  db_password="$(openssl rand -hex 24)"
  # Read SQL from stdin; postgres cannot open root-owned mode-600 temporary files.
  {
    cat <<'SQL'
DO $$ BEGIN IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname='fsp') THEN CREATE ROLE fsp LOGIN; END IF; END $$;
SQL
    printf "ALTER ROLE fsp WITH LOGIN PASSWORD '%s';\n" "$db_password"
  } | su - postgres -c 'psql -v ON_ERROR_STOP=1'
  su - postgres -c "psql -tAc \"SELECT 1 FROM pg_database WHERE datname='fsp'\"" | grep -q 1 \
    || su - postgres -c "createdb -O fsp fsp"
  # Publish only after bootstrap succeeds, so interruption leaves setup retryable.
  db_env="$(mktemp /etc/fsp/db.env.XXXXXXXX)"
  printf 'DATABASE_URL=postgres://fsp:%s@127.0.0.1:5432/fsp\n' "$db_password" > "$db_env"
  chown root:fsp "$db_env"
  chmod 640 "$db_env"
  mv "$db_env" /etc/fsp/db.env
fi

# Owner policy: no database backups, including on previously provisioned stands.
systemctl disable --now fsp-backup.timer 2>/dev/null || true
systemctl stop fsp-backup.service 2>/dev/null || true
rm -f /etc/systemd/system/fsp-backup.service /etc/systemd/system/fsp-backup.timer \
  /usr/local/libexec/fsp-backup-db
systemctl mask fsp-backup.service fsp-backup.timer
systemctl daemon-reload

ufw allow OpenSSH >/dev/null
ufw allow 'Nginx Full' >/dev/null
ufw --force enable >/dev/null

if [ ! -f /etc/fsp/mock-api.env ]; then
cat > /etc/fsp/mock-api.env <<EOF
FSP_MOCK_ALLOWED_ORIGINS=https://$PUBLIC_IP
EOF
fi
# Both implementations expose the explicit demo code in the public stand.
if [ ! -f /etc/fsp/frontend-build.env ]; then
cat > /etc/fsp/frontend-build.env <<EOF
VITE_DEMO_STAND=1
VITE_DEMO_EMAIL_CODE=123456
EOF
fi
# The Rust backend takes over once /etc/fsp/api-backend says "rust" (release.sh switches the
# services); until then it builds alongside without serving.
if [ ! -f /etc/fsp/rust-api.env ]; then
cat > /etc/fsp/rust-api.env <<EOF
FSP_AUTH_MODE=demo
FSP_DEMO_EMAIL_CODE=123456
FSP_SECURE_COOKIE=1
FSP_ALLOWED_ORIGINS=https://$PUBLIC_IP
FSP_PUBLIC_WEB_URL=https://$PUBLIC_IP
GITHUB_CALLBACK_URL=https://$PUBLIC_IP/api/v1/integrations/github/callback
FSP_GITHUB_INSIGHTS_URL=http://127.0.0.1:4030
EOF
fi
if [ ! -f /etc/fsp/interview.env ]; then
  interview_env="$(mktemp /etc/fsp/interview.env.XXXXXXXX)"
  printf 'INTERVIEW_API_TOKEN=%s\n' "$(openssl rand -hex 32)" > "$interview_env"
  chown root:fsp "$interview_env"
  chmod 640 "$interview_env"
  mv "$interview_env" /etc/fsp/interview.env
fi
# The GitHub analysis needs no token. An optional /etc/fsp/github-insights.env (root:fsp, 640) with a
# read-only GITHUB_TOKEN only raises GitHub's per-IP limits for anonymous requests.
install -m 644 "$HERE/fsp-github-insights.service" /etc/systemd/system/fsp-github-insights.service
install -m 644 "$HERE/fsp-interview.service" /etc/systemd/system/fsp-interview.service
chown root:fsp /etc/fsp/{mock-api,rust-api,frontend-build,interview}.env
chmod 640 /etc/fsp/{mock-api,rust-api,frontend-build,interview}.env
[ -f /etc/fsp/api-backend ] || echo mock > /etc/fsp/api-backend
install -m 644 "$HERE/fsp-api.service" /etc/systemd/system/fsp-api.service
install -m 644 "$HERE/fsp-mock-api.service" /etc/systemd/system/fsp-mock-api.service
# Optional LLM chain (Grok through CLIProxyAPI + Bifrost). Binaries, the xAI
# auth file and /etc/fsp/llm.env are host secrets placed outside the repo; the
# units are enabled once the env file exists. See infra/README.md#llm-chain.
install -m 644 "$HERE/fsp-cliproxy.service" /etc/systemd/system/fsp-cliproxy.service
install -m 644 "$HERE/fsp-bifrost.service" /etc/systemd/system/fsp-bifrost.service
install -d -m 700 -o fsp -g fsp /var/lib/fsp/cliproxy-auth /var/lib/fsp/bifrost
systemctl daemon-reload
case "$(cat /etc/fsp/api-backend)" in
  rust) systemctl disable fsp-mock-api.service; systemctl enable fsp-api.service fsp-interview.service ;;
  mock) systemctl disable fsp-api.service fsp-interview.service; systemctl enable fsp-mock-api.service ;;
  *) echo 'Invalid /etc/fsp/api-backend' >&2; exit 1 ;;
esac
if [ -f /etc/fsp/llm.env ]; then
  systemctl enable fsp-cliproxy.service fsp-bifrost.service >/dev/null
fi

install -m 644 "$HERE/nginx/fsp-security-headers.conf" /etc/nginx/snippets/fsp-security-headers.conf
rm -f /etc/nginx/sites-enabled/default
if [ ! -f /etc/letsencrypt/live/fsp-demo/fullchain.pem ]; then
  install -m 644 "$HERE/nginx/fsp-demo-acme.conf" /etc/nginx/sites-available/fsp-demo.conf
  ln -sfn /etc/nginx/sites-available/fsp-demo.conf /etc/nginx/sites-enabled/fsp-demo.conf
  nginx -t
  systemctl reload nginx
  certbot certonly --non-interactive --agree-tos --register-unsafely-without-email \
    --webroot --webroot-path /var/www/acme \
    --preferred-profile shortlived --ip-address "$PUBLIC_IP" \
    --cert-name fsp-demo --deploy-hook 'systemctl reload nginx'
fi
# Renew when less than three days of the 160-hour lifetime remain (about twice a week).
sed -i '/^renew_before_expiry/d' /etc/letsencrypt/renewal/fsp-demo.conf
sed -i '1i renew_before_expiry = 3 days' /etc/letsencrypt/renewal/fsp-demo.conf

bash "$HERE/render-nginx.sh" /etc/fsp/rust-api.env > /etc/nginx/sites-available/fsp-demo.conf
ln -sfn /etc/nginx/sites-available/fsp-demo.conf /etc/nginx/sites-enabled/fsp-demo.conf
nginx -t
systemctl reload nginx
systemctl list-timers snap.certbot.renew.timer --no-pager
echo "Provisioned $PUBLIC_IP"
