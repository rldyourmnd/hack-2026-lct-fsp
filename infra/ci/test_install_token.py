"""Credential failures must preserve the active CI secret and never disclose it."""
from datetime import datetime, timedelta, timezone
import fcntl
import io
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import urllib.error

import install_token as installer

TOKEN = "test-only-credential-never-valid"


class CredentialInstallation(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.config = self.root / "config"
        self.config.mkdir(mode=0o700)
        self.target = self.config / "github-token"
        self.target.write_text("existing-private-credential\n")
        self.target.chmod(0o600)
        self.headers = {"X-OAuth-Scopes": "repo", "GitHub-Authentication-Token-Expiration":
                        (datetime.now(timezone.utc) + timedelta(days=30)).isoformat()}
        self.repo = {"full_name": installer.REPO, "permissions": {"push": True}}
        self.login = "rldyourmnd"

    def request(self, token, path):
        self.assertEqual(token, TOKEN)
        return (self.repo, self.headers) if path.startswith("repos/") else ({"login": self.login}, {})

    def install(self):
        # Tests run unprivileged in CI; ownership is simulated, locking and file IO are real.
        actual_lstat = Path.lstat

        def lstat(path):
            value = actual_lstat(path)
            if path == self.config:
                fields = list(value)
                fields[4] = 0
                return os.stat_result(fields)
            return value

        with patch.object(installer, "request", side_effect=self.request), \
             patch.object(installer.os, "geteuid", return_value=0), patch.object(Path, "lstat", lstat):
            return installer.install(TOKEN, self.config, self.root)

    def assert_preserved(self):
        self.assertEqual(self.target.read_text(), "existing-private-credential\n")
        self.assertEqual(list(self.config.iterdir()), [self.target])

    def test_only_valid_narrow_expiring_token_replaces_file_privately(self):
        self.assertEqual(self.install()[0], self.login)
        self.assertEqual(self.target.read_text(), TOKEN + "\n")
        self.assertEqual(self.target.stat().st_mode & 0o777, 0o600)
        self.assertEqual(list(self.config.iterdir()), [self.target])

    def test_broad_missing_scopes_and_unusable_identity_preserve_active_token(self):
        for scopes in ("repo, admin:org, gist, workflow", "repo:status", ""):
            with self.subTest(scopes=scopes):
                self.headers["X-OAuth-Scopes"] = scopes
                with self.assertRaises(installer.CredentialError): self.install()
                self.assert_preserved()
        self.headers["X-OAuth-Scopes"] = "repo"
        self.repo["permissions"]["push"] = False
        with self.assertRaises(installer.CredentialError): self.install()
        self.assert_preserved()
        self.repo["permissions"]["push"] = True
        self.login = "other-account"
        with self.assertRaises(installer.CredentialError): self.install()
        self.assert_preserved()
        self.repo["permissions"] = None
        with self.assertRaises(installer.CredentialError): self.install()
        self.assert_preserved()

    def test_expired_missing_and_invalid_expiry_preserve_active_token(self):
        for expires in ("", "invalid", "2000-01-01 00:00:00 UTC", "2099-01-01T00:00:00"):
            with self.subTest(expires=expires):
                self.headers["GitHub-Authentication-Token-Expiration"] = expires
                with self.assertRaises(installer.CredentialError): self.install()
                self.assert_preserved()

    def test_github_utc_and_offset_expiration_formats_are_accepted(self):
        for expires in ("2099-01-01 00:00:00 UTC", "2099-01-01 03:00:00 +0300"):
            with self.subTest(expires=expires):
                self.headers["GitHub-Authentication-Token-Expiration"] = expires
                self.assertEqual(self.install()[1], "2099-01-01T00:00:00+00:00")

    def test_busy_controller_prevents_network_validation_and_installation(self):
        with (self.root / "controller.lock").open("a") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaisesRegex(installer.CredentialError, "CI is busy"):
                self.install()
        self.assert_preserved()

    def test_failed_atomic_replace_keeps_previous_file_and_removes_temporary(self):
        with patch.object(installer.os, "replace", side_effect=OSError("injected write failure")):
            with self.assertRaises(OSError): self.install()
        self.assert_preserved()

    def test_insecure_configuration_directory_is_rejected(self):
        self.config.chmod(0o755)
        with self.assertRaises(installer.CredentialError): self.install()
        self.assert_preserved()

    def test_provider_errors_and_redirects_cannot_echo_credentials(self):
        for code in (301, 401, 403, 500):
            error = urllib.error.HTTPError("https://api.github.com/user", code, TOKEN, {}, io.BytesIO(TOKEN.encode()))
            output = io.StringIO()
            with patch("sys.stdin", io.StringIO(TOKEN)), patch("sys.stderr", output), \
                 patch.object(installer.urllib.request.OpenerDirector, "open", side_effect=error):
                self.assertEqual(installer.main([]), 1)
            self.assertTrue(error.closed)
            self.assertNotIn(TOKEN, output.getvalue())
            self.assertIn(str(code), output.getvalue())
            self.assert_preserved()
        self.assertIsNone(installer.NoRedirect().redirect_request(None, None, 302, "", {}, "https://other.invalid"))

    def test_network_failure_and_malformed_secret_never_write(self):
        with patch.object(installer.urllib.request.OpenerDirector, "open", side_effect=TimeoutError(TOKEN)):
            with self.assertRaises(installer.CredentialError) as failure:
                installer.validate(TOKEN)
        self.assertNotIn(TOKEN, str(failure.exception))
        with patch.object(self, "request", side_effect=installer.CredentialError("GitHub unavailable")):
            with self.assertRaises(installer.CredentialError): self.install()
        self.assert_preserved()
        for token in ("", "a\nb", "x" * 513, "токен"):
            with patch.object(installer, "request") as request:
                with self.assertRaises(installer.CredentialError): installer.validate(token)
                request.assert_not_called()
        self.assert_preserved()


if __name__ == "__main__":
    unittest.main()
