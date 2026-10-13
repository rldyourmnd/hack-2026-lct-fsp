#!/usr/bin/env python3
"""Install FSP observability. No backups, snapshots or application activation."""
from __future__ import annotations

import base64
import grp
import hashlib
import json
import os
from pathlib import Path
import platform
import pwd
import secrets
import shlex
import shutil
import subprocess
import tarfile
import tempfile
import time
import urllib.request

from nginx import render
from dashboard import provision_dashboard

HERE = Path(__file__).resolve().parent
PINS = (
    ("openobserve", "https://downloads.openobserve.ai/releases/openobserve/v1.0.4/openobserve-v1.0.4-linux-amd64.tar.gz", "5c1b18bc072658c045ff32ca7dd0d2e3f22fac1209755d7a0ef5fd6448896e61"),
    ("vector", "https://github.com/vectordotdev/vector/releases/download/v0.59.0/vector-0.59.0-x86_64-unknown-linux-gnu.tar.gz", "fb15878d1cd68445e0658792a8fa7ccf02ae8a353c6cc5d6fe867b171993e19f"),
)


def run(*args: str, **kwargs) -> None:
    subprocess.run(args, check=True, **kwargs)


def read_env(path: Path) -> dict[str, str]:
    values = {}
    for line in path.read_text().splitlines():
        if line and not line.startswith("#"):
            key, value = line.split("=", 1)
            values[key] = shlex.split(value)[0] if value else ""
    return values


def provision_ingestion(values: dict[str, str]) -> None:
    """Keep the collector credential ingest-only and separate from the UI administrator."""
    secret_path = Path("/etc/fsp/vector-secrets.json")
    existing = json.loads(secret_path.read_text()) if secret_path.exists() else {}
    if existing.get("token_name") == "fsp-vector":
        return
    endpoint = "http://127.0.0.1:5080/api/default/ingestion-tokens"
    headers = {"Authorization": values["FSP_OBSERVE_AUTH"], "Content-Type": "application/json"}
    with urllib.request.urlopen(urllib.request.Request(endpoint, headers=headers), timeout=10) as response:
        tokens = json.load(response)["data"]
    if any(token["name"] == "fsp-vector" for token in tokens):
        raise SystemExit("The named collector token exists but its local credential is missing; recover it explicitly")
    payload = json.dumps({"name": "fsp-vector", "description": "FSP operational ingestion only"}).encode()
    with urllib.request.urlopen(urllib.request.Request(endpoint, data=payload, headers=headers), timeout=10) as response:
        token = json.load(response)["data"]["token"]
    authorization = "Basic " + base64.b64encode(f"fsp-vector:{token}".encode()).decode()
    with tempfile.NamedTemporaryFile(mode="w", dir=secret_path.parent, prefix=".vector-secret-", delete=False) as output:
        os.fchmod(output.fileno(), 0o640)
        os.fchown(output.fileno(), 0, grp.getgrnam("fsp-vector").gr_gid)
        json.dump({"authorization": authorization, "token_name": "fsp-vector"}, output)
        temporary_secret = Path(output.name)
    temporary_secret.replace(secret_path)


def main() -> None:
    if os.geteuid() != 0 or platform.machine() != "x86_64":
        raise SystemExit("Run as root on the FSP amd64 stand")
    contract = HERE.parent.parent / "contracts/openapi.json"
    if not contract.is_file():
        raise SystemExit("Missing contracts/openapi.json; use a complete reviewed checkout or release")
    with tempfile.TemporaryDirectory(prefix="fsp-observability-install-") as temporary:
        staging = Path(temporary)
        for binary, url, expected in PINS:
            archive = staging / f"{binary}.tar.gz"
            with urllib.request.urlopen(url, timeout=120) as response, archive.open("wb") as output:
                shutil.copyfileobj(response, output)
            with archive.open("rb") as content:
                if hashlib.file_digest(content, "sha256").hexdigest() != expected:
                    raise SystemExit(f"Checksum mismatch: {binary}")
            with tarfile.open(archive) as tar:
                matches = [member for member in tar.getmembers() if member.isfile() and Path(member.name).name == binary]
                if len(matches) != 1:
                    raise SystemExit(f"Ambiguous binary: {binary}")
                source = tar.extractfile(matches[0])
                assert source is not None
                with source, (staging / binary).open("wb") as output:
                    shutil.copyfileobj(source, output)
            (staging / binary).chmod(0o755)
        # All artifacts have been authenticated before changing the host.
        for user in ("fsp-observe", "fsp-vector"):
            if subprocess.run(["id", "-u", user], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
                run("useradd", "--system", "--no-create-home", "--shell", "/usr/sbin/nologin", user)
        run("install", "-d", "-m", "755", "/opt/fsp-observability/bin", "/etc/fsp")
        env_path = Path("/etc/fsp/observability.env")
        existing = read_env(env_path) if env_path.exists() else {}
        values = read_env(HERE / "openobserve.env.example")
        # Preserve the operator-authorized public UI endpoint across reinstallations.
        for key in ("ZO_WEB_URL", "ZO_CORS_ALLOWED_ORIGINS"):
            if key in existing:
                values[key] = existing[key]
        values["ZO_ROOT_USER_EMAIL"] = existing.get("ZO_ROOT_USER_EMAIL", "observe-admin@nddev.link")
        values["ZO_ROOT_USER_PASSWORD"] = existing.get("ZO_ROOT_USER_PASSWORD", secrets.token_urlsafe(36))
        values["FSP_OBSERVE_AUTH"] = "Basic " + base64.b64encode(
            f'{values["ZO_ROOT_USER_EMAIL"]}:{values["ZO_ROOT_USER_PASSWORD"]}'.encode()
        ).decode()
        # Temporary activation state, never a configuration backup.
        with tempfile.NamedTemporaryFile(mode="w", dir="/etc/fsp", prefix=".observe-env-", delete=False) as output:
            os.fchmod(output.fileno(), 0o600)
            for key, value in values.items():
                output.write(f"{key}={shlex.quote(value)}\n")
            temporary_env = Path(output.name)
        temporary_env.replace(env_path)
        for binary, _, _ in PINS:
            destination = Path("/opt/fsp-observability/bin") / binary
            run("install", "-m", "755", str(staging / binary), str(destination.with_suffix(".new")))
            destination.with_suffix(".new").replace(destination)
        for service in ("fsp-openobserve.service", "fsp-vector.service"):
            run("install", "-m", "644", str(HERE / service), f"/etc/systemd/system/{service}")
        run("install", "-m", "644", str(HERE / "vector.yaml"), "/etc/fsp/vector.yaml")
        run("install", "-m", "644", str(HERE / "logrotate.conf"), "/etc/logrotate.d/fsp-telemetry")
        # Match rotation permissions from the first write without truncating an existing log.
        log_path = Path("/var/log/nginx/fsp-telemetry.json")
        log_path.parent.mkdir(mode=0o755, exist_ok=True)
        descriptor = os.open(log_path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o640)
        with os.fdopen(descriptor, "a") as output:
            os.fchmod(output.fileno(), 0o640)
            os.fchown(output.fileno(), pwd.getpwnam("www-data").pw_uid, grp.getgrnam("adm").gr_gid)
        run("systemd-analyze", "verify", "/etc/systemd/system/fsp-openobserve.service", "/etc/systemd/system/fsp-vector.service")
        run("systemctl", "daemon-reload")
        run("systemctl", "enable", "fsp-openobserve.service")
        run("systemctl", "restart", "fsp-openobserve.service")
        for _ in range(60):
            try:
                with urllib.request.urlopen("http://127.0.0.1:5080/healthz", timeout=2) as response:
                    if response.status == 200:
                        break
            except OSError:
                time.sleep(1)
        else:
            raise SystemExit("OpenObserve health check failed; inspect its local journal")
        provision_ingestion(values)
        provision_dashboard(values["FSP_OBSERVE_AUTH"])
        request = urllib.request.Request("http://127.0.0.1:5080/api/default/rumtoken", headers={"Authorization": values["FSP_OBSERVE_AUTH"]})
        with urllib.request.urlopen(request, timeout=10) as response:
            rum_token = json.load(response)["data"]["rum_token"]
        public_url = read_env(Path("/etc/fsp/rust-api.env"))["FSP_PUBLIC_WEB_URL"]
        http, locations = render(public_url, rum_token, contract)
        nginx_directory = Path("/etc/nginx/fsp-observability")
        nginx_directory.mkdir(mode=0o700, exist_ok=True)
        for name, contents in (("http.conf", http), ("locations.conf", locations)):
            path = nginx_directory / name
            descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
            with os.fdopen(descriptor, "w") as output:
                output.write(contents)
        run("/opt/fsp-observability/bin/vector", "validate", "--no-environment", "/etc/fsp/vector.yaml")
        run("systemctl", "enable", "fsp-vector.service")
        run("systemctl", "restart", "fsp-vector.service")
        for service in ("fsp-openobserve.service", "fsp-vector.service"):
            state = subprocess.check_output(["systemctl", "is-active", service], text=True).strip()
            if state != "active":
                raise SystemExit(f"{service} is {state}")
        print("FSP observability installed; listeners remain loopback-only; credentials remain in /etc/fsp/observability.env")


if __name__ == "__main__":
    main()
