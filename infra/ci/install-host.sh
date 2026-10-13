#!/usr/bin/env bash
# Reviewed installation/update on an existing private Ubuntu CI host. Never called by PR jobs.
set -euo pipefail
[[ $EUID == 0 ]] || { echo 'Run as root on the authorized CI host' >&2; exit 1; }
here="$(cd "$(dirname "$0")" && pwd)"
getent passwd fsp-ci >/dev/null || useradd --system --home-dir /var/lib/fsp-ci/home --create-home --shell /bin/bash fsp-ci
getent passwd fsp >/dev/null || useradd --system --no-create-home fsp
usermod -aG docker fsp-ci
install -d -m 700 /etc/fsp-ci
install -d -m 755 /opt/fsp-ci /var/lib/fsp-ci
install -d -o fsp-ci -g fsp-ci /var/lib/fsp-ci/home /var/lib/fsp-ci/home/.local /var/lib/fsp-ci/home/.local/bin /var/lib/fsp-ci/home/.docker /var/lib/fsp-ci/home/.docker/cli-plugins /var/lib/fsp-ci/cache
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y --no-install-recommends bubblewrap poppler-utils shellcheck pkg-config libssl-dev build-essential curl ca-certificates xz-utils libnspr4 libnss3 libatk1.0-0 libatk-bridge2.0-0 libatspi2.0-0 libxcomposite1 libxdamage1 libxfixes3 libxrandr2 libgbm1 libxkbcommon0 libasound2t64 libcups2 libpango-1.0-0 libcairo2 fonts-liberation
bash "$here/../install-pdf-sandbox.sh"
runuser -u fsp-ci -- bash -s <<'TOOLS'
set -euo pipefail
export HOME=/var/lib/fsp-ci/home CARGO_HOME=/var/lib/fsp-ci/home/.cargo RUSTUP_HOME=/var/lib/fsp-ci/home/.rustup
if [[ ! -x "$CARGO_HOME/bin/rustup" ]]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o "$HOME/rustup-init.sh"
  sh "$HOME/rustup-init.sh" -y --profile minimal --default-toolchain 1.99.0 --no-modify-path
fi
"$CARGO_HOME/bin/rustup" toolchain install 1.99.0 --profile minimal --component rustfmt,clippy
cd "$HOME"
curl -fsSL https://nodejs.org/dist/v26.11.1/node-v26.11.1-linux-x64.tar.xz -o node.tar.xz
printf '%s\n' '3883bfc73f9a680ca4eab04b196068aaaab1373ffa77d8fc1a4408222495b651  node.tar.xz' | sha256sum -c -
tar -xJf node.tar.xz
ln -sfn "$HOME/node-v26.11.1-linux-x64/bin/node" "$HOME/.local/bin/node"
curl -fsSL https://github.com/docker/compose/releases/download/v5.6.0/docker-compose-linux-x86_64 -o .docker/cli-plugins/docker-compose
printf '%s\n' '40343e21ca777173e69cff5dbafeb37c6f81f3b0d57d9e597f036e95eb63e76a  .docker/cli-plugins/docker-compose' | sha256sum -c -
chmod 755 .docker/cli-plugins/docker-compose
[[ "$(/usr/local/bin/bun --version)" == 1.4.2 ]]
"$HOME/.local/bin/node" --version
docker compose version
TOOLS
install -m 755 "$here/"*.sh /opt/fsp-ci/
install -m 644 "$here/"*.py /opt/fsp-ci/
install -m 644 "$here/"*.service "$here/"*.timer /etc/systemd/system/
systemctl daemon-reload
printf '%s\n' 'Installed. Provision private /etc/fsp-ci files, then enable fsp-ci.timer. Deployment remains disabled without deploy-enabled.'
