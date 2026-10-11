import { SKILLS } from './catalog.ts';
import { fspResults } from './fsp-fixture.ts';
import type { components } from './generated/schema.d.ts';
import { githubFixtureReport, githubSource } from './github-fixture.ts';

type S = components['schemas'];
export type Grade = S['Profile']['selectedGrade'];
export type Specialization = S['Profile']['specialization'];
export type SkillId = S['Skill']['skillId'];
export type SkillStatus = S['Skill']['status'];
export type Scenario = S['MockResetInput']['scenario'];
export type Check = S['Check'];
export type SelfSkill = S['SelfSkill'];
export type Evidence = S['Evidence'];
export type Source = S['Source'];
export type Need = S['Need'];
export type EvidencePolicy = S['EvidencePolicy'];
export type Assessment = S['Assessment'];
export type Invitation = S['Invitation'];
export type Application = S['Application'];
export type TaskResponse = S['TaskResponse'];
export type Job = S['Job'];
export type Disclosure = S['Disclosure'];
export type Interview = S['Interview'];

export interface Candidate {
  id: string;
  alias: string;
  name: string;
  email: string;
  emailVerified: boolean;
  industry: S['Profile']['industry'];
  specialization: Specialization;
  selectedGrade: Grade;
  verifiedGrade: Grade | null;
  verifiedSpecialization: Specialization | null;
  gradeConfirmedAt: string | null;
  stack: SkillId[];
  teamRole: string;
  experienceMonths: number | null;
  experienceSince: number | null;
  experience: string;
  softSkills: string[];
  consents: S['Profile']['consents'];
  estimate: S['ProfileEstimate'] | null;
  autoFields: S['Profile']['autoFields'];
  revision: number;
  updatedAt: string;
  workModes: S['AnonymousCandidate']['workModes'];
  skills: SelfSkill[];
  sources: Source[];
}

export type Company = Omit<S['Company'], 'readyToInvite'>;

export interface Actor {
  id: string;
  role: S['Session']['role'];
  candidateId: string | null;
  companyId: string | null;
  email: string;
  /** Employers keep verification on the actor; candidates keep it on their profile. */
  emailVerified?: boolean;
}

export interface Challenge {
  id: string;
  role: Actor['role'];
  email: string;
  code: string;
  expiresAt: string;
  used: boolean;
  attempts: number;
}

export interface Shortlist {
  id: string;
  companyId: string;
  title: string;
  memberIds: string[];
  createdAt: string;
  updatedAt: string;
}

export type AssessmentRecord = Assessment & { candidateId: string };
/** An invitation keeps the full need it was sent for, so gap plans use the terms the candidate received. */
export type InvitationRecord = Invitation & { needDefinition: Need };
/** An application keeps the vacancy as it was when the candidate applied; its match is computed when read. */
export type ApplicationRecord = Omit<Application, 'match'> & { needDefinition: Need };
/** A company's short task; candidates see it through the weekly offer, companies with their response count. */
export type TaskRecord = Omit<S['Task'], 'responseCount'> & { companyId: string };
export type JobResult = NonNullable<Job['result']>;
export type JobRecord = Job & {
  ownerActorId: string;
  candidateId: string | null;
  work: () => JobResult;
  sourceId?: string;
  assessmentId?: string;
  inputRevision?: number;
  confirmed?: boolean;
};

export interface StoredResponse {
  fingerprint: string;
  status: number;
  body: unknown;
}

export interface State {
  scenario: Scenario;
  clock: string;
  policy: { gradeChangeCooldownDays: number };
  timers: Set<ReturnType<typeof setTimeout>>;
  candidates: Map<string, Candidate>;
  companies: Map<string, Company>;
  actors: Map<string, Actor>;
  sessions: Map<string, string>;
  challenges: Map<string, Challenge>;
  needs: Map<string, Need>;
  shortlists: Map<string, Shortlist>;
  invitations: Map<string, InvitationRecord>;
  applications: Map<string, ApplicationRecord>;
  tasks: Map<string, TaskRecord>;
  taskResponses: Map<string, TaskResponse>;
  disclosures: Map<string, Disclosure>;
  assessments: Map<string, AssessmentRecord>;
  interviews: Map<string, Interview>;
  jobs: Map<string, JobRecord>;
  idempotency: Map<string, StoredResponse>;
  counters: Record<string, number>;
}

export const CLOCK = '2026-10-05T12:00:00.000Z';
export { SKILLS } from './catalog.ts';

/** The skills the seeded demo candidates hold evidence for. */
const DEMO_SKILLS = SKILLS.filter((skill) =>
  ['rust', 'postgresql', 'docker', 'redis', 'typescript', 'react'].includes(skill.id),
);
export const COMPETITIONS: S['Meta']['competitions'] = [
  {
    id: 'competition-demo-api',
    name: 'Демо: проектирование надёжного API',
    synthetic: true,
    skillIds: ['rust', 'postgresql'],
    discipline: 'Продуктовое программирование',
  },
  {
    id: 'competition-demo-systems',
    name: 'Демо: распределённые системы',
    synthetic: true,
    skillIds: ['docker', 'redis'],
    discipline: 'Продуктовое программирование',
  },
];
export const RUBRIC: S['RubricCriterion'][] = [
  { id: 'correctness', label: 'Корректность', weight: 4 },
  { id: 'reliability', label: 'Надёжность', weight: 3 },
  { id: 'tests', label: 'Проверки', weight: 2 },
  { id: 'explanation', label: 'Объяснение решения', weight: 1 },
];
export const TASK: S['AssessmentTask'] = {
  id: 'task-idempotency-v1',
  family: 'idempotent-api',
  version: '1',
  variantId: 'orders-middle-a',
  title: 'Идемпотентное создание заказа',
  language: 'rust',
  prompt:
    'Опишите и реализуйте обработку создания заказа с ключом идемпотентности. Повторный и одновременный запросы с одним ключом должны возвращать один результат. Объясните границы транзакции и поведение после ошибки. Это демонстрационная проверка: код здесь не исполняется, результаты берутся из сценария.',
  starterCode: 'pub async fn create_order(key: &str, payload: OrderInput) -> Result<Order, Error> {\n    todo!()\n}\n',
  acceptanceCriteria: [
    'Первый запрос создаёт заказ',
    'Повторный запрос возвращает тот же заказ',
    'Одновременные запросы не создают дубликаты',
    'Повтор после ошибки не оставляет неконсистентные данные',
  ],
  rubric: RUBRIC,
  aiAllowed: true,
  externalAiAllowed: true,
  execution: 'simulated',
};
export function checks(outcome: 'preview' | 'pass' | 'fail' = 'preview'): Check[] {
  return TASK.acceptanceCriteria.map((label, index) => ({
    id: `check-${index + 1}`,
    label,
    status: outcome === 'pass' || index < 2 ? 'passed' : outcome === 'fail' || index === 2 ? 'failed' : 'not_run',
    message:
      outcome === 'pass'
        ? 'Успешно в демонстрационном сценарии'
        : index < 2
          ? 'Успешно в демонстрационном сценарии'
          : index === 2
            ? 'Проверьте атомарность операции'
            : 'Проверка ещё не запускалась',
  }));
}
const gradeDate = '2026-06-01T12:00:00.000Z';
const period = { from: '2026-06-01T00:00:00.000Z', to: CLOCK };
const names = [
  'order-api',
  'queue-worker',
  'metrics-api',
  'data-tools',
  'task-runner',
  'auth-example',
  'migration-kit',
  'cli-tools',
  'billing-api',
  'cache-service',
  'search-demo',
  'legacy-api',
];

interface SourceOptions {
  github?: boolean;
  fspLinked?: boolean;
  fspVerified?: boolean;
}

export function sources(candidateId: string, options: SourceOptions = {}): [Source, Source] {
  const connected = options.github !== false;
  const repositories: Source['repositories'] = connected
    ? names.map((name, index) => ({
        id: `repo-${candidateId}-${index + 1}`,
        name,
        url: `https://example.com/fixtures/github/${name}`,
        technologies: index === 1 ? ['rust', 'redis'] : index < 8 ? ['rust', 'postgresql', 'docker'] : ['rust'],
        status: index < 8 ? 'processed' : index < 11 ? 'queued' : 'unavailable',
        metrics: {
          stars: index * 3,
          forks: index,
          openIssues: index % 4,
          sizeKib: 1200 + index * 240,
          pushedAt: CLOCK,
        },
      }))
    : [];
  const fspConnected = Boolean(options.fspLinked || options.fspVerified);
  return [
    {
      id: `source-${candidateId}-github`,
      provider: 'github',
      status: connected ? 'connected' : 'not_connected',
      synthetic: true,
      activeJobId: null,
      accountLabel: connected ? 'Демо GitHub' : null,
      lastSyncedAt: connected ? CLOCK : null,
      counts: connected
        ? { found: 12, processed: 8, queued: 3, unavailable: 1 }
        : { found: 0, processed: 0, queued: 0, unavailable: 0 },
      repositories,
      activeDays: connected ? 27 : null,
      observationPeriod: connected ? period : null,
      fspResults: [],
      verifiedParticipation: false,
      competitionIds: [],
      notice:
        'Демонстрационные данные GitHub. В рабочем режиме анализируются только публичные репозитории; метрики — сигналы активности, а не оценка навыков.',
      github: githubSource(
        candidateId,
        connected ? githubFixtureReport(`demo-${candidateId.slice(-3)}`, CLOCK) : null,
        connected ? CLOCK : null,
      ),
    },
    {
      id: `source-${candidateId}-fsp`,
      provider: 'fsp',
      status: fspConnected ? 'connected' : 'not_connected',
      synthetic: true,
      activeJobId: null,
      accountLabel: fspConnected ? `FSP-DEMO-${candidateId.slice(-3)}` : null,
      lastSyncedAt: fspConnected ? CLOCK : null,
      counts: { found: 0, processed: 0, queued: 0, unavailable: 0 },
      repositories: [],
      activeDays: null,
      observationPeriod: null,
      fspResults: options.fspVerified ? fspResults('FSP-DEMO-024') : [],
      verifiedParticipation: Boolean(options.fspVerified),
      competitionIds: options.fspVerified ? ['competition-demo-api'] : [],
      notice:
        'Синтетический источник ФСП. Реальный API и данные участников не подключены. Участие в ФСП необязательно.',
      github: null,
    },
  ];
}

export function evidenceFor(candidateId: string, skill: SkillId, status: SkillStatus): Evidence[] {
  if (status === 'insufficient') return [];
  return [
    {
      id: `evidence-${candidateId}-${skill}`,
      source: status === 'confirmed' ? 'assessment' : 'github',
      title:
        status === 'confirmed'
          ? 'Практическая проверка'
          : skill === 'postgresql'
            ? 'Транзакции в order-api'
            : 'Использование технологии в проекте',
      url: status === 'confirmed' ? null : 'https://example.com/fixtures/github/order-api/pull/17',
      reference: status === 'confirmed' ? `assessment-seed-${candidateId}` : 'PR #17',
      observedAt: CLOCK,
      scope: 'individual',
      synthetic: true,
    },
  ];
}

const summaries: Record<SkillStatus, string> = {
  confirmed: 'Подтверждено проверкой платформы',
  evidence: 'Есть свидетельства использования; целевой уровень ещё не проверен',
  observed: 'Технология найдена в проекте; уровень не подтверждён',
  insufficient: 'Недостаточно свидетельств',
};

export function makeSkills(
  candidateId: string,
  qualified = false,
  missingPostgres = false,
  frontend = false,
): SelfSkill[] {
  return DEMO_SKILLS.filter((skill) =>
    frontend ? ['typescript', 'react'].includes(skill.id) : !['typescript', 'react'].includes(skill.id),
  ).map((skill) => {
    const status: SkillStatus =
      frontend || skill.id === 'rust' || (skill.id === 'postgresql' && qualified && !missingPostgres)
        ? 'confirmed'
        : skill.id === 'postgresql'
          ? missingPostgres
            ? 'insufficient'
            : 'evidence'
          : skill.id === 'docker'
            ? 'observed'
            : 'insufficient';
    const evidence = evidenceFor(candidateId, skill.id, status);
    return {
      skillId: skill.id,
      label: skill.label,
      status,
      evidenceGrade: status === 'confirmed' ? (qualified || frontend ? 'middle' : 'junior') : null,
      evidenceSources: evidence.map((item) => item.source),
      summary: summaries[status],
      evidenceCount: evidence.length,
      confirmedAt: status === 'confirmed' ? gradeDate : null,
      evidence,
    };
  });
}

interface CandidateOptions {
  name: string;
  email: string;
  qualified?: boolean;
  missingPostgres?: boolean;
  frontend?: boolean;
  fspLinked?: boolean;
  fspVerified?: boolean;
  fresh?: boolean;
}

export function makeCandidate(
  id: string,
  {
    name,
    email,
    qualified = false,
    missingPostgres = false,
    frontend = false,
    fspLinked = false,
    fspVerified = false,
    fresh = false,
  }: CandidateOptions,
): Candidate {
  return {
    id,
    alias: `Кандидат №${id.slice(-3)}`,
    name,
    email,
    emailVerified: !fresh,
    industry: 'it',
    specialization: frontend ? 'frontend' : 'backend',
    selectedGrade: fresh ? 'junior' : 'middle',
    verifiedGrade: fresh ? null : qualified || frontend ? 'middle' : 'junior',
    verifiedSpecialization: fresh ? null : frontend ? 'frontend' : 'backend',
    gradeConfirmedAt: fresh ? null : gradeDate,
    stack: fresh ? [] : frontend ? ['react'] : ['rust', 'postgresql', 'docker'],
    teamRole: fresh ? '' : 'Разработчик',
    experienceMonths: fresh ? null : 18,
    experienceSince: null,
    experience: fresh ? '' : 'Разработка API и командная работа над учебными и открытыми проектами.',
    softSkills: fresh ? [] : ['Командная работа', 'Объяснение решений'],
    consents: { processing: !fresh, publication: !fresh },
    estimate: null,
    autoFields: [],
    revision: 1,
    updatedAt: CLOCK,
    workModes: ['remote'],
    skills: fresh ? [] : makeSkills(id, qualified, missingPostgres, frontend),
    sources: sources(id, { fspLinked, fspVerified, github: !fresh }),
  };
}
export const DEFAULT_POLICY: EvidencePolicy = {
  acceptAssessments: true,
  acceptGithub: true,
  acceptFsp: true,
  competitionSelection: 'system',
  competitionIds: [],
};
function need(companyId: string, id: string, overrides: Partial<Need> = {}): Need {
  return {
    id,
    companyId,
    title: 'Бэкенд-разработчик',
    specialization: 'backend',
    grade: 'middle',
    workMode: 'remote',
    salary: { from: 200000, to: 260000, currency: 'RUB', period: 'month', taxBasis: 'gross' },
    description:
      'Разработка надёжных API на Rust и PostgreSQL. Удалённая работа, задачи по надёжности и качеству сервисов.',
    requirements: (['rust', 'postgresql', 'docker', 'redis'] as const).map((skillId, index) => ({
      skillId,
      criticality: index < 2 ? 'required' : 'desired',
      minimumGrade: 'middle',
      evidencePolicy: structuredClone(DEFAULT_POLICY),
    })),
    published: false,
    publishedAt: null,
    createdAt: CLOCK,
    updatedAt: CLOCK,
    revision: 1,
    ...overrides,
  };
}

const requirement = (
  skillId: SkillId,
  criticality: Need['requirements'][number]['criticality'],
  minimumGrade: Grade,
): Need['requirements'][number] => ({
  skillId,
  criticality,
  minimumGrade,
  evidencePolicy: structuredClone(DEFAULT_POLICY),
});
const daysBefore = (days: number) => new Date(Date.parse(CLOCK) - days * 86400000).toISOString();

/** Published needs are the vacancies candidates browse; need-001 stays unpublished for direct invitations. */
function seedNeeds(): Map<string, Need> {
  const items = [
    need('company-a', 'need-001'),
    need('company-a', 'need-002', {
      title: 'Бэкенд-разработчик сервиса уведомлений',
      grade: 'junior',
      workMode: 'hybrid',
      salary: { from: 120000, to: 160000, currency: 'RUB', period: 'month', taxBasis: 'gross' },
      description:
        'Сервис доставки уведомлений на Rust: очереди, повторные попытки и наблюдаемость. Гибрид, офис в Москве два дня в неделю, наставник в команде.',
      requirements: [
        requirement('rust', 'required', 'junior'),
        requirement('postgresql', 'desired', 'junior'),
        requirement('docker', 'desired', 'junior'),
      ],
      published: true,
      publishedAt: daysBefore(2),
      createdAt: daysBefore(3),
      updatedAt: daysBefore(3),
    }),
    need('company-b', 'need-b-001', { published: true, publishedAt: daysBefore(5) }),
    need('company-b', 'need-b-002', {
      title: 'Фронтенд-разработчик (React)',
      specialization: 'frontend',
      workMode: 'office',
      salary: { from: 180000, to: 230000, currency: 'RUB', period: 'month', taxBasis: 'gross' },
      description:
        'Интерфейсы внутренних сервисов на React и TypeScript: формы, таблицы, доступность. Офис в Санкт-Петербурге.',
      requirements: [requirement('react', 'required', 'middle')],
      published: true,
      publishedAt: daysBefore(1),
      createdAt: daysBefore(4),
      updatedAt: daysBefore(4),
    }),
  ];
  return new Map(items.map((item) => [item.id, item]));
}

function seedTasks(): Map<string, TaskRecord> {
  const task = (item: Omit<TaskRecord, 'status'>): TaskRecord => ({ ...item, status: 'active' });
  const items = [
    task({
      id: 'task-001',
      companyId: 'company-a',
      title: 'Повтор платежа без двойного списания',
      prompt:
        'Платёжный сервис получает повторный запрос после таймаута клиента. Как сделать, чтобы деньги не списались дважды? Решите или опишите подход: таблицы и ограничения PostgreSQL, порядок операций, что вернуть повторному запросу.',
      skillId: 'postgresql',
      grade: 'junior',
      estimateMinutes: 30,
      createdAt: daysBefore(20),
    }),
    task({
      id: 'task-002',
      companyId: 'company-a',
      title: 'Повторные попытки отправки уведомлений',
      prompt:
        'Сервис уведомлений на Rust вызывает внешний API, который иногда отвечает ошибкой или не отвечает. Предложите, как организовать повторные попытки: где хранить состояние, как не перегрузить внешний сервис и когда прекращать попытки.',
      skillId: 'rust',
      grade: 'junior',
      estimateMinutes: 45,
      createdAt: daysBefore(18),
    }),
    task({
      id: 'task-003',
      companyId: 'company-b',
      title: 'Форма регистрации с проверкой полей',
      prompt:
        'Нужна форма регистрации на React: почта, пароль и согласие с условиями. Опишите, как проверять поля и показывать ошибки так, чтобы форма была доступна с клавиатуры и для экранного диктора.',
      skillId: 'react',
      grade: 'middle',
      estimateMinutes: 40,
      createdAt: daysBefore(15),
    }),
  ];
  return new Map(items.map((item) => [item.id, item]));
}

function seedTaskResponses(
  candidates: Map<string, Candidate>,
  tasks: Map<string, TaskRecord>,
): Map<string, TaskResponse> {
  const companyName = { 'company-a': 'Север Тех', 'company-b': 'Вторая компания' } as Record<string, string>;
  const response = (
    id: string,
    candidateId: string,
    taskId: string,
    answer: string,
    feedback: string,
    submittedDaysAgo: number,
  ): TaskResponse => {
    const task = tasks.get(taskId);
    const candidate = candidates.get(candidateId);
    if (!task || !candidate) throw new Error(`Missing seed task response ${id}`);
    return {
      id,
      candidateId,
      candidateAlias: candidate.alias,
      task: {
        id: task.id,
        company: { id: task.companyId, displayName: companyName[task.companyId] ?? '' },
        title: task.title,
        prompt: task.prompt,
        skillId: task.skillId,
        grade: task.grade,
        estimateMinutes: task.estimateMinutes,
      },
      mode: 'approach',
      answer,
      status: 'accepted',
      feedback,
      submittedAt: daysBefore(submittedDaysAgo),
      reviewedAt: daysBefore(submittedDaysAgo - 1),
    };
  };
  const items = [
    response(
      'task-response-seed-031',
      'candidate031',
      'task-002',
      'Очередь задач в PostgreSQL со статусом и временем следующей попытки, экспоненциальная задержка с ограничением, ключ идемпотентности на стороне получателя, после пяти неудач — в очередь разбора.',
      'Хороший разбор задержек и идемпотентности.',
      3,
    ),
    response(
      'task-response-seed-041',
      'candidate041',
      'task-003',
      'Проверка при потере фокуса и при отправке, ошибки связаны с полями через aria-describedby, фокус переходит к первому ошибочному полю, состояние кнопки не блокирует отправку без объяснения.',
      'Учтены доступность и порядок фокуса.',
      10,
    ),
  ];
  return new Map(items.map((item) => [item.id, item]));
}

const seeded = (candidates: Map<string, Candidate>, id: string): Candidate => {
  const candidate = candidates.get(id);
  if (!candidate) throw new Error(`Missing seed candidate ${id}`);
  return candidate;
};

export function createState(scenario: Scenario = 'baseline'): State {
  const candidates = new Map<string, Candidate>([
    [
      'candidate024',
      makeCandidate('candidate024', {
        name: 'Алексей Морозов',
        email: 'alexey@example.com',
        qualified: scenario === 'qualified',
      }),
    ],
    [
      'candidate018',
      makeCandidate('candidate018', {
        name: 'Демо Участник 018',
        email: 'participant018@example.com',
        qualified: true,
        missingPostgres: true,
        fspLinked: true,
      }),
    ],
    [
      'candidate031',
      makeCandidate('candidate031', {
        name: 'Демо Участник 031',
        email: 'participant031@example.com',
        qualified: true,
        fspVerified: true,
      }),
    ],
    [
      'candidate041',
      makeCandidate('candidate041', { name: 'Демо Участник 041', email: 'participant041@example.com', frontend: true }),
    ],
    [
      'candidate-new',
      makeCandidate('candidate-new', { name: 'Новый участник', email: 'new@example.com', fresh: true }),
    ],
  ]);
  seeded(candidates, 'candidate-new').alias = 'Новый участник';
  if (scenario === 'qualified') {
    const qualified = seeded(candidates, 'candidate024');
    qualified.gradeConfirmedAt = CLOCK;
    for (const skill of qualified.skills) if (skill.status === 'confirmed') skill.confirmedAt = CLOCK;
  }
  if (scenario === 'no_fsp') for (const item of candidates.values()) item.sources[1] = sources(item.id)[1];
  const companies = new Map<string, Company>([
    [
      'company-a',
      {
        id: 'company-a',
        displayName: 'Север Тех',
        businessArea: 'Разработка программного обеспечения',
        description:
          'Демонстрационная технологическая компания. Разрабатываем сервисы для управления заказами и логистикой.',
        contactEmail: 'hr@sever-tech.example',
        website: 'https://example.com/sever-tech',
      },
    ],
    [
      'company-b',
      {
        id: 'company-b',
        displayName: 'Вторая компания',
        businessArea: 'Информационные технологии',
        description: 'Вторая демонстрационная компания для проверки разграничения доступа.',
        contactEmail: 'hr@second-company.example',
        website: null,
      },
    ],
  ]);
  const actors = new Map<string, Actor>([
    [
      'candidate024',
      {
        id: 'candidate024',
        role: 'candidate',
        candidateId: 'candidate024',
        companyId: null,
        email: 'alexey@example.com',
      },
    ],
    [
      'candidate-new',
      {
        id: 'candidate-new',
        role: 'candidate',
        candidateId: 'candidate-new',
        companyId: null,
        email: 'new@example.com',
      },
    ],
    [
      'employer-a',
      {
        id: 'employer-a',
        role: 'employer',
        candidateId: null,
        companyId: 'company-a',
        email: 'hr@sever-tech.example',
        emailVerified: true,
      },
    ],
    [
      'employer-b',
      {
        id: 'employer-b',
        role: 'employer',
        candidateId: null,
        companyId: 'company-b',
        email: 'hr@second-company.example',
        emailVerified: true,
      },
    ],
  ]);
  const initial = seeded(candidates, 'candidate024');
  const confirmedAt = initial.gradeConfirmedAt ?? CLOCK;
  const verifiedGrade = initial.verifiedGrade ?? 'junior';
  const historyId = 'assessment-seed-candidate024';
  const history: AssessmentRecord = {
    id: historyId,
    candidateId: initial.id,
    specialization: 'backend',
    grade: verifiedGrade,
    status: 'passed',
    task: { ...structuredClone(TASK), variantId: `orders-${verifiedGrade}-a` },
    answer: {
      code: '// Демонстрационный результат ранее завершённой проверки',
      explanation: 'Исторический результат из набора демонстрационных данных.',
      externalAiUsed: true,
    },
    checks: checks('pass'),
    result: {
      assessmentId: historyId,
      outcome: 'passed',
      confirmedGrade: verifiedGrade,
      previousGrade: scenario === 'qualified' ? 'junior' : null,
      gradeChanged: true,
      rubric: RUBRIC.map((criterion) => ({
        criterionId: criterion.id,
        label: criterion.label,
        outcome: 'met',
        feedback: 'Критерий выполнен в демонстрационном наборе.',
      })),
      checks: checks('pass'),
      feedback: 'Исторический демонстрационный результат.',
      nextEligibleAt: new Date(Date.parse(confirmedAt) + 90 * 86400000).toISOString(),
      simulation: true,
    },
    createdAt: confirmedAt,
    updatedAt: confirmedAt,
    revision: 1,
    simulation: true,
  };
  const tasks = seedTasks();
  return {
    scenario,
    clock: CLOCK,
    policy: { gradeChangeCooldownDays: 90 },
    timers: new Set(),
    candidates,
    companies,
    actors,
    sessions: new Map(),
    challenges: new Map(),
    needs: seedNeeds(),
    shortlists: new Map([
      [
        'shortlist-001',
        {
          id: 'shortlist-001',
          companyId: 'company-a',
          title: 'Бэкенд · Мидл',
          memberIds: ['candidate024', 'candidate018', 'candidate041'],
          createdAt: CLOCK,
          updatedAt: CLOCK,
        },
      ],
    ]),
    invitations: new Map(),
    applications: new Map(),
    tasks,
    taskResponses: seedTaskResponses(candidates, tasks),
    disclosures: new Map(),
    assessments: new Map([[historyId, history]]),
    interviews: new Map(),
    jobs: new Map(),
    idempotency: new Map(),
    counters: {},
  };
}

export function nextId(state: State, prefix: string): string {
  let id: string;
  do {
    state.counters[prefix] = (state.counters[prefix] ?? 0) + 1;
    id = `${prefix}-${String(state.counters[prefix]).padStart(3, '0')}`;
  } while (Object.values(state).some((value) => value instanceof Map && value.has(id)));
  return id;
}
