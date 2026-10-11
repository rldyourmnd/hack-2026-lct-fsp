//! The competence graph: the candidate's category, the concepts (catalog skill categories) it
//! covers, the technologies in each with their level, and the sources behind them.
//!
//! The graph is built from stored facts only, so it is deterministic and complete: every skill of
//! the profile appears once, under its concept. Employers see no login, links or evidence titles.
use std::collections::BTreeMap;

use super::*;

impl PlatformService {
    pub async fn my_graph(&self, session: &SessionView) -> Result<GraphView, ApplicationError> {
        let candidate_id = candidate_id(session)?;
        let record = self.candidate_index(candidate_id).await?;
        let sources = self
            .repository
            .sources(candidate_id)
            .await
            .map_err(ApplicationError::storage)?;
        Ok(competence_graph("self", &record, &sources))
    }

    pub async fn candidate_graph(
        &self,
        session: &SessionView,
        requested_candidate_id: &str,
    ) -> Result<GraphView, ApplicationError> {
        employer_company_id(session)?;
        let record = self.candidate_index(requested_candidate_id).await?;
        if !record.profile.visible_to_employers(record.email_verified) {
            return Err(ApplicationError::new(
                404,
                "not_found",
                "Кандидат не найден",
            ));
        }
        let sources = self
            .repository
            .sources(requested_candidate_id)
            .await
            .map_err(ApplicationError::storage)?;
        Ok(competence_graph("employer", &record, &sources))
    }
}

fn node(id: String, kind: &str, label: String, detail: Option<String>) -> GraphNodeView {
    GraphNodeView {
        id,
        kind: kind.into(),
        label,
        skill_id: None,
        status: None,
        detail,
    }
}

fn edge(from: &str, to: &str, relation: &str) -> GraphEdgeView {
    GraphEdgeView {
        id: format!("edge-{from}-{to}"),
        from: from.into(),
        to: to.into(),
        relation: relation.into(),
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

/// The level of a technology: a test-confirmed grade, otherwise how widely the public work uses it.
fn skill_level(skill: &fsp_domain::SkillEvidence, repositories: Option<u32>) -> String {
    if let (EvidenceStatus::Confirmed, Some(grade)) = (skill.status, skill.evidence_grade) {
        return format!("Тест · {}", grade.label());
    }
    match repositories {
        Some(count @ 10..) => format!(
            "Основная · {count} {}",
            plural(count, ("проект", "проекта", "проектов"))
        ),
        Some(count @ 3..) => format!(
            "Регулярно · {count} {}",
            plural(count, ("проект", "проекта", "проектов"))
        ),
        Some(count @ 1..) => format!(
            "Встречается · {count} {}",
            plural(count, ("проект", "проекта", "проектов"))
        ),
        _ => match skill.status {
            EvidenceStatus::Observed => "Указан в анкете".into(),
            EvidenceStatus::Insufficient => "Нужно подтверждение".into(),
            _ => "Есть свидетельства".into(),
        },
    }
}

fn competence_graph(
    audience: &'static str,
    record: &CandidateIndexRecord,
    sources: &[fsp_domain::SourceRecord],
) -> GraphView {
    let catalog = fsp_domain::catalog::catalog();
    let profile = &record.profile;
    let report = sources
        .iter()
        .filter(|source| source.provider == fsp_domain::SourceProvider::Github)
        .find_map(|source| source.analysis.as_ref()?.report.as_ref());
    let repositories: BTreeMap<&str, u32> = report
        .map(|report| {
            report
                .technologies
                .iter()
                .map(|item| (item.skill_id.as_str(), item.repositories))
                .collect()
        })
        .unwrap_or_default();

    let category = profile.category();
    let role_label = category.map_or_else(
        || {
            fsp_domain::catalog::specialization(profile.specialization)
                .role
                .clone()
        },
        |category| {
            format!(
                "{} · {}",
                fsp_domain::catalog::specialization(category.specialization).short,
                category.grade.label()
            )
        },
    );
    let role_detail = match category.map(|category| category.source) {
        Some(fsp_domain::CategorySource::Assessment) => "Подтверждено тестом",
        Some(fsp_domain::CategorySource::Estimate) => "Оценка по открытым данным",
        None => "Категории пока нет",
    };
    let mut nodes = vec![node(
        "role-main".into(),
        "role",
        role_label,
        Some(role_detail.into()),
    )];
    let mut edges = Vec::new();

    // Concepts in catalog order, technologies inside them by level and scale.
    let mut concepts: BTreeMap<usize, Vec<&fsp_domain::SkillEvidence>> = BTreeMap::new();
    for skill in &record.skills {
        let category = fsp_domain::catalog::skill(&skill.skill_id)
            .map_or("practice", |info| info.category.as_str());
        let index = catalog
            .skill_categories
            .iter()
            .position(|item| item.id == category)
            .unwrap_or(usize::MAX);
        concepts.entry(index).or_default().push(skill);
    }
    let mut supported: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (index, mut skills) in concepts {
        let (concept_key, concept_label) = catalog
            .skill_categories
            .get(index)
            .map_or(("other", "Другое"), |item| {
                (item.id.as_str(), item.label.as_str())
            });
        let concept_id = format!("concept-{concept_key}");
        skills.sort_by(|a, b| {
            let rank = |skill: &fsp_domain::SkillEvidence| {
                (
                    skill.status != EvidenceStatus::Confirmed,
                    std::cmp::Reverse(
                        repositories
                            .get(skill.skill_id.as_str())
                            .copied()
                            .unwrap_or(0),
                    ),
                )
            };
            rank(a).cmp(&rank(b)).then(a.skill_id.cmp(&b.skill_id))
        });
        let count = skills.len() as u32;
        nodes.push(node(
            concept_id.clone(),
            "concept",
            concept_label.into(),
            Some(format!(
                "{count} {}",
                plural(count, ("навык", "навыка", "навыков"))
            )),
        ));
        edges.push(edge("role-main", &concept_id, "uses"));
        for skill in skills {
            let id = format!("skill-{}", skill.skill_id);
            nodes.push(GraphNodeView {
                id: id.clone(),
                kind: "skill".into(),
                label: fsp_domain::catalog::skill_label(&skill.skill_id).into(),
                skill_id: Some(skill.skill_id.clone()),
                status: Some(evidence_status_name(skill.status).into()),
                detail: Some(skill_level(
                    skill,
                    repositories.get(skill.skill_id.as_str()).copied(),
                )),
            });
            edges.push(edge(&concept_id, &id, "uses"));
            for source in &skill.sources {
                let key = match source {
                    fsp_domain::EvidenceSource::Github => "source-github",
                    fsp_domain::EvidenceSource::Assessment => "source-assessment",
                    fsp_domain::EvidenceSource::Fsp => "source-fsp",
                };
                let concepts = supported.entry(key).or_default();
                if !concepts.contains(&concept_id) {
                    concepts.push(concept_id.clone());
                }
            }
        }
    }

    // Sources: GitHub and tests support the concepts they evidence; FSP supports the category.
    if let Some(report) = report {
        let detail = if audience == "self" {
            format!(
                "{} · {} {}",
                report.login,
                report.coverage.repositories_analyzed,
                plural(
                    report.coverage.repositories_analyzed,
                    ("репозиторий", "репозитория", "репозиториев")
                )
            )
        } else {
            "Публичные проекты".into()
        };
        nodes.push(node(
            "source-github".into(),
            "source",
            "GitHub".into(),
            Some(detail),
        ));
        for concept in supported.get("source-github").into_iter().flatten() {
            edges.push(edge("source-github", concept, "supports"));
        }
    }
    let published = sources
        .iter()
        .filter(|source| source.provider == fsp_domain::SourceProvider::Fsp)
        .flat_map(|source| &source.fsp_results)
        .filter(|result| result.status == "published")
        .collect::<Vec<_>>();
    if !published.is_empty() {
        let count = published.len() as u32;
        let best = published
            .iter()
            .filter_map(|result| result.place.map(|place| (place, result.field_size)))
            .min_by(|a, b| {
                (u64::from(a.0) * u64::from(b.1)).cmp(&(u64::from(b.0) * u64::from(a.1)))
            });
        let detail = match (audience, best) {
            ("self", Some((place, size))) => format!(
                "{count} {} · лучшее: {place} из {size}",
                plural(count, ("результат", "результата", "результатов"))
            ),
            _ => format!(
                "{count} {}",
                plural(count, ("результат", "результата", "результатов"))
            ),
        };
        nodes.push(node(
            "source-fsp".into(),
            "source",
            "ФСП".into(),
            Some(detail),
        ));
        edges.push(edge("source-fsp", "role-main", "supports"));
    }
    if let Some(concepts) = supported.get("source-assessment") {
        let detail = profile.verified_grade.map_or_else(
            || "Подтверждённые навыки".into(),
            |grade| format!("Подтверждён {}", grade.label()),
        );
        nodes.push(node(
            "source-assessment".into(),
            "source",
            "Тест знаний".into(),
            Some(detail),
        ));
        edges.push(edge("source-assessment", "role-main", "supports"));
        for concept in concepts {
            edges.push(edge("source-assessment", concept, "supports"));
        }
    }
    GraphView {
        audience,
        nodes,
        edges,
        simulation: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(
        id: &str,
        status: EvidenceStatus,
        sources: Vec<fsp_domain::EvidenceSource>,
    ) -> fsp_domain::SkillEvidence {
        fsp_domain::SkillEvidence {
            skill_id: id.into(),
            status,
            evidence_grade: (status == EvidenceStatus::Confirmed)
                .then_some(fsp_domain::Grade::Middle),
            sources,
        }
    }

    fn record(skills: Vec<fsp_domain::SkillEvidence>) -> CandidateIndexRecord {
        let profile = CandidateProfile {
            id: "candidate-1".into(),
            alias: "Кандидат №001".into(),
            email: "a@example.test".into(),
            name: "Анна".into(),
            specialization: fsp_domain::Specialization::Backend,
            selected_grade: fsp_domain::Grade::Middle,
            stack: vec!["rust".into()],
            team_role: "Разработчик".into(),
            experience_months: None,
            experience_since: None,
            experience: "Проекты".into(),
            soft_skills: Vec::new(),
            consents: fsp_domain::Consents {
                processing: true,
                publication: true,
            },
            verified_grade: Some(fsp_domain::Grade::Middle),
            verified_specialization: Some(fsp_domain::Specialization::Backend),
            grade_confirmed_at: None,
            estimate: None,
            auto_fields: Vec::new(),
            revision: 1,
            updated_at: "2026-10-10T00:00:00Z".into(),
        };
        CandidateIndexRecord {
            profile,
            email_verified: true,
            work_modes: Vec::new(),
            available_for_offers: true,
            skills,
            source_kinds: Vec::new(),
            fsp_verified: false,
            last_signal_at: None,
        }
    }

    #[test]
    fn every_skill_sits_under_its_concept_with_a_level_and_its_sources() {
        use fsp_domain::EvidenceSource::{Assessment, Github};
        let record = record(vec![
            skill("rust", EvidenceStatus::Confirmed, vec![Assessment, Github]),
            skill("docker", EvidenceStatus::Evidence, vec![Github]),
            skill("redis", EvidenceStatus::Insufficient, Vec::new()),
        ]);
        let graph = competence_graph("employer", &record, &[]);
        let ids = graph
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();
        for id in [
            "role-main",
            "concept-language",
            "concept-database",
            "concept-infrastructure",
            "skill-rust",
            "skill-docker",
            "skill-redis",
            "source-assessment",
        ] {
            assert!(ids.contains(&id), "{id}: {ids:?}");
        }
        let detail = |id: &str| {
            graph
                .nodes
                .iter()
                .find(|node| node.id == id)
                .and_then(|node| node.detail.clone())
                .unwrap()
        };
        assert_eq!(detail("role-main"), "Подтверждено тестом");
        assert_eq!(detail("skill-rust"), "Тест · Мидл");
        assert_eq!(detail("skill-redis"), "Нужно подтверждение");
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.from == "concept-language" && edge.to == "skill-rust")
        );
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.from == "source-assessment" && edge.to == "concept-language")
        );
        // Every edge joins existing nodes, once.
        for edge in &graph.edges {
            assert!(ids.contains(&edge.from.as_str()) && ids.contains(&edge.to.as_str()));
        }
        let mut edge_ids = graph
            .edges
            .iter()
            .map(|edge| edge.id.clone())
            .collect::<Vec<_>>();
        edge_ids.sort();
        edge_ids.dedup();
        assert_eq!(edge_ids.len(), graph.edges.len());
        assert_eq!(
            graph
                .nodes
                .iter()
                .filter(|node| node.kind == "skill")
                .count(),
            3
        );
    }
}
