"""Release-policy regressions; build and browser gates remain real integrations."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import URLError

import controller as ci


class ReleasePolicy(unittest.TestCase):
    def setUp(self):
        self.pr = {"number": 7, "draft": False, "state": "open",
                   "user": {"login": "rldyourmnd"},
                   "head": {"ref": "danil", "sha": "a" * 40, "repo": {"full_name": ci.REPO}},
                   "base": {"ref": "dev", "sha": "b" * 40}}
        self.current = copy.deepcopy(self.pr)
        self.statuses = [{"context": f"fsp/{gate}", "state": "success"} for gate in ci.GATES]
        self.checks = []
        self.merged = False

    def api(self, path, body=None, method=None):
        if path.endswith("/merge"):
            self.assertEqual(body, {"sha": "a" * 40, "merge_method": "merge"})
            self.merged = True
            return {"merged": True}
        if path.startswith("pulls/"):
            return self.current
        if "/status?" in path:
            return {"statuses": self.statuses}
        if "/check-runs?" in path:
            return {"check_runs": self.checks}
        raise AssertionError(path)

    def attempt(self, tree="tree", code=0):
        with patch.object(ci, "api", side_effect=self.api), \
             patch.object(ci.subprocess, "run", return_value=subprocess.CompletedProcess([], code, tree + "\n")), \
             patch.object(ci.subprocess, "check_output", return_value="tree\n"):
            return ci.merge_if_unchanged(self.pr, "a" * 40, Path("source.git"))

    def test_all_gates_and_unchanged_tree_merge(self):
        self.assertTrue(self.attempt())
        self.assertTrue(self.merged)

    def test_changed_head_or_base_never_merge(self):
        for side in ("head", "base"):
            self.current = copy.deepcopy(self.pr)
            self.current[side]["sha"] = "c" * 40
            self.assertFalse(self.attempt())
        self.assertFalse(self.merged)

    def test_missing_failed_pending_gates_never_merge(self):
        for value in ("failure", "pending", "error"):
            self.statuses[0]["state"] = value
            self.assertFalse(self.attempt())
        self.statuses = self.statuses[1:]
        self.assertFalse(self.attempt())
        self.assertFalse(self.merged)

    def test_new_base_content_or_conflicts_never_merge(self):
        self.assertFalse(self.attempt(tree="different"))
        self.assertFalse(self.attempt(code=1))
        self.assertFalse(self.merged)

    def test_forks_drafts_and_feature_prs_never_auto_merge(self):
        for change in ("fork", "draft", "feature", "author"):
            self.current = copy.deepcopy(self.pr)
            if change == "fork": self.current["head"]["repo"]["full_name"] = "other/repo"
            if change == "draft": self.current["draft"] = True
            if change == "feature": self.current["head"]["ref"] = "feat/example"
            if change == "author": self.current["user"]["login"] = "untrusted"
            self.assertFalse(self.attempt())
        self.assertFalse(self.merged)

    def test_security_checks_are_preserved_and_retired_actions_are_explicit(self):
        self.checks = [{"app": {"slug": "github-actions"}, "status": "completed", "conclusion": "failure"}]
        self.assertTrue(self.attempt())
        self.merged = False
        self.checks.append({"app": {"slug": "gitguardian"}, "status": "completed", "conclusion": "failure"})
        self.assertFalse(self.attempt())
        self.assertFalse(self.merged)

    def test_independent_commit_status_failure_blocks_merge(self):
        self.statuses.append({"context": "security/scan", "state": "failure"})
        self.assertFalse(self.attempt())
        self.assertFalse(self.merged)

    def test_duplicate_status_writes_are_bounded(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(ci, "ROOT", Path(directory)), patch.object(ci, "api") as api:
            (Path(directory) / "evidence").mkdir()
            ci.status("a" * 40, "platform", "success", "ok")
            ci.status("a" * 40, "platform", "success", "same tree")
            self.assertEqual(api.call_count, 1)
            ci.status("a" * 40, "platform", "pending", "rerun")
            self.assertEqual(api.call_count, 2)

    def test_evidence_write_is_atomic_json(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "proof.json"
            ci.save(path, {"success": False})
            self.assertEqual(json.loads(path.read_text()), {"success": False})
            self.assertFalse(path.with_suffix(".tmp").exists())


class DeploymentRecovery(unittest.TestCase):
    def test_accepted_release_retries_status_without_redeployment(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            config = root / "config"
            config.mkdir()
            (root / "evidence").mkdir()
            for name, value in {"deploy-enabled": "", "public-url": "https://nddev.link", "auth-mode": "resend", "resend-acceptance-key": "test-key"}.items():
                (config / name).write_text(value)
            sha = "a" * 40
            failed = False

            def provider(path, body=None):
                nonlocal failed
                if path == "commits/main":
                    return {"sha": sha}
                self.assertEqual(path, f"statuses/{sha}")
                if body["state"] == "success" and not failed:
                    failed = True
                    raise URLError("temporary GitHub outage")

            with patch.object(ci, "ROOT", root), patch.object(ci, "CONFIG", config), \
                 patch.object(ci, "api", side_effect=provider) as api, patch.object(ci, "run") as run:
                with self.assertRaises(URLError):
                    ci.deploy(sha, root / "source.git")
                accepted = (root / "deployed.json").read_bytes()
                receipt = root / "evidence" / f"status-{sha}-deployment.json"
                self.assertEqual(json.loads(receipt.read_text())["state"], "pending")
                runs = run.call_count
                ci.deploy(sha, root / "source.git")
                self.assertEqual(json.loads(receipt.read_text())["state"], "success")
                self.assertEqual((root / "deployed.json").read_bytes(), accepted)
                self.assertEqual(run.call_count, runs)
                requests = api.call_count
                ci.deploy(sha, root / "source.git")
                self.assertEqual(api.call_count, requests)
                self.assertEqual(run.call_count, runs)

    def test_failed_release_keeps_fix_prs_unblocked_and_bounds_retries(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            config = root / "config"
            config.mkdir()
            for name, value in {"deploy-enabled": "", "public-url": "https://nddev.link", "auth-mode": "resend", "resend-acceptance-key": "test-key"}.items():
                (config / name).write_text(value)
            sha = "a" * 40
            with patch.object(ci, "ROOT", root), patch.object(ci, "CONFIG", config), \
                 patch.object(ci, "api", return_value={"sha": sha}), patch.object(ci, "status") as status, \
                 patch.object(ci, "run") as run:
                def fail_deploy(args, **kwargs):
                    if args[0] == "timeout":
                        raise subprocess.CalledProcessError(1, args)
                run.side_effect = fail_deploy
                ci.deploy(sha, root / "source.git")
                self.assertFalse((root / "deployed.json").exists())
                self.assertEqual(status.call_args.args[2], "failure")
                self.assertEqual(json.loads((root / "deploy-attempts.json").read_text())["count"], 1)
                before = run.call_count
                ci.deploy(sha, root / "source.git")
                self.assertEqual(run.call_count, before)
                ci.save(root / "deploy-attempts.json", {"sha": sha, "configuration": json.loads((root / "deploy-attempts.json").read_text())["configuration"], "count": 3, "lastAt": 0})
                ci.deploy(sha, root / "source.git")
                self.assertEqual(run.call_count, before)
                # Runtime changes require fresh public acceptance even with the same main SHA.
                (config / "public-url").write_text("https://new.example")
                run.side_effect = None
                ci.deploy(sha, root / "source.git")
                self.assertTrue((root / "deployed.json").exists())
                self.assertEqual(status.call_args.args[2], "success")
                before = run.call_count
                ci.deploy(sha, root / "source.git")
                self.assertEqual(run.call_count, before)


if __name__ == "__main__":
    unittest.main()
