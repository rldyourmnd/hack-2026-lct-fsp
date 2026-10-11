# Domain and email release — issue #38

Owner policy update (2026-10-08, [#54](https://github.com/KirpaDmitriy/sport-programming-federation/issues/54)):
backups, snapshots and redundant mirror copies are disabled; existing backup archives were deleted.
Backup/mirror actions below are historical evidence, not instructions to repeat them.

Historical delivery record. For current behavior, verification and explicit limits, read
[current-state.md](current-state.md); completed baseline findings below are not open defects.

The approved target is `https://nddev.link` on the existing stand at
`188.166.88.183`. Real verification mail replaces public demo codes. The owner
authorized a delivery/login test to `danil@nddev.ai`. GitHub OAuth stays deferred;
the private fictional FSP provider and knowledge assessments stay in place.

## Implementation and acceptance sequence

1. Inspect current DNS, certificates, authentication, deployment and public tests.
   Cloudflare zone is active and initially had no records. The application only
   supported fixed demo codes; deployment required an IPv4 public URL.
2. Obtain an origin certificate while the IP stand stays available. Parameterize
   the nginx hostname and certificate, preserve renewal/provisioning, trust client
   IP headers only from Cloudflare's published networks, and prevent API caching.
   Verify both direct origin TLS and Cloudflare Full (strict) TLS.
3. Add a bounded Resend HTTP adapter, cryptographically random six-digit codes,
   challenge-bound HMAC digests and a separate session HMAC namespace. Require
   production secrets at startup before migration. Enabling the new secret must
   invalidate old demo sessions/codes without deleting account data.
4. Serialize real-mail issuance limits in PostgreSQL: one request per minute and
   five per fifteen minutes per normalized email, including failed sends. Preserve
   the cumulative five-attempt verification budget and single-use consumption.
   Test races, failed delivery, provider failures and production login over HTTP.
5. Verify a sending subdomain in Resend using its actual DNS records; disable
   tracking, publish SPF/DKIM/DMARC, and install a domain-restricted sending key
   outside git. Provider access is configured; the owner confirmed the first test message reached their inbox.
6. Update the frontend's real-mail guidance and release acceptance. Automated
   production login must read a real delivered OTP through a dedicated test inbox;
   no public fixed codes, hidden login bypass or silent skipped release gate.
7. Run relevant Rust, contract, frontend, browser and release-failure checks.
   Commit cohesive changes on feature branches, PR into `danil`, let checked
   integration PRs reach `main`, and deploy that exact commit. Retain rollback support.
   Actions billing prevented jobs from starting, and runner registration requires unavailable admin rights.
   Issue #40 replaces Actions with four real private-server gates and checked PR integration, as authorized.
8. Activate domain/mail configuration, verify real delivery and browser login,
   expired/wrong/reused code behavior, cookies, origins and private API caching.
   Provider delivery proves recipient-server acceptance; inbox placement requires
   mailbox evidence or owner confirmation. Obtain independent read-only proof.
9. Reconcile documentation/memories with implemented behavior, synchronize local,
   both remotes and the server, and take/verify a final off-host database backup.

## Primary references checked on 2026-10-08

- [Cloudflare Full (strict)](https://developers.cloudflare.com/ssl/origin-configuration/ssl-modes/full-strict/)
- [Cloudflare original visitor IPs](https://developers.cloudflare.com/support/troubleshooting/restoring-visitor-ips/restoring-original-visitor-ips/)
- [Resend domains](https://resend.com/docs/dashboard/domains/introduction)
- [Resend send API](https://resend.com/docs/api-reference/emails/send-email)
- [Resend restricted API keys](https://resend.com/docs/api-reference/api-keys/create-api-key)
- [OWASP verification token guidance](https://cheatsheetseries.owasp.org/cheatsheets/Forgot_Password_Cheat_Sheet.html)

The sequence above is implemented. Exact integrated release identities and final synchronization receipts are
recorded in issues #38 and #40; use those ledgers and the CI release marker for the current SHA.

## Execution evidence — 2026-10-08

- PR #41 shipped the domain/mail/CI implementation; PR #39 remains superseded history. GitGuardian flagged
  only an intermediate disposable test password in #39. The corrected branch used random credentials from
  its first CI commit and passed the scanner normally; no exclusions or fabricated statuses were added.
- All four private-server gates passed at `1e7a215` and `5c00c2f`: 26 mock tests, 36 component tests,
  36 ordinary browser tests (one opt-in screenshot test skipped), 67 Rust tests, real PostgreSQL HTTP tests,
  exact-systemd PDF extraction, Compose smoke, 16 release fault cases, 10 CI controller regressions and
  isolated backup/restore. PR #44 fixed the Node 26 JSON-import attribute and added public-suite collection
  to the pre-deployment gate after the first live acceptance loader failed. That failure remains visible.
- Origin cutover at 13:12 UTC preserved PostgreSQL data and activated real Resend authentication. The owner
  confirmed the first direct test message reached the inbox. A subsequent real Chromium login to
  `danil@nddev.ai` verified delivery, wrong-code rejection, resend limiting, single use, Secure/HttpOnly/
  SameSite cookies, reload persistence, foreign-origin rejection and API no-store at 13:19 UTC.
- Both public browser scenarios passed against `https://nddev.link`: actual delivered/authenticated email,
  new candidate, knowledge assessment, PDF export, two new companies, invitation acceptance and contact
  isolation. No mock controls, request interception, production reset or authentication bypass were used.
- Cloudflare proxy and Always Use HTTPS are enabled with Full (strict). Direct-origin and edge TLS both
  validate, API responses are DYNAMIC/no-store, and old IP traffic redirects to the domain. A forged direct
  CF-Connecting-IP is ignored; proxied requests retain the real visitor IP in nginx logs.
- Domain and original-IP certificate renewal dry runs passed. Independent read-only browser/HTTP/log
  verification of the cutover found no JavaScript errors or server 5xx. No OpenObserve is installed;
  operational verification uses systemd/nginx and CI logs.

Private delivery/login proof and provider configuration stay in the ignored `.fsp-infra/`.
The CI host records accepted releases only after deployment and the real public mail/browser journey pass;
`/var/lib/fsp-ci/deployed.json` and `fsp/deployment` are authoritative for that result. CI logs retain 14 days.
Backup creation and off-host copies are disabled by the subsequent owner policy (#54); existing backup archives
were deleted. The earlier restore receipts above are historical evidence only.
