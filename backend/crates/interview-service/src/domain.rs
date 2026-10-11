use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompetencyKind {
    Technical,
    Behavioral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Unknown,
    Observed,
    Partial,
    Confirmed,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompetencyNode {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: CompetencyKind,
    pub criticality: u8,
    pub prerequisites: &'static [&'static str],
    pub question_templates: &'static [&'static str],
    pub concept_groups: &'static [&'static [&'static str]],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewBlueprint {
    pub id: &'static str,
    pub label: &'static str,
    pub start_node_id: &'static str,
    pub nodes: &'static [CompetencyNode],
}

impl InterviewBlueprint {
    pub fn node(&self, id: &str) -> Option<&CompetencyNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        let ids: BTreeSet<_> = self.nodes.iter().map(|node| node.id).collect();
        if self.nodes.is_empty()
            || !ids.contains(self.start_node_id)
            || ids.len() != self.nodes.len()
        {
            return Err("invalid node identifiers");
        }
        if self.nodes.iter().any(|node| {
            node.criticality == 0
                || node.criticality > 5
                || node.question_templates.is_empty()
                || node.concept_groups.is_empty()
                || node
                    .prerequisites
                    .iter()
                    .any(|prerequisite| !ids.contains(prerequisite))
        }) {
            return Err("invalid node definition");
        }
        fn visit(
            blueprint: &InterviewBlueprint,
            id: &str,
            visiting: &mut BTreeSet<String>,
            visited: &mut BTreeSet<String>,
        ) -> bool {
            if visited.contains(id) {
                return true;
            }
            if !visiting.insert(id.to_owned()) {
                return false;
            }
            let acyclic = blueprint.node(id).is_some_and(|node| {
                node.prerequisites
                    .iter()
                    .all(|next| visit(blueprint, next, visiting, visited))
            });
            visiting.remove(id);
            visited.insert(id.to_owned());
            acyclic
        }
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        if self
            .nodes
            .iter()
            .all(|node| visit(self, node.id, &mut visiting, &mut visited))
        {
            Ok(())
        } else {
            Err("competency graph contains a cycle")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewQuestion {
    pub id: String,
    pub node_id: String,
    pub prompt: String,
    pub sequence: u8,
    #[serde(default)]
    pub recommended_response_mode: ResponseMode,
    pub delivery: QuestionDelivery,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseMode {
    #[default]
    Text,
    Voice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionDelivery {
    pub speech_text: String,
    pub avatar_cue: AvatarCue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarCue {
    Speaking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewEvidence {
    pub node_id: String,
    pub state: EvidenceState,
    pub question_id: String,
    pub answer_excerpt: String,
    pub rationale: String,
    pub evaluator: String,
    pub rubric_version: String,
    #[serde(default)]
    pub response_mode: ResponseMode,
    #[serde(default = "default_signal_strength")]
    pub signal_strength: u8,
}

fn default_signal_strength() -> u8 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterviewStatus {
    Active,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewSession {
    pub id: String,
    pub candidate_id: String,
    pub blueprint_id: String,
    pub status: InterviewStatus,
    pub revision: u32,
    pub max_turns: u8,
    pub current_question: Option<InterviewQuestion>,
    pub node_states: BTreeMap<String, EvidenceState>,
    pub attempts: BTreeMap<String, u8>,
    pub evidence: Vec<InterviewEvidence>,
    pub model_mode: String,
}

impl InterviewSession {
    pub fn public_view(&self, blueprint: &InterviewBlueprint) -> InterviewView {
        let nodes = blueprint
            .nodes
            .iter()
            .map(|node| InterviewNodeView {
                id: node.id,
                label: node.label,
                kind: node.kind,
                criticality: node.criticality,
                prerequisites: node.prerequisites,
                state: self
                    .node_states
                    .get(node.id)
                    .copied()
                    .unwrap_or(EvidenceState::Unknown),
            })
            .collect();
        InterviewView {
            id: self.id.clone(),
            candidate_id: self.candidate_id.clone(),
            blueprint_id: self.blueprint_id.clone(),
            status: self.status,
            revision: self.revision,
            current_question: self.current_question.clone(),
            nodes,
            evidence: self.evidence.clone(),
            model_mode: self.model_mode.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewNodeView {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: CompetencyKind,
    pub criticality: u8,
    pub prerequisites: &'static [&'static str],
    pub state: EvidenceState,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewView {
    pub id: String,
    pub candidate_id: String,
    pub blueprint_id: String,
    pub status: InterviewStatus,
    pub revision: u32,
    pub current_question: Option<InterviewQuestion>,
    pub nodes: Vec<InterviewNodeView>,
    pub evidence: Vec<InterviewEvidence>,
    pub model_mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewListView {
    pub items: Vec<InterviewView>,
}

const IDEMPOTENCY_CONCEPTS: &[&[&str]] = &[
    &["ключ идемпотентности", "idempotency key"],
    &["уникальный индекс", "unique constraint", "unique index"],
    &["транзакц", "atomic"],
    &["повтор", "сохранить ответ", "cached response"],
];
const UNIQUE_OPERATION_CONCEPTS: &[&[&str]] = &[
    &["ключ", "идентификатор операции"],
    &["уникаль", "unique"],
    &["область", "scope", "пользователь"],
];
const TRANSACTION_CONCEPTS: &[&[&str]] = &[
    &["транзакц", "transaction"],
    &["commit", "коммит", "rollback", "откат"],
    &["атомар", "atomic"],
];
const CONCURRENCY_CONCEPTS: &[&[&str]] = &[
    &["уникальный индекс", "unique constraint", "unique index"],
    &["конфликт", "duplicate", "23505"],
    &["одноврем", "параллель", "race"],
];
const ISOLATION_CONCEPTS: &[&[&str]] = &[
    &["read committed", "repeatable read", "serializable"],
    &["аномал", "phantom", "lost update"],
    &["блокиров", "retry", "повтор транзакции"],
];
const INCIDENT_CONCEPTS: &[&[&str]] = &[
    &["корреляц", "request id", "trace id"],
    &["лог", "метрик", "трейс"],
    &["гипотез", "воспроиз"],
];
const REQUIREMENTS_CONCEPTS: &[&[&str]] = &[
    &["уточ", "вопрос"],
    &["предполож", "ограничен"],
    &["соглас", "критер"],
    &["риск", "компромисс"],
];
const DISAGREEMENT_CONCEPTS: &[&[&str]] = &[
    &["цель", "интерес", "позици"],
    &["факт", "данн", "критер"],
    &["вариант", "компромисс", "эксперимент"],
    &["договор", "решен", "эскалац"],
];
const FEEDBACK_CONCEPTS: &[&[&str]] = &[
    &["конкрет", "пример", "наблюден"],
    &["влия", "последств", "результат"],
    &["спрос", "выслуш", "точк"],
    &["договор", "следующ", "провер"],
];

static BACKEND_MIDDLE_NODES: &[CompetencyNode] = &[
    CompetencyNode {
        id: "backend.idempotency",
        label: "Проектирует идемпотентную запись",
        kind: CompetencyKind::Technical,
        criticality: 5,
        prerequisites: &[
            "backend.unique-operation",
            "backend.transactions",
            "backend.concurrency",
        ],
        question_templates: &[
            "Клиент повторяет POST-запрос после таймаута, и заказ создаётся дважды. Как вы измените систему?",
            "Платёжный webhook доставляется несколько раз и повторно начисляет деньги. Спроектируйте безопасную обработку.",
            "После сетевого сбоя мобильное приложение повторяет создание заявки. Как гарантировать один результат?",
        ],
        concept_groups: IDEMPOTENCY_CONCEPTS,
    },
    CompetencyNode {
        id: "backend.unique-operation",
        label: "Определяет уникальность операции",
        kind: CompetencyKind::Technical,
        criticality: 4,
        prerequisites: &[],
        question_templates: &[
            "Как сформировать и ограничить область действия ключа операции, чтобы не склеить разные запросы?",
            "Кто должен создавать идентификатор повторяемой операции и когда один ключ перестаёт быть действительным?",
        ],
        concept_groups: UNIQUE_OPERATION_CONCEPTS,
    },
    CompetencyNode {
        id: "backend.transactions",
        label: "Использует транзакционные границы",
        kind: CompetencyKind::Technical,
        criticality: 5,
        prerequisites: &[],
        question_templates: &[
            "Какие изменения должны попасть в одну транзакцию при создании заказа и записи результата операции?",
            "Где провести границу транзакции, если запрос одновременно меняет баланс и журнал операций?",
        ],
        concept_groups: TRANSACTION_CONCEPTS,
    },
    CompetencyNode {
        id: "backend.concurrency",
        label: "Обрабатывает конкурентные записи",
        kind: CompetencyKind::Technical,
        criticality: 5,
        prerequisites: &["backend.transactions"],
        question_templates: &[
            "Два одинаковых запроса одновременно не нашли существующую операцию. Как исключить двойную запись?",
            "Два обработчика параллельно создают одну сущность. Где должна находиться окончательная защита от гонки?",
        ],
        concept_groups: CONCURRENCY_CONCEPTS,
    },
    CompetencyNode {
        id: "backend.isolation",
        label: "Выбирает уровень изоляции по аномалии",
        kind: CompetencyKind::Technical,
        criticality: 4,
        prerequisites: &["backend.transactions"],
        question_templates: &[
            "Две транзакции читают одно состояние и обе принимают несовместимое решение. Как определить нужную изоляцию?",
            "Когда Read Committed недостаточно и какие издержки появятся у более строгой изоляции?",
        ],
        concept_groups: ISOLATION_CONCEPTS,
    },
    CompetencyNode {
        id: "backend.incident-diagnosis",
        label: "Диагностирует инцидент по наблюдаемым данным",
        kind: CompetencyKind::Technical,
        criticality: 4,
        prerequisites: &[],
        question_templates: &[
            "В продакшене изредка возникают дубли, но локально ошибка не повторяется. Какие данные вы соберёте и в каком порядке?",
            "После релиза выросло число повторных операций. Как построите проверяемую диагностику, не меняя всё сразу?",
        ],
        concept_groups: INCIDENT_CONCEPTS,
    },
    CompetencyNode {
        id: "collaboration.requirements",
        label: "Проясняет неоднозначные требования и риски",
        kind: CompetencyKind::Behavioral,
        criticality: 3,
        prerequisites: &[],
        question_templates: &[
            "Менеджер просит убрать защиту от дублей ради релиза сегодня. Как вы проясните ситуацию и предложите решение?",
            "Требование допускает два разных толкования, а срок уже близко. Как вы будете действовать?",
        ],
        concept_groups: REQUIREMENTS_CONCEPTS,
    },
    CompetencyNode {
        id: "collaboration.disagreement",
        label: "Разбирает рабочее разногласие без эскалации конфликта",
        kind: CompetencyKind::Behavioral,
        criticality: 4,
        prerequisites: &["collaboration.requirements"],
        question_templates: &[
            "Вы считаете решение рискованным, а коллега настаивает на нём и встреча зашла в тупик. Как продолжите разговор?",
            "Архитектор и продуктовый менеджер предлагают несовместимые приоритеты. Как поможете команде принять решение?",
        ],
        concept_groups: DISAGREEMENT_CONCEPTS,
    },
    CompetencyNode {
        id: "collaboration.feedback",
        label: "Даёт и принимает рабочую обратную связь",
        kind: CompetencyKind::Behavioral,
        criticality: 3,
        prerequisites: &["collaboration.requirements"],
        question_templates: &[
            "Коллега второй раз срывает договорённость и болезненно реагирует на замечания. Как вы дадите обратную связь?",
            "На ревью вам резко указали на системную ошибку. Как вы отреагируете и что сделаете дальше?",
        ],
        concept_groups: FEEDBACK_CONCEPTS,
    },
];

pub static BACKEND_MIDDLE: InterviewBlueprint = InterviewBlueprint {
    id: "backend-middle-v1",
    label: "Бэкенд-разработчик · Мидл",
    start_node_id: "backend.idempotency",
    nodes: BACKEND_MIDDLE_NODES,
};

pub fn blueprint(id: &str) -> Option<&'static InterviewBlueprint> {
    (id == BACKEND_MIDDLE.id).then_some(&BACKEND_MIDDLE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_blueprint_is_a_valid_dag() {
        assert_eq!(BACKEND_MIDDLE.validate(), Ok(()));
    }
}
