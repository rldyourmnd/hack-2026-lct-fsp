use std::collections::BTreeMap;

use chrono::DateTime;

use crate::{
    ApplicationView, HiringBottleneckView, HiringFlowFacts, HiringFlowStageView, VacancyFlowView,
};

pub(crate) fn hiring_flow(items: &[ApplicationView]) -> HiringFlowFacts {
    let total = items.len();
    let reviewed = items.iter().filter(|item| item.viewed_at.is_some()).count();
    let decided = items
        .iter()
        .filter(|item| item.responded_at.is_some())
        .count();
    let accepted = items
        .iter()
        .filter(|item| item.status == fsp_domain::InteractionStatus::Accepted)
        .count();
    let stages = vec![
        stage("received", "Получены", total, total),
        stage("reviewed", "Просмотрены", reviewed, total),
        stage("decided", "Получили решение", decided, total),
        stage("accepted", "Приняты", accepted, total),
    ];
    let median_time_to_view_hours = median_hours(
        items
            .iter()
            .filter_map(|item| elapsed_hours(&item.created_at, item.viewed_at.as_deref())),
    );
    let median_time_to_decision_hours = median_hours(
        items
            .iter()
            .filter_map(|item| elapsed_hours(&item.created_at, item.responded_at.as_deref())),
    );
    let vacancies = vacancy_breakdown(items);
    let mut bottlenecks = Vec::new();
    let awaiting_review = total.saturating_sub(reviewed);
    if total == 0 {
        bottlenecks.push(HiringBottleneckView {
            kind: "no_data",
            severity: "info",
            title: "Пока нечего сравнивать".into(),
            evidence: "По опубликованным вакансиям ещё нет откликов.".into(),
            recommendation: "Проверьте видимость вакансий и вернитесь после первых откликов."
                .into(),
        });
    } else if awaiting_review >= 3 && percent(awaiting_review, total) >= 25 {
        bottlenecks.push(HiringBottleneckView {
            kind: "review_queue",
            severity: "high",
            title: "Отклики накапливаются до просмотра".into(),
            evidence: format!(
                "Не просмотрено {awaiting_review} из {total} откликов ({}%).",
                percent(awaiting_review, total)
            ),
            recommendation:
                "Назначьте владельца первичного разбора и ежедневное окно для новых откликов."
                    .into(),
        });
    }
    let awaiting_decision = reviewed.saturating_sub(decided);
    if reviewed >= 2 && percent(awaiting_decision, reviewed) >= 40 {
        bottlenecks.push(HiringBottleneckView {
            kind: "decision_queue",
            severity: "high",
            title: "Просмотр не превращается в решение".into(),
            evidence: format!(
                "Без решения остаются {awaiting_decision} из {reviewed} просмотренных откликов ({}%).",
                percent(awaiting_decision, reviewed)
            ),
            recommendation: "Зафиксируйте критерии следующего шага и срок решения после просмотра.".into(),
        });
    }
    if decided >= 3 && percent(accepted, decided) < 20 {
        bottlenecks.push(HiringBottleneckView {
            kind: "low_conversion",
            severity: "medium",
            title: "Мало положительных решений".into(),
            evidence: format!(
                "Принято {accepted} из {decided} решений ({}%).",
                percent(accepted, decided)
            ),
            recommendation:
                "Сверьте требования вакансии с профилями входящего потока и источниками кандидатов."
                    .into(),
        });
    }
    if bottlenecks.is_empty() {
        bottlenecks.push(HiringBottleneckView {
            kind: "stable",
            severity: "info",
            title: "Явного узкого места пока нет".into(),
            evidence: "Ни один этап не пересёк порог операционного риска.".into(),
            recommendation: "Следите за временем до просмотра и решения по мере роста потока."
                .into(),
        });
    }
    HiringFlowFacts {
        total,
        stages,
        median_time_to_view_hours,
        median_time_to_decision_hours,
        acceptance_rate_percent: percent(accepted, decided),
        vacancies,
        bottlenecks,
    }
}

fn vacancy_breakdown(items: &[ApplicationView]) -> Vec<VacancyFlowView> {
    let mut groups: BTreeMap<(&str, &str), Vec<&ApplicationView>> = BTreeMap::new();
    for item in items {
        groups
            .entry((&item.vacancy.id, &item.vacancy.title))
            .or_default()
            .push(item);
    }
    groups
        .into_iter()
        .map(|((vacancy_id, title), items)| {
            let total = items.len();
            let required_met = items
                .iter()
                .map(|item| item.r#match.required_met)
                .sum::<usize>();
            let required_total = items
                .iter()
                .map(|item| item.r#match.required_total)
                .sum::<usize>();
            VacancyFlowView {
                vacancy_id: vacancy_id.into(),
                title: title.into(),
                total,
                awaiting_review: items.iter().filter(|item| item.viewed_at.is_none()).count(),
                awaiting_decision: items
                    .iter()
                    .filter(|item| item.viewed_at.is_some() && item.responded_at.is_none())
                    .count(),
                accepted: items
                    .iter()
                    .filter(|item| item.status == fsp_domain::InteractionStatus::Accepted)
                    .count(),
                closed: items.iter().filter(|item| item.status.terminal()).count(),
                required_coverage_percent: percent(required_met, required_total),
                grade_match_percent: percent(
                    items
                        .iter()
                        .filter(|item| item.r#match.grade_matches)
                        .count(),
                    total,
                ),
            }
        })
        .collect()
}

fn stage(id: &'static str, label: &'static str, count: usize, total: usize) -> HiringFlowStageView {
    HiringFlowStageView {
        id,
        label,
        count,
        share_percent: percent(count, total),
    }
}

fn percent(value: usize, total: usize) -> u8 {
    value
        .saturating_mul(100)
        .saturating_add(total / 2)
        .checked_div(total)
        .unwrap_or(0)
        .min(100) as u8
}

fn elapsed_hours(start: &str, end: Option<&str>) -> Option<u32> {
    let start = DateTime::parse_from_rfc3339(start).ok()?;
    let end = DateTime::parse_from_rfc3339(end?).ok()?;
    u32::try_from((end - start).num_minutes().max(0) / 60).ok()
}

fn median_hours(values: impl Iterator<Item = u32>) -> Option<u32> {
    let mut values = values.collect::<Vec<_>>();
    values.sort_unstable();
    let middle = values.len() / 2;
    match values.len() {
        0 => None,
        length if length % 2 == 1 => values.get(middle).copied(),
        _ => Some((values[middle - 1] + values[middle]) / 2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApplicationVacancyView, CandidateMatchView, CompanyCardView, SalaryView};
    use fsp_domain::{Grade, InteractionStatus, Specialization};

    fn application(index: usize, viewed: bool, decided: bool) -> ApplicationView {
        ApplicationView {
            id: format!("application-{index}"),
            candidate_id: format!("candidate-{index}"),
            candidate_alias: format!("Кандидат {index}"),
            company: CompanyCardView {
                id: "company-1".into(),
                display_name: "Компания".into(),
                business_area: "IT".into(),
                description: String::new(),
                contact_email: "hr@example.com".into(),
            },
            vacancy: ApplicationVacancyView {
                id: "vacancy-1".into(),
                title: "Backend".into(),
                specialization: Specialization::Backend,
                grade: Grade::Middle,
                work_mode: "remote".into(),
                salary: SalaryView {
                    from: 1,
                    to: 2,
                    currency: "RUB",
                    period: "month",
                    tax_basis: "gross",
                },
                description: String::new(),
            },
            message: String::new(),
            status: if decided {
                InteractionStatus::Declined
            } else if viewed {
                InteractionStatus::Viewed
            } else {
                InteractionStatus::Sent
            },
            created_at: "2026-10-01T00:00:00Z".into(),
            viewed_at: viewed.then(|| "2026-10-01T04:00:00Z".into()),
            responded_at: decided.then(|| "2026-10-02T00:00:00Z".into()),
            disclosure_fields: Vec::new(),
            r#match: CandidateMatchView {
                need_id: "vacancy-1".into(),
                status: "partial",
                grade_matches: true,
                required_met: 1,
                required_total: 2,
                desired_met: 0,
                desired_total: 0,
                requirements: Vec::new(),
                explanation: String::new(),
                algorithm_version: "test",
            },
        }
    }

    #[test]
    fn detects_review_queue_without_changing_counts() {
        let items = (0..4)
            .map(|index| application(index, index == 3, false))
            .collect::<Vec<_>>();
        let result = hiring_flow(&items);
        assert_eq!(
            result
                .stages
                .iter()
                .map(|stage| stage.count)
                .collect::<Vec<_>>(),
            vec![4, 1, 0, 0]
        );
        assert_eq!(result.bottlenecks[0].kind, "review_queue");
        assert_eq!(result.vacancies[0].total, 4);
    }

    #[test]
    fn empty_and_malformed_timestamps_do_not_invent_durations() {
        let empty = hiring_flow(&[]);
        assert_eq!(empty.total, 0);
        assert_eq!(empty.bottlenecks[0].kind, "no_data");
        assert_eq!(empty.median_time_to_view_hours, None);

        let mut malformed = application(1, true, true);
        malformed.viewed_at = Some("not-a-date".into());
        malformed.responded_at = Some("also-not-a-date".into());
        let result = hiring_flow(&[malformed]);
        assert_eq!(result.median_time_to_view_hours, None);
        assert_eq!(result.median_time_to_decision_hours, None);
    }
}
