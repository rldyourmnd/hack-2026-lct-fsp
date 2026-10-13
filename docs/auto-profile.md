# Automatic profile and estimated category

Decided with the owner on 2026-10-10: a candidate fills in nothing. Connecting GitHub and/or the FSP account builds
the profile, and an estimate from that evidence gives the employer-visible category at once. A knowledge test is
optional and confirms the category; question banks cover every specialization and grade. The LLM (Grok through
Bifrost) assists. This document is the plan of record; [business rules](product/BUSINESS_RULES.md) change with each
delivered phase.

## Relation to the organizer's specification

The specification derives the category from «опрос по отрасли и специализации, самостоятельный выбор
предполагаемого грейда и тестирование на заявленный грейд» and ranks «по силе подтверждённого профиля»
(§2, p1; §2.1, p4). It also values automatic enrichment from FSP results and ML for categorization (§1; §3.2, p4–5).
The owner chose to let an evidence-based estimate assign the category as well. To stay inside the spirit of the
specification:

- the questionnaire still exists, answered by the system from evidence and visible to the candidate; any answer
  the candidate edits is kept and never overwritten;
- every category states its source: «подтверждено тестом» or «оценка по открытым данным» with its confidence;
- within a category, test-confirmed candidates rank above estimated ones, then by estimate confidence;
- a test result always replaces the estimate, and the estimate never lowers a confirmed grade.

## Catalog v2

One data file, `backend/crates/domain/data/catalog.json`, is the single source for Rust, the contract generator and
the mock; the frontend receives labels through the generated types and `/meta`.

- **Grades:** стажёр (intern), джуниор, мидл, сеньор, лид (lead), ordered.
- **Specializations (25):** backend, frontend, fullstack, iOS, Android, cross-platform mobile, DevOps/SRE, data
  engineering, data science and ML, ML engineering/MLOps, manual QA, test automation, information security,
  embedded and IoT, systems programming, game development, 1C development, ERP/SAP, system analysis, business
  analysis, data analytics/BI, database administration, system and network administration, UI/UX design, product
  and project management. Each names two test topics and the skills that indicate it.
- **Skills (255):** languages (including the 1C language, ABAP, Delphi, COBOL, Verilog/VHDL), frameworks,
  databases, infrastructure, data/ML, mobile, QA, security, embedded, game engines, the 1C ecosystem (1С:Предприятие,
  БСП, СКД, query language, ERP/УТ/ЗУП/Бухгалтерия, обмены, EDT, 1С-Битрикс), ERP/SAP, analytics/BI, analysis
  notations, design tools and management practices. Each has a category and the specializations it indicates.
- GitHub detection maps its technologies and languages (including `.bsl`/`.os` 1C sources and 1C configuration
  dumps) to catalog skills, so evidence covers the whole catalog.

## Estimate (`estimate-v6`)

A pure domain function turns the GitHub report, FSP results and the stated experience into an estimate:

- **Specialization:** shares from the candidate's own changed files by area and from the stack each specialization
  indicates; fullstack when backend and frontend both carry substantial shares. When the top two are close, the
  LLM review may choose between them. The candidate may pick any of the leading specializations shown; the
  category then uses that choice. Outside them the estimate's leader stands.
- **Seniority score (0–100) → grade.** The factors follow the axes engineering level frameworks rank by: scope and
  complexity, tenure, ownership, technical quality, influence and impact
  ([levels.fyi framework](https://www.levels.fyi/blog/swe-level-framework.html),
  [Monzo progression framework](https://monzo.com/documents/engineering-progression-framework-v4-0.pdf),
  [Staff Engineer archetypes](https://lethain.com/static/blog/staffeng/staffeng-2020-12-16.pdf)). Weights of the
  GitHub dimensions: scope and complexity 20, tenure 15, own projects 15, engineering practices 15, collaboration 15,
  recognition 10, engagement 5, regularity 5. Volume and rhythm together stay at a tenth: they show sustained
  practice, not level. FSP results add up to 15 (doubled when they are the only evidence). Bands: < 20 intern,
  20–39 junior, 40–59 middle, 60–79 senior, ≥ 80 lead.
- **Scope and complexity** (GitHub dimension `scope`, `github-insights-v3`): own projects developed for half a year or
  longer, engineering areas with at least a 15% share, system layers used in two or more projects, and projects with
  architecture decisions, API contracts or schema migrations.
- **Tenure.** Senior needs 24 months of experience and lead 48, counted in months. Public months run from the first
  analyzed commit or the first year with ten active days. Stated experience never lowers the evidence: it may stand
  in for the tenure dimension when it is longer than the public record (full at six years) and lifts the caps, but a
  grade that only stated experience unlocks keeps the confidence at medium until a test. Without GitHub, stated
  experience alone gives up to 30 points. (`estimate-v2` blended stated experience into the score, so five months
  typed for five years cut a strong profile by about twenty points.)
- **Lead means leading people.** The catalog defines лид as «ведёт направление и людей, определяет технические
  решения». Scope shows the direction; working with people needs confirmed public collaboration (the collaboration
  dimension at 50 or more, where the report calls it confirmed) or a leading team role the candidate states
  (тимлид, руководитель, архитектор, стафф-инженер, принципал, главный…, head, principal, staff…; `estimate-v5` added
  the Russian spellings «стафф», «принципал» and «главный» after «стафф-инженер» was not recognized). Otherwise a lead-band score gives сеньор. A lead
  grade that only the stated role unlocks keeps the confidence at medium.
- **Notes** say in plain words why the grade is what it is: which cap applied and what removes it, that tenure came
  from the questionnaire, that lead needs work with people, that the GitHub report is from an earlier analysis
  version, and how many points the next band needs with the two factors that have the most points left.
- **Confidence** follows the amount and coverage of evidence; with too little public data the estimate says so and
  gives no category until a test or more evidence. An outdated GitHub report lowers it one level.
- **Profile answers:** industry IT, specialization, target grade = estimated grade, stack (top evidenced catalog
  skills), experience text, team role from ownership and
  collaboration, soft skills with public evidence behind them (documentation, descriptive commits, led projects,
  several related projects, reviews, accepted outside contributions, issues, team competitions), and the name from
  the public GitHub profile when the candidate has none.
- Every factor carries points and an explanation shown to the candidate; employers see the grade, confidence and
  the source.

### How to raise the grade (`estimate-v4`)

Every estimate carries `growth`: the conditions that hold the grade back (tenure, work with people), then the GitHub
factors with the most points left as concrete actions with the candidate's own numbers (for example, the shares of
projects without tests or linters, approved pull requests and independent reviews), FSP results and, last, the
optional test. `focusSkills` lists up to five catalog skills of the specialization the evidence does not show —
general ones first, then frameworks of languages the candidate already uses, never another language. The review adds
two to five personal recommendations. The UI shows them as «Как повысить грейд» and in «Следующий шаг» instead of
pushing a test. FSP results together add at most 30 to the specialization scores, so a long competition history never
outweighs the work.

### What public evidence cannot show

No study validates a GitHub-to-seniority mapping: research predicts long-time contributors or identifies library
experts with modest precision ([Montandon et al., MSR 2019](https://ar5iv.arxiv.org/html/1903.08113);
[Bao et al., TSE](https://xin-xia.github.io/publication/tse191.pdf)). Staff-level work — direction across teams,
mentoring, organizational influence — rarely leaves public traces, and private repositories are invisible without
the account owner's authorization (GitHub OAuth is deferred; the profile page's calendar is not scraped). The
estimate therefore states its limits and stays an estimate: the knowledge test for the target grade confirms the
category, and confirmed candidates rank first.

### Answers the candidate owns

Experience is the candidate's own answer, never filled from evidence (`estimate-v6`): public GitHub activity shows a
lower bound of tenure, not how long someone has studied and worked, so it stays evidence for the caps and the notes
(«публично видно 1 год 9 мес.»). The questionnaire asks «В IT с какого года» (`experienceSince`): study, own projects
and work, private repositories included. Experience counts whole years from that year up to the current one, so it
stays correct as time passes; months typed or imported from a résumé (`experienceMonths`) still work and are replaced
by a start year. Earlier estimates that wrote public tenure into the answer are cleared on the next estimate. The
estimate card asks only the two answers evidence cannot give — the start year and the role — in «Уточните два
ответа». Editing either recomputes the estimate at once. Opening the profile refreshes an estimate of earlier rules
(or a missing one with finished sources); «Пересчитать» also re-syncs FSP sources and re-analyzes a GitHub report from
an earlier analysis version (the job applies the new estimate when it ends). «Заполнить анкету по источникам»
(`POST /me/profile/estimate?answers=replace`) hands every evidence-based answer but an edited name back to the
evidence; the stated experience stays. A résumé PDF fills it too: Grok extracts the start year in IT with the
page excerpt that names it, and the candidate confirms the preview.

### LLM assistance

Bifrost (Grok) receives the anonymized evidence — the portrait, engineering areas, technologies by layer, led
projects (descriptions without the login, names or organizations), every report metric, factors and notes, public
and stated tenure, FSP results — and grades it with a decomposed rubric (scope, complexity, ownership, technical
quality, influence, tenure) anchored per grade, as recommended for LLM judges
([calibrating LLM evaluators](https://kinde.com/learn/ai-for-software-engineering/best-practice/llm-as-a-judge-done-right-calibrating-guarding-debiasing-your-evaluators/)).
It returns, validated against the catalog, a second opinion on specialization and grade with a rationale naming the
deciding axes and the missing evidence, growth recommendations, and the experience and team-role texts. The review
runs in the background with `reasoning_effort` set by `BIFROST_REASONING_EFFORT` (default `medium`;
see [measured latency and accuracy limits](ai-latency.md)), a 16k-token budget and up to 330
seconds under Bifrost's 360-second provider timeout. One review runs per candidate at a time; a newer estimate is
reviewed once after it. Each queued review rechecks processing consent before requesting the model; the
transaction also discards a response if consent was revoked or the estimate changed while it ran. Already sent
provider requests cannot be recalled. Interactive Grok calls explicitly request low reasoning and keep the 45-second ceiling. The category stays deterministic:
the LLM may only choose between close specializations, and a disagreement on the grade lowers high confidence to
medium (never below, so a review never removes the category) and is shown. Without Bifrost the rule-based estimate and template texts work alone.

## Category and search

- Effective category = confirmed (test) when present, otherwise the estimate with at least medium confidence.
- The bank, filters and ranking use the effective category; a «Подтверждение» filter separates test-confirmed and
  estimated categories. Skill-level confirmation still needs a test; GitHub and FSP give evidence.
- Visibility requires a verified e-mail, both consents and an effective category.

## Question banks

Every specialization has two topics and every grade six questions per topic, as the backend bank does: balanced
5+5 variants per attempt. Delivered as topic banks (`knowledge-v3-*.json`, 1,230 questions on 39 topics) shared by
the specializations tested on each topic; fullstack is tested on HTTP/REST API and SQL. Intern and lead extend the
existing junior–senior structure for Rust, PostgreSQL, TypeScript and React.

## Delivery phases

1. Catalog v2: data file, generated contract enums, Rust enums, labels, GitHub skill mapping, the mock image fix.
   Delivered in PR #73.
2. Estimate, automatic profile, estimated category, LLM review, onboarding and employer UI. Delivered on
   `feat/auto-profile`: `ProfileEstimate`, `autoFields` and `category.source/confidence` in the profile; the
   anonymous candidate carries the category, its source and confidence and the score with factor points;
   `POST /me/profile/estimate` recomputes; the bank filters by `categorySource`; applications and weekly tasks need a
   category (`category` reason); migration 0006 adds the estimate and auto-field columns. The source transaction
   applies the rules at once; the LLM review runs in the background after commit (live Grok answers took over 20
   seconds) and joins the estimate only if it is unchanged. The mock serves Rust-generated golden estimates.
3. Question banks for every specialization and grade. Delivered on `feat/knowledge-banks`.
4. Estimate v3 (`feat/estimate-v3`): the scope dimension and level-framework weights, stated experience that never
   lowers evidence, notes, soft skills, rebuilding answers from sources, re-estimating on edited experience,
   re-analysis of outdated reports and the rubric review.
5. Estimate v4 (`feat/growth-plan`): the growth plan, focus skills and review advice with deep reasoning, automatic
   refresh of outdated estimates, FSP re-sync on recalculation, the short refine form, one-click rebuild and the
   deterministic competence graph.
6. Estimates v5–v6 (#125, #128, #131, #132): Russian spellings of leading roles, experience from the year the
   candidate started in IT (never from public tenure), the start year from a résumé through Grok with whole-catalog
   grounding of résumé values, refine answers kept during background updates, and wrapped graph columns.

Each phase goes through the usual gates and deployment; business rules, traceability and screenshots change with it.
