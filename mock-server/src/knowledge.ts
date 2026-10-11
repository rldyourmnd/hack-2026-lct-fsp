import legacy from '../../backend/crates/domain/data/knowledge-v1.json';
import additions from '../../backend/crates/domain/data/knowledge-v2-additions.json';
import topicData from '../../backend/crates/domain/data/knowledge-v3-data.json';
import topicEnterprise from '../../backend/crates/domain/data/knowledge-v3-enterprise.json';
import topicInfrastructure from '../../backend/crates/domain/data/knowledge-v3-infrastructure.json';
import topicMobile from '../../backend/crates/domain/data/knowledge-v3-mobile.json';
import topicProduct from '../../backend/crates/domain/data/knowledge-v3-product.json';
import topicQuality from '../../backend/crates/domain/data/knowledge-v3-quality.json';
import topicSystems from '../../backend/crates/domain/data/knowledge-v3-systems.json';
import topicWeb from '../../backend/crates/domain/data/knowledge-v3-web.json';
import { GRADE_ORDER, SPECIALIZATIONS, testTopics } from './catalog.ts';
import type { components } from './generated/schema.d.ts';

type Schemas = components['schemas'];
type Task = Schemas['AssessmentTask'];
type Answer = Schemas['AssessmentAnswer'];
type Grade = Schemas['Assessment']['grade'];
type Specialization = Schemas['Assessment']['specialization'];
type SkillId = Schemas['KnowledgeQuestion']['skillId'];
type BankQuestion = {
  id: string;
  /** `null` in the topic banks: the question serves every specialization tested on its topic. */
  specialization: Specialization | null;
  grade: Grade;
  skillId: SkillId;
  prompt: string;
  options: { id: string; label: string }[];
  correctOptionId: string;
  explanation: string;
  source: string;
};
const topics = (specialization: Specialization) => testTopics(specialization);

const bank: BankQuestion[] = [
  ...legacy.questions.map((question) => ({
    ...question,
    specialization: 'backend' as const,
    grade: question.grade as Grade,
    skillId: question.skillId as SkillId,
  })),
  ...additions.questions.map((question) => ({
    ...question,
    specialization: question.specialization as Specialization,
    grade: question.grade as Grade,
    skillId: question.skillId as SkillId,
  })),
  ...[
    topicData,
    topicEnterprise,
    topicInfrastructure,
    topicMobile,
    topicProduct,
    topicQuality,
    topicSystems,
    topicWeb,
  ].flatMap((topicBank) =>
    topicBank.questions.map((question) => ({
      ...question,
      specialization: null,
      grade: question.grade as Grade,
      skillId: question.skillId as SkillId,
    })),
  ),
];

/** Questions a test of this specialization and grade may draw from, as the Rust `question_pool`. */
const pool = (specialization: Specialization, grade: Grade) =>
  bank.filter(
    (question) =>
      (question.specialization === null || question.specialization === specialization) && question.grade === grade,
  );

function stableHash(value: string): number {
  let hash = 0x811c9dc5;
  for (const byte of new TextEncoder().encode(value)) hash = Math.imul(hash ^ byte, 0x01000193);
  return hash >>> 0;
}

/** Specializations and grades the bank can test: five questions for each of the two topics. */
export function knowledgeScopes(): { specialization: Specialization; grades: Grade[] }[] {
  return SPECIALIZATIONS.map((item) => ({
    specialization: item.id,
    grades: GRADE_ORDER.filter((grade) => knowledgeAvailable(item.id, grade)),
  })).filter((scope) => scope.grades.length > 0);
}

/** Whether the bank can test this specialization and grade, as the Rust `knowledge_available`. */
export function knowledgeAvailable(specialization: Specialization, grade: Grade): boolean {
  return knowledgeTask(specialization, grade, 'availability') !== null;
}

export function knowledgeTask(specialization: Specialization, grade: Grade, variantKey: string): Task | null {
  const selectedTopics = topics(specialization);
  const questions = selectedTopics.flatMap(([skillId]) =>
    pool(specialization, grade)
      .filter((question) => question.skillId === skillId)
      .sort(
        (left, right) =>
          stableHash(`${variantKey}:${skillId}:${left.id}`) - stableHash(`${variantKey}:${skillId}:${right.id}`),
      )
      .slice(0, 5),
  );
  if (questions.length !== 10) return null;
  const fingerprint = questions.map((question) => question.id).join('.');
  const specializationTitle = `${selectedTopics[0][1]} и ${selectedTopics[1][1]}`;
  return {
    id: `knowledge-${specialization}-${grade}-knowledge-v2`,
    family: `${specialization}-knowledge`,
    version: 'knowledge-v2',
    variantId: `${specialization}-${grade}-v2-${stableHash(fingerprint).toString(16)}`,
    title: `Тест знаний: ${specializationTitle}`,
    prompt:
      'Выберите один ответ в каждом из 10 вопросов. Для зачёта нужны минимум 8 верных ответов из 10 и минимум 3 из 5 по каждой теме. ИИ и справочники разрешены. Результат показывает знания по этому тесту, а не опыт написания кода.',
    language: specialization,
    starterCode: '',
    execution: 'questionnaire',
    aiAllowed: true,
    externalAiAllowed: true,
    acceptanceCriteria: [
      'Не менее 8 верных ответов из 10',
      `Не менее 3 из 5 по ${selectedTopics[0][1]} и 3 из 5 по ${selectedTopics[1][1]}`,
    ],
    rubric: selectedTopics.map(([id, label]) => ({ id, label, weight: 5 })),
    questions: questions.map((question) => ({
      id: question.id,
      skillId: question.skillId,
      prompt: question.prompt,
      options: structuredClone(question.options),
    })),
  };
}

export function validKnowledgeAnswer(task: Task, answer: Answer): boolean {
  const responses = answer.responses ?? [];
  return (
    !answer.code &&
    !answer.explanation &&
    responses.length <= 10 &&
    new Set(responses.map((response) => response.questionId)).size === responses.length &&
    responses.every((response) =>
      task.questions?.some(
        (question) =>
          question.id === response.questionId && question.options.some((option) => option.id === response.optionId),
      ),
    )
  );
}

export function knowledgeCompletion(task: Task, answer: Answer): Schemas['Check'][] {
  return (task.questions ?? []).map((question) => {
    const chosen = answer.responses?.some((response) => response.questionId === question.id);
    return {
      id: question.id,
      label: question.prompt,
      status: chosen ? 'passed' : 'not_run',
      message: chosen ? 'Ответ выбран; правильность проверяется после отправки' : 'Выберите ответ',
    };
  });
}

export function scoreKnowledge(specialization: Specialization, grade: Grade, task: Task, answer: Answer) {
  const selectedTopics = topics(specialization);
  if (task.version !== 'knowledge-v2' || !validKnowledgeAnswer(task, answer) || answer.responses?.length !== 10)
    return null;
  const selected = new Set(task.questions?.map((question) => question.id));
  const questions = pool(specialization, grade).filter((question) => selected.has(question.id));
  if (
    questions.length !== 10 ||
    selectedTopics.some(([skillId]) => questions.filter((question) => question.skillId === skillId).length !== 5)
  )
    return null;
  const counts = new Map(selectedTopics.map(([skillId]) => [skillId, 0]));
  const checks: Schemas['Check'][] = questions.map((question) => {
    const correct = answer.responses?.some(
      (response) => response.questionId === question.id && response.optionId === question.correctOptionId,
    );
    if (correct) counts.set(question.skillId, (counts.get(question.skillId) ?? 0) + 1);
    const label = question.options.find((option) => option.id === question.correctOptionId)?.label;
    return {
      id: question.id,
      label: question.prompt,
      status: correct ? 'passed' : 'failed',
      message: `Верный ответ: ${label}. ${question.explanation} Источник: ${question.source}`,
    };
  });
  const scores: [number, number] = [counts.get(selectedTopics[0][0]) ?? 0, counts.get(selectedTopics[1][0]) ?? 0];
  const total = scores.reduce((sum, score) => sum + score, 0);
  return {
    passed: total >= 8 && scores.every((score) => score >= 3),
    checks,
    rubric: selectedTopics.map(([skillId, label], index) => {
      const score = scores[index] ?? 0;
      return {
        criterionId: skillId,
        label,
        outcome: score >= 3 ? ('met' as const) : ('not_met' as const),
        feedback: `${score} из 5 верных ответов; порог по теме — 3.`,
      };
    }),
    feedback: `Верных ответов: ${total} из 10. ${selectedTopics[0][1]}: ${scores[0]}/5; ${selectedTopics[1][1]}: ${scores[1]}/5. Для зачёта нужны 8/10 и не менее 3/5 по каждой теме. Проверены знания по тесту версии ${task.version}; практический опыт разработки не оценивался. ИИ и справочники разрешены.`,
  };
}
