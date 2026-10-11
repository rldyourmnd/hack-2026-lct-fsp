//! The automatic profile: specialization, grade and questionnaire answers estimated from evidence.
//!
//! Rules are deterministic and documented in `docs/auto-profile.md`. Every factor keeps its points
//! and an explanation, so the candidate and the employer see why an estimate came out as it did.
//! An LLM review may only choose between close specializations and lower the confidence.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    Archetype, FspResult, GitHubDimensionId, GitHubReport, Grade, SignalConfidence, Specialization,
};

pub const ESTIMATE_VERSION: &str = "estimate-v6";

/// Stack items an automatic profile fills in.
const STACK_LIMIT: usize = 12;
/// Soft skills an automatic profile fills in (the questionnaire allows twelve).
const SOFT_SKILL_LIMIT: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileEstimate {
    pub version: String,
    pub computed_at: String,
    /// `None` when the evidence names no specialization.
    pub specialization: Option<Specialization>,
    /// The leading specializations with their shares, highest first.
    pub specializations: Vec<SpecializationShare>,
    /// `None` when there is too little evidence for a grade.
    pub grade: Option<Grade>,
    /// Seniority score, 0–100.
    pub score: u8,
    pub confidence: SignalConfidence,
    pub factors: Vec<EstimateFactor>,
    /// Profile answers derived from the evidence.
    pub answers: EstimateAnswers,
    /// Where the evidence came from.
    pub sources: Vec<String>,
    /// Why there is no grade, when there is none.
    pub insufficient: Option<String>,
    /// How tenure and caps shaped the grade and what would raise it, in plain words.
    #[serde(default)]
    pub notes: Vec<String>,
    /// What raises the grade, most points first.
    #[serde(default)]
    pub growth: Vec<GrowthStep>,
    /// Catalog skills of the specialization that the evidence does not show yet.
    #[serde(default)]
    pub focus_skills: Vec<String>,
    pub review: Option<EstimateReview>,
}

/// One step towards a higher grade: what to do and how many points it can add.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrowthStep {
    pub id: String,
    pub title: String,
    pub action: String,
    /// Points the step can still add; zero for a condition such as tenure.
    pub points: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecializationShare {
    pub id: Specialization,
    pub percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimateFactor {
    pub id: String,
    pub label: String,
    pub points: u8,
    pub max: u8,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimateAnswers {
    pub stack: Vec<String>,
    pub experience_months: Option<u32>,
    pub experience: String,
    pub team_role: String,
    pub name: Option<String>,
    /// Working habits the evidence shows: documentation, reviews, ownership, teamwork.
    #[serde(default)]
    pub soft_skills: Vec<String>,
}

/// The LLM's second opinion, validated against the catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimateReview {
    pub specialization: Specialization,
    pub grade: Grade,
    pub rationale: String,
    /// Whether the review agreed with the rule-based grade.
    pub agrees: bool,
    /// The model's personal advice towards the next grade.
    #[serde(default)]
    pub recommendations: Vec<String>,
}

/// What the LLM returns before it is applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileReviewReply {
    pub specialization: Specialization,
    pub grade: Grade,
    pub rationale: String,
    pub experience: String,
    pub team_role: String,
    #[serde(default)]
    pub recommendations: Vec<String>,
}

/// Anonymized evidence for the LLM: no login, names, organizations or links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileReviewRequest {
    pub summary: String,
    pub specializations: Vec<SpecializationShare>,
    pub rule_grade: Option<Grade>,
    pub score: u8,
    pub factors: Vec<EstimateFactor>,
    pub notes: Vec<String>,
    pub stack: Vec<String>,
    /// Months of public work the analysis sees.
    pub public_experience_months: Option<u32>,
    /// Months the candidate stated; not verified.
    pub stated_experience_months: Option<u32>,
    /// The team role the candidate stated; not verified.
    pub stated_team_role: Option<String>,
    pub fsp_results: Vec<String>,
    /// Engineering areas with their shares and evidence.
    pub areas: Vec<String>,
    /// Detected technologies by system layer.
    pub technologies: Vec<String>,
    /// The main projects without names or links.
    pub projects: Vec<ReviewProject>,
    /// Every report metric as «label: value», by dimension.
    pub metrics: Vec<String>,
    /// The rule-based steps towards the next grade.
    pub growth: Vec<GrowthStep>,
    /// Labels of the specialization's skills the evidence does not show yet.
    pub focus_skills: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewProject {
    pub description: String,
    pub language: Option<String>,
    pub stars: u32,
    pub own_commits: u32,
    pub own_share_percent: u8,
    pub technologies: Vec<String>,
}

pub struct EstimateInput<'a> {
    pub github: Option<&'a GitHubReport>,
    pub fsp: &'a [FspResult],
    /// Experience the candidate stated or imported from a résumé, in months.
    pub stated_experience_months: Option<u32>,
    /// The team role the candidate typed, not one an estimate filled in.
    pub stated_team_role: Option<&'a str>,
    pub now: &'a str,
}

/// Estimates the profile from the evidence; deterministic for the same input.
#[must_use]
pub fn estimate_profile(input: &EstimateInput<'_>) -> ProfileEstimate {
    let published = input
        .fsp
        .iter()
        .filter(|result| result.status == "published")
        .collect::<Vec<_>>();
    let mut sources = Vec::new();
    if input.github.is_some() {
        sources.push("github".to_owned());
    }
    if !published.is_empty() {
        sources.push("fsp".to_owned());
    }
    let specializations = specialization_shares(input.github, &published);
    let specialization = specializations.first().map(|share| share.id);
    let stated_months = input.stated_experience_months;
    let public_months = input.github.and_then(public_experience_months);
    let experience_months = match (stated_months, public_months) {
        (Some(stated), Some(public)) => Some(stated.max(public)),
        (stated, public) => stated.or(public),
    };

    let mut factors = Vec::new();
    let mut notes = Vec::new();
    let mut base = 0.0;
    if let Some(report) = input.github {
        for (id, label, weight) in DIMENSION_WEIGHTS {
            let mut score = dimension(report, id);
            let mut detail = report
                .dimensions
                .iter()
                .find(|item| item.id == id)
                .map_or_else(String::new, |item| item.summary.clone());
            // Stated experience may stand in for public tenure but never lowers it: private work
            // is invisible on GitHub, and a typo must not cost a strong profile its evidence.
            if id == GitHubDimensionId::Longevity
                && let Some(stated) = stated_months
            {
                let stated_score =
                    (ramp(f64::from(stated), 0.0, TENURE_FULL_MONTHS) * 100.0).round() as u8;
                if stated_score > score {
                    score = stated_score;
                    detail = format!(
                        "По анкете — {}; публично видно {}",
                        duration(stated),
                        public_months.map_or_else(|| "—".to_owned(), duration)
                    );
                }
            }
            let points = (f64::from(score) * weight).round();
            base += points;
            factors.push(EstimateFactor {
                id: format!("github_{}", dimension_key(id)),
                label: label.into(),
                points: points as u8,
                max: (100.0 * weight).round() as u8,
                detail,
            });
        }
        if report.algorithm_version != crate::GITHUB_INSIGHTS_VERSION {
            notes.push(
                "Отчёт GitHub построен прошлой версией анализа: нажмите «Пересчитать», анализ \
                 обновится, и оценка учтёт все факторы"
                    .to_owned(),
            );
        }
    } else if let Some(stated) = stated_months {
        // Without public evidence, stated experience is the only measure of work.
        let points = (ramp(f64::from(stated), 0.0, 96.0) * 30.0).round();
        base = points;
        factors.push(EstimateFactor {
            id: "stated_experience".into(),
            label: "Опыт по анкете или резюме".into(),
            points: points as u8,
            max: 30,
            detail: format!("{} опыта", duration(stated)),
        });
    }
    let fsp_points = fsp_points(&published);
    if !published.is_empty() {
        factors.push(EstimateFactor {
            id: "fsp".into(),
            label: "Результаты соревнований ФСП".into(),
            points: fsp_points,
            max: FSP_MAX,
            detail: fsp_detail(&published),
        });
    }
    // Competition results alone show skill but not work experience: they reach junior at most.
    let fsp_weight = if input.github.is_none() && stated_months.is_none() {
        2.0
    } else {
        1.0
    };
    let score = (base + f64::from(fsp_points) * fsp_weight)
        .round()
        .clamp(0.0, 100.0) as u8;

    let mut from_answers = false;
    // What held the grade back, for the growth plan: the band the tenure capped and the lead gate.
    let mut tenure_cap = None;
    let mut people_cap = false;
    let (grade, insufficient) = if input.github.is_none()
        && published.is_empty()
        && stated_months.is_none()
    {
        (
            None,
            Some("Подключите GitHub или ФСП, чтобы оценить уровень".into()),
        )
    } else if input.github.is_some_and(|report| {
        report.index.confidence == SignalConfidence::Low && report.coverage.commits_analyzed < 30
    }) && stated_months.is_none()
        && published.is_empty()
    {
        (
            None,
            Some("Публичных данных пока мало для оценки уровня".into()),
        )
    } else {
        let mut grade = band(score);
        // The catalog's lead «ведёт направление и людей»: scope shows the direction, and working
        // with people needs confirmed public collaboration or a leading role the candidate states.
        let mut people_note = None;
        let mut role_from_answers = false;
        if grade == Grade::Lead
            && let Some(report) = input.github
        {
            let collaboration = dimension(report, GitHubDimensionId::Collaboration);
            if collaboration < LEAD_COLLABORATION {
                if input.stated_team_role.is_some_and(leading_role) {
                    role_from_answers = true;
                    people_note = Some(format!(
                        "Руководящая роль взята из анкеты: публичных следов работы с людьми мало \
                         («Совместная работа» — {collaboration} из 100), поэтому уверенность оценки — \
                         не выше средней, пока уровень не подтверждён тестом"
                    ));
                } else {
                    grade = Grade::Senior;
                    people_cap = true;
                    people_note = Some(format!(
                        "Баллы соответствуют уровню «Лид», но он означает, что человек ведёт \
                         направление и людей, а публичных следов работы с людьми мало («Совместная \
                         работа» — {collaboration} из 100, нужно от {LEAD_COLLABORATION}). Если вы \
                         руководите людьми, укажите это в роли в команде или подтвердите уровень тестом"
                    ));
                }
            }
        }
        let months = experience_months.unwrap_or(0);
        let capped = cap_grade(grade, months);
        if capped != grade {
            tenure_cap = Some(grade);
            notes.push(format!(
                // Two and four years both read «от 2 лет», «от 4 лет».
                "Баллы соответствуют уровню «{}», но он требует от {} лет опыта{}; учтено {} {}, \
                 поэтому уровень — «{}». Укажите, с какого года вы в IT, или подтвердите уровень тестом",
                grade.label(),
                required_months(grade) / 12,
                if grade == Grade::Lead && capped == Grade::Middle {
                    ", а «Сеньор» — от 2"
                } else {
                    ""
                },
                duration(months),
                if stated_months.is_some_and(|stated| stated >= months) {
                    "по анкете"
                } else {
                    "по публичной активности"
                },
                capped.label()
            ));
        } else if input.github.is_some() && cap_grade(grade, public_months.unwrap_or(0)) != grade {
            // Only the stated experience lifts the cap: say so and keep confidence moderate.
            from_answers = true;
            notes.push(format!(
                "Стаж {} взят из анкеты (публично видно {}): пока уровень не подтверждён тестом, \
                 уверенность оценки — не выше средней",
                duration(months),
                public_months.map_or_else(|| "меньше месяца".to_owned(), duration)
            ));
        }
        // A stated leading role matters only when the grade stays lead.
        if role_from_answers && capped != Grade::Lead {
            people_note = None;
        }
        from_answers |= role_from_answers && capped == Grade::Lead;
        notes.extend(people_note);
        if input.github.is_some()
            && let Some(note) = next_band_note(score, &factors)
        {
            notes.push(note);
        }
        (Some(capped), None)
    };
    let mut confidence = confidence(input, &published, grade.is_some());
    if from_answers && confidence == SignalConfidence::High {
        confidence = SignalConfidence::Medium;
    }
    // Only a stated experience answers the questionnaire; public tenure stays evidence.
    let answers = answers(input.github, &published, specialization, stated_months);
    let growth = growth_plan(&GrowthInput {
        github: input.github,
        factors: &factors,
        fsp_points,
        published: published.len(),
        tenure_cap: tenure_cap.as_ref(),
        people_cap,
        grade,
        public_months,
    });
    let focus_skills = focus_skills(specialization, input.github, &answers.stack);
    ProfileEstimate {
        version: ESTIMATE_VERSION.into(),
        computed_at: input.now.into(),
        specialization,
        specializations,
        grade,
        score,
        confidence,
        factors,
        answers,
        sources,
        insufficient,
        notes,
        growth,
        focus_skills,
        review: None,
    }
}

/// What the next band needs and which factors have the most points left.
fn next_band_note(score: u8, factors: &[EstimateFactor]) -> Option<String> {
    let (next, threshold) = match band(score) {
        Grade::Intern => (Grade::Junior, 20),
        Grade::Junior => (Grade::Middle, 40),
        Grade::Middle => (Grade::Senior, 60),
        Grade::Senior => (Grade::Lead, 80),
        Grade::Lead => return None,
    };
    let mut open = factors
        .iter()
        .filter(|factor| factor.max > factor.points)
        .collect::<Vec<_>>();
    open.sort_by(|a, b| {
        (b.max - b.points)
            .cmp(&(a.max - a.points))
            .then(a.id.cmp(&b.id))
    });
    let open = open
        .iter()
        .take(2)
        .map(|factor| format!("«{}» (ещё до {})", factor.label, factor.max - factor.points))
        .collect::<Vec<_>>();
    if open.is_empty() {
        return None;
    }
    Some(format!(
        "До уровня «{}» не хватает {} {}; больше всего можно добавить в {}",
        next.label(),
        threshold - score,
        plural(u32::from(threshold - score), ("балла", "баллов", "баллов")),
        open.join(" и ")
    ))
}

/// Questionnaire fields an estimate may answer, by their contract names. Experience is never
/// among them: public activity shows a lower bound of tenure, not how long someone has worked.
pub const AUTO_FIELDS: [&str; 7] = [
    "name",
    "specialization",
    "selectedGrade",
    "stack",
    "experience",
    "teamRole",
    "softSkills",
];

/// The name a new account starts with; an estimate may replace it.
pub const DEFAULT_CANDIDATE_NAME: &str = "Новый кандидат";

impl crate::CandidateProfile {
    /// The team role the candidate typed, not the one an estimate filled in.
    #[must_use]
    pub fn stated_team_role(&self) -> Option<&str> {
        let role = self.team_role.trim();
        (!role.is_empty() && !self.auto_fields.iter().any(|field| field == "teamRole"))
            .then_some(role)
    }

    /// The experience the candidate stated in `year`: counted from the year they started in IT
    /// when given (it stays current as years pass), otherwise the typed or imported months. A
    /// value an earlier estimate filled in is not a statement.
    #[must_use]
    pub fn stated_experience_months(&self, year: i32) -> Option<u32> {
        if let Some(since) = self.experience_since {
            return Some(experience_since_months(since, year));
        }
        self.experience_months.filter(|_| {
            !self
                .auto_fields
                .iter()
                .any(|field| field == "experienceMonths")
        })
    }

    /// Hands every answer except the name and the experience back to the evidence, at the
    /// candidate's request: the next estimate rewrites them. A test-confirmed category still keeps
    /// its specialization and target grade.
    pub fn release_answers(&mut self) {
        let name = self.auto_fields.iter().any(|field| field == "name");
        self.auto_fields = AUTO_FIELDS
            .iter()
            .filter(|field| **field != "name" || name)
            .map(|field| (*field).to_owned())
            .collect();
    }

    /// Answers every released field from the estimate; a field the estimate cannot answer goes
    /// back to the candidate.
    pub fn rebuild_from_estimate(&mut self, estimate: ProfileEstimate) {
        let filled = self.apply_estimate(estimate);
        self.auto_fields
            .retain(|field| filled.iter().any(|item| item == field));
    }

    /// Stores the estimate and answers the questionnaire fields the candidate has not answered:
    /// empty ones, ones an earlier estimate filled, and the specialization and target grade while
    /// the questionnaire is incomplete. A field the candidate edits is never overwritten, and a
    /// test-confirmed category keeps its specialization and target grade. Returns the fields
    /// this estimate answered.
    pub fn apply_estimate(&mut self, estimate: ProfileEstimate) -> Vec<&'static str> {
        let incomplete = !self.questionnaire_complete();
        let confirmed = self.verified_grade.is_some();
        let auto =
            |profile: &Self, field: &str| profile.auto_fields.iter().any(|item| item == field);
        let mut filled = Vec::new();
        let answers = &estimate.answers;
        if let Some(name) = answers.name.as_ref().filter(|name| !name.trim().is_empty())
            && (auto(self, "name")
                || self.name.trim().is_empty()
                || self.name == DEFAULT_CANDIDATE_NAME)
        {
            self.name = name.trim().chars().take(120).collect();
            filled.push("name");
        }
        if let Some(specialization) = estimate.specialization
            && !confirmed
            && (auto(self, "specialization") || incomplete)
        {
            self.specialization = specialization;
            filled.push("specialization");
        }
        if let Some(grade) = estimate.grade
            && !confirmed
            && (auto(self, "selectedGrade") || incomplete)
        {
            self.selected_grade = grade;
            filled.push("selectedGrade");
        }
        if !answers.stack.is_empty() && (auto(self, "stack") || self.stack.is_empty()) {
            self.stack = answers.stack.clone();
            filled.push("stack");
        }
        // Earlier estimates wrote public tenure into the answer; it was never the candidate's.
        if auto(self, "experienceMonths") {
            self.experience_months = None;
            self.auto_fields.retain(|field| field != "experienceMonths");
        }
        if !answers.experience.trim().is_empty()
            && (auto(self, "experience") || self.experience.trim().is_empty())
        {
            self.experience = answers.experience.chars().take(4000).collect();
            filled.push("experience");
        }
        if !answers.team_role.trim().is_empty()
            && (auto(self, "teamRole") || self.team_role.trim().is_empty())
        {
            self.team_role = answers.team_role.chars().take(120).collect();
            filled.push("teamRole");
        }
        if !answers.soft_skills.is_empty()
            && (auto(self, "softSkills") || self.soft_skills.is_empty())
        {
            self.soft_skills = answers
                .soft_skills
                .iter()
                .take(SOFT_SKILL_LIMIT)
                .map(|skill| skill.chars().take(100).collect())
                .collect();
            filled.push("softSkills");
        }
        for field in &filled {
            if !auto(self, field) {
                self.auto_fields.push((*field).to_owned());
            }
        }
        self.estimate = Some(estimate);
        filled
    }
}

/// Applies a validated LLM review: it may pick between the two leading specializations when they
/// are within ten points, take over the texts, and a disagreement on the grade lowers high
/// confidence to medium, never below.
pub fn apply_review(estimate: &mut ProfileEstimate, reply: ProfileReviewReply) {
    if let [first, second, ..] = estimate.specializations.as_slice()
        && first.percent.saturating_sub(second.percent) <= 10
        && (reply.specialization == first.id || reply.specialization == second.id)
    {
        estimate.specialization = Some(reply.specialization);
    }
    let agrees = estimate.grade == Some(reply.grade);
    // A second opinion may temper confidence but never takes the category away: the rules own
    // the grade, so a disagreement lowers high to medium and stops there.
    if !agrees && estimate.confidence == SignalConfidence::High {
        estimate.confidence = SignalConfidence::Medium;
    }
    if let Some(experience) = clean_text(&reply.experience, 1200) {
        estimate.answers.experience = experience;
    }
    if let Some(team_role) = clean_text(&reply.team_role, 120) {
        estimate.answers.team_role = team_role;
    }
    estimate.review = Some(EstimateReview {
        specialization: reply.specialization,
        grade: reply.grade,
        rationale: clean_text(&reply.rationale, 600).unwrap_or_default(),
        agrees,
        recommendations: reply
            .recommendations
            .iter()
            .filter_map(|item| clean_text(item, 240))
            .take(5)
            .collect(),
    });
}

/// Model text for a profile: trimmed, bounded, single-spaced and free of links and addresses.
fn clean_text(value: &str, limit: usize) -> Option<String> {
    let text = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = text.to_lowercase();
    if text.is_empty()
        || ["://", "www.", "@", "github.com"]
            .iter()
            .any(|item| lower.contains(item))
    {
        return None;
    }
    Some(text.chars().take(limit).collect())
}

/// The anonymized evidence the LLM reviews.
#[must_use]
pub fn review_request(
    estimate: &ProfileEstimate,
    github: Option<&GitHubReport>,
    fsp: &[FspResult],
    stated_experience_months: Option<u32>,
    stated_team_role: Option<&str>,
) -> ProfileReviewRequest {
    let names = github.map(private_names).unwrap_or_default();
    let anonymous = |text: &str, limit: usize| anonymize(text, &names, limit);
    ProfileReviewRequest {
        summary: github
            .and_then(|report| anonymous(&report.profile.public_summary, 1200))
            .unwrap_or_default(),
        specializations: estimate.specializations.clone(),
        rule_grade: estimate.grade,
        score: estimate.score,
        factors: estimate.factors.clone(),
        notes: estimate.notes.clone(),
        growth: estimate.growth.clone(),
        focus_skills: estimate
            .focus_skills
            .iter()
            .map(|skill| crate::catalog::skill_label(skill).to_owned())
            .collect(),
        stack: estimate
            .answers
            .stack
            .iter()
            .map(|skill| crate::catalog::skill_label(skill).to_owned())
            .collect(),
        public_experience_months: github.and_then(public_experience_months),
        stated_experience_months,
        stated_team_role: stated_team_role.and_then(|role| anonymous(role, 120)),
        fsp_results: fsp
            .iter()
            .filter(|result| result.status == "published")
            .map(|result| {
                format!(
                    "{}: {} место из {}",
                    result.discipline,
                    result
                        .place
                        .map_or_else(|| "—".to_owned(), |place| place.to_string()),
                    result.field_size
                )
            })
            .collect(),
        areas: github.map_or_else(Vec::new, |report| {
            report
                .profile
                .archetypes
                .iter()
                .filter_map(|area| {
                    anonymous(
                        &format!(
                            "{} — {}%: {}",
                            area.label,
                            area.percent,
                            area.evidence.join("; ")
                        ),
                        300,
                    )
                })
                .collect()
        }),
        technologies: github.map_or_else(Vec::new, technology_layers),
        projects: github.map_or_else(Vec::new, |report| {
            report
                .repositories
                .iter()
                .filter(|repository| repository.led && !repository.fork)
                .take(8)
                .map(|repository| ReviewProject {
                    description: repository
                        .description
                        .as_deref()
                        .and_then(|text| anonymous(text, 200))
                        .unwrap_or_default(),
                    language: repository.primary_language.clone(),
                    stars: repository.stars,
                    own_commits: repository.user_commits,
                    own_share_percent: repository.user_share_percent,
                    technologies: repository.stack.iter().take(8).cloned().collect(),
                })
                .collect()
        }),
        metrics: github.map_or_else(Vec::new, |report| {
            report
                .dimensions
                .iter()
                .flat_map(|dimension| {
                    dimension.metrics.iter().map(move |metric| {
                        format!("{} / {}: {}", dimension.label, metric.label, metric.display)
                    })
                })
                .collect()
        }),
    }
}

/// The login, display name and organizations a review must not see.
fn private_names(report: &GitHubReport) -> Vec<String> {
    let mut names = vec![report.login.clone()];
    names.extend(report.display_name.iter().flat_map(|name| {
        name.split_whitespace()
            .filter(|part| part.chars().count() >= 3)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    }));
    names.extend(
        report
            .organizations
            .memberships
            .iter()
            .map(|organization| organization.login.clone()),
    );
    names.extend(
        report
            .organizations
            .contributed
            .iter()
            .map(|organization| organization.login.clone()),
    );
    names.extend(report.repositories.iter().flat_map(|repository| {
        repository
            .name_with_owner
            .split('/')
            .map(str::to_owned)
            .collect::<Vec<_>>()
    }));
    names.retain(|name| name.chars().count() >= 3);
    names.sort_by_key(|name| std::cmp::Reverse(name.chars().count()));
    names.dedup();
    names
}

/// Text without the private names (replaced by «…»), links and addresses; `None` when empty.
fn anonymize(text: &str, names: &[String], limit: usize) -> Option<String> {
    let mut text = text.to_owned();
    for name in names {
        text = replace_ignoring_case(&text, name, "…");
    }
    clean_text(&text, limit)
}

/// Replaces whole-word occurrences of `needle`, ignoring case: «api» stays inside «rapid».
fn replace_ignoring_case(text: &str, needle: &str, replacement: &str) -> String {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let lower = text.to_lowercase();
    let needle = needle.to_lowercase();
    if needle.is_empty() || lower.len() != text.len() {
        // Case folding changed byte lengths; leave the text to the link filter.
        return text.to_owned();
    }
    let mut result = String::with_capacity(text.len());
    let mut position = 0;
    let mut from = 0;
    while let Some(found) = lower[from..].find(&needle) {
        let start = from + found;
        let end = start + needle.len();
        let before = lower[..start].chars().next_back();
        let after = lower[end..].chars().next();
        if before.is_some_and(word) || after.is_some_and(word) {
            from = start + lower[start..].chars().next().map_or(1, char::len_utf8);
            continue;
        }
        result.push_str(&text[position..start]);
        result.push_str(replacement);
        position = end;
        from = end;
    }
    result.push_str(&text[position..]);
    result
}

/// Detected technologies grouped by system layer, most used first.
fn technology_layers(report: &GitHubReport) -> Vec<String> {
    let mut layers: BTreeMap<crate::StackCategory, Vec<&crate::GitHubStackItem>> = BTreeMap::new();
    for item in &report.stack {
        layers.entry(item.category).or_default().push(item);
    }
    layers
        .into_iter()
        .map(|(category, mut items)| {
            items.sort_by(|a, b| {
                b.repositories
                    .cmp(&a.repositories)
                    .then(a.name.cmp(&b.name))
            });
            format!(
                "{}: {}",
                layer_label(category),
                items
                    .iter()
                    .take(12)
                    .map(|item| format!("{} ({})", item.name, item.repositories))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect()
}

fn layer_label(category: crate::StackCategory) -> &'static str {
    match category {
        crate::StackCategory::Language => "Языки",
        crate::StackCategory::Backend => "Бэкенд",
        crate::StackCategory::Frontend => "Фронтенд",
        crate::StackCategory::Data => "Данные",
        crate::StackCategory::Infrastructure => "Инфраструктура",
        crate::StackCategory::Quality => "Качество",
        crate::StackCategory::Ai => "ИИ",
        crate::StackCategory::Mobile => "Мобильная разработка",
        crate::StackCategory::Tooling => "Инструменты",
    }
}

/// Seniority weights of the GitHub dimensions after the axes of engineering level frameworks:
/// scope and complexity, tenure, ownership, technical quality, influence and impact. Volume and
/// rhythm together stay at a tenth: they show sustained practice, not level.
const DIMENSION_WEIGHTS: [(GitHubDimensionId, &str, f64); 8] = [
    (GitHubDimensionId::Scope, "Масштаб и сложность", 0.20),
    (GitHubDimensionId::Longevity, "Стаж", 0.15),
    (GitHubDimensionId::Ownership, "Собственные проекты", 0.15),
    (GitHubDimensionId::Craft, "Инженерная культура", 0.15),
    (GitHubDimensionId::Collaboration, "Совместная работа", 0.15),
    (GitHubDimensionId::Impact, "Признание", 0.10),
    (GitHubDimensionId::Engagement, "Вовлечённость", 0.05),
    (GitHubDimensionId::Consistency, "Регулярность", 0.05),
];

/// Whole years in IT from the year someone started, in months.
#[must_use]
pub fn experience_since_months(since: u16, year: i32) -> u32 {
    u32::try_from(year - i32::from(since)).unwrap_or(0) * 12
}

/// Stated experience that counts as full tenure, in months.
const TENURE_FULL_MONTHS: f64 = 72.0;

/// Collaboration score from which the report calls collaboration confirmed («Есть подтверждённая
/// совместная работа»); lead needs it unless the candidate states a leading role.
const LEAD_COLLABORATION: u8 = 50;

/// A stated team role that leads people or technical direction, in Russian or English spelling
/// («тимлид», «стафф-инженер», «principal engineer», «главный архитектор», «head of …»).
fn leading_role(role: &str) -> bool {
    let lower = role.to_lowercase();
    [
        "лид",
        "lead",
        "руковод",
        "head",
        "cto",
        "архитект",
        "architect",
        "principal",
        "принципал",
        "staff",
        "стафф",
        "стаф ",
        "главн",
        "директор",
        "director",
    ]
    .iter()
    .any(|key| lower.contains(key))
}

const FSP_MAX: u8 = 15;

/// The most all FSP results together add to specialization scores.
const FSP_SHARE_TOTAL: f64 = 30.0;

fn dimension_key(id: GitHubDimensionId) -> &'static str {
    match id {
        GitHubDimensionId::Consistency => "consistency",
        GitHubDimensionId::Engagement => "engagement",
        GitHubDimensionId::Longevity => "longevity",
        GitHubDimensionId::Collaboration => "collaboration",
        GitHubDimensionId::Ownership => "ownership",
        GitHubDimensionId::Craft => "craft",
        GitHubDimensionId::Impact => "impact",
        GitHubDimensionId::Scope => "scope",
    }
}

fn dimension(report: &GitHubReport, id: GitHubDimensionId) -> u8 {
    report
        .dimensions
        .iter()
        .find(|item| item.id == id)
        .map_or(0, |item| item.score)
}

fn ramp(value: f64, zero: f64, full: f64) -> f64 {
    ((value - zero) / (full - zero)).clamp(0.0, 1.0)
}

/// Grade bands of the seniority score.
fn band(score: u8) -> Grade {
    match score {
        0..=19 => Grade::Intern,
        20..=39 => Grade::Junior,
        40..=59 => Grade::Middle,
        60..=79 => Grade::Senior,
        _ => Grade::Lead,
    }
}

/// Senior needs two years of public work or stated experience, lead four. Months, not calendar
/// years: thirteen months across a new year are not two years of work.
fn cap_grade(grade: Grade, experience_months: u32) -> Grade {
    match grade {
        Grade::Lead if experience_months < required_months(Grade::Lead) => {
            cap_grade(Grade::Senior, experience_months)
        }
        Grade::Senior if experience_months < required_months(Grade::Senior) => Grade::Middle,
        other => other,
    }
}

fn required_months(grade: Grade) -> u32 {
    match grade {
        Grade::Lead => 48,
        Grade::Senior => 24,
        _ => 0,
    }
}

/// Months in words: «5 мес.», «2 года», «1 год 9 мес.».
fn duration(months: u32) -> String {
    let (years, rest) = (months / 12, months % 12);
    match (years, rest) {
        (0, rest) => format!("{rest} мес."),
        (years, 0) => format!("{years} {}", plural(years, ("год", "года", "лет"))),
        (years, rest) => format!(
            "{years} {} {rest} мес.",
            plural(years, ("год", "года", "лет"))
        ),
    }
}

fn plural(value: u32, forms: (&'static str, &'static str, &'static str)) -> &'static str {
    match (value % 10, value % 100) {
        (_, 11..=14) => forms.2,
        (1, _) => forms.0,
        (2..=4, _) => forms.1,
        _ => forms.2,
    }
}

fn lower(confidence: SignalConfidence) -> SignalConfidence {
    match confidence {
        SignalConfidence::High => SignalConfidence::Medium,
        _ => SignalConfidence::Low,
    }
}

fn confidence(
    input: &EstimateInput<'_>,
    published: &[&FspResult],
    graded: bool,
) -> SignalConfidence {
    if !graded {
        return SignalConfidence::Low;
    }
    match input.github {
        Some(report) => {
            let mut level = report.index.confidence;
            if report.coverage.partial {
                level = lower(level);
            }
            // A report of an earlier analysis version misses factors until it is re-analyzed.
            if report.algorithm_version != crate::GITHUB_INSIGHTS_VERSION {
                level = lower(level);
            }
            level
        }
        None if input.stated_experience_months.is_some() || published.len() >= 2 => {
            SignalConfidence::Medium
        }
        None => SignalConfidence::Low,
    }
}

/// Months of public work: from the first analyzed commit, or from the start of the first year with
/// sustained activity (ten active days) when GitHub search reaches further back, to the report date.
fn public_experience_months(report: &GitHubReport) -> Option<u32> {
    let month = |value: &str| -> Option<i64> {
        let year = value.get(0..4)?.parse::<i64>().ok()?;
        let month = value.get(5..7)?.parse::<i64>().ok()?;
        Some(year * 12 + month)
    };
    let first_sustained_year = report
        .activity
        .yearly
        .iter()
        .filter(|year| year.active_days >= 10)
        .map(|year| i64::from(year.year) * 12 + 1)
        .min();
    let start = match (month(&report.coverage.from), first_sustained_year) {
        (Some(analyzed), Some(searched)) => analyzed.min(searched),
        (analyzed, searched) => analyzed.or(searched)?,
    };
    let span = month(&report.coverage.to)? - start;
    (span > 0).then(|| u32::try_from(span).unwrap_or(0))
}

/// Up to fifteen points: the best placement share, a final, and repeated participation.
fn fsp_points(published: &[&FspResult]) -> u8 {
    if published.is_empty() {
        return 0;
    }
    let best = published
        .iter()
        .filter_map(|result| {
            let place = result.place?;
            (result.field_size > 0).then(|| f64::from(place) / f64::from(result.field_size))
        })
        .fold(f64::INFINITY, f64::min);
    let placement = if best <= 0.1 {
        10
    } else if best <= 0.25 {
        7
    } else if best.is_finite() {
        4
    } else {
        2
    };
    let repeated = if published.len() >= 3 {
        5
    } else if published.len() == 2 {
        3
    } else {
        1
    };
    (placement + repeated).min(FSP_MAX)
}

fn fsp_detail(published: &[&FspResult]) -> String {
    let best = published
        .iter()
        .filter_map(|result| result.place.map(|place| (place, result.field_size)))
        .min_by_key(|(place, _)| *place);
    match best {
        Some((place, size)) => format!(
            "Опубликованных результатов: {}; лучшее место — {} из {}",
            published.len(),
            place,
            size
        ),
        None => format!("Опубликованных результатов: {}", published.len()),
    }
}

/// Specialization shares from the candidate's areas of work, catalog indicators and FSP disciplines.
fn specialization_shares(
    github: Option<&GitHubReport>,
    published: &[&FspResult],
) -> Vec<SpecializationShare> {
    let mut scores: BTreeMap<Specialization, f64> = BTreeMap::new();
    if let Some(report) = github {
        let languages = report
            .languages
            .iter()
            .map(|item| (item.language.as_str(), f64::from(item.percent)))
            .collect::<BTreeMap<_, _>>();
        let share = |language: &str| languages.get(language).copied().unwrap_or(0.0);
        let mut areas: BTreeMap<Specialization, f64> = BTreeMap::new();
        for archetype in &report.profile.archetypes {
            let percent = f64::from(archetype.percent);
            let mut add = |id, weight: f64| *areas.entry(id).or_default() += percent * weight;
            match archetype.id {
                Archetype::Backend => add(Specialization::Backend, 1.0),
                Archetype::Frontend => add(Specialization::Frontend, 1.0),
                Archetype::Platform => {
                    add(Specialization::Devops, 0.8);
                    add(Specialization::Sysadmin, 0.2);
                }
                // Tools, libraries and CLIs without a web stack: system or backend engineering.
                Archetype::Systems => {
                    add(Specialization::Systems, 0.5);
                    add(Specialization::Backend, 0.5);
                }
                Archetype::Data => {
                    add(Specialization::DataScience, 0.6);
                    add(Specialization::DataEngineering, 0.4);
                }
                Archetype::Mobile => {
                    let swift = share("Swift") + share("Objective-C");
                    let kotlin = share("Kotlin");
                    let dart = share("Dart");
                    let total = (swift + kotlin + dart).max(1.0);
                    add(Specialization::Ios, swift / total);
                    add(Specialization::Android, kotlin / total);
                    add(
                        Specialization::MobileCrossplatform,
                        if swift + kotlin + dart == 0.0 {
                            1.0
                        } else {
                            dart / total
                        },
                    );
                }
            }
        }
        // Full stack when both backend and frontend carry a substantial share.
        let backend = areas.get(&Specialization::Backend).copied().unwrap_or(0.0);
        let frontend = areas.get(&Specialization::Frontend).copied().unwrap_or(0.0);
        if backend >= 25.0 && frontend >= 25.0 {
            areas.insert(Specialization::Fullstack, backend + frontend);
        }
        let indicators = indicator_shares(report);
        for specialization in Specialization::ALL {
            let area = areas.get(&specialization).copied().unwrap_or(0.0);
            let indicator = indicators.get(&specialization).copied().unwrap_or(0.0);
            let score = 0.6 * area + 0.4 * indicator;
            if score > 0.0 {
                *scores.entry(specialization).or_default() += score;
            }
        }
    }
    // Competitions refine the direction; however many there are, they never outweigh the work.
    let weight = (FSP_SHARE_TOTAL / published.len().max(1) as f64).min(15.0);
    for result in published {
        for specialization in discipline_specializations(&result.discipline) {
            *scores.entry(specialization).or_default() += weight;
        }
    }
    let total: f64 = scores.values().sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let mut shares = scores
        .into_iter()
        .map(|(id, score)| SpecializationShare {
            id,
            percent: (score / total * 100.0).round() as u8,
        })
        .filter(|share| share.percent > 0)
        .collect::<Vec<_>>();
    shares.sort_by(|a, b| b.percent.cmp(&a.percent).then(a.id.cmp(&b.id)));
    shares.truncate(3);
    shares
}

/// Shares of catalog indicator evidence per specialization, 0–100 in total.
fn indicator_shares(report: &GitHubReport) -> BTreeMap<Specialization, f64> {
    let evidence = report
        .technologies
        .iter()
        .map(|item| {
            (
                item.skill_id.as_str(),
                (1.0 + f64::from(item.repositories)).ln(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut raw: BTreeMap<Specialization, f64> = BTreeMap::new();
    for info in &crate::catalog::catalog().specializations {
        let value: f64 = info
            .indicators
            .iter()
            .filter_map(|skill| evidence.get(skill.as_str()))
            .sum();
        // Broad specializations list many indicators; normalize by the square root of their count.
        if value > 0.0 {
            raw.insert(info.id, value / (info.indicators.len() as f64).sqrt());
        }
    }
    let total: f64 = raw.values().sum();
    if total <= 0.0 {
        return raw;
    }
    raw.into_iter()
        .map(|(id, value)| (id, value / total * 100.0))
        .collect()
}

fn discipline_specializations(discipline: &str) -> Vec<Specialization> {
    let lower = discipline.to_lowercase();
    if lower.contains("продукт") {
        vec![Specialization::Fullstack, Specialization::Backend]
    } else if lower.contains("безопас") {
        vec![Specialization::Security]
    } else if lower.contains("робот") || lower.contains("беспилот") || lower.contains("бас")
    {
        vec![Specialization::Embedded]
    } else if lower.contains("алгоритм") {
        vec![Specialization::Backend, Specialization::Systems]
    } else {
        Vec::new()
    }
}

struct GrowthInput<'a> {
    github: Option<&'a GitHubReport>,
    factors: &'a [EstimateFactor],
    fsp_points: u8,
    published: usize,
    /// The band a tenure cap lowered, if one did.
    tenure_cap: Option<&'a Grade>,
    people_cap: bool,
    grade: Option<Grade>,
    public_months: Option<u32>,
}

/// Steps towards a higher grade: conditions that hold the grade back first, then the factors with
/// the most points left, competitions, and the optional test.
fn growth_plan(input: &GrowthInput<'_>) -> Vec<GrowthStep> {
    let step = |id: &str, title: &str, action: String, points: u8| GrowthStep {
        id: id.into(),
        title: title.into(),
        action,
        points,
    };
    let mut steps = Vec::new();
    if input.grade.is_none() && input.github.is_none() && input.published == 0 {
        return vec![step(
            "sources",
            "Источники",
            "Подключите GitHub и ФСП ID: по ним система заполнит анкету и оценит уровень".into(),
            0,
        )];
    }
    if let Some(banded) = input.tenure_cap {
        steps.push(step(
            "tenure",
            "Общий стаж",
            format!(
                "Укажите, с какого года вы в IT: учёба, свои проекты и работа в закрытых \
                 репозиториях. «{}» требует от {} лет, а публично видно {}",
                banded.label(),
                required_months(*banded) / 12,
                input
                    .public_months
                    .map_or_else(|| "меньше месяца".to_owned(), duration)
            ),
            0,
        ));
    }
    if input.people_cap {
        steps.push(step(
            "people",
            "Работа с людьми",
            "Если руководите людьми или техническим направлением, укажите это в роли в команде. \
             Публично это видно по ревью чужого кода и принятым изменениям в чужих проектах"
                .into(),
            0,
        ));
    }
    if let Some(report) = input.github {
        let mut open = input
            .factors
            .iter()
            .filter(|factor| factor.id.starts_with("github_") && factor.max >= factor.points + 2)
            .collect::<Vec<_>>();
        open.sort_by(|a, b| {
            (b.max - b.points)
                .cmp(&(a.max - a.points))
                .then(a.id.cmp(&b.id))
        });
        for factor in open.into_iter().take(4) {
            if let Some(action) = factor_action(&factor.id, report) {
                steps.push(step(
                    &factor.id,
                    &factor.label,
                    action,
                    factor.max - factor.points,
                ));
            }
        }
    } else {
        steps.push(step(
            "github",
            "GitHub",
            "Подключите GitHub: собственные проекты, их масштаб и инженерная культура дают \
             основную часть оценки"
                .into(),
            0,
        ));
    }
    if input.fsp_points < FSP_MAX {
        steps.push(step(
            "fsp",
            "Соревнования ФСП",
            if input.published == 0 {
                "Укажите ФСП ID или участвуйте в соревнованиях: результат в топ-10% даёт до 10 \
                 баллов, три опубликованных участия — ещё 5"
                    .into()
            } else {
                "Новые опубликованные результаты ФСП: место в топ-10% и три участия дают до 15 \
                 баллов"
                    .into()
            },
            FSP_MAX - input.fsp_points,
        ));
    }
    if let Some(grade) = input.grade {
        steps.push(step(
            "assessment",
            "Тест знаний",
            format!(
                "По желанию: тест на уровень «{}» подтверждает категорию, и подтверждённые \
                 кандидаты стоят выше в выдаче",
                grade.label()
            ),
            0,
        ));
    }
    steps
}

/// A concrete action for a GitHub factor, with the candidate's own numbers where they help.
fn factor_action(id: &str, report: &GitHubReport) -> Option<String> {
    let metric = |dimension: GitHubDimensionId, metric: &str| {
        report
            .dimensions
            .iter()
            .find(|item| item.id == dimension)
            .and_then(|item| item.metrics.iter().find(|item| item.id == metric))
            .map_or(0, |item| item.value)
    };
    Some(match id {
        "github_scope" => {
            let sustained = metric(GitHubDimensionId::Scope, "sustained_systems");
            let designed = metric(GitHubDimensionId::Scope, "design_artifacts");
            format!(
                "Развивайте свои проекты дольше полугода (сейчас таких {sustained}) и фиксируйте \
                 архитектурные решения, контракты API и миграции (сейчас они есть в {designed} {})",
                if designed % 10 == 1 && designed % 100 != 11 {
                    "проекте"
                } else {
                    "проектах"
                }
            )
        }
        "github_longevity" => "Стаж на GitHub растёт со временем; учёбу, свои и закрытые проекты \
             учтёт год начала в IT из анкеты"
            .into(),
        "github_ownership" => "Ведите собственные проекты до стабильного состояния: регулярные \
             коммиты в течение года, связанные репозитории — в своей организации"
            .into(),
        "github_craft" => {
            let craft = &report.craft;
            let practices = [
                (craft.tests_percent, "тесты"),
                (craft.ci_percent, "CI"),
                (craft.lint_percent, "линтеры"),
                (craft.lockfile_percent, "фиксацию зависимостей"),
                (
                    craft.dependency_updates_percent,
                    "автообновление зависимостей",
                ),
                (craft.license_percent, "лицензии"),
                (craft.readme_percent, "README"),
            ]
            .into_iter()
            .filter(|(percent, _)| *percent < 70)
            .map(|(percent, name)| format!("{name} (сейчас в {percent}% проектов)"))
            .take(3)
            .collect::<Vec<_>>();
            if practices.is_empty() {
                "Поддерживайте инженерные практики во всех новых проектах".into()
            } else {
                format!("Добавьте в проекты {}", practices.join(", "))
            }
        }
        "github_collaboration" => format!(
            "Ревьюйте чужие pull request'ы и доводите изменения до принятия в независимых проектах: \
             сейчас одобренных другими PR — {}, ревью в независимых репозиториях — {}",
            report.collaboration.approved_pull_requests, report.collaboration.independent_reviews
        ),
        "github_impact" => "Делайте проекты полезными другим: документация, релизы и анонсы \
             приносят звёзды и форки, а вклад в проекты от 1000 звёзд ценится отдельно"
            .into(),
        "github_engagement" => "Сохраняйте активность: больше активных дней в году".into(),
        "github_consistency" => {
            "Работайте регулярно, неделя за неделей, без длинных перерывов".into()
        }
        _ => return None,
    })
}

/// Frameworks tied to a language: they are worth learning only with that language in the stack.
const FRAMEWORK_LANGUAGES: [(&str, &[&str]); 16] = [
    ("spring", &["java", "kotlin"]),
    ("hibernate", &["java", "kotlin"]),
    ("django", &["python"]),
    ("flask", &["python"]),
    ("fastapi", &["python"]),
    ("sqlalchemy", &["python"]),
    ("nodejs", &["javascript", "typescript"]),
    ("nestjs", &["javascript", "typescript"]),
    ("react", &["javascript", "typescript"]),
    ("vue", &["javascript", "typescript"]),
    ("nextjs", &["javascript", "typescript"]),
    ("aspnet", &["csharp"]),
    ("laravel", &["php"]),
    ("rails", &["ruby"]),
    ("actix", &["rust"]),
    ("tokio", &["rust"]),
];

/// Skills the specialization's catalog lists that the evidence does not show: general ones first,
/// then frameworks of languages the candidate already uses; never another language.
fn focus_skills(
    specialization: Option<Specialization>,
    github: Option<&GitHubReport>,
    stack: &[String],
) -> Vec<String> {
    let Some(specialization) = specialization else {
        return Vec::new();
    };
    let evidenced = |skill: &str| {
        stack.iter().any(|item| item == skill)
            || github.is_some_and(|report| {
                report
                    .technologies
                    .iter()
                    .any(|item| item.skill_id == skill)
            })
    };
    let mut general = Vec::new();
    let mut frameworks = Vec::new();
    for skill in &crate::catalog::specialization(specialization).indicators {
        if evidenced(skill)
            || crate::catalog::skill(skill).is_none_or(|info| info.category == "language")
        {
            continue;
        }
        match FRAMEWORK_LANGUAGES.iter().find(|(id, _)| id == skill) {
            Some((_, languages)) if languages.iter().any(|language| evidenced(language)) => {
                frameworks.push(skill.clone());
            }
            Some(_) => {}
            None => general.push(skill.clone()),
        }
    }
    general.extend(frameworks);
    general.truncate(5);
    general
}

/// Questionnaire answers: the strongest catalog skills, experience and role.
fn answers(
    github: Option<&GitHubReport>,
    published: &[&FspResult],
    specialization: Option<Specialization>,
    experience_months: Option<u32>,
) -> EstimateAnswers {
    let mut stack = Vec::new();
    let mut team_role = String::new();
    let mut experience = String::new();
    let mut name = None;
    if let Some(report) = github {
        // Languages the candidate writes most, then the technologies used across most projects.
        for language in report.languages.iter().filter(|item| item.percent >= 5) {
            if let Some(skill) = language_skill(&language.language)
                && !stack.contains(&skill.to_owned())
            {
                stack.push(skill.to_owned());
            }
        }
        let mut technologies = report
            .technologies
            .iter()
            .filter(|item| crate::catalog::is_skill(&item.skill_id))
            .collect::<Vec<_>>();
        technologies.sort_by(|a, b| {
            b.repositories
                .cmp(&a.repositories)
                .then(a.skill_id.cmp(&b.skill_id))
        });
        for item in technologies {
            if stack.len() >= STACK_LIMIT {
                break;
            }
            if !stack.contains(&item.skill_id) {
                stack.push(item.skill_id.clone());
            }
        }
        stack.truncate(STACK_LIMIT);
        experience = report.profile.summary.clone();
        let projects = report
            .profile
            .top_projects
            .iter()
            .take(3)
            .map(|project| {
                let name = project.name_with_owner.as_str();
                name.rsplit_once('/')
                    .map_or(name, |(_, repository)| repository)
            })
            .collect::<Vec<_>>();
        if !projects.is_empty() {
            experience.push_str(&format!(" Главные проекты: {}.", projects.join(", ")));
        }
        let ownership = &report.ownership;
        let collaboration = &report.collaboration;
        team_role = if ownership.led_repositories >= 5 && ownership.own_organizations >= 1 {
            "Ведёт собственные проекты и организации".into()
        } else if collaboration.external_merged >= 10 || collaboration.reviews_of_others >= 20 {
            "Командная разработка, ревью и вклад в открытые проекты".into()
        } else if ownership.led_repositories >= 1 {
            "Самостоятельно ведёт проекты".into()
        } else {
            String::new()
        };
        name = report.display_name.clone();
    }
    if team_role.is_empty() {
        team_role = specialization
            .map(|item| crate::catalog::specialization(item).role.clone())
            .unwrap_or_default();
    }
    EstimateAnswers {
        stack,
        experience_months,
        experience,
        team_role,
        name,
        soft_skills: soft_skills(github, published),
    }
}

/// Working habits with public evidence behind them; nothing is inferred from personality.
fn soft_skills(github: Option<&GitHubReport>, published: &[&FspResult]) -> Vec<String> {
    let mut skills = Vec::new();
    if let Some(report) = github {
        let craft = &report.craft;
        let ownership = &report.ownership;
        let collaboration = &report.collaboration;
        let evidence = [
            (
                craft.substantive_repositories >= 3 && craft.readme_percent >= 80,
                "Документирует свои проекты",
            ),
            (
                craft
                    .descriptive_messages_percent
                    .is_some_and(|value| value >= 80),
                "Понятно описывает изменения в коммитах",
            ),
            (
                ownership.led_repositories >= 3,
                "Самостоятельно ведёт проекты от идеи до поддержки",
            ),
            (
                ownership.own_organizations >= 1 && ownership.own_organization_repositories >= 5,
                "Организует работу нескольких связанных проектов",
            ),
            (
                collaboration.reviews_of_others >= 20,
                "Ревьюит код других разработчиков",
            ),
            (
                collaboration.external_merged >= 5,
                "Доводит изменения до принятия в чужих проектах",
            ),
            (
                collaboration.issues_total >= 20,
                "Ведёт задачи и обсуждения в issues",
            ),
        ];
        skills.extend(
            evidence
                .into_iter()
                .filter(|(shown, _)| *shown)
                .map(|(_, skill)| skill.to_owned()),
        );
    }
    if published.iter().any(|result| result.scope == "team") {
        skills.push("Работает в команде на соревнованиях".to_owned());
    }
    if !published.is_empty() {
        skills.push("Решает задачи в ограниченное время".to_owned());
    }
    skills.truncate(SOFT_SKILL_LIMIT);
    skills
}

fn language_skill(language: &str) -> Option<&'static str> {
    Some(match language {
        "Python" => "python",
        "Java" => "java",
        "JavaScript" => "javascript",
        "TypeScript" => "typescript",
        "C#" => "csharp",
        "C++" => "cpp",
        "C" => "c",
        "Go" => "go",
        "Rust" => "rust",
        "Kotlin" => "kotlin",
        "Swift" => "swift",
        "PHP" => "php",
        "Ruby" => "ruby",
        "Scala" => "scala",
        "Dart" => "dart",
        "1C Enterprise" => "onec_language",
        "ABAP" => "abap",
        "Delphi" => "delphi",
        "SQL" => "sql",
        "Shell" => "bash",
        "PowerShell" => "powershell",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> GitHubReport {
        serde_json::from_str(include_str!(
            "../../../../contracts/fixtures/github-report.json"
        ))
        .unwrap()
    }

    fn fsp(discipline: &str, place: u32, size: u32) -> FspResult {
        FspResult {
            competition_id: format!("competition-{place}"),
            competition_name: "Демо".into(),
            discipline: discipline.into(),
            starts_at: "2026-03-01".into(),
            ends_at: "2026-03-02".into(),
            scope: "individual".into(),
            team_name: None,
            team_size: None,
            status: "published".into(),
            place: Some(place),
            field_size: size,
            score: Some(100),
            protocol_reference: None,
            published_at: Some("2026-03-03".into()),
            synthetic: true,
        }
    }

    fn input<'a>(github: Option<&'a GitHubReport>, fsp: &'a [FspResult]) -> EstimateInput<'a> {
        EstimateInput {
            github,
            fsp,
            stated_experience_months: None,
            stated_team_role: None,
            now: "2026-10-10T00:00:00Z",
        }
    }

    #[test]
    fn a_github_report_gives_a_specialization_grade_and_answers() {
        let report = report();
        let estimate = estimate_profile(&input(Some(&report), &[]));
        assert_eq!(estimate.version, ESTIMATE_VERSION);
        assert_eq!(estimate.specialization, Some(Specialization::Backend));
        assert!(estimate.grade.is_some());
        assert_eq!(estimate.factors.len(), DIMENSION_WEIGHTS.len());
        assert_eq!(
            estimate
                .factors
                .iter()
                .map(|factor| u32::from(factor.max))
                .sum::<u32>(),
            100
        );
        assert!(estimate.answers.stack.contains(&"rust".to_owned()));
        assert!(estimate.answers.stack.len() <= STACK_LIMIT);
        assert!(
            estimate
                .answers
                .stack
                .iter()
                .all(|skill| crate::catalog::is_skill(skill))
        );
        assert!(!estimate.answers.experience.is_empty());
        assert!(!estimate.answers.team_role.is_empty());
        assert!(!estimate.answers.soft_skills.is_empty());
        assert!(estimate.answers.soft_skills.len() <= SOFT_SKILL_LIMIT);
        assert_eq!(estimate.sources, ["github"]);
        // A report of an earlier analysis version says so and lowers the confidence.
        let mut outdated = report.clone();
        outdated.algorithm_version = "github-insights-v2".into();
        let outdated = estimate_profile(&input(Some(&outdated), &[]));
        assert!(outdated.notes[0].starts_with("Отчёт GitHub построен прошлой версией анализа"));
        assert_eq!(outdated.confidence, lower(estimate.confidence));
        // Below lead the notes say what the next band needs.
        assert!(
            estimate
                .notes
                .iter()
                .any(|note| note.starts_with("До уровня")),
            "{:?}",
            estimate.notes
        );
        assert_eq!(
            estimate,
            estimate_profile(&input(Some(&report), &[])),
            "deterministic"
        );
    }

    #[test]
    fn without_evidence_there_is_no_grade() {
        let estimate = estimate_profile(&input(None, &[]));
        assert_eq!(estimate.grade, None);
        assert_eq!(estimate.specialization, None);
        assert_eq!(estimate.confidence, SignalConfidence::Low);
        assert!(estimate.insufficient.is_some());
    }

    #[test]
    fn fsp_results_alone_name_a_specialization_and_a_modest_grade() {
        let results = [
            fsp("Программирование систем информационной безопасности", 2, 40),
            fsp("Программирование систем информационной безопасности", 5, 60),
        ];
        let estimate = estimate_profile(&input(None, &results));
        assert_eq!(estimate.specialization, Some(Specialization::Security));
        assert_eq!(estimate.grade, Some(Grade::Junior));
        assert_eq!(estimate.score, 26);
        assert_eq!(estimate.confidence, SignalConfidence::Medium);
        assert_eq!(estimate.sources, ["fsp"]);
        let fsp_factor = estimate
            .factors
            .iter()
            .find(|factor| factor.id == "fsp")
            .unwrap();
        assert_eq!(fsp_factor.points, 13);
    }

    #[test]
    fn volume_without_years_never_reaches_senior() {
        let mut report = report();
        for dimension in &mut report.dimensions {
            dimension.score = 100;
        }
        // Thirteen months across two calendar years are not two years of work.
        report.activity.active_years = 2;
        report.activity.yearly.retain(|year| year.year >= 2025);
        report.coverage.from = "2025-09-01T00:00:00Z".into();
        report.coverage.to = "2026-10-01T00:00:00Z".into();
        for year in &mut report.activity.yearly {
            year.active_days = 9;
        }
        let estimate = estimate_profile(&input(Some(&report), &[]));
        assert_eq!(estimate.score, 100);
        assert_eq!(estimate.grade, Some(Grade::Middle));
        assert!(
            estimate.notes[0].contains("«Лид»") && estimate.notes[0].contains("4 лет"),
            "{:?}",
            estimate.notes
        );
        // Sustained activity found by search reaches back further than the analyzed commits.
        report.activity.yearly.push(crate::GitHubYear {
            year: 2021,
            commits: 300,
            active_days: 120,
            pull_requests: None,
        });
        let estimate = estimate_profile(&input(Some(&report), &[]));
        // Public tenure is evidence for the caps, never the candidate's answer.
        assert_eq!(estimate.answers.experience_months, None);
        assert_eq!(estimate.grade, Some(Grade::Lead));
    }

    #[test]
    fn stated_experience_never_lowers_public_evidence() {
        let report = report();
        let public = estimate_profile(&input(Some(&report), &[]));
        // Five months typed instead of five years costs nothing.
        let mut typo = input(Some(&report), &[]);
        typo.stated_experience_months = Some(5);
        let typo = estimate_profile(&typo);
        assert_eq!(typo.score, public.score);
        assert_eq!(typo.grade, public.grade);
        assert_eq!(typo.answers.experience_months, Some(5));

        // Longer stated experience stands in for tenure only, never for the other evidence.
        let mut long = input(Some(&report), &[]);
        long.stated_experience_months = Some(120);
        let long = estimate_profile(&long);
        let tenure = |estimate: &ProfileEstimate| {
            estimate
                .factors
                .iter()
                .find(|factor| factor.id == "github_longevity")
                .unwrap()
                .clone()
        };
        assert_eq!(tenure(&long).points, tenure(&long).max);
        assert!(tenure(&long).detail.starts_with("По анкете — 10 лет"));
        assert_eq!(
            u32::from(long.score) - u32::from(public.score),
            u32::from(tenure(&long).points - tenure(&public).points)
        );
        assert_eq!(long.answers.experience_months, Some(120));
    }

    #[test]
    fn a_cap_lifted_only_by_stated_experience_keeps_confidence_moderate() {
        let mut report = report();
        for dimension in &mut report.dimensions {
            dimension.score = 100;
        }
        report.activity.yearly.retain(|year| year.year >= 2025);
        report.coverage.from = "2025-09-01T00:00:00Z".into();
        report.coverage.to = "2026-10-01T00:00:00Z".into();
        for year in &mut report.activity.yearly {
            year.active_days = 9;
        }
        report.index.confidence = SignalConfidence::High;
        let mut stated = input(Some(&report), &[]);
        stated.stated_experience_months = Some(60);
        let estimate = estimate_profile(&stated);
        assert_eq!(estimate.grade, Some(Grade::Lead));
        assert_eq!(estimate.confidence, SignalConfidence::Medium);
        assert!(
            estimate.notes[0]
                .starts_with("Стаж 5 лет взят из анкеты (публично видно 1 год 1 мес.)"),
            "{:?}",
            estimate.notes
        );
    }

    #[test]
    fn lead_needs_work_with_people_in_public_or_a_stated_leading_role() {
        let mut report = report();
        for dimension in &mut report.dimensions {
            dimension.score = if dimension.id == GitHubDimensionId::Collaboration {
                38
            } else {
                100
            };
        }
        report.index.confidence = SignalConfidence::High;
        report.activity.yearly.push(crate::GitHubYear {
            year: 2019,
            commits: 300,
            active_days: 120,
            pull_requests: None,
        });
        let solo = estimate_profile(&input(Some(&report), &[]));
        assert!(solo.score >= 80, "{}", solo.score);
        assert_eq!(solo.grade, Some(Grade::Senior));
        assert!(solo.notes[0].contains("«Совместная работа» — 38 из 100"));
        // Without the tenure a stated role changes nothing, confidence included.
        let mut young = report.clone();
        young.activity.yearly.retain(|year| year.year >= 2025);
        young.coverage.from = "2025-09-01T00:00:00Z".into();
        let mut stated = input(Some(&young), &[]);
        stated.stated_team_role = Some("Архитектор");
        let stated = estimate_profile(&stated);
        assert_eq!(stated.grade, Some(Grade::Middle));
        assert_eq!(stated.confidence, SignalConfidence::High);
        assert!(
            stated.notes[0].contains("поэтому уровень — «Мидл»"),
            "{:?}",
            stated.notes
        );
        let mut developer = input(Some(&report), &[]);
        developer.stated_team_role = Some("Разработчик");
        assert_eq!(estimate_profile(&developer).grade, Some(Grade::Senior));
        // Russian and English spellings of leading roles count alike.
        for role in [
            "Стафф-инженер",
            "staff engineer",
            "Принципал-инженер",
            "Главный архитектор",
            "Head of Platform",
        ] {
            assert!(leading_role(role), "{role}");
        }
        for role in ["Разработчик", "Инженер-программист", "Software engineer"]
        {
            assert!(!leading_role(role), "{role}");
        }
        let mut lead = input(Some(&report), &[]);
        lead.stated_team_role = Some("Тимлид платформы");
        let lead = estimate_profile(&lead);
        assert_eq!(lead.grade, Some(Grade::Lead));
        assert_eq!(lead.confidence, SignalConfidence::Medium);
        assert!(lead.notes[0].starts_with("Руководящая роль взята из анкеты"));
        // Confirmed public collaboration needs no stated role.
        for dimension in &mut report.dimensions {
            dimension.score = 100;
        }
        let open = estimate_profile(&input(Some(&report), &[]));
        assert_eq!(open.grade, Some(Grade::Lead));
        assert_eq!(open.confidence, SignalConfidence::High);
    }

    #[test]
    fn the_growth_plan_names_what_holds_the_grade_and_what_to_work_on() {
        let mut report = report();
        for dimension in &mut report.dimensions {
            dimension.score = if dimension.id == GitHubDimensionId::Collaboration {
                38
            } else {
                100
            };
        }
        report.activity.yearly.retain(|year| year.year >= 2025);
        report.coverage.from = "2025-09-01T00:00:00Z".into();
        let estimate = estimate_profile(&input(Some(&report), &[]));
        let ids = estimate
            .growth
            .iter()
            .map(|step| step.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(&ids[..3], ["tenure", "people", "github_collaboration"]);
        assert!(ids.contains(&"fsp"));
        assert_eq!(ids.last(), Some(&"assessment"));
        assert!(
            estimate.growth[0]
                .action
                .contains("«Сеньор» требует от 2 лет")
        );
        assert!(estimate.growth[2].action.contains("одобренных другими PR"));
        // Focus skills: the specialization's own, never a language or another language's framework.
        assert!(!estimate.focus_skills.is_empty());
        for skill in &estimate.focus_skills {
            assert_ne!(crate::catalog::skill(skill).unwrap().category, "language");
            assert!(
                !["spring", "hibernate", "aspnet", "laravel", "rails"].contains(&skill.as_str())
            );
        }
    }

    #[test]
    fn competitions_refine_the_direction_without_outweighing_the_work() {
        let report = report();
        let two = [
            fsp("Продуктовое программирование", 1, 64),
            fsp("Продуктовое программирование", 2, 120),
        ];
        let five = [
            fsp("Продуктовое программирование", 1, 64),
            fsp("Продуктовое программирование", 2, 120),
            fsp("Продуктовое программирование", 3, 18),
            fsp("Продуктовое программирование", 4, 80),
            fsp("Продуктовое программирование", 5, 90),
        ];
        let shares = |results: &[FspResult]| {
            specialization_shares(Some(&report), &results.iter().collect::<Vec<_>>())
        };
        assert_eq!(shares(&two), shares(&five));
        assert_eq!(shares(&two)[0].id, Specialization::Backend);
    }

    #[test]
    fn durations_read_as_years_and_months() {
        assert_eq!(duration(5), "5 мес.");
        assert_eq!(duration(12), "1 год");
        assert_eq!(duration(21), "1 год 9 мес.");
        assert_eq!(duration(24), "2 года");
        assert_eq!(duration(60), "5 лет");
        assert_eq!(duration(134), "11 лет 2 мес.");
    }

    #[test]
    fn stated_experience_counts_private_work() {
        let mut stated = input(None, &[]);
        stated.stated_experience_months = Some(72);
        let estimate = estimate_profile(&stated);
        assert_eq!(estimate.grade, Some(Grade::Junior));
        assert_eq!(estimate.answers.experience_months, Some(72));
        assert_eq!(estimate.confidence, SignalConfidence::Medium);
    }

    #[test]
    fn a_review_picks_between_close_specializations_and_lowers_confidence_on_disagreement() {
        let report = report();
        let mut estimate = estimate_profile(&input(Some(&report), &[]));
        estimate.specializations = vec![
            SpecializationShare {
                id: Specialization::Backend,
                percent: 45,
            },
            SpecializationShare {
                id: Specialization::Devops,
                percent: 40,
            },
        ];
        estimate.confidence = SignalConfidence::High;
        let rule_grade = estimate.grade.unwrap();
        let other = if rule_grade == Grade::Lead {
            Grade::Senior
        } else {
            Grade::Lead
        };
        apply_review(
            &mut estimate,
            ProfileReviewReply {
                specialization: Specialization::Devops,
                grade: other,
                rationale: "Основная работа — платформа и CI".into(),
                experience: "Строит платформу и автоматизацию.".into(),
                team_role: "Ведёт инфраструктуру".into(),
                recommendations: vec![
                    "Ревьюйте чужие изменения в платформенных проектах".into(),
                    "См. https://example.test".into(),
                ],
            },
        );
        assert_eq!(estimate.specialization, Some(Specialization::Devops));
        assert_eq!(
            estimate.grade,
            Some(rule_grade),
            "the grade stays rule-based"
        );
        assert_eq!(estimate.confidence, SignalConfidence::Medium);
        assert!(!estimate.review.as_ref().unwrap().agrees);
        assert_eq!(estimate.answers.team_role, "Ведёт инфраструктуру");
        // Advice with links is dropped; the rest is kept.
        assert_eq!(
            estimate.review.as_ref().unwrap().recommendations,
            ["Ревьюйте чужие изменения в платформенных проектах"]
        );
        // A further disagreement keeps the category: confidence stops at medium.
        apply_review(
            &mut estimate,
            ProfileReviewReply {
                specialization: Specialization::Devops,
                grade: other,
                rationale: String::new(),
                experience: String::new(),
                team_role: String::new(),
                recommendations: Vec::new(),
            },
        );
        assert_eq!(estimate.confidence, SignalConfidence::Medium);
        // A far specialization is not taken.
        apply_review(
            &mut estimate,
            ProfileReviewReply {
                specialization: Specialization::Design,
                grade: rule_grade,
                rationale: String::new(),
                experience: String::new(),
                team_role: String::new(),
                recommendations: Vec::new(),
            },
        );
        assert_eq!(estimate.specialization, Some(Specialization::Devops));
        // Texts with links or addresses are dropped; the earlier ones stay.
        apply_review(
            &mut estimate,
            ProfileReviewReply {
                specialization: Specialization::Devops,
                grade: rule_grade,
                rationale: "См. https://example.test".into(),
                experience: "Пишите на mail@example.test".into(),
                team_role: "  Ведёт\n платформу  ".into(),
                recommendations: Vec::new(),
            },
        );
        assert_eq!(
            estimate.answers.experience,
            "Строит платформу и автоматизацию."
        );
        assert_eq!(estimate.answers.team_role, "Ведёт платформу");
        assert_eq!(estimate.review.as_ref().unwrap().rationale, "");
    }

    fn new_profile() -> crate::CandidateProfile {
        crate::CandidateProfile {
            id: "candidate-1".into(),
            alias: "Кандидат №001".into(),
            email: "new@example.test".into(),
            name: DEFAULT_CANDIDATE_NAME.into(),
            specialization: Specialization::Backend,
            selected_grade: Grade::Junior,
            stack: Vec::new(),
            team_role: String::new(),
            experience_months: None,
            experience_since: None,
            experience: String::new(),
            soft_skills: Vec::new(),
            consents: crate::Consents {
                processing: true,
                publication: false,
            },
            verified_grade: None,
            verified_specialization: None,
            grade_confirmed_at: None,
            estimate: None,
            auto_fields: Vec::new(),
            revision: 1,
            updated_at: "2026-10-10T00:00:00Z".into(),
        }
    }

    #[test]
    fn an_estimate_answers_a_new_profile_and_keeps_what_the_candidate_edited() {
        let report = report();
        let mut estimate = estimate_profile(&input(Some(&report), &[]));
        estimate.specialization = Some(Specialization::Devops);
        estimate.grade = Some(Grade::Middle);
        let mut profile = new_profile();
        profile.apply_estimate(estimate.clone());
        assert!(profile.questionnaire_complete());
        assert_eq!(profile.specialization, Specialization::Devops);
        assert_eq!(profile.selected_grade, Grade::Middle);
        assert_eq!(profile.stack, estimate.answers.stack);
        assert_eq!(profile.auto_fields.len(), AUTO_FIELDS.len());
        assert_eq!(profile.stated_experience_months(2026), None);
        assert_eq!(
            profile.experience_months, None,
            "public tenure is not filled in"
        );

        // A start year is a statement that stays current; an old auto-filled value is dropped.
        let mut started = new_profile();
        started.experience_since = Some(2016);
        assert_eq!(started.stated_experience_months(2026), Some(120));
        assert_eq!(started.stated_experience_months(2027), Some(132));
        let mut legacy = new_profile();
        legacy.experience_months = Some(21);
        legacy.auto_fields.push("experienceMonths".into());
        assert_eq!(legacy.stated_experience_months(2026), None);
        legacy.apply_estimate(estimate.clone());
        assert_eq!(legacy.experience_months, None);
        assert!(
            !legacy
                .auto_fields
                .iter()
                .any(|field| field == "experienceMonths")
        );

        // The candidate's own answer stays; auto answers follow the next estimate.
        profile.team_role = "Тимлид".into();
        profile.auto_fields.retain(|field| field != "teamRole");
        estimate.grade = Some(Grade::Senior);
        profile.apply_estimate(estimate.clone());
        assert_eq!(profile.team_role, "Тимлид");
        assert_eq!(profile.selected_grade, Grade::Senior);

        // Rebuilding hands every answer but an edited name back to the evidence.
        profile.name = "Анна".into();
        profile.auto_fields.retain(|field| field != "name");
        profile.soft_skills = vec!["Своё".into()];
        profile.auto_fields.retain(|field| field != "softSkills");
        profile.release_answers();
        assert_eq!(profile.stated_experience_months(2026), None);
        profile.rebuild_from_estimate(estimate.clone());
        assert_eq!(profile.team_role, estimate.answers.team_role);
        assert_eq!(profile.soft_skills, estimate.answers.soft_skills);
        assert_eq!(profile.name, "Анна");
        assert!(!profile.auto_fields.iter().any(|field| field == "name"));
        assert!(
            profile
                .auto_fields
                .iter()
                .any(|field| field == "softSkills")
        );

        // A test-confirmed category keeps its specialization and target grade.
        profile.verified_grade = Some(Grade::Lead);
        profile.verified_specialization = Some(Specialization::Devops);
        profile.selected_grade = Grade::Lead;
        estimate.specialization = Some(Specialization::Frontend);
        profile.apply_estimate(estimate);
        assert_eq!(profile.specialization, Specialization::Devops);
        assert_eq!(profile.selected_grade, Grade::Lead);
    }

    #[test]
    fn the_review_request_carries_no_names_or_links() {
        let report = report();
        let estimate = estimate_profile(&input(Some(&report), &[]));
        let request = serde_json::to_string(&review_request(
            &estimate,
            Some(&report),
            &[],
            Some(60),
            Some("Тимлид"),
        ))
        .unwrap();
        for private in ["owner/", "acme-labs", "github.com", "Иван"] {
            assert!(!request.contains(private), "{private}");
        }
    }
}
