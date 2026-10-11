import type { components } from '@/shared/api';
import { CATALOG_VERSION, GRADES, SKILL_CATEGORIES, SKILLS, SPECIALIZATIONS } from '@/shared/api';

type S = components['schemas'];

export const candidateSession: S['Session'] = {
  actorId: 'candidate024',
  role: 'candidate',
  emailVerified: true,
  candidateId: 'candidate024',
  companyId: null,
  displayName: 'Алексей Морозов',
};

export const employerSession: S['Session'] = {
  actorId: 'employer-a',
  role: 'employer',
  emailVerified: true,
  candidateId: null,
  companyId: 'company-a',
  displayName: 'Север Тех',
};

export function profile(overrides: Partial<S['Profile']> = {}): S['Profile'] {
  return {
    id: 'candidate024',
    alias: 'Кандидат №024',
    email: 'alexey@example.com',
    emailVerified: true,
    name: 'Алексей Морозов',
    industry: 'it',
    specialization: 'backend',
    selectedGrade: 'middle',
    stack: ['rust', 'postgresql'],
    teamRole: 'Разработчик',
    experienceMonths: 18,
    experienceSince: null,
    experience: 'API',
    softSkills: [],
    consents: { processing: true, publication: true },
    verifiedGrade: 'junior',
    verifiedSpecialization: 'backend',
    gradeConfirmedAt: '2026-06-01T12:00:00.000Z',
    category: { specialization: 'backend', grade: 'junior', eligible: true, source: 'assessment', confidence: null },
    estimate: null,
    autoFields: [],
    freshness: { lastSignalAt: '2026-06-01T12:00:00.000Z', lastSignal: 'assessment', fresh: false, windowDays: 90 },
    onboarding: {
      emailVerified: true,
      questionnaireComplete: true,
      assessmentPassed: true,
      readyForPublication: true,
      visibleToEmployers: true,
      blockers: [],
    },
    assessmentEligibility: {
      selectedSpecialization: 'backend',
      selectedGrade: 'middle',
      canStartSelected: true,
      selectedReason: null,
      canStartUpgrade: true,
      nextEligibleAt: '2026-08-30T12:00:00.000Z',
      cooldownDays: 90,
      reason: null,
    },
    revision: 1,
    updatedAt: '2026-10-05T12:00:00.000Z',
    ...overrides,
  };
}

export function invitation(overrides: Partial<S['Invitation']> = {}): S['Invitation'] {
  return {
    id: 'invitation-001',
    candidateId: 'candidate024',
    candidateAlias: 'Кандидат №024',
    company: {
      id: 'company-a',
      displayName: 'Север Тех',
      businessArea: 'Разработка программного обеспечения',
      description: 'Демонстрационная компания.',
      contactEmail: 'hr@sever-tech.example',
    },
    need: {
      id: 'need-001',
      title: 'Бэкенд-разработчик',
      workMode: 'remote',
      salary: { from: 200000, to: 260000, currency: 'RUB', period: 'month', taxBasis: 'gross' },
      description: 'Разработка API.',
    },
    message: 'Приглашаем обсудить роль.',
    status: 'viewed',
    createdAt: '2026-10-05T12:00:00.000Z',
    viewedAt: '2026-10-05T12:00:00.000Z',
    respondedAt: null,
    disclosureFields: [],
    ...overrides,
  };
}

export const shortlist: S['Shortlist'] = {
  id: 'shortlist-001',
  title: 'Бэкенд · Мидл',
  count: 1,
  createdAt: '2026-10-05T12:00:00.000Z',
  updatedAt: '2026-10-05T12:00:00.000Z',
  members: [{ candidateId: 'candidate018', alias: 'Кандидат №018', available: true }],
};

export const problem = (code: string, message: string, status = 400) => ({
  status,
  body: { code, message, requestId: 'req-test', details: [] },
});

export const meta: S['Meta'] = {
  contractVersion: '0.1.0',
  mode: 'mock',
  serverTime: '2026-10-05T12:00:00.000Z',
  scenario: 'baseline',
  capabilities: {
    emailVerification: true,
    profilePdf: true,
    profilePdfImport: true,
    githubFixture: true,
    githubOAuth: false,
    githubInsights: false,
    graphInterviews: true,
    neuralTts: false,
    hiringAnalytics: true,
    fspFixture: true,
    matching: true,
    directInvitations: true,
    vacancyPublication: false,
    selfApplication: false,
    chat: false,
    recurringTasks: false,
    realCodeExecution: false,
    realLlm: false,
    realEmail: false,
  },
  assessmentPolicy: {
    successfulGradeChangeCooldownDays: 90,
    failedAttemptCooldownDays: 0,
    serviceFailureCooldownDays: 0,
    sameGradeRetryCooldownDays: 0,
    aiAllowed: true,
    externalAiAllowed: true,
    supportedScopes: [{ specialization: 'backend', grades: ['junior', 'middle', 'senior'] }],
  },
  competitions: [],
  catalogVersion: CATALOG_VERSION,
  grades: [...GRADES],
  specializations: SPECIALIZATIONS.map(({ id, label, short, group }) => ({ id, label, short, group })),
  skillCategories: [...SKILL_CATEGORIES],
  skills: [...SKILLS],
};

export function assessment(overrides: Partial<S['Assessment']> = {}): S['Assessment'] {
  return {
    id: 'assessment-001',
    specialization: 'backend',
    grade: 'middle',
    status: 'draft',
    task: {
      id: 'task-idempotency-v1',
      family: 'idempotent-api',
      version: '1',
      variantId: 'orders-middle-a',
      title: 'Идемпотентное создание заказа',
      prompt: 'Опишите решение.',
      language: 'rust',
      starterCode: 'fn main() {}\n',
      acceptanceCriteria: ['Первый запрос создаёт заказ'],
      rubric: [{ id: 'correctness', label: 'Корректность', weight: 4 }],
      aiAllowed: true,
      externalAiAllowed: true,
      execution: 'questionnaire',
      questions: Array.from({ length: 10 }, (_, i) => ({
        id: `q-${i}`,
        skillId: 'rust',
        prompt: `Вопрос ${i + 1}`,
        options: [
          { id: 'a', label: `Вариант A ${i + 1}` },
          { id: 'b', label: `Вариант B ${i + 1}` },
        ],
      })),
    },
    answer: { code: '', explanation: '', externalAiUsed: false, responses: [] },
    checks: [
      {
        id: 'check-1',
        label: 'Первый запрос создаёт заказ',
        status: 'not_run',
        message: 'Проверка ещё не запускалась',
      },
    ],
    result: null,
    createdAt: '2026-10-05T12:00:00.000Z',
    updatedAt: '2026-10-05T12:00:00.000Z',
    revision: 1,
    simulation: true,
    ...overrides,
  };
}
