use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::{Grade, Specialization};

pub const MATCHING_ALGORITHM_VERSION: &str = "deterministic-rules-v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Criticality {
    Required,
    Desired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Confirmed,
    Evidence,
    Observed,
    Insufficient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceSource {
    Assessment,
    Github,
    Fsp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillEvidence {
    pub skill_id: String,
    pub status: EvidenceStatus,
    pub evidence_grade: Option<Grade>,
    pub sources: Vec<EvidenceSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankableCandidate {
    pub id: String,
    pub specialization: Specialization,
    /// The category grade: confirmed by a test or estimated.
    pub grade: Grade,
    /// Category strength: 3 a test, 2 a high and 1 a medium-confidence estimate.
    pub confirmation: u8,
    pub fsp_verified: bool,
    pub skills: Vec<SkillEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requirement {
    pub skill_id: String,
    pub criticality: Criticality,
    pub minimum_grade: Grade,
    pub evidence_policy: EvidencePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePolicy {
    pub accept_assessments: bool,
    pub accept_github: bool,
    pub accept_fsp: bool,
    pub competition_selection: String,
    pub competition_ids: Vec<String>,
}

impl EvidencePolicy {
    pub fn accepts(&self, source: EvidenceSource) -> bool {
        match source {
            EvidenceSource::Assessment => self.accept_assessments,
            EvidenceSource::Github => self.accept_github,
            EvidenceSource::Fsp => self.accept_fsp,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HiringNeed {
    pub specialization: Specialization,
    pub grade: Grade,
    pub requirements: Vec<Requirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchScore {
    pub grade_matches: bool,
    pub required_met: usize,
    pub required_total: usize,
    pub desired_met: usize,
    pub desired_total: usize,
    pub required_evidenced: usize,
    pub desired_evidenced: usize,
    pub algorithm_version: &'static str,
}

#[must_use]
pub fn score(candidate: &RankableCandidate, need: &HiringNeed) -> MatchScore {
    let required = need
        .requirements
        .iter()
        .filter(|item| item.criticality == Criticality::Required)
        .collect::<Vec<_>>();
    let desired = need
        .requirements
        .iter()
        .filter(|item| item.criticality == Criticality::Desired)
        .collect::<Vec<_>>();
    MatchScore {
        grade_matches: candidate.specialization == need.specialization
            && candidate.grade >= need.grade,
        required_met: required
            .iter()
            .filter(|item| requirement_met(candidate, item))
            .count(),
        required_total: required.len(),
        desired_met: desired
            .iter()
            .filter(|item| requirement_met(candidate, item))
            .count(),
        desired_total: desired.len(),
        required_evidenced: required
            .iter()
            .filter(|item| requirement_evidence(&candidate.skills, item).supported())
            .count(),
        desired_evidenced: desired
            .iter()
            .filter(|item| requirement_evidence(&candidate.skills, item).supported())
            .count(),
        algorithm_version: MATCHING_ALGORITHM_VERSION,
    }
}

fn requirement_met(candidate: &RankableCandidate, requirement: &Requirement) -> bool {
    requirement_evidence(&candidate.skills, requirement).meets
}

/// The aggregate skill grade belongs to an assessment. GitHub/FSP observations must never
/// borrow it when that assessment is excluded by the employer's policy.
pub struct RequirementEvidence {
    pub meets: bool,
    pub status: EvidenceStatus,
    pub grade: Option<Grade>,
    pub sources: Vec<EvidenceSource>,
    pub accepted_sources: Vec<EvidenceSource>,
}

impl RequirementEvidence {
    pub fn supported(&self) -> bool {
        self.status != EvidenceStatus::Insufficient && !self.accepted_sources.is_empty()
    }
}

#[must_use]
pub fn requirement_evidence(
    skills: &[SkillEvidence],
    requirement: &Requirement,
) -> RequirementEvidence {
    let skill = skills
        .iter()
        .find(|skill| skill.skill_id == requirement.skill_id);
    let status = skill.map_or(EvidenceStatus::Insufficient, |skill| skill.status);
    let sources = skill.map_or_else(Vec::new, |skill| skill.sources.clone());
    // FSP skill evidence is not persisted with competition provenance yet. In particular, a
    // generic participation flag cannot satisfy a selected-competition policy.
    let accepted_sources: Vec<_> = sources
        .iter()
        .copied()
        .filter(|source| {
            requirement.evidence_policy.accepts(*source)
                && (*source != EvidenceSource::Fsp
                    || requirement.evidence_policy.competition_selection == "system")
        })
        .collect();
    let grade = skill.and_then(|skill| skill.evidence_grade);
    RequirementEvidence {
        meets: status == EvidenceStatus::Confirmed
            && accepted_sources.contains(&EvidenceSource::Assessment)
            && grade.is_some_and(|grade| grade >= requirement.minimum_grade),
        status,
        grade,
        sources,
        accepted_sources,
    }
}

#[must_use]
pub fn rank_candidates(
    candidates: &[RankableCandidate],
    need: &HiringNeed,
) -> Vec<(String, MatchScore)> {
    let mut ranked = candidates
        .iter()
        .map(|candidate| Ranked {
            id: candidate.id.clone(),
            fsp_verified: candidate.fsp_verified,
            confirmation: candidate.confirmation,
            score: score(candidate, need),
        })
        .collect::<Vec<_>>();
    ranked.sort_by(compare_rank);
    ranked
        .into_iter()
        .map(|item| (item.id, item.score))
        .collect()
}

struct Ranked {
    id: String,
    fsp_verified: bool,
    confirmation: u8,
    score: MatchScore,
}

/// Need fit first; within it a test-confirmed category, then estimate confidence, then FSP.
fn compare_rank(left: &Ranked, right: &Ranked) -> Ordering {
    right
        .score
        .grade_matches
        .cmp(&left.score.grade_matches)
        .then_with(|| right.score.required_met.cmp(&left.score.required_met))
        .then_with(|| right.score.desired_met.cmp(&left.score.desired_met))
        .then_with(|| {
            right
                .score
                .required_evidenced
                .cmp(&left.score.required_evidenced)
        })
        .then_with(|| {
            right
                .score
                .desired_evidenced
                .cmp(&left.score.desired_evidenced)
        })
        .then_with(|| right.confirmation.cmp(&left.confirmation))
        .then_with(|| right.fsp_verified.cmp(&left.fsp_verified))
        .then_with(|| left.id.cmp(&right.id))
}

/// Compare vacancies of different sizes by coverage, not raw counts. Stable tie-breakers
/// (publication time and id) belong to the application, not the evidence calculation.
pub fn compare_vacancy_fit(left: &MatchScore, right: &MatchScore) -> Ordering {
    fn coverage(l: usize, lt: usize, r: usize, rt: usize) -> Ordering {
        let (l, lt) = if lt == 0 { (1, 1) } else { (l, lt) };
        let (r, rt) = if rt == 0 { (1, 1) } else { (r, rt) };
        (r as u128 * lt as u128).cmp(&(l as u128 * rt as u128))
    }
    right
        .grade_matches
        .cmp(&left.grade_matches)
        .then_with(|| {
            coverage(
                left.required_met,
                left.required_total,
                right.required_met,
                right.required_total,
            )
        })
        .then_with(|| {
            coverage(
                left.desired_met,
                left.desired_total,
                right.desired_met,
                right.desired_total,
            )
        })
        .then_with(|| {
            coverage(
                left.required_evidenced,
                left.required_total,
                right.required_evidenced,
                right.required_total,
            )
        })
        .then_with(|| {
            coverage(
                left.desired_evidenced,
                left.desired_total,
                right.desired_evidenced,
                right.desired_total,
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_cannot_borrow_a_disallowed_assessment_grade() {
        let mut candidate = candidate("mixed", Grade::Middle, true, false);
        candidate.skills[0].sources.push(EvidenceSource::Github);
        let mut need = need();
        need.requirements[0].evidence_policy.accept_assessments = false;
        need.requirements[0].evidence_policy.accept_github = true;
        let evidence = requirement_evidence(&candidate.skills, &need.requirements[0]);
        assert!(!evidence.meets);
        assert!(evidence.supported());
        assert_eq!(score(&candidate, &need).required_met, 0);
        assert_eq!(score(&candidate, &need).required_evidenced, 1);
    }

    #[test]
    fn observed_technology_improves_fit_only_when_the_policy_accepts_it() {
        let mut candidate = candidate("observed", Grade::Middle, false, false);
        candidate.skills[0].status = EvidenceStatus::Evidence;
        candidate.skills[0].sources = vec![EvidenceSource::Github];
        let mut need = need();
        assert_eq!(score(&candidate, &need).required_evidenced, 0);
        need.requirements[0].evidence_policy.accept_github = true;
        let score = score(&candidate, &need);
        assert_eq!((score.required_met, score.required_evidenced), (0, 1));
    }

    #[test]
    fn selected_competitions_require_provenance_and_never_prove_a_grade() {
        let mut candidate = candidate("fsp", Grade::Middle, false, false);
        candidate.skills[0].status = EvidenceStatus::Evidence;
        candidate.skills[0].sources = vec![EvidenceSource::Fsp];
        let mut need = need();
        need.requirements[0].evidence_policy.accept_fsp = true;
        need.requirements[0].evidence_policy.competition_selection = "selected".into();
        need.requirements[0].evidence_policy.competition_ids = vec!["competition-other".into()];
        assert_eq!(score(&candidate, &need).required_evidenced, 0);
        assert_eq!(score(&candidate, &need).required_met, 0);
    }

    #[test]
    fn vacancy_ranking_compares_coverage_instead_of_number_of_requirements() {
        let candidate = candidate("candidate", Grade::Middle, true, false);
        let mut full = score(&candidate, &need());
        full.required_met = 1;
        full.required_total = 1;
        let mut partial = full.clone();
        partial.required_met = 8;
        partial.required_total = 10;
        assert_eq!(compare_vacancy_fit(&full, &partial), Ordering::Less);
        assert_eq!(compare_vacancy_fit(&partial, &full), Ordering::Greater);
    }

    fn candidate(id: &str, grade: Grade, required: bool, desired: bool) -> RankableCandidate {
        RankableCandidate {
            id: id.into(),
            specialization: Specialization::Backend,
            grade,
            confirmation: 3,
            fsp_verified: false,
            skills: [
                ("rust", required, Grade::Middle),
                ("postgresql", desired, Grade::Junior),
            ]
            .into_iter()
            .map(|(skill_id, confirmed, grade)| SkillEvidence {
                skill_id: skill_id.into(),
                status: if confirmed {
                    EvidenceStatus::Confirmed
                } else {
                    EvidenceStatus::Insufficient
                },
                evidence_grade: confirmed.then_some(grade),
                sources: vec![EvidenceSource::Assessment],
            })
            .collect(),
        }
    }

    fn need() -> HiringNeed {
        HiringNeed {
            specialization: Specialization::Backend,
            grade: Grade::Middle,
            requirements: vec![
                Requirement {
                    skill_id: "rust".into(),
                    criticality: Criticality::Required,
                    minimum_grade: Grade::Middle,
                    evidence_policy: assessment_policy(),
                },
                Requirement {
                    skill_id: "postgresql".into(),
                    criticality: Criticality::Desired,
                    minimum_grade: Grade::Junior,
                    evidence_policy: assessment_policy(),
                },
            ],
        }
    }

    fn assessment_policy() -> EvidencePolicy {
        EvidencePolicy {
            accept_assessments: true,
            accept_github: false,
            accept_fsp: false,
            competition_selection: "system".into(),
            competition_ids: Vec::new(),
        }
    }

    #[test]
    fn ranking_is_stable_and_prioritizes_match_components() {
        let candidates = vec![
            candidate("candidate-z", Grade::Middle, true, false),
            candidate("candidate-b", Grade::Middle, true, true),
            candidate("candidate-a", Grade::Middle, true, true),
            candidate("candidate-low-grade", Grade::Junior, true, true),
        ];
        let forward = rank_candidates(&candidates, &need());
        let reversed = rank_candidates(&candidates.into_iter().rev().collect::<Vec<_>>(), &need());
        let ids = forward
            .iter()
            .map(|item| item.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            [
                "candidate-a",
                "candidate-b",
                "candidate-z",
                "candidate-low-grade"
            ]
        );
        assert_eq!(forward, reversed);
    }

    #[test]
    fn experiment_record_matches_the_implemented_version() {
        let record: serde_json::Value = serde_json::from_str(include_str!(
            "../../../experiments/candidate-ranking-v1.json"
        ))
        .expect("experiment record is valid JSON");
        assert_eq!(record["algorithmVersion"], MATCHING_ALGORITHM_VERSION);
        assert_eq!(record["expectedOrder"][0], "candidate-a");
        assert_eq!(
            record["failureCondition"],
            "order_changes_when_input_is_reversed"
        );
    }
}
