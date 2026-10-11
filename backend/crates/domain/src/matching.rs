use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::{Grade, Specialization};

pub const MATCHING_ALGORITHM_VERSION: &str = "deterministic-rules-v1";

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
    fn accepts(&self, source: EvidenceSource) -> bool {
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
        algorithm_version: MATCHING_ALGORITHM_VERSION,
    }
}

fn requirement_met(candidate: &RankableCandidate, requirement: &Requirement) -> bool {
    candidate.skills.iter().any(|skill| {
        skill.skill_id == requirement.skill_id
            && skill.status == EvidenceStatus::Confirmed
            && skill
                .sources
                .iter()
                .any(|source| requirement.evidence_policy.accepts(*source))
            && skill
                .evidence_grade
                .is_some_and(|grade| grade >= requirement.minimum_grade)
    })
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
        .then_with(|| right.confirmation.cmp(&left.confirmation))
        .then_with(|| right.fsp_verified.cmp(&left.fsp_verified))
        .then_with(|| left.id.cmp(&right.id))
}

#[cfg(test)]
mod tests {
    use super::*;

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
