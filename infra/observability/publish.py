#!/usr/bin/env python3
"""Publish the owner-authorized FSP OpenObserve UI through the existing nginx."""
import os
from pathlib import Path
import shlex
import subprocess
import tempfile

PUBLIC_URL = "https://observe.nddev.link"


def main() -> None:
    if os.geteuid() != 0:
        raise SystemExit("Run as root on the FSP stand")
    for name in ("fullchain.pem", "privkey.pem"):
        if not (Path("/etc/letsencrypt/live/observe.nddev.link") / name).is_file():
            raise SystemExit("Issue the observe.nddev.link TLS certificate first")
    env_path = Path("/etc/fsp/observability.env")
    values = {
        key: shlex.split(value)[0] if value else ""
        for key, value in (
            line.split("=", 1) for line in env_path.read_text().splitlines()
            if line and not line.startswith("#")
        )
    }
    if not values.get("ZO_ROOT_USER_EMAIL") or not values.get("ZO_ROOT_USER_PASSWORD"):
        raise SystemExit("Provision the authenticated OpenObserve instance first")
    values["ZO_WEB_URL"] = PUBLIC_URL
    origins = set(filter(None, values.get("ZO_CORS_ALLOWED_ORIGINS", "").split(",")))
    origins.add(PUBLIC_URL)
    values["ZO_CORS_ALLOWED_ORIGINS"] = ",".join(sorted(origins))
    # Atomic activation state; no secret/configuration backup is created.
    with tempfile.NamedTemporaryFile(mode="w", dir=env_path.parent, prefix=".observe-public-", delete=False) as output:
        os.fchmod(output.fileno(), 0o600)
        for key, value in values.items():
            output.write(f"{key}={shlex.quote(value)}\n")
        temporary = Path(output.name)
    temporary.replace(env_path)
    source = Path(__file__).with_name("nginx-public.conf")
    target = Path("/etc/nginx/sites-available/fsp-observe.conf")
    subprocess.run(["install", "-m", "644", str(source), str(target)], check=True)
    enabled = Path("/etc/nginx/sites-enabled/fsp-observe.conf")
    if not enabled.is_symlink():
        enabled.symlink_to(target)
    elif enabled.resolve() != target:
        raise SystemExit("Unexpected OpenObserve nginx site target")
    subprocess.run(["nginx", "-t"], check=True)
    subprocess.run(["systemctl", "restart", "fsp-openobserve"], check=True)
    subprocess.run(["systemctl", "reload", "nginx"], check=True)
    print(f"OpenObserve published at {PUBLIC_URL}; existing account authentication retained")


if __name__ == "__main__":
    main()
