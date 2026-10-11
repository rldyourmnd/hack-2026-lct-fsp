//! Versioned knowledge tests. The private answer key never enters the public task DTO.
use std::{collections::HashSet, sync::LazyLock};

use serde::{Deserialize, Serialize};

use crate::{
    AssessmentAnswer, AssessmentCheck, AssessmentTask, Grade, RubricCriterion, RubricOutcome,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnowledgeOption {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeQuestion {
    pub id: String,
    pub skill_id: String,
    pub prompt: String,
    pub options: Vec<KnowledgeOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnowledgeResponse {
    pub question_id: String,
    pub option_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BankQuestion {
    #[serde(flatten)]
    question: KnowledgeQuestion,
    /// `None` in the topic banks: the question serves every specialization tested on its topic.
    #[serde(default)]
    specialization: Option<crate::Specialization>,
    grade: Grade,
    correct_option_id: String,
    explanation: String,
    source: String,
}

#[derive(Deserialize)]
struct Bank {
    version: String,
    questions: Vec<BankQuestion>,
}

static BANK: LazyLock<Bank> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/knowledge-v1.json"))
        .expect("versioned knowledge bank must be valid")
});

static V2_ADDITIONS: LazyLock<Bank> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/knowledge-v2-additions.json"))
        .expect("knowledge-v2 additions must be valid")
});

/// Topic banks: questions per catalog skill and grade, shared by the specializations tested on them.
static TOPIC_BANKS: LazyLock<Vec<Bank>> = LazyLock::new(|| {
    [
        include_str!("../data/knowledge-v3-data.json"),
        include_str!("../data/knowledge-v3-enterprise.json"),
        include_str!("../data/knowledge-v3-infrastructure.json"),
        include_str!("../data/knowledge-v3-mobile.json"),
        include_str!("../data/knowledge-v3-product.json"),
        include_str!("../data/knowledge-v3-quality.json"),
        include_str!("../data/knowledge-v3-systems.json"),
        include_str!("../data/knowledge-v3-web.json"),
    ]
    .into_iter()
    .map(|text| serde_json::from_str(text).expect("knowledge topic banks must be valid"))
    .collect()
});

/// Questions a test of this specialization and grade may draw from. The first bank has no
/// specialization field and belongs to backend; topic-bank questions belong to their topic.
fn question_pool(
    specialization: crate::Specialization,
    grade: Grade,
) -> impl Iterator<Item = &'static BankQuestion> {
    BANK.questions
        .iter()
        .chain(V2_ADDITIONS.questions.iter())
        .filter(move |item| {
            item.specialization
                .unwrap_or(crate::Specialization::Backend)
                == specialization
        })
        .chain(
            TOPIC_BANKS
                .iter()
                .flat_map(|bank| bank.questions.iter())
                .filter(|item| item.specialization.is_none()),
        )
        .filter(move |item| item.grade == grade)
}

const V2_VERSION: &str = "knowledge-v2";

/// The two catalog topics of a specialization's test with their labels.
fn topics(specialization: crate::Specialization) -> [(&'static str, &'static str); 2] {
    let [first, second] = &crate::catalog::specialization(specialization).test_topics;
    [
        (first.as_str(), crate::catalog::skill_label(first)),
        (second.as_str(), crate::catalog::skill_label(second)),
    ]
}

fn stable_hash(value: &str) -> u64 {
    value.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

fn v2_questions(
    specialization: crate::Specialization,
    grade: Grade,
    variant_key: &str,
) -> Option<Vec<KnowledgeQuestion>> {
    let topics = topics(specialization);
    let mut selected = Vec::with_capacity(10);
    for (topic_id, _) in topics {
        let mut questions = question_pool(specialization, grade)
            .filter(|item| item.question.skill_id == topic_id)
            .collect::<Vec<_>>();
        questions.sort_by_key(|item| {
            stable_hash(&format!("{variant_key}:{topic_id}:{}", item.question.id))
        });
        if questions.len() < 5 {
            return None;
        }
        selected.extend(
            questions
                .into_iter()
                .take(5)
                .map(|item| item.question.clone()),
        );
    }
    Some(selected)
}

fn legacy_knowledge_task(grade: Grade) -> AssessmentTask {
    build_task(
        crate::Specialization::Backend,
        grade,
        BANK.version.clone(),
        format!("backend-{}-v1", grade.as_str()),
        BANK.questions
            .iter()
            .filter(|q| q.grade == grade)
            .map(|q| q.question.clone())
            .collect(),
    )
}

fn build_task(
    specialization: crate::Specialization,
    grade: Grade,
    version: String,
    variant_id: String,
    questions: Vec<KnowledgeQuestion>,
) -> AssessmentTask {
    let topic_list = topics(specialization);
    let specialization_id = specialization.as_str();
    let title = format!("Тест знаний: {} и {}", topic_list[0].1, topic_list[1].1);
    AssessmentTask {
        id: format!("knowledge-{specialization_id}-{}-{version}", grade.as_str()),
        family: format!("{specialization_id}-knowledge"),
        version,
        variant_id,
        title,
        prompt: "Выберите один ответ в каждом из 10 вопросов. Для зачёта нужны минимум 8 верных ответов из 10 и минимум 3 из 5 по каждой теме. ИИ и справочники разрешены. Результат показывает знания по этому тесту, а не опыт написания кода.".into(),
        language: specialization_id.into(),
        starter_code: String::new(),
        acceptance_criteria: vec!["Не менее 8 верных ответов из 10".into(), format!("Не менее 3 из 5 по {} и 3 из 5 по {}", topic_list[0].1, topic_list[1].1)],
        rubric: topic_list.iter().map(|(id, label)| RubricCriterion {
            id: (*id).into(), label: (*label).into(), weight: 5,
        }).collect(),
        ai_allowed: true,
        external_ai_allowed: true,
        execution: "questionnaire".into(),
        questions,
    }
}

/// Whether the bank holds a test for this specialization and grade: five questions per topic.
#[must_use]
pub fn knowledge_available(specialization: crate::Specialization, grade: Grade) -> bool {
    v2_questions(specialization, grade, "availability").is_some()
}

/// Every specialization and grade the bank can test, in catalog order.
#[must_use]
pub fn knowledge_scopes() -> Vec<(crate::Specialization, Vec<Grade>)> {
    crate::Specialization::ALL
        .into_iter()
        .filter_map(|specialization| {
            let grades = Grade::ALL
                .into_iter()
                .filter(|grade| knowledge_available(specialization, *grade))
                .collect::<Vec<_>>();
            (!grades.is_empty()).then_some((specialization, grades))
        })
        .collect()
}

#[must_use]
pub fn knowledge_task(
    specialization: crate::Specialization,
    grade: Grade,
    variant_key: &str,
) -> Option<AssessmentTask> {
    let questions = v2_questions(specialization, grade, variant_key)?;
    let fingerprint = questions.iter().fold(0xcbf29ce484222325, |hash, question| {
        (hash ^ stable_hash(&question.id)).wrapping_mul(0x100000001b3)
    });
    let specialization_id = specialization.as_str();
    Some(build_task(
        specialization,
        grade,
        V2_VERSION.into(),
        format!(
            "{specialization_id}-{}-v2-{fingerprint:016x}",
            grade.as_str()
        ),
        questions,
    ))
}

/// Reject foreign, duplicated and malformed selections even when saving a partial draft.
#[must_use]
pub fn valid_knowledge_answer(task: &AssessmentTask, answer: &AssessmentAnswer) -> bool {
    if !answer.code.is_empty()
        || !answer.explanation.is_empty()
        || answer.responses.len() > task.questions.len()
    {
        return false;
    }
    let mut seen = HashSet::new();
    answer.responses.iter().all(|r| {
        seen.insert(&r.question_id)
            && task
                .questions
                .iter()
                .any(|q| q.id == r.question_id && q.options.iter().any(|o| o.id == r.option_id))
    })
}

#[must_use]
pub fn knowledge_completion(
    task: &AssessmentTask,
    answer: &AssessmentAnswer,
) -> Vec<AssessmentCheck> {
    task.questions
        .iter()
        .map(|q| {
            let chosen = answer.responses.iter().any(|r| r.question_id == q.id);
            AssessmentCheck {
                id: q.id.clone(),
                label: q.prompt.clone(),
                status: if chosen { "passed" } else { "not_run" }.into(),
                message: if chosen {
                    "Ответ выбран; правильность проверяется после отправки"
                } else {
                    "Выберите ответ"
                }
                .into(),
            }
        })
        .collect()
}

pub struct KnowledgeScore {
    pub passed: bool,
    pub checks: Vec<AssessmentCheck>,
    pub rubric: Vec<RubricOutcome>,
    pub feedback: String,
}

/// Only the exact immutable version can be scored. Unknown versions fail closed.
#[must_use]
pub fn score_knowledge(
    specialization: crate::Specialization,
    grade: Grade,
    task: &AssessmentTask,
    answer: &AssessmentAnswer,
) -> Option<KnowledgeScore> {
    let selected_bank = if task.version == BANK.version {
        if specialization != crate::Specialization::Backend || task != &legacy_knowledge_task(grade)
        {
            return None;
        }
        &BANK.questions
    } else if task.version == V2_VERSION {
        let topic_list = topics(specialization);
        let selected = task
            .questions
            .iter()
            .map(|question| question.id.as_str())
            .collect::<HashSet<_>>();
        let bank_questions = question_pool(specialization, grade)
            .filter(|item| selected.contains(item.question.id.as_str()))
            .collect::<Vec<_>>();
        if bank_questions.len() != 10
            || topic_list.iter().any(|(topic, _)| {
                bank_questions
                    .iter()
                    .filter(|item| item.question.skill_id == *topic)
                    .count()
                    != 5
            })
            || bank_questions
                .iter()
                .any(|item| !task.questions.contains(&item.question))
        {
            return None;
        }
        return score_selected(specialization, task, answer, bank_questions);
    } else {
        return None;
    };
    if !valid_knowledge_answer(task, answer) || answer.responses.len() != task.questions.len() {
        return None;
    }
    score_selected(
        specialization,
        task,
        answer,
        selected_bank.iter().filter(|q| q.grade == grade).collect(),
    )
}

fn score_selected(
    specialization: crate::Specialization,
    task: &AssessmentTask,
    answer: &AssessmentAnswer,
    questions: Vec<&BankQuestion>,
) -> Option<KnowledgeScore> {
    if !valid_knowledge_answer(task, answer) || answer.responses.len() != 10 {
        return None;
    }
    let topic_list = topics(specialization);
    let mut topic_scores = [0_u32; 2];
    let checks = questions
        .into_iter()
        .map(|q| {
            let correct = answer
                .responses
                .iter()
                .any(|r| r.question_id == q.question.id && r.option_id == q.correct_option_id);
            if correct
                && let Some(index) = topic_list
                    .iter()
                    .position(|(id, _)| *id == q.question.skill_id)
            {
                topic_scores[index] += 1;
            }
            let label = q
                .question
                .options
                .iter()
                .find(|o| o.id == q.correct_option_id)
                .map_or("", |o| o.label.as_str());
            AssessmentCheck {
                id: q.question.id.clone(),
                label: q.question.prompt.clone(),
                status: if correct { "passed" } else { "failed" }.into(),
                message: format!(
                    "Верный ответ: {label}. {} Источник: {}",
                    q.explanation, q.source
                ),
            }
        })
        .collect();
    let total: u32 = topic_scores.iter().sum();
    Some(KnowledgeScore {
        passed: total >= 8 && topic_scores.iter().all(|count| *count >= 3),
        checks,
        rubric: topic_list
            .iter()
            .zip(topic_scores)
            .map(|((id, label), count)| RubricOutcome {
                criterion_id: (*id).into(),
                label: (*label).into(),
                outcome: if count >= 3 { "met" } else { "not_met" }.into(),
                feedback: format!("{count} из 5 верных ответов; порог по теме — 3."),
            })
            .collect(),
        feedback: format!(
            "Верных ответов: {total} из 10. {}: {}/5; {}: {}/5. Для зачёта нужны 8/10 и не менее 3/5 по каждой теме. Проверены знания по тесту версии {}; практический опыт разработки не оценивался. ИИ и справочники разрешены.",
            topic_list[0].1, topic_scores[0], topic_list[1].1, topic_scores[1], task.version
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_bank_integrity_and_private_answer_key() {
        for grade in [Grade::Junior, Grade::Middle, Grade::Senior] {
            let task = legacy_knowledge_task(grade);
            assert_eq!(task.questions.len(), 10);
            let wire = serde_json::to_string(&task).unwrap();
            assert!(
                !wire.contains("correctOptionId")
                    && !wire.contains("explanation")
                    && !wire.contains("https://")
            );
            let mut ids = HashSet::new();
            for question in BANK.questions.iter().filter(|q| q.grade == grade) {
                assert!(ids.insert(&question.question.id));
                assert_eq!(question.question.options.len(), 3);
                assert_eq!(
                    question
                        .question
                        .options
                        .iter()
                        .filter(|o| o.id == question.correct_option_id)
                        .count(),
                    1
                );
                assert!(
                    !question.explanation.is_empty() && question.source.starts_with("https://")
                );
            }
            let mut answer = AssessmentAnswer {
                code: String::new(),
                explanation: String::new(),
                external_ai_used: false,
                responses: vec![],
            };
            assert!(
                score_knowledge(crate::Specialization::Backend, grade, &task, &answer).is_none()
            );
            answer.responses = BANK
                .questions
                .iter()
                .filter(|q| q.grade == grade)
                .map(|q| KnowledgeResponse {
                    question_id: q.question.id.clone(),
                    option_id: q.correct_option_id.clone(),
                })
                .collect();
            assert!(
                score_knowledge(crate::Specialization::Backend, grade, &task, &answer)
                    .unwrap()
                    .passed
            );
            for (index, item) in answer.responses.iter_mut().enumerate() {
                item.option_id = task.questions[index]
                    .options
                    .iter()
                    .find(|o| o.id != item.option_id)
                    .unwrap()
                    .id
                    .clone();
            }
            assert!(
                !score_knowledge(crate::Specialization::Backend, grade, &task, &answer)
                    .unwrap()
                    .passed
            );
            answer.responses[1] = answer.responses[0].clone();
            assert!(!valid_knowledge_answer(&task, &answer));
        }
    }

    #[test]
    fn v2_variants_are_reproducible_distinct_and_balanced() {
        let scopes = [
            (crate::Specialization::Backend, Grade::Junior),
            (crate::Specialization::Backend, Grade::Middle),
            (crate::Specialization::Backend, Grade::Senior),
            (crate::Specialization::Frontend, Grade::Junior),
            (crate::Specialization::Frontend, Grade::Middle),
        ];
        for (specialization, grade) in scopes {
            let first = knowledge_task(specialization, grade, "candidate-a").unwrap();
            let repeated = knowledge_task(specialization, grade, "candidate-a").unwrap();
            let second = knowledge_task(specialization, grade, "candidate-b").unwrap();
            assert_eq!(first, repeated);
            assert_ne!(first.variant_id, second.variant_id);
            assert_eq!(first.questions.len(), 10);
            for (topic, _) in topics(specialization) {
                assert_eq!(
                    first
                        .questions
                        .iter()
                        .filter(|question| question.skill_id == topic)
                        .count(),
                    5
                );
            }
            let wire = serde_json::to_string(&first).unwrap();
            assert!(
                !wire.contains("correctOptionId")
                    && !wire.contains("explanation")
                    && !wire.contains("https://")
            );
        }
        // Every catalog specialization is testable at every grade.
        assert_eq!(knowledge_scopes().len(), crate::Specialization::ALL.len());
        for (_, grades) in knowledge_scopes() {
            assert_eq!(grades, Grade::ALL);
        }
    }

    fn bank_question(id: &str) -> &'static BankQuestion {
        BANK.questions
            .iter()
            .chain(V2_ADDITIONS.questions.iter())
            .chain(TOPIC_BANKS.iter().flat_map(|bank| bank.questions.iter()))
            .find(|item| item.question.id == id)
            .unwrap()
    }

    #[test]
    fn topic_banks_are_complete_consistent_and_not_guessable_by_length() {
        let mut ids = HashSet::new();
        let mut longest = 0;
        let mut total = 0;
        let mut counts = std::collections::BTreeMap::<(&str, Grade), usize>::new();
        let topics = crate::catalog::catalog()
            .specializations
            .iter()
            .flat_map(|item| item.test_topics.iter().map(String::as_str))
            .collect::<HashSet<_>>();
        for item in BANK
            .questions
            .iter()
            .chain(V2_ADDITIONS.questions.iter())
            .chain(TOPIC_BANKS.iter().flat_map(|bank| bank.questions.iter()))
        {
            assert!(
                ids.insert(item.question.id.as_str()),
                "{}",
                item.question.id
            );
            assert_eq!(item.question.options.len(), 3, "{}", item.question.id);
            let correct = item
                .question
                .options
                .iter()
                .filter(|option| option.id == item.correct_option_id)
                .collect::<Vec<_>>();
            assert_eq!(correct.len(), 1, "{}", item.question.id);
            assert!(!item.explanation.is_empty() && item.source.starts_with("https://"));
            assert!(topics.contains(item.question.skill_id.as_str()));
            *counts
                .entry((item.question.skill_id.as_str(), item.grade))
                .or_default() += 1;
            if item.specialization.is_none() {
                total += 1;
                let correct_length = correct[0].label.chars().count();
                if item
                    .question
                    .options
                    .iter()
                    .filter(|option| option.id != item.correct_option_id)
                    .all(|option| option.label.chars().count() < correct_length)
                {
                    longest += 1;
                }
            }
        }
        for topic in &topics {
            for grade in Grade::ALL {
                assert!(
                    counts.get(&(*topic, grade)).copied().unwrap_or(0) >= 6,
                    "{topic} {}",
                    grade.as_str()
                );
            }
        }
        // Picking the longest option must not pass the test: at most half of the answers are longest.
        assert!(longest * 2 <= total, "{longest} of {total}");
    }

    #[test]
    fn v2_scores_backend_and_frontend_without_exposing_the_key() {
        for (specialization, grade) in [
            (crate::Specialization::Backend, Grade::Senior),
            (crate::Specialization::Frontend, Grade::Middle),
            (crate::Specialization::Onec, Grade::Middle),
            (crate::Specialization::Dba, Grade::Lead),
            (crate::Specialization::DataAnalytics, Grade::Intern),
        ] {
            let task = knowledge_task(specialization, grade, "scoring-test").unwrap();
            let responses = task
                .questions
                .iter()
                .map(|question| {
                    let bank_question = bank_question(&question.id);
                    KnowledgeResponse {
                        question_id: question.id.clone(),
                        option_id: bank_question.correct_option_id.clone(),
                    }
                })
                .collect();
            let answer = AssessmentAnswer {
                code: String::new(),
                explanation: String::new(),
                external_ai_used: false,
                responses,
            };
            assert!(
                score_knowledge(specialization, grade, &task, &answer)
                    .unwrap()
                    .passed
            );

            let wrong = AssessmentAnswer {
                responses: task
                    .questions
                    .iter()
                    .map(|question| KnowledgeResponse {
                        question_id: question.id.clone(),
                        option_id: question
                            .options
                            .iter()
                            .find(|option| {
                                answer.responses.iter().any(|response| {
                                    response.question_id == question.id
                                        && response.option_id != option.id
                                })
                            })
                            .unwrap()
                            .id
                            .clone(),
                    })
                    .collect(),
                ..answer
            };
            assert!(
                !score_knowledge(specialization, grade, &task, &wrong)
                    .unwrap()
                    .passed
            );
        }
    }
}
