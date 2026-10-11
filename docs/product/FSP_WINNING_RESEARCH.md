# FSP track research and winning product direction

Reviewed on 2026-10-10 against the organizer specification, the presentation template, the written Q&A,
the exported Telegram threads and images, the published meeting recording and transcript summary, public FSP
resources, and `main` at `8a352a9`.

## Executive decision

Keep the graph idea, but narrow and strengthen the claim. The winning proposition is not "we draw a skills
graph" and not "an FSP result proves every skill used in a competition". It is:

> The platform turns candidate evidence, assessment results and employer requirements into one auditable
> competence graph, then finds the smallest evidence gap between a person and a job.

This graph must be the decision model for matching, assessment assembly and the candidate's next action. If it
remains a role-to-tags visualization while ranking uses flat skill IDs, it is not a defensible differentiator.

The organizer's semantic center is employer-initiated contact, not graphs. The graph is our mechanism for making
that required reverse-hiring flow more precise, explainable and useful to both sides. It can therefore strengthen
the two largest criteria together: matching (35%) and testing, validation and categorization (25%).

## Source hierarchy

1. The organizer specification is authoritative for requirements and scoring.
2. The organizer's written Q&A resolves ambiguities in the specification.
3. Statements by the mentor and organizers in the recorded meeting clarify intent, but do not override the
   written specification or later written answers.
4. Telegram announcements define delivery details such as repository visibility, timing and mandatory slides.
5. Public FSP pages describe the real organization and available public evidence, but are not an API contract.
6. The AI-produced meeting summary is a navigation aid, not an independent authority.

## What the organizer actually requires

### Product core

- A standalone IT hiring platform, not a scraper or a showcase of vacancies from other platforms.
- Candidate and employer workspaces.
- Mandatory candidate onboarding: questionnaire, candidate-selected target grade and a test for that grade.
- A category visible to employers based on specialization plus a test-confirmed grade.
- Employer search over categorized candidates, explainable ranking and an employer-initiated invitation with a
  salary range.
- Candidate contacts remain hidden until the candidate accepts an invitation or applies.
- FSP history enriches and ranks a profile inside a category; it does not replace the grade test.
- A correct path for candidates with no FSP history.
- A team-defined validation procedure and reported results because no official dataset or gold labels are supplied.

The organizer calls employer-initiated contact the semantic center and main evaluation criterion. The score weights
are 35% for the new matching mechanic, 25% for testing/validation/categorization, 10% UI, 7% technical work,
6% completeness/documentation, 7% optional functionality and 10% the final pitch.

### Why test generation dominates the Q&A

The requirement is not "use an LLM to generate tests". The requirement is to solve two opposing risks:

- a common fixed test leaks;
- unconstrained per-person LLM generation produces incomparable difficulty and unreliable grades.

The written Q&A clarifies that the primary assessment is category-level at registration, with different tasks for
different candidates. Vacancy-specific assignments are a separate optional recurring-employer-task feature.
The team must demonstrate test-retest stability and the ability to separate stronger from weaker candidates;
synthetic candidates are allowed.

The meeting recording confirms the same interpretation:

- `04:19-04:47`: the team chooses the test content and format; a code sandbox is not mandatory, but the assessment
  mechanic and objectivity matter;
- `04:54-06:01`: the primary test assigns a category/grade and is not vacancy-specific; employer assignments are a
  separate optional mechanic;
- `06:12-06:50`: an LLM may generate unique items, but generation does not by itself establish equal difficulty or
  low error rate;
- `07:49-08:23`: resistance to candidates sharing identical items is required, while proctoring and plagiarism
  protection are optional advantages for the MVP;
- `08:23-09:05`: judges may test repeatability and separation between grades using synthetic profiles;
- `09:07-09:36`: an unconfirmed candidate may remain visible with an unconfirmed status and a lower rank.

Therefore LLM test generation is not the requirement. Comparable alternate forms, defensible scoring and evidence
that the mechanic separates levels are the requirement. A controlled item bank and graph-based blueprint are the
safe core; an LLM can propose new items before deterministic checks and review.

### Explicitly optional or future-facing

- Published vacancies and candidate applications.
- Recurring employer tasks.
- Anti-cheating, proctoring and plagiarism checks.
- Mobile layout.
- Fraudulent-employer protection and ATS integration as a concept.
- Keycloak/FSP ID compatibility. Real FSP credentials, API and schema are not supplied.

### Delivery facts from the organizer chat

- The repository must be public for judging.
- Slides 7 through 11 of the supplied deck are mandatory and must retain the template's design and structure.
- The final format is five minutes of presentation plus five minutes of questions within a ten-minute slot.
- Infrastructure is not provided by the organizer.
- Integrations for which access is not supplied must be mocked.
- The FSP brand book may be used.

## What public FSP data can and cannot prove

The public FSP site exposes a calendar, discipline descriptions, regulations and result protocols, commonly as
documents rather than a stable public participant API. The official disciplines currently presented are product,
algorithmic, information-security, robotics and unmanned-aircraft programming. Official competition results can
prove participation, placement, score, discipline, date and sometimes team membership.

They do not automatically prove an atomic individual competence:

- a product competition result can be collective, with no reliable attribution of who implemented what;
- a discipline label is much broader than a job skill;
- a place depends on field strength, format and year;
- without task statements, rubrics or solutions, a skill mapping is an inference rather than a fact.

Therefore an FSP result should enter the system as signed evidence with provenance and scope. It may increase rank
inside the confirmed grade, exactly as the organizer requested. It should raise confidence in a skill only when the
competition or task has an auditable skill mapping, and should not independently assign the platform grade.

## The graph model worth defending

### Layer 1: canonical competence DAG

A competence node is an assessable claim, not a technology label. Examples are "select the correct PostgreSQL
isolation level for an anomaly" or "design an idempotent HTTP write" rather than only "PostgreSQL" or "backend".
Each node has a stable ID, definition, specialization, level, and evidence rule.

The only initial semantic edge should be `prerequisite`. Keep it directed and acyclic. Store prerequisite groups
explicitly:

- `allOf`: every prerequisite is needed;
- `anyOf`: one of several alternative paths is sufficient.

Knowing two of three `allOf` prerequisites does not mean 67% mastery of the dependent skill. It means the dependent
claim is not yet justified, and the missing prerequisite is the next actionable gap. This is the key advantage over
subtracting two flat sets.

### Layer 2: evidence ledger

Evidence is not another competence. It is a separately typed record linked to competence nodes:

- platform assessment response;
- individual FSP result;
- team FSP result;
- GitHub repository fact;
- recurring employer task;
- self-declared profile fact.

Every record carries source, immutable reference, timestamp, individual/team scope, mapping method, reviewer or
model version, and confidence category. Use calibrated numbers only after collecting calibration data; before that,
use honest states such as `confirmed`, `supported`, `observed`, `self-declared` and `unknown`.

### Layer 3: vacancy requirement graph

The employer describes a vacancy in text. Bifrost proposes canonical competence nodes, minimum levels,
criticality and prerequisite relations. The employer reviews the draft and chooses acceptable evidence policies,
including specific competitions or a reviewed system catalog. The LLM proposes; deterministic validation and the
employer decide.

### Graph difference

For each required vacancy node, compute one of:

- `meets`: accepted evidence confirms the required level;
- `verify`: relevant evidence exists but does not meet the employer's confirmation rule;
- `blocked`: a prerequisite is missing;
- `missing`: neither the node nor an actionable prerequisite is evidenced.

Prioritize the gap plan by vacancy criticality, prerequisite reach, evidence uncertainty and the cost of the next
verification. The result is not a scalar percentage alone. It is a shortest defensible path to eligibility.

### Role of the LLM

Bifrost should:

- extract a draft vacancy graph from unstructured text;
- extract a draft competition-to-competence mapping from task descriptions and rubrics;
- extract repository facts and propose their mappings;
- generate or rewrite candidate assessment items for a selected node;
- explain deterministic match results.

Every extracted node and edge must cite an exact input fragment, resolve to the versioned ontology, pass cycle and
reference validation, and remain a suggestion until approved. The LLM must never confirm a grade, invent an FSP
result or silently promote evidence.

## Graph-driven assessment mechanic

For each category and grade, define an assessment blueprint as a required subgraph. Each item has one primary
competence node, difficulty band, format, source/reviewer and immutable answer key. Assemble variants under hard
constraints:

- identical node coverage and difficulty-band distribution;
- anchor items or equivalent calibrated groups across forms;
- bounded item exposure;
- no item repeated within a candidate's cooldown window;
- deterministic scoring and immutable attempt snapshots.

For the hackathon, do not claim production computerized adaptive testing without response data. Demonstrate a
constrained multi-form assembler and a synthetic validation harness instead. Report, at minimum:

- test-retest grade agreement across alternate forms;
- score variance for the same synthetic ability profile;
- grade separation for weak, borderline and strong profiles;
- item exposure and pairwise form overlap;
- coverage of every required graph node.

This directly answers the organizer's leakage-versus-comparability challenge while making the graph operational.

## Current `main` audit

### Strong and defensible today

- The required reverse-hiring journey is implemented end to end: onboarding, assessment, category, employer search,
  direct invitation, salary and controlled contact disclosure.
- Candidate and employer authorization and privacy boundaries are implemented server-side.
- The Rust backend follows a clear port/adapter split, persists in PostgreSQL and has a generated OpenAPI contract.
- Email delivery, PDF extraction and configured Bifrost paths are real; the FSP provider is a separate and clearly
  labelled fictional HTTP service, which is appropriate because the organizer supplies no API.
- Vacancy/application and recurring-task flows cover optional points.
- `main` at `8a352a9` has successful platform, backend, compose, release-safety and live deployment statuses.

### Graph claim: incomplete

- Candidate graphs are primarily `role -> skill -> evidence` projections.
- Need graphs are `vacancy -> skill` projections.
- The frontend renderer uses a fixed three-tier layout and does not encode prerequisite logic.
- Matching compares flat skill IDs, grade and evidence policy; it does not traverse or subtract graphs.
- The gap plan iterates flat unmet requirements and recommends from two hard-coded synthetic competitions.
- The need-graph endpoint has no frontend consumer.
- Bifrost may propose `requires` edges, but the input contains only existing broad skill facts. The validator checks
  references and supported relation names, not ontology membership, acyclicity, prerequisite semantics or edge truth.

The current graph is a useful evidence view, but it is not yet the product's algorithmic advantage.

### Assessment claim: materially incomplete

Each supported specialization/grade/topic has six questions and every form selects five. Any two forms for the same
scope therefore share at least eight of ten questions. The current experiment proves reproducibility and a 5+5 topic
balance, but explicitly does not establish test-retest reliability or grade separation. This is not strong resistance
to answer leakage and cannot support a winning claim for the 25% criterion.

### Matching claim: materially incomplete

The ranking experiment proves deterministic order for four constructed candidates. It does not measure relevance,
precision at the top, expert agreement or improvement over a flat baseline. The specification explicitly says the
judge may test the share of relevant candidates near the top on vacancy-candidate pairs. A deterministic sort is
necessary but not evidence of matching quality.

### Submission risk

No final presentation is tracked in this repository. The required five-minute story must foreground the matching
and assessment mechanics, not spend most of the pitch on implementation breadth or infrastructure.

## Winning demo in five minutes

1. Import a fictional but clearly labelled FSP profile. Show that an official team result is evidence, not an
   automatic individual skill or grade.
2. Show the candidate competence graph with provenance and one uncertain/missing prerequisite.
3. Complete a graph-balanced assessment variant. Show exactly which nodes became confirmed and why the grade changed.
4. Paste a vacancy description. Let Bifrost draft the requirement graph; the employer reviews criticality and
   accepted evidence sources.
5. Show the graph difference: confirmed nodes, evidence that still needs verification and one blocking prerequisite.
6. Open the ranked bank and explain why the top candidate is above the next one with evidence-backed factors.
7. Send a salary-bearing invitation, accept it as the candidate and reveal only selected contacts.

One sentence for the pitch:

> Vygotsky converts competition results, tests and work artifacts into an auditable competence graph, so an
> employer sees not keywords but proven fit, and a candidate sees the shortest path to the job.

## Highest-value work before judging

1. Make graph difference real in the domain model and matching path; do not start with a richer visualization.
2. Replace broad technology tags in one narrow backend track with 12-20 atomic competence nodes and reviewed
   prerequisite edges.
3. Map assessment items to those nodes and add a constrained form assembler with a materially larger item pool.
4. Add machine-readable experiments for alternate-form overlap, test-retest stability, grade separation and
   matching Precision@K/expert agreement. State expected effect and failure condition before each experiment.
5. Replace the hard-coded competition suggestions with a reviewed competition catalog whose mappings cite task or
   rubric text. Keep team evidence weaker than individual evidence.
6. Put the graph-difference journey into the demo script and presentation.

Do not spend the remaining time on more specializations, a general-purpose graph database, a GNN, scraping foreign
vacancy sites, a local large model, production IRT claims, or cosmetic graph animation. None of those repairs the
two highest-scoring evidence gaps.

## Primary materials

- `docs/product/sources/organizer-spec.pdf` — task, requirements and scoring.
- `/Users/dima/Downloads/fsp/Ответы на вопросы трек ФСП ЛЦТ__.xlsx` — written organizer answers.
- `/Users/dima/Downloads/fsp/ЛЦТ_2026_Шаблон презентации ФСП.pptx` — mandatory presentation structure.
- `/Users/dima/Downloads/fsp/lct.zip` — Telegram exports, screenshots, meeting summary and duplicated Q&A workbook.
- Google Drive file `11L9QI3apRb04DslW6hhass4SIksRUCYK` — 55:46 published meeting recording.
- Organizer-posted FSP brand book folder: `https://disk.yandex.ru/d/nxMN6oQTZ25wZQ`.
- Official FSP site, calendar, documents and athlete login: `https://fsp-russia.ru/`,
  `https://fsp-russia.ru/calendar/`, `https://fsp-russia.ru/about/documents/`,
  `https://lk.fsp-russia.ru/login`.
