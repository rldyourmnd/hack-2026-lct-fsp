# FSP observability

The stand uses native OpenObserve OSS **1.0.4**, Vector **0.59.0** and the bundled OpenObserve browser SDK
**0.4.3**. Installer archive SHA-256 pins and npm integrity are committed. There is no Docker requirement, hosted
collector or backup producer. Application activation still goes through the normal verified main release.

## Signals and privacy

- `fsp_logs`: allowlisted structured application events and nginx access measurements. API commands record method,
  route template, status, error code, duration, generated request ID, authenticated internal actor ID and role.
  A valid W3C trace ID joins browser requests to API logs. This is correlation, not an OTLP span exporter.
- `llm_call` events from the AI adapters: task, model, effort, duration and completion outcome. No prompts, model
  answers, reasoning content, repository names, résumé text or interview answers are collected.
- `fsp_host_*` and `fsp_collector_*` metric streams: host CPU, memory, root filesystem, disk/network totals and
  collector throughput, backpressure and drops. Process arguments and arbitrary host logs are not collected.
- `_rumdata` and `_sessionreplay`: page performance, API timing, sanitized errors, safe action names and masked
  replay. No second browser APM SDK is installed. Browser/OS and anonymous in-memory session IDs are retained;
  identity is not set from account data. Reloading starts a new diagnostic session.

The collector rebuilds log envelopes from an allowlist and discards free-form messages, headers and bodies.
Pre-existing unstructured service messages become generic service events. API audit is operational diagnostic
evidence, not a transactionally guaranteed or tamper-proof compliance ledger. The log queue is capped at 256 MiB;
when full it drops new events and exposes the drop counter. It never blocks application requests indefinitely.

Replay masks **all text and inputs**, including rendered personal data; it does not record audio or video. Login,
settings, onboarding and assessment/interview routes do not start replay. Unknown URLs, fragments and queries
other than a syntactically valid opaque `needId` also disable replay because SDK replay metadata contains the real
URL and cannot be changed by `beforeSend`. Sanitized RUM performance events continue. Route transitions stop
recording before the next snapshot; manual view tracking starts it again only on an accepted URL. Error messages,
stacks and custom contexts are removed; action names come only from a fixed label map or normalized navigation
route. Other clicks retain their interaction type. Consequently, the replay shows layout and interactions rather
than readable candidate content.

Browser intake uses only POST `/observe/rum` and `/observe/replay`, an exact same-origin check, a 1 MiB body limit
and a 10 request/s per-client budget with a bounded burst. nginx discards all incoming headers except content type
and injects the RUM credential from its private config. Query parameters and other `/observe/*` paths are rejected.
No cookie, authorization header, referrer, original client IP or reusable ingestion secret is forwarded from the
browser. The administrator API/UI listens on loopback; its owner-authorized HTTPS endpoint is
`https://observe.nddev.link`, protected by native account authentication. The FSP application browser has no
OpenObserve credentials.

## Installation

Run from a reviewed checkout on the authorized FSP stand as root:

```sh
python3 infra/observability/install.py
python3 infra/observability/verify.py
python3 infra/observability/verify_intake.py
```

Keep `contracts/openapi.json` with the checkout: it generates the allowlisted API route map. Release archives
include this source as well. The installer rejects an incomplete checkout before changing the host.

The installer verifies artifacts before activation, preserves existing administrator credentials, creates a named
ingest-only collector token and installs sandboxed `fsp-openobserve` / `fsp-vector` services. Their IP policy
allows only loopback. OpenObserve vendor telemetry, its own UI RUM, events, tracing export, GeoIP download and
browser download are disabled. Retention is 14 days; service memory and collector queues are bounded.

The normal release renderer includes private nginx snippets when installed, and enables the browser SDK only
when the intake configuration is present. `VITE_RELEASE_SHA` identifies the shipped source. The installer itself
does not reload nginx or activate an application release. Provision observability before shipping the release.
Local development and Docker remain telemetry-off unless explicitly enabled.

Credentials stay in `/etc/fsp/observability.env` (root, 0600), `/etc/fsp/vector-secrets.json` (root:fsp-vector, 0640)
and `/etc/nginx/fsp-observability/locations.conf` (root, 0600). Never put them in Git, browser configuration,
screenshots, chat or logs. The collector credential cannot read streams; `verify.py` checks that boundary.

## Public account access

On October 12, 2026 the owner explicitly authorized public access at **https://observe.nddev.link** (#191).
The existing administrator login is `observe-admin@nddev.link`. Its password remains in the root-only environment;
the owner-requested delivery file is `~/Desktop/FSP-OpenObserve-access.txt` on the Amsterdam workspace host (0600).
The UI never bypasses account authentication or injects administrator credentials into nginx requests.

The dedicated nginx site handles HTTP-to-HTTPS redirects, TLS, WebSocket upgrades, login rate limiting and
no-store responses. Public `/metrics` is blocked. OpenObserve ports 5080/5081 and its outbound network policy
remain loopback-only. This site is independent of application releases and does not change browser intake.

The `observe.nddev.link` A record points directly to `188.166.88.183` with Cloudflare proxying disabled
(`proxied=false`, TTL 300). This preserves access without Cloudflare CDN for the Russian audience. Account
authentication and nginx's no-store policy still apply.

To reproduce the publication on the authorized stand, create that DNS-only A record for the existing origin,
issue the certificate through the existing ACME webroot, then run the reviewed publisher:

```sh
certbot certonly --webroot -w /var/www/acme -d observe.nddev.link \
  --cert-name observe.nddev.link --non-interactive --agree-tos --deploy-hook 'systemctl reload nginx'
python3 infra/observability/publish.py
```

The publisher sets `ZO_WEB_URL`, adds the exact public CORS origin, installs `nginx-public.conf`, validates nginx
and activates the two affected services. Reinstalling observability preserves the configured public URL/origins
and existing credentials. Certbot renews the certificate automatically. No backups or new infrastructure are created.

The optional private UI remains accessible through a pinned SSH connection:

```sh
ssh -F .fsp-infra/ssh/config -NT -L 127.0.0.1:5080:127.0.0.1:5080 fsp-demo
```

Visit `http://127.0.0.1:5080` in the browser on the machine running the tunnel. The administrator login is
`observe-admin@nddev.link`. The Amsterdam workspace also has an enabled user service `fsp-observe-tunnel.service`
at `http://127.0.0.1:50800`; public access does not require that tunnel.

Playwriter verified account login, populated dashboard/audit data, actual public RUM/replay ingestion and masked
persisted playback on October 12. Upstream viewer limitations remain tracked in #189: a cold SPA transition can
race the player import (a page refresh succeeds), and the optional `_rumlog` request returns 400 because raw browser
console collection is deliberately disabled. Do not describe those upstream defects as fixed.

## Verification and operation

`verify.py` sends a unique structured event through journald and Vector, queries it in OpenObserve, checks removal
of private canaries, verifies that the collector cannot read streams, and queries a recent host-memory metric.
`verify_intake.py` runs a temporary loopback nginx using the real renderer and checks method, origin, query and path
boundaries plus actual ingestion. Its access log must contain none of the supplied header/query canaries.
It does not reload the production nginx.

The platform CI additionally runs:

```sh
cd frontend
FSP_RUM_TEST=1 VITE_OBSERVABILITY=1 bun run e2e e2e/observability.spec.ts
```

This inspects actual outgoing SDK events and decompresses replay segments, inserts canaries in inputs, rendered
text, links, attributes, errors and query parameters, and rejects external requests. An optional loopback
`FSP_RUM_UPSTREAM` and process-only `FSP_RUM_TOKEN` run the same test against real OpenObserve; never prefix the
token with `VITE_`. Public browser acceptance after release is still required.

Useful SQL in Logs (select a recent time range):

```sql
SELECT route, status, count(*) AS requests, avg(duration_ms) AS mean_ms,
       approx_percentile_cont(duration_ms, 0.95) AS p95_ms
FROM fsp_logs WHERE event = 'http_request' GROUP BY route, status ORDER BY requests DESC;

SELECT _timestamp, actor_id, role, route, method, status, code, request_id
FROM fsp_logs WHERE event = 'audit_command' ORDER BY _timestamp DESC;

SELECT task, reasoning_effort, outcome, count(*) AS calls, avg(duration_ms) AS mean_ms
FROM fsp_logs WHERE event = 'llm_call' GROUP BY task, reasoning_effort, outcome;
```

Use the built-in RUM views to inspect sessions, route performance, errors and replay. Look at failure rates and
durations together; a successful health probe does not prove a successful candidate/employer workflow. Inspect
`fsp_collector_*` drops and root-filesystem headroom before increasing sampling or retention. No external alert
destination is configured. This deployment currently has no on-call notification delivery.

The installer also creates **ФСП: состояние и диагностика**, a version-8 dashboard with nine panels for API
status/latency/errors, AI timing, command audit, CPU, available memory, free disk and the log queue. API panels
select only API service events, avoiding double-counting nginx requests. Re-running the installer preserves
operator dashboard edits. `verify.py` executes all nine queries, including real recent metric samples.
Before the instrumented application release, the application panels correctly have no matching events.

Sources: [OpenObserve RUM](https://openobserve.ai/docs/user-guide/data-exploration/rum/setup/),
[replay privacy](https://openobserve.ai/docs/user-guide/data-exploration/rum/session-replay/),
[environment settings](https://openobserve.ai/docs/administration/configuration/environment-variables/),
[Vector secrets](https://vector.dev/docs/reference/configuration/secrets/),
[Prometheus ingestion](https://openobserve.ai/docs/ingestion/metrics/prometheus/).
