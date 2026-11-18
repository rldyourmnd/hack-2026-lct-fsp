# External CI and release

GitHub hosts code, PRs and commit statuses. The existing private CI server executes builds through
`fsp-ci.timer` and `controller.py`; it does not use Actions minutes or require runner-registration rights.
The October 8 migration was explicitly authorized after canonical repository billing blocked Actions and
repository-level runner registration was denied to the collaborator. Historical failed Actions runs remain
visible; they are superseded by the new `fsp/*` policy, never rewritten as successes.

The public repository's `dev` workflow runs the four source gates on GitHub-hosted Ubuntu.
The jobs use the same gate script with the hosted toolchain and disposable databases. The
stand's protected deployment and live browser journey use the existing release controller.

## Gates and trust

`gate.sh` preserves platform types/lint/contracts/components/build/browser acceptance, Rust fmt/clippy/tests
with real PostgreSQL, real PDF extraction under the API's exact systemd restrictions, Compose images and HTTP
checks, deployment fault recovery, nginx configuration, backup/restore and controller regression tests.
All four statuses (`fsp/platform`, `fsp/backend`, `fsp/compose`, `fsp/release-safety`) must pass. Exact Git tree
and installed controller content identify evidence; identical integration trees reuse successful evidence and
explicitly name its source SHA in the status. Failures do not pass and are not repeatedly retried automatically.

Only this private canonical repository and PRs authored by its two maintainers are admitted. Fork PRs never run.
Tests run as `fsp-ci` with an empty inherited environment. `/etc/fsp-ci` is root-only. Docker access is root-equivalent:
this is a trusted-maintainer runner, not an isolation boundary for hostile code. Never admit public/fork contributions
or put another repository's credentials in the project directory. Project containers/volumes use the `fsp-ci` prefix;
cleanup never prunes the shared host. The controller is installed separately and is not automatically updated by a PR.
Host CPU/memory limits, serial jobs and two Cargo build jobs leave capacity for existing runner slots.
New cycles refuse to build below 15 GiB free. Logs are retained for 14 days; compact evidence is retained for reuse.

Feature PRs require the maintainer to merge. The controller opens and merges personal -> dev -> main integration
PRs only after gates pass, head and base remain unchanged, and Git's prospective merge tree equals the tested head.
Independent security checks still gate merging. GitHub's merge API matches the head SHA; concurrent base changes
are checked before merging, and the resulting main tree must pass again before deployment. Permanent branches
remain permanent. Delete feature branches after their PR is merged. There is no branch protection administration
permission in this account, so collaborators must use this documented merge gate rather than bypass it manually.

## Installation and private configuration

Run reviewed `install-host.sh` as root on the authorized Ubuntu server. It installs dependencies and a separate
FSP user's Rust 1.99.0, Node 26.11.1 and checksum-verified Compose 5.6.0; Bun 1.4.2 must already exist. It installs the
executable-specific AppArmor profile, never changes global namespace restrictions, and does not modify other slots.
Configure mode-600 files in mode-700 `/etc/fsp-ci`:

- `github-token`: GitHub token able to read this private repository, write commit statuses and merge PRs.
- `ssh-config`, `deploy-key`, `known_hosts`: pinned SSH identity for the existing FSP stand only; host alias `stand`.
- `public-url`: current HTTPS URL, and `auth-mode`: `demo` or `resend`.
- `resend-acceptance-key`: the dedicated receiving-email test key, never the runtime sending key.
- `deploy-enabled`: optional empty marker; create only after private deployment configuration is verified.

Enable `systemctl enable --now fsp-ci.timer`. `systemctl start fsp-ci.service` requests an immediate cycle; an
OS file lock also prevents duplicate manual controllers. Secrets never enter the checkout, evidence JSON or logs.
Deployment uses only verified current main and existing `infra/deploy.sh` locking/rollback. `public.sh` then executes
the actual HTTPS browser journey; failure produces a failed deployment status, not a live-success marker. Credential
availability or delivery failures leave checks failed. The controller never resets the production database.

## Operation and recovery

Inspect `journalctl -u fsp-ci.service` and private `/var/lib/fsp-ci/logs/<sha>/*.log`; evidence lives in
`/var/lib/fsp-ci/evidence/`. A failed revision is retried by removing only its matching evidence JSON and requesting a
cycle. New content automatically receives new checks. A deployment failure remains red, retries at most three times with five-minute spacing, and never blocks
PR checks needed to fix it. After fixing infrastructure, remove only `deploy-attempts.json` to retry; a new main
SHA or changed public URL/auth mode/inbox key starts a fresh acceptance cycle. `deployed.json` records only actual accepted releases. Stop the timer to pause
new cycles; stop the service separately only when deliberately interrupting a running check/release.

For controller upgrades: stop the timer, wait for the current service to finish, review the diff, run `install-host.sh`
from the reviewed release, then restart the timer. For a single script update, review and validate it,
install it with the existing root ownership/mode while the service is inactive, and restart the timer.
Controller content changes invalidate cached proof. Keep the
stand's earlier compatible releases/backups; reverting CI never requires database changes. Do not re-enable retired
Actions workflows without restoring billing/permissions and deliberately migrating the gate policy.

Primary references: [GitHub commit statuses](https://docs.github.com/en/rest/commits/statuses),
[runner administration](https://docs.github.com/en/rest/actions/self-hosted-runners),
[merge API](https://docs.github.com/en/rest/pulls/pulls#merge-a-pull-request),
[Git merge-tree](https://git-scm.com/docs/git-merge-tree), [Docker security](https://docs.docker.com/engine/security/).
