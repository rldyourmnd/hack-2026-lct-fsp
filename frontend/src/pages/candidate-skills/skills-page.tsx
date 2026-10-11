import { useQuery, useSuspenseQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Suspense, useCallback, useMemo, useState } from 'react';
import { assessmentsQuery } from '@/entities/assessment';
import { competitionsForSkill, useMeta } from '@/entities/meta';
import { type Profile, profileQuery } from '@/entities/profile';
import { useScope } from '@/entities/session';
import {
  type Evidence,
  type Graph,
  levelContext,
  myGraphQuery,
  mySkillsQuery,
  type SelfSkill,
  SkillStatusChip,
} from '@/entities/skill';
import { isDemoReference } from '@/entities/source';
import {
  evidenceSourceLabel,
  formatDate,
  gradeLabel,
  gradeRank,
  specializationLabel,
  useDocumentTitle,
} from '@/shared/lib';
import {
  ButtonLink,
  Card,
  EmptyState,
  ErrorState,
  Icon,
  type IconName,
  LoadingState,
  PageHeader,
  Segmented,
  TextField,
} from '@/shared/ui';
import { CompetenceGraph } from '@/widgets/competence-graph';
import styles from './skills-page.module.css';

export type SkillsView = 'list' | 'graph';

const evidenceIcon: Record<Evidence['source'], IconName> = { github: 'file', fsp: 'link', assessment: 'target' };

function EvidenceItem({ evidence, assessmentIds }: { evidence: Evidence; assessmentIds: Set<string> }) {
  const assessmentLink =
    evidence.source === 'assessment' && evidence.reference && assessmentIds.has(evidence.reference);
  return (
    <li className={styles.evidence}>
      <span className={styles.evidenceIcon}>
        <Icon name={evidenceIcon[evidence.source]} />
      </span>
      <div className="stack" style={{ gap: '0.25rem' }}>
        <strong>{evidence.title}</strong>
        <span className="small muted">
          {evidenceSourceLabel[evidence.source]}
          {evidence.reference && !assessmentLink ? ` · ${evidence.reference}` : ''} · {formatDate(evidence.observedAt)}
        </span>
        <span className="small">{evidence.scope === 'individual' ? 'Личный вклад' : 'Командная работа'}</span>
        {evidence.url &&
          (isDemoReference(evidence.url) ? (
            <span className="small muted">Демо-ссылка на источник: страница не размещена</span>
          ) : (
            <a className="small" href={evidence.url} target="_blank" rel="noreferrer noopener">
              Открыть источник ↗
            </a>
          ))}
        {assessmentLink && evidence.reference && (
          <Link
            className="small"
            to="/candidate/assessments/$assessmentId/result"
            params={{ assessmentId: evidence.reference }}
          >
            Открыть результат проверки →
          </Link>
        )}
        {evidence.synthetic && <span className="small muted">Синтетические данные демонстрации</span>}
      </div>
    </li>
  );
}

function SkillInspector({ skill, profile }: { skill: SelfSkill; profile: Profile }) {
  const meta = useMeta();
  const scope = useScope();
  const assessments = useQuery(assessmentsQuery(scope));
  const assessmentIds = useMemo(() => new Set(assessments.data?.items.map((item) => item.id)), [assessments.data]);
  const competitions = competitionsForSkill(meta, skill.skillId);
  const target = profile.selectedGrade;
  const needsCheck =
    skill.status !== 'confirmed' ||
    (skill.evidenceGrade !== null && gradeOrderIndex(skill.evidenceGrade) < gradeOrderIndex(target));
  return (
    <div className="stack-lg">
      <div className="row">
        <h2 style={{ margin: 0 }}>{skill.label}</h2>
        <SkillStatusChip status={skill.status} />
      </div>
      <p>{levelContext(skill, target)}</p>
      <p className="small muted">{skill.summary}</p>
      <section className="stack" aria-labelledby="inspector-evidence">
        <h3 id="inspector-evidence" style={{ margin: 0 }}>
          Основания
        </h3>
        {skill.evidence.length === 0 ? (
          <p className="muted">Свидетельств пока нет. Это пробел в данных, а не вывод об отсутствии навыка.</p>
        ) : (
          <ul className={styles.evidenceList}>
            {skill.evidence.map((evidence) => (
              <EvidenceItem key={evidence.id} evidence={evidence} assessmentIds={assessmentIds} />
            ))}
          </ul>
        )}
      </section>
      {needsCheck && (
        <section className="stack" aria-labelledby="inspector-next">
          <h3 id="inspector-next" style={{ margin: 0 }}>
            Что ещё проверить
          </h3>
          <p className="small">
            Тест знаний уровня «{gradeLabel[target]}» подтверждает навыки, которые входят в её рубрику.
          </p>
          <ButtonLink to="/candidate/assessments" variant="primary" iconAfter="arrowRight">
            {profile.assessmentEligibility.canStartSelected
              ? `Перейти к проверке «${gradeLabel[target]}»`
              : 'Условия проверки'}
          </ButtonLink>
          {competitions.length > 0 && (
            <div className="stack small">
              <strong>Соревнования из каталога</strong>
              <ul style={{ margin: 0, paddingLeft: '1.25rem' }}>
                {competitions.map((competition) => (
                  <li key={competition.id}>
                    {competition.name}
                    {competition.synthetic && ' — демонстрационная запись, не реальное событие'}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </section>
      )}
      <p className="small muted" style={{ display: 'flex', gap: '0.5rem' }}>
        <Icon name="info" size={18} />
        Источник и результат проверки хранятся отдельно: наблюдение в репозитории не превращается в подтверждённый
        навык.
      </p>
    </div>
  );
}

const gradeOrderIndex = (grade: Profile['selectedGrade']) => gradeRank(grade);

const sourceText: Record<string, string> = {
  'source-github':
    'Публичные репозитории: технологии, масштаб проектов и инженерная культура. Анализ не видит закрытых проектов.',
  'source-fsp': 'Результаты соревнований ФСП уточняют направление и уровень, но не подтверждают отдельные технологии.',
  'source-assessment': 'Тест знаний подтверждает уровень навыков и категорию.',
};

/** A concept lists its technologies; a source says what it proves; an evidence node names its skill. */
function GraphNodeInspector({ graph, nodeId, skills }: { graph: Graph; nodeId: string; skills: SelfSkill[] }) {
  const node = graph.nodes.find((item) => item.id === nodeId);
  if (!node) return <p className="muted">Элемент не найден</p>;
  if (node.kind === 'concept') {
    const members = graph.edges
      .filter((edge) => edge.from === node.id)
      .map((edge) => graph.nodes.find((item) => item.id === edge.to))
      .filter((item) => item?.kind === 'skill');
    return (
      <div className="stack">
        <h2 style={{ margin: 0 }}>{node.label}</h2>
        {node.detail && <p className="muted">{node.detail}</p>}
        <ul className={styles.evidenceList}>
          {members.map(
            (member) =>
              member && (
                <li key={member.id}>
                  <strong>{member.label}</strong>
                  {member.detail && <span className="small muted"> · {member.detail}</span>}
                </li>
              ),
          )}
        </ul>
      </div>
    );
  }
  if (node.kind === 'evidence') {
    const evidence = skills
      .flatMap((skill) => skill.evidence.map((item) => ({ item, skill })))
      .find(({ item }) => item.id === nodeId);
    if (evidence)
      return (
        <div className="stack">
          <h2 style={{ margin: 0 }}>{evidence.item.title}</h2>
          <p className="muted">Подтверждает навык: {evidence.skill.label}</p>
          <ul className={styles.evidenceList}>
            <EvidenceItem evidence={evidence.item} assessmentIds={new Set()} />
          </ul>
        </div>
      );
  }
  return (
    <div className="stack">
      <h2 style={{ margin: 0 }}>{node.label}</h2>
      {node.detail && <p className="muted">{node.detail}</p>}
      {sourceText[node.id] && <p className="small">{sourceText[node.id]}</p>}
    </div>
  );
}

type Props = {
  view: SkillsView;
  selected: string | undefined;
  onChange: (next: { view?: SkillsView; node?: string | undefined }) => void;
};

export function CandidateSkillsPage({ view, selected, onChange }: Props) {
  useDocumentTitle('Навыки и подтверждения');
  const scope = useScope();
  const { data: profile } = useSuspenseQuery(profileQuery(scope));
  const { data: skills } = useSuspenseQuery(mySkillsQuery(scope));
  const graph = useQuery({ ...myGraphQuery(scope), enabled: view === 'graph' });
  const [filter, setFilter] = useState('');
  const query = filter.trim().toLowerCase();
  const items = skills.items.filter((skill) => !query || skill.label.toLowerCase().includes(query));
  const selectedId = selected ?? (skills.items[0] ? `skill-${skills.items[0].skillId}` : null);
  const selectedSkill = skills.items.find((skill) => `skill-${skill.skillId}` === selectedId);
  const select = useCallback((node: string) => onChange({ node }), [onChange]);
  const highlight = useCallback(
    (node: Graph['nodes'][number]) => !query || node.kind === 'role' || node.label.toLowerCase().includes(query),
    [query],
  );

  const subtitle = [
    specializationLabel[profile.verifiedSpecialization ?? profile.specialization],
    profile.verifiedGrade
      ? `Подтверждён ${gradeLabel[profile.verifiedGrade]}`
      : profile.category.grade
        ? `Оценка ${gradeLabel[profile.category.grade]}`
        : 'Категории нет',
    `Цель ${gradeLabel[profile.selectedGrade]}`,
  ].join(' · ');

  return (
    <>
      <PageHeader
        title="Навыки и подтверждения"
        subtitle={subtitle}
        actions={
          <Segmented<SkillsView>
            label="Представление"
            value={view}
            onChange={(next) => onChange({ view: next })}
            options={[
              { value: 'list', label: 'Список' },
              { value: 'graph', label: 'Граф' },
            ]}
          />
        }
      />
      {skills.items.length === 0 ? (
        <Card>
          <EmptyState
            icon="chart"
            title="Навыков пока нет"
            actions={
              <div className="row">
                <ButtonLink to="/candidate/sources">Подключить GitHub</ButtonLink>
                <ButtonLink to="/candidate/assessments" variant="primary">
                  Пройти проверку
                </ButtonLink>
              </div>
            }
          >
            Граф компетенций строится из проверок и автоматического анализа источников.
          </EmptyState>
        </Card>
      ) : (
        <div className={styles.layout}>
          <Card>
            <div className={styles.search}>
              <TextField
                id="skill-filter"
                label="Найти навык"
                type="search"
                value={filter}
                onChange={(event) => setFilter(event.target.value)}
              />
            </div>
            {view === 'graph' ? (
              <>
                {graph.isPending && <LoadingState label="Строим граф…" />}
                {graph.isError && (
                  <>
                    <ErrorState error={graph.error} onRetry={() => graph.refetch()} retrying={graph.isFetching} />
                    <p className="small muted" style={{ marginTop: '1rem' }}>
                      Пока граф недоступен, используйте список.
                    </p>
                  </>
                )}
                {graph.data && !graph.isError && (
                  <Suspense fallback={<LoadingState label="Загружаем граф…" />}>
                    <CompetenceGraph
                      graph={graph.data}
                      selectedId={selectedId}
                      highlight={highlight}
                      onSelect={select}
                      label="Граф компетенций: категория, источники, понятия и технологии"
                    />
                  </Suspense>
                )}
                <div className={styles.legend}>
                  <span>
                    Сверху — источники (GitHub, ФСП, тест), под ними категория, ниже — понятия и технологии с уровнем:
                    основная, регулярно, встречается или подтверждена тестом.
                  </span>
                  <span>
                    <Icon name="target" size={16} /> Тест знаний
                  </span>
                  <span>
                    <Icon name="minusCircle" size={16} /> Нужно подтверждение
                  </span>
                </div>
              </>
            ) : (
              <ul className={styles.list} aria-label="Навыки">
                {items.map((skill) => (
                  <li key={skill.skillId}>
                    <button
                      type="button"
                      className={styles.item}
                      aria-pressed={`skill-${skill.skillId}` === selectedId}
                      onClick={() => select(`skill-${skill.skillId}`)}
                    >
                      <span className="stack" style={{ gap: '0.25rem' }}>
                        <strong>{skill.label}</strong>
                        <span className="small muted">
                          {levelContext(skill, profile.selectedGrade)} · свидетельств: {skill.evidenceCount}
                        </span>
                      </span>
                      <SkillStatusChip status={skill.status} />
                    </button>
                  </li>
                ))}
                {items.length === 0 && <li className="muted">Ничего не найдено по запросу «{filter}».</li>}
              </ul>
            )}
          </Card>
          <Card as="aside" className={styles.inspector} aria-label="Подробности выбранного элемента">
            {selectedSkill ? (
              <SkillInspector skill={selectedSkill} profile={profile} />
            ) : selectedId && graph.data ? (
              selectedId === 'role-main' ? (
                <div className="stack">
                  <h2 style={{ margin: 0 }}>{subtitle}</h2>
                  <p className="muted">Выберите навык, чтобы увидеть основания и уровень, к которому они относятся.</p>
                </div>
              ) : (
                <GraphNodeInspector graph={graph.data} nodeId={selectedId} skills={skills.items} />
              )
            ) : (
              <p className="muted">Выберите навык в списке или на графе.</p>
            )}
          </Card>
        </div>
      )}
    </>
  );
}
