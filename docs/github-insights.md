# GitHub insights

A candidate connects a GitHub account by login or profile link. The separate loopback service
[`fsp-github-insights`](../backend/crates/github-insights) analyzes the account's **public** work on our server,
without any token from the candidate or from us. It turns the work into a developer portrait (areas, languages,
stack, main projects), documented heuristics, a 0–100 index and integrity flags. The career API stores the report
with the source, shows it to the candidate in full and gives employers an anonymous projection. The algorithm is
`github-insights-v3` (v3 added the scope dimension and moved weight from rhythm to it); changing a threshold or
weight means a new version. Reports of an earlier version stay readable and are re-analyzed when the candidate
recalculates the estimate.

## Principles

- **Public only, no token.** Everything comes from GitHub search, the public REST API and blobless clones of public
  repositories. Private repositories are never seen, so an account's report is the same whoever starts the analysis.
- **Own work counts as own work.** Repositories the account leads and organizations it runs (found from commits,
  direct pushes and merging its own pull requests, not only from public memberships, which GitHub hides by default)
  form the *ownership* dimension. Working mostly in one's own organizations, and working through AI coding agents,
  are normal and not flagged.
- **Collaboration needs other people.** It counts pull requests approved by someone else, reviews of other people's
  work, and changes accepted into **independent** projects outside the account's own organizations.
- **Verifiable over self-declared.** Languages come from the files the candidate changed; technologies need an
  artifact (a manifest dependency, a configuration file) in a project where the candidate works.
- **Saturating, not cumulative.** Every metric maps onto 0–100 with a fixed threshold. Volume beyond the threshold
  earns nothing.
- **Flags lower confidence, never add points.** Anomalies are explained to the candidate and, when they are
  warnings, to employers.
- **No grade.** The index measures the amount and quality of public evidence. Only the knowledge assessment confirms a
  skill or a grade; the index does not change matching or search order ([business rules §3](product/BUSINESS_RULES.md#3-sources-and-evidence)).
- **Conservative public figures.** Numbers shown to employers are rounded **down**, so they never overstate.

## Data collected

Anonymous GitHub limits apply per server IP: 60 REST requests per hour and 10 search requests per minute
([rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)). The client spaces
search requests 6.2 seconds apart, reads `x-ratelimit-*` and `retry-after`, and waits for a reset of up to
70 seconds inside a request; a longer reset re-queues the analysis until the limit resets (within an hour).

| Stage | Source | Bound |
| --- | --- | --- |
| Profile: login, creation date, followers, bio (ownership check only, never stored) and public memberships | REST `/users/{login}`, `/users/{login}/orgs` | 2 requests |
| Commit windows: four quarters of the last year, then calendar years back to the account's creation | commit search `author:X author-date:…` | ≤ 8 years plus one window for anything older; exact totals, 100 newest commits per window as a sample |
| The newest pull requests | issue search `type:pr author:X` | 100 |
| Owners the account works with: public memberships, then owners of sampled commits and pull requests | commit search `author:X user:OWNER` | 6 owners; exact commit totals per owner |
| Repository metadata (stars, forks, language, topics, license, size, default branch) | repository search `user:X`, `org:…`, batches of `repo:` | — |
| Default-branch history and tree of repositories with sampled commits (by commits), then owned ones and those pushed within two years in owners where the account has at least 20 public commits, which newest-first samples miss; the last count only with the account's commits | `git clone --bare --filter=blob:none --single-branch`, `git log --name-only`, `git ls-tree` | 80 repositories under 1.5 GB, 6 at a time, 3 minutes each; clones are deleted after reading |
| Manifests: `Cargo.toml`, `package.json`, `pyproject.toml`, `requirements*.txt`, `go.mod` | `raw.githubusercontent.com` | 12 per repository, shallow paths first, vendored trees skipped |
| Pull-request counts: last year, approved by others (`review:approved`), reviews of others' pull requests, issues | issue search | totals |
| Pull requests into other owners' projects | issue search `type:pr author:X -user:X -org:…` | adaptive, up to three rounds of 100 |
| Reviews in independent projects and their metadata | issue and repository search | totals; 16 notable repositories |

The clones give what the JSON APIs cannot: author times **with the author's own UTC offset** (`%aI`), the files
each commit changed, the whole tree, and whether the account committed its own commits itself (direct pushes).
A blobless clone downloads commits and trees but no file contents; rename detection is off so none are fetched.
Commit search indexes default branches only, so commits on other branches are not counted.

A heavy account (13 500 public commits in a year, 78 repositories, 92% of the year's commits analyzed) takes about three minutes. A stage that fails
marks the report partial and keeps what it has.

### Background analysis

`POST /v1/analyses` queues an analysis or returns the running or fresh one; `GET /v1/analyses/{login}` reports
`queued`, `running`, `done` or `failed` with the stage and progress. Before queuing, one REST request rejects a
mistyped login or an organization account at once (`github_account_not_found`). Two analyses run at a time,
finished reports stay cached for six hours and a failure can be retried after five minutes. Updating a source
within six hours reuses the report and only re-reads the bio, so an ownership check takes seconds. In the career API, connecting or updating a
GitHub source creates a `source_sync` job and sets the source's `activeJobId`. Each poll of the job (every two
seconds) asks the service and, once the report is ready, applies it in one transaction with the job's result; a
concurrent poll finds the job already finished. If the service restarted and forgot the analysis, the poll starts it
again. A job still running after 90 minutes fails with `github_unavailable`. The sources list shows the stage and
progress of a running analysis.

## Ownership

A login alone is a claim. The candidate proves control by placing a per-candidate code (`fsp-` plus ten hex
characters, an HMAC of the candidate id under the server key) in the GitHub bio and updating the source; the service
checks the bio (re-read when older than ten seconds) and discards it. An OAuth token of the same account (when OAuth
is configured) proves it too. A proven login belongs to one candidate (`409 github_account_taken`); an unproven claim
stays visible to employers as «не подтверждено».

## Account roles

| Term | Rule |
| --- | --- |
| Own organization | A public membership; an owner where the account committed to three or more sampled repositories; an owner of a cloned repository where the account pushed its own commits directly; an owner where its merged pull requests in the external sample span three repositories or exceed thirty (merge rights) |
| Led repository | At least 10 of the account's commits and at least half of the default-branch history |
| Independent repository | Not owned, not in an own organization, not led |
| Substantive repository | Not a fork; owned, led or with at least 10 of the account's commits |

## Developer portrait

- **Languages** are shares of the files the candidate changed, by extension, over all analyzed commits.
- **Areas** (backend, frontend, DevOps and platform, systems tools and CLI, data and AI, mobile) attribute each changed
  file by its path and language, read against the repository's detected stack: CI, deployment and infrastructure
  paths and shell scripts are platform; `.tsx`, UI and web paths are frontend; Python under `api`, `routes`,
  `services` and similar paths of a web service is backend; notebooks and code of projects that use analytics, ML or
  LLM libraries are data and AI; Rust or Go in a web service is backend, otherwise systems. Storage, ORMs, project
  topics and AI coding-agent configuration do not make a project data work. Each area lists its two main languages
  with file counts and the stack behind it.
- **Stack** comes from manifest dependencies and files (Dockerfile, workflows, Terraform, Kubernetes, systemd, nginx,
  `AGENTS.md`/`CLAUDE.md`/`.cursor` agent configuration, MCP) in repositories the candidate owns or committed to at
  least three times, grouped by category. Catalog technologies become skill evidence only from projects the candidate
  substantially develops (owned, led or ten commits).
- **Topics** are the most common topics of owned and led repositories; license and profile topics are skipped.
- **Main projects** are owned or led repositories ranked by the candidate's commits.
- **Summary** names the areas, languages, projects, organizations, yearly public commits and key technologies. The
  employer version drops organization names and rounds the commit count down.

## Dimensions and metrics

Scores use `ramp(value, zero, full)` (linear, clamped), `log(value, full)` (`ln(1+v)/ln(1+full)`, clamped) and
`inverse(value, full_until, zero_at)`. A dimension is the weighted mean of its available metrics; the index is the
weighted mean of the dimensions. Bands: 0–24 weak, 25–49 moderate, 50–74 strong, 75–100 very strong.

| Dimension (weight) | Metric | Score | Weight |
| --- | --- | --- | --- |
| Регулярность (15%) | Active weeks of the last 52 | `ramp(0, 40)` | 35 |
| | Longest run of active weeks | `ramp(0, 26)` | 20 |
| | Current run of active weeks | `ramp(0, 12)` | 15 |
| | Active months of the last 24 | `ramp(0, 20)` | 15 |
| | Weekly burstiness B = (σ − μ)/(σ + μ) | 100 at B ≤ 0, 0 at B ≥ 0.8 | 15 |
| Вовлечённость (15%) | Public commits in the last year (search total) | `log(1000)` | 15 |
| | Active days in the last year | `ramp(0, 180)` | 20 |
| | Longest run of consecutive days | `ramp(0, 30)` | 15 |
| | Median active days per active week | `ramp(0, 5)` | 10 |
| | Median time per active day (≥ 20 commits) | `ramp(0, 6 h)` | 15 |
| | Median time per active week (≥ 20 commits) | `ramp(0, 30 h)` | 10 |
| | Days since the last activity | `inverse(14, 180)` | 15 |
| Стаж (10%) | Account age, years | `ramp(0, 6)` | 30 |
| | Years with public commits | `ramp(0, 5)` | 40 |
| | Years with 50+ active days | `ramp(0, 4)` | 30 |
| Совместная работа (15%) | Pull requests approved by other people | `log(25)` | 35 |
| | Reviews in independent repositories | `log(50)` | 25 |
| | Independent repositories with accepted work | `ramp(0, 10)` | 20 |
| | Merged pull requests into independent repositories | `log(25)` | 20 |
| | Merge rate into independent repositories (when known) | `ramp(30%, 90%)` | 10 |
| Собственные проекты и организации (15%) | Led repositories | `ramp(0, 10)` | 25 |
| | Owned or led repositories with commits in the last year | `ramp(0, 8)` | 20 |
| | Own organizations | `ramp(0, 3)` | 15 |
| | Repositories with the account's commits in own organizations | `ramp(0, 15)` | 20 |
| | Pull requests in the last year | `log(300)` | 20 |
| Инженерная культура (15%, needs a substantive repository) | Shares of substantive repositories with README, license, CI, tests, lockfile, linters, dependency updates | the percentage | 10, 10, 20, 20, 10, 10, 5 |
| | Descriptive commit headlines | the percentage | 10 |
| | Median files per commit | 100 for 1–10, 60 for 11–30, 20 above | 5 |
| Признание (5%) | Stars of substantive repositories | `log(500)` | 35 |
| | Forks of owned repositories | `log(100)` | 15 |
| | Independent projects from 1 000 stars with ≥ 3 commits or a merged pull request | `ramp(0, 3)` | 40 |
| | Followers | `log(200)` | 10 |
| Масштаб и сложность (10%) | Owned or led repositories with ≥ 10 own commits spanning ≥ 180 days | `ramp(0, 5)` | 35 |
| | Engineering areas of the portrait with at least 15% | `ramp(0, 3)` | 20 |
| | Non-language stack categories used in two or more repositories | `ramp(0, 6)` | 20 |
| | Substantive repositories with design artifacts: architecture decision records or design documents, OpenAPI/AsyncAPI/GraphQL/Protocol Buffers contracts, schema migrations | `ramp(0, 5)` | 25 |

Scope and complexity is the axis engineering level frameworks rank first (the
[levels.fyi framework](https://www.levels.fyi/blog/swe-level-framework.html) defines levels by scope, impact and
ambiguity): it counts systems sustained for months, breadth across areas and layers, and design work beyond code,
never commit volume.

Hygiene is read from the whole default-branch tree, so tests anywhere in the tree count. Stars and followers are
gameable ([StarScout](https://arxiv.org/abs/2412.13459) found millions of fake stars), so impact carries 5% and the
notable-project metric needs accepted work.

## Work rhythm

From the last year's commits in the author's own local time (two years when the last has fewer than 20): the
hour-of-day and weekday distributions; the shares on weekdays 09:00–18:59, evenings after 19:00, nights before 06:00
and weekends; the time zones of the commits. Sessions follow the
[git-hours](https://github.com/kimmobrunfeldt/git-hours) convention: commits up to 120 minutes apart form a session,
which is credited its span plus 30 minutes before the first commit. The credit goes to the day the session starts and
a session past midnight is split between the local days it covers. The report gives the median and 90th-percentile
time per active day, the median per active week, sessions per active day and the longest session. These are
estimates from timestamps, not working time; the score saturates at six hours a day and thirty a week. Commit times
vary widely between people, so rhythm is described rather than judged
([Claes et al., ICSE 2018](https://arxiv.org/abs/1802.05084)).

## Integrity flags

| Flag | Severity | Condition |
| --- | --- | --- |
| `future_dated` | warning | Commits authored more than a day in the future |
| `uniform_commit_minute` | warning | ≥ 20 commits and ≥ 40% at one local minute of the day |
| `repeated_messages` | warning | ≥ 20 commits and ≥ 50% with one headline |
| `padding_repository` | warning | An owned or led repository under 10 stars holds ≥ 80% of the commits (≥ 30 there), ≥ 80% of them change one file and ≥ 30% repeat one message |
| `new_account_burst` | info | Account younger than 120 days with ≥ 300 commits in the year |
| `fork_heavy` | info | ≥ 70% of ≥ 10 owned repositories are forks |
| `partial_coverage` | info | Under half of ≥ 50 public commits of the year are in the analyzed repositories |
| `low_public_data` | info | Fewer than 10 active days and commits |

Confidence is high with ≥ 150 analyzed commits, ≥ 60 active days and two active years; medium with ≥ 30 commits and
≥ 15 active days; low otherwise. A warning lowers it one level; a partial report or coverage under 30% is at most
medium. Coverage is the share of the year's public commits (search total) found in the analyzed repositories.

## Employer projection

Before any disclosure employers see the index, band, confidence, ownership state, the public summary, areas with
shares, up to six languages and sixteen technologies, dimension scores with public metric displays, time per active
day and week in bands, office-hour and weekend shares, highlights without names and warning flags. Counts are rounded
down to two significant figures, shares to 5%. The projection never contains the login, repository or organization
names, links, calendars or exact counts: a calendar or an exact total could identify the person. After the candidate
accepts an invitation or applies, the chosen «source links» disclose the profile.

## Validation

- Unit tests cover commit windows, affiliation, session splitting, file attribution, topic filtering, manifests,
  plural forms and rounding (`cargo test -p fsp-github-insights`).
- A golden test builds the report of a synthetic account (an owned Rust/React project, a led organization project
  and two commits in someone else's library) and checks exact totals, local rhythm, own versus independent work,
  attribution of the candidate's own changes and that the employer projection names nothing.
- A service test runs the HTTP API against a local fake of GitHub's REST, search and raw endpoints and public
  repositories cloned from local directories over `file://`: background states, the bio code, owner totals,
  deletion of clones and failures.
- The golden report and projection in [`contracts/fixtures`](../contracts/fixtures) are the Rust serialization; the
  mock suite validates them against the OpenAPI schemas and the mock and the frontend render them, so the backend and
  the contract cannot drift. The career API's asynchronous flow is tested against PostgreSQL.
- Live tokenless runs on public accounts calibrated the rules. On a developer who works many hours a day through AI
  agents in their own organizations, they surfaced and fixed: commit samples that missed most organizations (owner
  searches now cover them, with exact totals), self-merged work in an own organization counted as external
  collaboration (merge rights), yearly totals shifted by rolling windows (calendar years are searched exactly),
  sessions over midnight credited to one day, and database and agent tooling inflating the data-and-AI area.

The thresholds are a transparent prototype, not a calibrated predictor of job performance. Measuring agreement with
expert ratings or hiring outcomes needs labelled data that does not exist yet. Research on hiring signals found that
employers value long-term activity and contributions to established projects but mostly glance at aggregates
([Marlow & Dabbish, CSCW 2013](https://www.cs.cmu.edu/~xia/resources/Documents/Marlow-cscw13.pdf)); predicting
library expertise from activity features proved hard ([Montandon et al., MSR 2019](https://arxiv.org/abs/1903.08113)).
The report therefore explains every number and leaves the decision to people.

## Operation

The service listens on `127.0.0.1:4030` (`fsp-github-insights.service`) and needs `git` on the host. Clones live in
the unit's `CacheDirectory` only while one analysis reads them. No token is required; an optional read-only
`GITHUB_TOKEN` in `/etc/fsp/github-insights.env` only raises GitHub's limits. The API enables the integration with
`FSP_GITHUB_INSIGHTS_URL` (`meta.capabilities.githubInsights`). All analyses share the server's anonymous search
budget, so concurrent analyses queue behind each other. Use of GitHub data follows GitHub's
[Acceptable Use Policies](https://docs.github.com/en/site-policy/acceptable-use-policies/github-acceptable-use-policies):
the candidate starts the analysis for their own profile, nothing is collected about people who did not connect, and
no personal data is sold or used for unsolicited contact.
