#!/usr/bin/env python3
"""Private-repository CI. Installed separately from the revisions it verifies."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time
import urllib.request

REPO = "KirpaDmitriy/sport-programming-federation"
GATES = ("platform", "backend", "compose", "release-safety")
ROOT = Path(os.environ.get("FSP_CI_ROOT", "/var/lib/fsp-ci"))
CONTROL = Path(__file__).resolve().parent
CONFIG = Path(os.environ.get("FSP_CI_CONFIG", "/etc/fsp-ci"))


def api(path, body=None, method=None):
    token = (CONFIG / "github-token").read_text().strip()
    request = urllib.request.Request(
        f"https://api.github.com/repos/{REPO}/{path}",
        data=json.dumps(body).encode() if body is not None else None,
        method=method,
        headers={"Authorization": f"Bearer {token}", "Accept": "application/vnd.github+json",
                 "X-GitHub-Api-Version": "2022-11-28", "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        data = response.read()
        return json.loads(data) if data else None


def run(args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def save(path, value):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def status(sha, gate, state, description):
    receipt = ROOT / "evidence" / f"status-{sha}-{gate}.json"
    if receipt.exists() and json.loads(receipt.read_text()).get("state") == state:
        return
    api(f"statuses/{sha}", {"state": state, "context": f"fsp/{gate}",
        "description": description[:140],
        "target_url": f"https://github.com/{REPO}/commit/{sha}"})
    save(receipt, {"state": state, "description": description})


def trusted(pr):
    return (pr["head"]["repo"] is not None
            and pr["head"]["repo"]["full_name"] == REPO
            and pr["base"]["ref"] in {"danil", "dima", "dev", "main"}
            and pr["user"]["login"] in {"rldyourmnd", "KirpaDmitriy"})


def lane(pr):
    return (pr["head"]["ref"], pr["base"]["ref"]) in {
        ("danil", "dev"), ("dima", "dev"), ("dev", "main")}


def fingerprint(tree):
    digest = hashlib.sha256(tree.encode())
    for path in sorted(CONTROL.glob("*")):
        if path.is_file():
            digest.update(path.name.encode() + path.read_bytes())
    return digest.hexdigest()


def fetch():
    mirror = ROOT / "source.git"
    if not mirror.exists():
        run(["git", "init", "--bare", str(mirror)])
    env = {**os.environ, "GIT_ASKPASS": str(CONTROL / "askpass.sh"),
           "GIT_TERMINAL_PROMPT": "0", "FSP_CI_CONFIG": str(CONFIG)}
    run(["git", "--git-dir", str(mirror), "fetch", "--prune",
         f"https://github.com/{REPO}.git", "+refs/heads/*:refs/heads/*"], env=env)
    return mirror


def check(sha, mirror):
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("Invalid commit SHA")
    tree = subprocess.check_output(["git", "--git-dir", str(mirror), "rev-parse", f"{sha}^{{tree}}"], text=True).strip()
    evidence = ROOT / "evidence" / f"{fingerprint(tree)}.json"
    if evidence.exists():
        result = json.loads(evidence.read_text())
        if result["success"]:
            for gate in GATES:
                status(sha, gate, "success", f"Identical tree verified at {result['sha'][:12]}; {tree[:12]}")
            return True
        # A failed revision is retried only by removing its evidence file, or a new commit.
        if result["sha"] == sha:
            return False
    work = ROOT / "work"
    shutil.rmtree(work, ignore_errors=True)
    run(["git", "clone", "--no-local", "--no-checkout", str(mirror), str(work)])
    run(["git", "-C", str(work), "checkout", "--detach", sha])
    run(["chown", "-R", "fsp-ci:fsp-ci", str(work)])
    logs = ROOT / "logs" / sha
    logs.mkdir(parents=True, exist_ok=True)
    for gate in GATES:
        status(sha, gate, "pending", "Queued on the private FSP CI server")
    passed = True
    for gate in GATES:
        status(sha, gate, "pending", "Running on the private FSP CI server")
        with (logs / f"{gate}.log").open("w") as log:
            try:
                run(["timeout", "--signal=TERM", "--kill-after=30s", "45m",
                     "runuser", "-u", "fsp-ci", "--", str(CONTROL / "gate.sh"), gate, str(work)],
                    stdout=log, stderr=subprocess.STDOUT, env={"PATH": "/usr/sbin:/usr/bin:/sbin:/bin"})
                if gate == "backend":
                    # The exact successful binary and exact API restrictions, with no application secrets.
                    binary = ROOT / "cache" / "target" / "debug" / "fsp-career-api"
                    run(["bash", str(work / "infra/tests/pdf-runtime.sh"), str(binary)], stdout=log, stderr=subprocess.STDOUT)
                status(sha, gate, "success", f"Passed on private CI; tree {tree[:12]}")
            except subprocess.CalledProcessError:
                passed = False
                status(sha, gate, "failure", "Failed on private CI; see private server logs")
    save(evidence, {"sha": sha, "tree": tree, "success": passed, "checkedAt": int(time.time())})
    return passed


def merge_if_unchanged(pr, sha, mirror):
    current = api(f"pulls/{pr['number']}")
    if (not trusted(current) or not lane(current) or current["draft"]
            or current["state"] != "open" or current["head"]["sha"] != sha
            or current["base"]["sha"] != pr["base"]["sha"]):
        return False
    # Permanent lanes can differ only by merge ancestry. Require the actual merged
    # tree to equal the verified head; conflicts or new base content need a fresh PR.
    merged = subprocess.run(["git", "--git-dir", str(mirror), "merge-tree", "--write-tree",
                             current["base"]["sha"], sha], capture_output=True, text=True)
    head_tree = subprocess.check_output(["git", "--git-dir", str(mirror), "rev-parse", f"{sha}^{{tree}}"], text=True).strip()
    if merged.returncode or merged.stdout.strip() != head_tree:
        return False
    latest = api(f"commits/{sha}/status?per_page=100")
    contexts = {item["context"]: item["state"] for item in reversed(latest["statuses"])}
    if any(not context.startswith("fsp/") and state != "success" for context, state in contexts.items()):
        return False
    if any(contexts.get(f"fsp/{gate}") != "success" for gate in GATES):
        return False
    # Preserve independent security checks. Only retired Actions checks are superseded.
    checks = api(f"commits/{sha}/check-runs?per_page=100")["check_runs"]
    if any(item["app"]["slug"] != "github-actions" and
           (item["status"] != "completed" or item["conclusion"] not in {"success", "neutral", "skipped"})
           for item in checks):
        return False
    result = api(f"pulls/{pr['number']}/merge", {"sha": sha, "merge_method": "merge"}, "PUT")
    return result.get("merged", False)


def integrations(open_prs):
    for head, base in (("danil", "dev"), ("dima", "dev"), ("dev", "main")):
        if any(pr["head"]["ref"] == head and pr["base"]["ref"] == base for pr in open_prs):
            continue
        compare = api(f"compare/{base}...{head}")
        if compare["ahead_by"] and compare["files"]:
            api("pulls", {"head": head, "base": base, "title": f"Merge {head} into {base}",
                "body": "External FSP CI integration. Merge requires all four fsp/* gates on the unchanged head, no untested base changes, and successful independent security checks."})


def deploy(sha, mirror):
    marker = ROOT / "deployed.json"
    if not (CONFIG / "deploy-enabled").exists():
        return
    configuration = hashlib.sha256(b"".join(
        (CONFIG / name).read_bytes() for name in ("public-url", "auth-mode", "resend-acceptance-key")
    )).hexdigest()
    if marker.exists():
        deployed = json.loads(marker.read_text())
        if deployed["sha"] == sha and deployed.get("configuration") == configuration:
            # Acceptance is durable even if its final GitHub status write failed.
            status(sha, "deployment", "success", "Live release and actual public browser journey passed")
            return
    attempts_file = ROOT / "deploy-attempts.json"
    attempts = {"sha": sha, "configuration": configuration, "count": 0, "lastAt": 0}
    if attempts_file.exists():
        previous = json.loads(attempts_file.read_text())
        if previous["sha"] == sha and previous["configuration"] == configuration:
            attempts = previous
    if attempts["count"] >= 3 or time.time() - attempts["lastAt"] < 300:
        return
    if api("commits/main")["sha"] != sha:
        return
    attempts.update(count=attempts["count"] + 1, lastAt=int(time.time()))
    save(attempts_file, attempts)
    deploy_work = ROOT / "deploy"
    shutil.rmtree(deploy_work, ignore_errors=True)
    run(["git", "clone", "--shared", str(mirror), str(deploy_work)])
    run(["git", "-C", str(deploy_work), "checkout", "--detach", sha])
    status(sha, "deployment", "pending", "Deploying verified main and checking the public browser journey")
    log_path = ROOT / "logs" / sha / "deployment.log"
    log_path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with log_path.open("a") as log:
            run(["timeout", "--kill-after=30s", "45m", "bash", str(deploy_work / "infra/deploy.sh"), sha],
                cwd=deploy_work, env={**os.environ, "FSP_DEPLOY_HOST": "stand", "FSP_SSH_CONFIG": str(CONFIG / "ssh-config")},
                stdout=log, stderr=subprocess.STDOUT)
            run(["timeout", "--kill-after=30s", "15m", str(CONTROL / "public.sh"), str(deploy_work)], stdout=log, stderr=subprocess.STDOUT)
        save(marker, {"sha": sha, "configuration": configuration, "deployedAt": int(time.time())})
        status(sha, "deployment", "success", "Live release and actual public browser journey passed")
    except subprocess.CalledProcessError:
        status(sha, "deployment", "failure", "Deployment or public acceptance failed; inspect private deployment log")
        # Keep checking PRs: a broken deployment must not block the fix from reaching main.
        return


def cycle():
    if shutil.disk_usage(ROOT).free < 15 * 1024**3:
        raise RuntimeError("CI host has less than 15 GiB free; refusing new builds")
    for log_directory in (ROOT / "logs").iterdir():
        if log_directory.is_dir() and time.time() - log_directory.stat().st_mtime > 14 * 86400:
            shutil.rmtree(log_directory)
    mirror = fetch()
    main = api("commits/main")["sha"]
    prs = api("pulls?state=open&per_page=100")
    # Skip the pre-migration main until the external CI contract has landed.
    if subprocess.run(["git", "--git-dir", str(mirror), "cat-file", "-e", f"{main}:infra/ci/gate.sh"],
                      stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0:
        if check(main, mirror):
            deploy(main, mirror)
    for pr in prs:
        if trusted(pr):
            sha = pr["head"]["sha"]
            if check(sha, mirror) and not pr["draft"] and lane(pr):
                merge_if_unchanged(pr, sha, mirror)
    integrations(api("pulls?state=open&per_page=100"))


if __name__ == "__main__":
    ROOT.mkdir(parents=True, exist_ok=True)
    for name in ("evidence", "logs"):
        (ROOT / name).mkdir(mode=0o700, exist_ok=True)
    with (ROOT / "controller.lock").open("w") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise SystemExit(0)
        cycle()
