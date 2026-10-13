# External CI and release

GitHub hosts code, PRs and commit statuses. The existing private CI server executes builds through
`fsp-ci.timer` and `controller.py`; it does not use Actions minutes or require runner-registration rights.
The October 8 migration was explicitly authorized after canonical repository billing blocked Actions and
repository-level runner registration was denied to the collaborator. Historical failed Actions runs remain
visible; they are superseded by the new `fsp/*` policy, never rewritten as successes.

**Recovery mode:** an external operation deleted `gha-runner-1` on October 11, 2026 at 08:16 UTC;
`server-gha-runner-2` was retired earlier. Their private receipts and dedicated token are unavailable. Do not use
those retired hosts. [#161](https://github.com/KirpaDmitriy/sport-programming-federation/issues/161) tracks recovery.

The existing Amsterdam workspace host now has a separately installed FSP CI user and the reviewed gate scripts.
An operator-assisted one-off bridge uses the owner's existing GitHub CLI session for API operations and canonical
fetches, then calls the installed controller's gate, lane-merge and deployment functions. It preserves four gate
statuses, exact-tree evidence, the controller lock, independent security checks and actual public/mail acceptance.
The bridge does not copy the GitHub token or inject credentials into test jobs. The cold recovery run used the normal
15 GiB admission guard. After host failures, a bounded operator retry reuses its completed caches only when
the build dependency graphs and container build definitions are unchanged and at least 5 GiB remains; it repeats every gate. The subsequent integration includes the canonical Silero source change, using those same Rust/TypeScript caches; its Python parser/encoder checks also run in release safety. This is private
recovery tooling, not a change to the unattended controller's 15 GiB policy. The timer remains disabled until
unattended authentication is configured. Ubuntu SSH troubleshooting was stopped at the owner's request; do not
claim that runner or a new dedicated PAT has been restored. Private logs/evidence and accepted-release receipts
use `/var/lib/fsp-ci` on Amsterdam. A release is accepted only when its actual receipt and commit status agree.

## Gates and trust

`gate.sh` preserves platform types/lint/contracts/components/build/browser acceptance, Rust fmt/clippy/tests
with real PostgreSQL, real PDF extraction under the API's exact systemd restrictions, Compose images and HTTP
checks, deployment fault recovery, nginx configuration and controller regression tests. Backup creation is disabled by owner policy.
All four statuses (`fsp/platform`, `fsp/backend`, `fsp/compose`, `fsp/release-safety`) must pass. Exact Git tree
and installed controller content identify evidence; identical integration trees reuse successful evidence and
explicitly name its source SHA in the status. Failures do not pass and are not repeatedly retried automatically.

Only this private canonical repository and PRs authored by its two maintainers are admitted. Fork PRs never run.
Tests run as `fsp-ci` with an empty inherited environment. `/etc/fsp-ci` is root-only. Docker access is root-equivalent:
this is a trusted-maintainer runner, not an isolation boundary for hostile code. Never admit public/fork contributions
or put another repository's credentials in the project directory. Project containers/volumes use the `fsp-ci` prefix;
cleanup never prunes the shared host. The controller is installed separately and is not automatically updated by a PR.
Host CPU/memory limits, serial jobs and two Cargo build jobs leave capacity for existing runner slots.
Compose checks let Docker allocate loopback ports for the web/API, interview and PostgreSQL services, avoiding
collisions with other workloads. Origin checks use a fixed configured test origin independently of those ports.
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
If Ubuntu already provides `/etc/apparmor.d/bwrap`, reuse it and remove only the redundant FSP attachment;
otherwise install the FSP profile. Two profiles for `/usr/bin/bwrap` prevent attachment and break extraction.
Installation probes namespace creation as an unprivileged user; backend CI additionally extracts an actual PDF.
Configure mode-600 files in mode-700 `/etc/fsp-ci`:

- `github-token`: GitHub token able to read this private repository, write commit statuses and merge PRs.
- `ssh-config`, `deploy-key`, `known_hosts`: pinned SSH identity for the existing FSP stand only; host alias `stand`.
- `public-url`: current HTTPS URL, and `auth-mode`: `demo` or `resend`.
- `resend-acceptance-key`: the dedicated receiving-email test key, never the runtime sending key.
- `deploy-enabled`: optional empty marker; create only after private deployment configuration is verified.

### Dedicated GitHub credential

On October 11, 2026, [#139](https://github.com/KirpaDmitriy/sport-programming-federation/issues/139) replaced the CI
copy of the shared credential with a separate classic PAT for `rldyourmnd`, with only `repo` scope. That
credential expires **November 10, 2026 at 06:31:54 UTC**, but its only active secret copy was on the subsequently
deleted CI host. Recovery must establish whether an authorized migration preserved it; otherwise replace the
dedicated token and revoke its orphaned predecessor. Do not confuse its future expiry with available access.
The original shared credential remains unchanged in its owner-managed tooling. Installation validation and
subsequent CI/release receipts are recorded in the issue.

For the current repository-collaborator identity, issue a separate classic PAT with only `repo` scope and an
expiration date. It remains limited by the account's actual repository permissions; it is not a repository-specific
installation token. Do not reduce or revoke a credential shared with other projects. Reconsider a repository-scoped
GitHub App when the repository owner can install one; the current operator has push access without administration.

Save the new PAT privately in the original checkout's ignored `.fsp-infra/ci-github-token`, mode `600`. Never put it
in command arguments, Git, issues or chat. Read-only preflight from the repository root:

```sh
python3 infra/ci/install_token.py < .fsp-infra/ci-github-token
```

Install from the accepted `main` checkout on the authorized CI host (the input redirection is local):

```sh
ssh "$FSP_CI_HOST" 'python3 /var/lib/fsp-ci/deploy/infra/ci/install_token.py --install' < .fsp-infra/ci-github-token
```

Set `FSP_CI_HOST` to the explicitly authorized replacement host after #161 is resolved. The installer checks the canonical repository, push access, an admitted maintainer, exactly `repo` scope and a
future expiration through bounded GitHub requests. Redirects are rejected. It holds the same file lock as the
controller, so a running or newly starting cycle cannot race the change. A busy controller or failed validation
leaves the active token untouched; retry after the cycle finishes. The replacement is atomic and root-only, with
no old-token backup. The timer and the stand continue normally. Temporary activation files are removed on ordinary
failure; a hard process/host crash can leave a private `.github-token-*` file requiring inspection.

Then request a normal controller cycle and verify canonical fetch/API access; the next ordinary PR cycle must
prove actual status writes and PR operations before declaring the replacement verified. `deployed.json` changes
only for a newly accepted release, not merely a token change. Retain the shared token in its original owner-managed
tooling. Remove the one-time local handoff file after successful replacement; the canonical active copy is
`/etc/fsp-ci/github-token`.

Tests run with `python3 -m unittest discover -s infra/ci -p 'test_*.py'` and in `fsp/release-safety`. They cover scope,
identity, expiry, controller contention, provider errors, failed atomic replacement, private modes and secret-free
errors; rejected HTTP responses are explicitly closed before reporting a sanitized status code. Sources: [GitHub PAT limitations](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens),
[scope headers](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps),
[expiration header](https://github.blog/changelog/2021-07-26-expiration-options-for-personal-access-tokens/).

### Start the controller

Enable `systemctl enable --now fsp-ci.timer`. `systemctl start fsp-ci.service` requests an immediate cycle; an
OS file lock also prevents duplicate manual controllers. Secrets never enter the checkout, evidence JSON or logs.
Deployment uses only verified current main and existing `infra/deploy.sh` locking/rollback. `public.sh` then executes
the actual HTTPS browser journey; failure produces a failed deployment status, not a live-success marker. Credential
availability or delivery failures leave checks failed. The controller never resets the production database.

## Operation and recovery

For a lost host, rebuild the reviewed controller from canonical source on the authorized destination. Restore
the FSP-only private configuration from available owner-managed credentials, or issue replacements where the
only copy was lost. Verify pinned stand SSH identity, file permissions, toolchains, free disk and resource limits
before enabling the timer. Run all four gates and actual public acceptance to generate new evidence and
`deployed.json`; historical GitHub statuses are evidence of past runs, not substitutes for these checks.
Do not create backups, snapshots, redundant mirrors or a paid replacement without the owner's instruction.

Inspect `journalctl -u fsp-ci.service` and private `/var/lib/fsp-ci/logs/<sha>/*.log`; evidence lives in
`/var/lib/fsp-ci/evidence/`. A failed revision is retried by removing only its matching evidence JSON and requesting a
cycle. New content automatically receives new checks. A deployment failure remains red, retries at most three times with five-minute spacing, and never blocks
PR checks needed to fix it. After fixing infrastructure, remove only `deploy-attempts.json` to retry; a new main
SHA or changed public URL/auth mode/inbox key starts a fresh acceptance cycle. `deployed.json` records only actual accepted releases. If GitHub rejects the final success-status write, the next
cycle reconciles it from the matching accepted SHA/configuration without redeploying or repeating browser
acceptance; successful status receipts still bound duplicate writes. Stop the timer to pause
new cycles; stop the service separately only when deliberately interrupting a running check/release.

For controller upgrades: stop the timer, wait for the current service to finish, review the diff, run `install-host.sh`
from the reviewed release, then restart the timer. For a single script update, review and validate it,
install it with the existing root ownership/mode while the service is inactive, and restart the timer.
Controller content changes invalidate cached proof. Keep the
stand's earlier compatible application releases; reverting CI never requires database changes. Do not re-enable retired
Actions workflows without restoring billing/permissions and deliberately migrating the gate policy.

Primary references: [GitHub commit statuses](https://docs.github.com/en/rest/commits/statuses),
[runner administration](https://docs.github.com/en/rest/actions/self-hosted-runners),
[merge API](https://docs.github.com/en/rest/pulls/pulls#merge-a-pull-request),
[Git merge-tree](https://git-scm.com/docs/git-merge-tree), [Docker security](https://docs.docker.com/engine/security/).
