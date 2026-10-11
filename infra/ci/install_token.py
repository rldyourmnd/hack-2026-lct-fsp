#!/usr/bin/env python3
"""Validate and atomically install a dedicated CI PAT supplied through stdin."""
import argparse
from datetime import datetime, timezone
import fcntl
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import urllib.error
import urllib.request

REPO = "KirpaDmitriy/sport-programming-federation"
MAINTAINERS = {"rldyourmnd", "KirpaDmitriy"}
CONFIG = Path("/etc/fsp-ci")
ROOT = Path("/var/lib/fsp-ci")


class CredentialError(Exception):
    """Messages deliberately exclude credentials and provider response bodies."""


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def request(token, path):
    req = urllib.request.Request(
        f"https://api.github.com/{path}",
        headers={"Authorization": f"Bearer {token}", "Accept": "application/vnd.github+json",
                 "X-GitHub-Api-Version": "2022-11-28"},
    )
    try:
        with urllib.request.build_opener(NoRedirect()).open(req, timeout=10) as response:
            payload = response.read(65_537)
            if len(payload) > 65_536:
                raise CredentialError("GitHub response exceeded the validation limit")
            return json.loads(payload), response.headers
    except urllib.error.HTTPError as error:
        error.close()
        raise CredentialError(f"GitHub credential validation failed: HTTP {error.code}") from None
    except (OSError, ValueError):
        raise CredentialError("GitHub credential validation failed: network or response error") from None


def validate(token):
    if not token or len(token) > 512 or not token.isascii() or any(c.isspace() for c in token):
        raise CredentialError("Supply one nonempty token through stdin")
    repo, headers = request(token, f"repos/{REPO}")
    scopes = {scope.strip() for scope in headers.get("X-OAuth-Scopes", "").split(",") if scope.strip()}
    if scopes != {"repo"}:
        raise CredentialError("Expected a dedicated classic PAT with exactly the repo scope")
    try:
        expires = datetime.fromisoformat(headers.get("GitHub-Authentication-Token-Expiration", "").replace(" UTC", "+00:00"))
    except ValueError:
        raise CredentialError("The dedicated PAT must have a valid expiration date") from None
    if expires.tzinfo is None or expires <= datetime.now(timezone.utc):
        raise CredentialError("The dedicated PAT must expire in the future")
    if not isinstance(repo, dict) or repo.get("full_name") != REPO:
        raise CredentialError("The token must have push access to the canonical repository")
    permissions = repo.get("permissions")
    if not isinstance(permissions, dict) or permissions.get("push") is not True:
        raise CredentialError("The token must have push access to the canonical repository")
    user, _ = request(token, "user")
    if not isinstance(user, dict) or user.get("login") not in MAINTAINERS:
        raise CredentialError("The token must belong to an admitted project maintainer")
    return user["login"], expires.astimezone(timezone.utc).isoformat()


def install(token, config=CONFIG, root=ROOT):
    if os.geteuid() != 0:
        raise CredentialError("Installation requires root on the authorized CI host")
    info = config.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o077:
        raise CredentialError("The CI configuration directory must be root-owned and private")
    # The same lock excludes both a running controller and a new timer cycle.
    with (root / "controller.lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise CredentialError("CI is busy; retry after its current cycle finishes") from None
        identity = validate(token)
        fd, name = tempfile.mkstemp(prefix=".github-token-", dir=config)
        temporary = Path(name)
        try:
            with os.fdopen(fd, "w") as output:
                os.fchmod(output.fileno(), 0o600)
                output.write(token + "\n")
                output.flush()
                os.fsync(output.fileno())
            os.replace(temporary, config / "github-token")
            directory = os.open(config, os.O_RDONLY | os.O_DIRECTORY)
            try:
                os.fsync(directory)
            finally:
                os.close(directory)
        finally:
            temporary.unlink(missing_ok=True)
        return identity


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--install", action="store_true", help="replace the CI host credential; default is read-only validation")
    args = parser.parse_args(argv)
    if sys.stdin.isatty():
        print("Refusing terminal input; redirect the private token file into stdin", file=sys.stderr)
        return 1
    token = sys.stdin.read(514).strip()
    try:
        login, expires = install(token) if args.install else validate(token)
    except CredentialError as error:
        print(str(error), file=sys.stderr)
        return 1
    except (OSError, ValueError, TypeError):
        print("Credential operation failed; inspect file permissions and host state", file=sys.stderr)
        return 1
    print(f"{'Installed' if args.install else 'Validated'} dedicated repo-scoped credential for {login}; expires {expires}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
