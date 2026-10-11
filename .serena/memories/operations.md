# Operations and delivery

Public site https://nddev.link; existing origin 188.166.88.183. Domain/email cutover completed on October 8; #38 tracks proof;
external CI migration in #40. Both deliveries are complete; #50 checked consistency; #54 disables backups and redundant copies. See docs/current-state.md for current limits.
Private stand/Resend state is in the original checkout's ignored .fsp-infra/; never print credentials.
User explicitly authorized existing CI servers after canonical Actions billing failed and runner registration
was denied. Project CI lives on server-gha-runner-1 (moved 2026-10-11; server-gha-runner-2 was retired); other projects/runner slots are not part of this task.

Canonical upstream KirpaDmitriy/sport-programming-federation; the additional GitHub mirror is retired. Feature -> danil/dima
-> dev -> main uses PRs. infra/ci controller reports four fsp/* statuses, preserves independent security checks,
checks unchanged heads/bases and merged tree identity, and deploys verified current main. Actions is retired.
Controller installation is reviewed separately; no PR auto-updates it. Root-only /etc/fsp-ci stores credentials;
builds run as fsp-ci with no injected secrets. Docker is root-equivalent; trusted private maintainers only.
CI timer/OS lock serialize jobs, retain logs 14 days, refuse builds below 15 GiB free. A matching accepted
receipt repairs a failed final GitHub status write without redeployment; status receipts prevent duplicate writes.

Stand units: fsp-api:4010, fsp-simulator:4020, fsp-github-insights:4030 (tokenless; needs git, clones in its
CacheDirectory; API needs FSP_GITHUB_INSIGHTS_URL), fsp-interview:4040, fsp-bifrost, fsp-cliproxy, nginx, postgresql@16-main.
/health is unversioned. Full-SHA immutable releases, host lock, nginx checks and rollback preserve current data.
Same-SHA replay health-checks without rebuilding. Current/previous releases are retained. Schema guards reject
changed histories/legacy strict SQLx archives; only additive forward-compatible migrations are supported.
Runtime guards reject rollback lacking domain/resend support after cutover. Never edit applied migrations.
PDF requires Bifrost, Poppler/bwrap and executable-specific AppArmor; exact systemd restrictions are probed.
No /proc mount or global namespace relaxation. Failed activation restores configs/units; SIGKILL cannot run traps.

Cloudflare Full (strict), origin Let's Encrypt cert, canonical IP redirect, trusted CF IP ranges, API no-store.
Host-only HSTS is one week on HTTPS (including errors), absent on plain HTTP; nosniff/cache headers are unique.
The renewal unit is snap.certbot.renew.timer. Historical smoke API on 4011 was terminated after identity checks (#154).
Resend auth.nddev.link sends as login@auth.nddev.link with DKIM/SPF/DMARC, tracking off and enforced TLS.
Runtime key is sending-only/domain-restricted; private acceptance key reads real CI inboxes. HMAC key changes
invalidate old sessions/codes without deleting accounts. SMTP delivery and actual inbox placement are distinct.

Owner standard: no backups, snapshots, off-host backup copies or redundant repository mirroring unless explicitly
requested later. FSP backup units are masked; provisioning/CI must not recreate archives. Existing project backup
archives were deleted. Sync local branches with canonical upstream only. No OpenObserve: inspect systemd/nginx logs.
GitHub OAuth remains deferred. No production DB reset, unrelated-host changes or unrequested credential rotation.

Latest review #138; compare upstream/main, /opt/fsp/current and deployed.json for the accepted release.
Credential follow-up #139: CI now uses a separate classic PAT for rldyourmnd with exactly repo scope, installed
October 11; expires November 10, 2026 at 06:31:54 UTC. Renew before expiry using infra/ci/README.md.
The original broad credential stays unchanged in the owner's CLI; never reduce/revoke it for this project.
The reviewed `infra/ci/install_token.py` reads stdin, validates exact repo scope, expiration, identity and access,
then replaces root-only credentials under the controller lock. Run from accepted main; see infra/ci/README.md.
Only /etc/fsp-ci/github-token holds the active CI copy; the one-time local handoff was removed. Issue #139 holds
the operational verification receipts. A classic repo-scoped PAT is not restricted to a single repository.
Follow-ups #152/#154/#155 cover status recovery, edge/process cleanup and explicit HTTPError response closure.
Last commits: e078301 (status recovery), b3f80bf (credential response cleanup), 63bb5eb (headers); 20 CI tests.
Read issue receipts and compare live refs for current accepted release and installed controller/helper hashes.
