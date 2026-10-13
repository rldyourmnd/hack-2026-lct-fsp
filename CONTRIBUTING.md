> This is Danil's private hackathon maintenance copy. The competition upstream is frozen at `6a5faa6` after the October 11, 2026 deadline (23:59 Moscow / October 12, 01:59 Almaty). Contribute through branches and pull requests in this repository only. The event workflow documented below is historical; do not push, open integration PRs or run private deployment controllers against the competition upstream.

# Contributing

Use the [README](README.md) for setup and [business rules](docs/product/BUSINESS_RULES.md) for product behavior.
Current code takes precedence over historical plans and delivery notes.

## Ownership

| Area | Maintainer | Reference |
| --- | --- | --- |
| Rust backend | Dima | [Backend architecture and configuration](backend/README.md) |
| Frontend and development mock | Danil | [Frontend contribution guide](frontend/CONTRIBUTING.md), [mock API](mock-server/README.md) |
| Stand infrastructure | Danil | [Operations](infra/README.md) |
| Contract, containers and product rules | Shared | [Contract workflow](contracts/README.md), [business rules](docs/product/BUSINESS_RULES.md) |

Coordinate changes in another maintainer's area before editing it. Keep changes focused and include the
documentation affected by the final implementation.

## Branches and pull requests

1. Synchronize with the canonical `upstream` repository and create a feature branch. Use names such as
   `feat/profile-search` or `fix/interview-timeout`; avoid the `danil/` and `dima/` prefixes because `danil`
   and `dima` are permanent branch names.
2. Commit logical changes separately, with English Conventional Commit messages. Describe the behavior change
   and actual validation results in the pull request.
3. Open the feature PR into the responsible maintainer's branch, `danil` or `dima`.
4. Integrate the verified personal branch into `dev`, then `dev` into `main`, following the
   [private CI runbook](infra/ci/README.md). Preserve merge history; do not rewrite shared branches.
5. Delete the feature branch after merging. Keep `danil`, `dima`, `dev` and `main` permanent.

Only `main` is deployed to the stand. CI uses the external private controller, not GitHub Actions; its recovery
status and release gates are documented in the runbook. Synchronize only with canonical `upstream`; the retired
GitHub mirror must not be updated.

## Contract and code boundaries

The [OpenAPI contract](contracts/README.md) is the boundary between the frontend, mock and backend. Change it
in one reviewed change that updates the contract source, generated files, every consumer and relevant tests.
Run `just contract` to regenerate it; do not hand-edit generated types or maintain a separate private API dialect.

JavaScript-ecosystem source is strict TypeScript 7 checked by `tsc`; do not add hand-written JavaScript or an
older TypeScript compiler. Use pinned dependencies and lockfiles. Rust follows the inward dependency rule
described in the backend guide. Write code, comments, documentation and commit messages in English; product
interface text is Russian.

The API owns grades, matching, ranking, eligibility and authorization. Anonymous candidate projections and
company-specific disclosure are server boundaries, not fields to fetch and hide in the browser. Model output
does not decide test-confirmed grades. Fictional data must stay labelled.

## Validation

| Area | Command |
| --- | --- |
| Contract, mock and frontend | `just verify` |
| Browser behavior | `just e2e` |
| Rust API and services | `just db-up`, then `just backend-check` |
| Stand and release workflow | Checks in the [CI runbook](infra/ci/README.md) and [operations guide](infra/README.md) |

Select checks appropriate to the change. Behavior changes need relevant regression coverage and the affected
module's checks. Documentation-only changes need correct commands, working references and `git diff --check`.
Report failures, skipped checks and deployment results accurately; a successful local test does not establish
that a release is running on the stand.

## Secrets and infrastructure

Never commit credentials, tokens, session material or populated environment files. Private stand access and
local operational state belong in the ignored `.fsp-infra/`; service secrets belong in protected environment
files on the host. Follow the documented resource and privacy boundaries when collecting diagnostics.

Do not create or schedule backups, snapshots, off-host copies or redundant repository mirrors without an
explicit owner request. FSP backup units remain masked. Cleanup is limited to authorized project resources;
deleting an existing archive requires an explicit deletion request. Ordinary Git history and temporary build
or release activation state are operational state. See the [operations policy](infra/README.md#backups-and-recovery).
