import { useQuery, useSuspenseQuery } from '@tanstack/react-query';
import { Suspense, useCallback, useState } from 'react';
import {
  candidateGapPlanQuery,
  candidateGraphQuery,
  candidateQuery,
  disclosureQuery,
  type MatchRequirement,
  RequirementStatusChip,
} from '@/entities/candidate';
import { companyQuery } from '@/entities/company';
import { companyInvitationsQuery, InvitationStatusChip, isOpenInvitation } from '@/entities/invitation';
import { skillLabel, useMeta } from '@/entities/meta';
import { needsQuery } from '@/entities/need';
import { CategoryBadge, FreshnessChip } from '@/entities/qualification';
import { useScope } from '@/entities/session';
import { shortlistsQuery } from '@/entities/shortlist';
import { SkillStatusChip, skillStatusView } from '@/entities/skill';
import { isDemoReference } from '@/entities/source';
import { InvitationComposer } from '@/features/send-invitation';
import { BookmarkButton, useShortlistMembership } from '@/features/shortlist-membership';
import {
  criticalityLabel,
  evidenceSourceLabel,
  formatDate,
  formatDateTime,
  gradeLabel,
  specializationLabel,
  useDocumentTitle,
  workModeLabel,
} from '@/shared/lib';
import {
  Avatar,
  Button,
  ButtonLink,
  Card,
  EmptyState,
  ErrorState,
  Icon,
  LoadingState,
  panelId,
  SalaryBlock,
  Segmented,
  SelectField,
  StatusChip,
  type TabItem,
  TableWrap,
  Tabs,
  tabId,
} from '@/shared/ui';
import { CompetenceGraph } from '@/widgets/competence-graph';
import { GitHubSignalCard } from '@/widgets/github-insights';
import styles from './candidate-page.module.css';

export type CandidateTab = 'match' | 'skills' | 'checks';

function MatchRow({ requirement }: { requirement: MatchRequirement }) {
  const meta = useMeta();
  const [open, setOpen] = useState(false);
  const label = skillLabel(meta, requirement.skillId);
  return (
    <>
      <tr>
        <td>
          <strong>{label}</strong> <span className="muted">· {criticalityLabel[requirement.criticality]}</span>
        </td>
        <td>
          <RequirementStatusChip status={requirement.status} plain />
        </td>
        <td>{skillStatusView[requirement.evidenceStatus].label}</td>
        <td>
          <Button
            variant="quiet"
            icon={open ? 'chevronUp' : 'chevronDown'}
            aria-expanded={open}
            aria-label={`${open ? 'Скрыть' : 'Показать'} основания: ${label}`}
            onClick={() => setOpen((current) => !current)}
          />
        </td>
      </tr>
      {open && (
        <tr>
          <td colSpan={4} className={styles.detailCell}>
            <div className="stack">
              <p>{requirement.reason}</p>
              <p className="small muted">
                Статус свидетельств по навыку: «{skillStatusView[requirement.evidenceStatus].label}». Названия проектов,
                ссылки и фрагменты кода не показываются до согласия кандидата. Наблюдение в репозитории — основание, а
                не подтверждение уровня.
              </p>
            </div>
          </td>
        </tr>
      )}
    </>
  );
}

function MatchTab({ candidateId, needId }: { candidateId: string; needId: string | undefined }) {
  const scope = useScope();
  const meta = useMeta();
  const candidate = useQuery(candidateQuery(scope, candidateId, needId));
  const gaps = useQuery({ ...candidateGapPlanQuery(scope, candidateId, needId ?? 'none'), enabled: Boolean(needId) });
  if (!needId) {
    return (
      <EmptyState icon="briefcase" title="Выберите потребность">
        Соответствие считается по требованиям конкретной потребности.
      </EmptyState>
    );
  }
  if (!candidate.data) return <LoadingState />;
  const match = candidate.data.match;
  if (!match) return <p className="muted">Сравнение недоступно.</p>;
  return (
    <div className="stack-lg">
      <div className="row" style={{ justifyContent: 'space-between' }}>
        <h2 style={{ margin: 0, fontSize: '1.25rem' }}>Проверка соответствия требованиям</h2>
        <StatusChip tone={match.status === 'meets_required' ? 'success' : 'warning'}>
          {match.requiredMet} из {match.requiredTotal} обязательных требований подтверждены
        </StatusChip>
      </div>
      {!match.gradeMatches && <p>{match.explanation}</p>}
      <TableWrap label="Проверка соответствия требованиям">
        <table className="table">
          <thead>
            <tr>
              <th scope="col">Требование</th>
              <th scope="col">Результат</th>
              <th scope="col">Свидетельства</th>
              <th scope="col">
                <span className="visually-hidden">Подробнее</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {match.requirements.map((requirement) => (
              <MatchRow key={requirement.skillId} requirement={requirement} />
            ))}
          </tbody>
        </table>
      </TableWrap>
      <p className="small muted">
        Подтверждённый грейд и требование по отдельному навыку — разные статусы. Алгоритм: {match.algorithmVersion}; это
        объяснение правил, а не вероятность успеха.
      </p>
      {gaps.data && gaps.data.items.length > 0 && (
        <section className="stack" aria-labelledby="gap-plan-title">
          <h3 id="gap-plan-title" style={{ margin: 0, fontSize: '1rem' }}>
            Как закрыть пробелы
          </h3>
          <ul className="stack" style={{ margin: 0, paddingLeft: '1.25rem' }}>
            {gaps.data.items.map((item) => (
              <li key={item.skillId}>
                <strong>{skillLabel(meta, item.skillId)}:</strong> {item.action}
                {item.acceptedCompetitionIds.length > 0 && (
                  <span className="small muted">
                    {' '}
                    Принимаемые соревнования:{' '}
                    {item.acceptedCompetitionIds
                      .map((id) => meta.competitions.find((competition) => competition.id === id)?.name ?? id)
                      .join(', ')}
                    .
                  </span>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

function SkillsTab({ candidateId }: { candidateId: string }) {
  const scope = useScope();
  const meta = useMeta();
  const candidate = useQuery(candidateQuery(scope, candidateId));
  const [view, setView] = useState<'list' | 'graph'>('list');
  const graph = useQuery({ ...candidateGraphQuery(scope, candidateId), enabled: view === 'graph' });
  const [selected, setSelected] = useState<string | null>(null);
  const select = useCallback((id: string) => setSelected(id), []);
  if (!candidate.data) return <LoadingState />;
  return (
    <div className="stack">
      <Segmented<'list' | 'graph'>
        label="Представление навыков"
        value={view}
        onChange={setView}
        options={[
          { value: 'list', label: 'Список' },
          { value: 'graph', label: 'Граф' },
        ]}
      />
      {view === 'graph' ? (
        <>
          {graph.isPending && <LoadingState label="Строим граф…" />}
          {graph.isError && (
            <>
              <ErrorState error={graph.error} onRetry={() => graph.refetch()} retrying={graph.isFetching} />
              <p className="small muted">Пока граф недоступен, используйте список.</p>
            </>
          )}
          {graph.data && !graph.isError && (
            <Suspense fallback={<LoadingState label="Загружаем граф…" />}>
              <CompetenceGraph
                graph={graph.data}
                selectedId={selected}
                onSelect={select}
                label="Граф навыков кандидата"
              />
            </Suspense>
          )}
        </>
      ) : (
        <ul className="stack" style={{ margin: 0, padding: 0, listStyle: 'none' }}>
          {candidate.data.skills.map((skill) => (
            <li
              key={skill.skillId}
              className={styles.skill}
              aria-current={selected === `skill-${skill.skillId}` || undefined}
            >
              <div className="row" style={{ justifyContent: 'space-between' }}>
                <strong>{skillLabel(meta, skill.skillId)}</strong>
                <SkillStatusChip status={skill.status} />
              </div>
              <span className="small muted">
                {skill.summary}
                {skill.evidenceGrade ? ` · уровень «${gradeLabel[skill.evidenceGrade]}»` : ''}
                {skill.evidenceSources.length
                  ? ` · источники: ${skill.evidenceSources.map((source) => evidenceSourceLabel[source]).join(', ')}`
                  : ''}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function ChecksTab({ candidateId }: { candidateId: string }) {
  const scope = useScope();
  const meta = useMeta();
  const candidate = useQuery(candidateQuery(scope, candidateId));
  if (!candidate.data) return <LoadingState />;
  const confirmed = candidate.data.skills.filter((skill) => skill.evidenceSources.includes('assessment'));
  const { specialization, grade, estimate, verifiedGrade, gradeConfirmedAt } = candidate.data;
  const category = `${specializationLabel[specialization]} · ${gradeLabel[grade]}`;
  return (
    <div className="stack">
      {candidate.data.categorySource === 'assessment' ? (
        <p>
          Категория «{category}» подтверждена тестом знаний платформы
          {gradeConfirmedAt ? ` ${formatDate(gradeConfirmedAt)}` : ''}. Результат не подтверждает практический опыт
          разработки.
        </p>
      ) : (
        <p>
          Категория «{category}» — автоматическая оценка по открытым данным кандидата (GitHub, ФСП). Тестом она пока не
          подтверждена{verifiedGrade ? `; подтверждённый грейд — «${gradeLabel[verifiedGrade]}»` : ''}.
        </p>
      )}
      {estimate && (
        <section className="stack" aria-label="Факторы оценки">
          <strong>
            Балл уровня: {estimate.score} из 100 · рассчитан {formatDate(estimate.computedAt)}
          </strong>
          <TableWrap label="Факторы оценки">
            <table className="table">
              <thead>
                <tr>
                  <th scope="col">Фактор</th>
                  <th scope="col">Баллы</th>
                </tr>
              </thead>
              <tbody>
                {estimate.factors.map((factor) => (
                  <tr key={factor.id}>
                    <td>{factor.label}</td>
                    <td>
                      {factor.points} из {factor.max}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </TableWrap>
          <p className="small muted">
            Пороги: до 20 — стажёр, 20–39 — джуниор, 40–59 — мидл, 60–79 — сеньор, от 80 — лид. Объём коммитов сам по
            себе грейд не повышает.
          </p>
        </section>
      )}
      <ul className="stack" style={{ margin: 0, paddingLeft: '1.25rem' }}>
        {confirmed.map((skill) => (
          <li key={skill.skillId}>
            {skillLabel(meta, skill.skillId)}: подтверждено на уровне «
            {skill.evidenceGrade ? gradeLabel[skill.evidenceGrade] : '—'}»
            {skill.confirmedAt ? `, ${formatDate(skill.confirmedAt)}` : ''}
          </li>
        ))}
      </ul>
      <p className="small muted">Решения и ответы кандидата не раскрываются. ИИ на проверках разрешён.</p>
    </div>
  );
}

function DisclosureCard({ candidateId }: { candidateId: string }) {
  const scope = useScope();
  const disclosure = useQuery(disclosureQuery(scope, candidateId));
  const grant = disclosure.data;
  if (!grant?.granted) {
    return (
      <p className="small muted" style={{ display: 'flex', gap: '0.5rem' }}>
        <Icon name="lock" size={18} />
        Контакты и ссылки скрыты до согласия кандидата.
      </p>
    );
  }
  const fields = grant.fields;
  return (
    <section className={styles.disclosure} aria-labelledby="disclosure-title">
      <div className="stack">
        <h3 id="disclosure-title" style={{ margin: 0, fontSize: '1rem' }}>
          Контакты, открытые кандидатом
        </h3>
        <p className="small muted">
          {grant.grantedAt ? formatDateTime(grant.grantedAt) : ''}. Доступны только вашей компании и только выбранные
          кандидатом поля.
        </p>
        {fields.name && (
          <p>
            <strong>{fields.name}</strong>
          </p>
        )}
        {fields.email && <a href={`mailto:${fields.email}`}>{fields.email}</a>}
        {fields.sourceLinks && fields.sourceLinks.length > 0 && (
          <ul className="small" style={{ margin: 0, paddingLeft: '1.25rem' }}>
            {fields.sourceLinks.map((link) => (
              <li key={link}>
                {isDemoReference(link) ? (
                  `${link.replace('https://example.com/fixtures/', '')} (демо-ссылка)`
                ) : (
                  <a href={link} target="_blank" rel="noreferrer noopener">
                    {link}
                  </a>
                )}
              </li>
            ))}
          </ul>
        )}
        {!fields.sourceLinks && <p className="small muted">Ссылки на источники кандидат не открывал.</p>}
      </div>
    </section>
  );
}

type Props = {
  candidateId: string;
  needId: string | undefined;
  tab: CandidateTab;
  shortlistId: string | undefined;
  onChange: (patch: { needId?: string | undefined; tab?: CandidateTab }) => void;
};

export function EmployerCandidatePage({ candidateId, needId, tab, shortlistId, onChange }: Props) {
  const scope = useScope();
  const { data: candidate } = useSuspenseQuery(candidateQuery(scope, candidateId, needId));
  useDocumentTitle(candidate.alias);
  const { data: needs } = useSuspenseQuery(needsQuery(scope));
  const { data: shortlists } = useSuspenseQuery(shortlistsQuery(scope));
  const company = useQuery(companyQuery(scope));
  const invitations = useQuery(companyInvitationsQuery(scope));
  const membership = useShortlistMembership(shortlistId ?? shortlists.items[0]?.id ?? null);
  const [composing, setComposing] = useState(false);
  const need = needs.items.find((item) => item.id === needId);
  const existing = invitations.data?.items.find(
    (item) => item.candidateId === candidateId && (!needId || item.need.id === needId),
  );
  const tabs: TabItem<CandidateTab>[] = [
    { id: 'match', label: 'Соответствие' },
    { id: 'skills', label: 'Навыки' },
    { id: 'checks', label: 'Категория' },
  ];
  const member = membership.isMember(candidateId);

  return (
    <>
      <ButtonLink to="/employer/candidates" search={needId ? { needId } : {}} variant="quiet" icon="arrowLeft">
        Назад к кандидатам
      </ButtonLink>
      <Card style={{ margin: '0.75rem 0 1.5rem' }}>
        <div className={styles.hero}>
          <div className="row" style={{ alignItems: 'flex-start', gap: '1.5rem' }}>
            <Avatar kind="anonymous" size="lg" />
            <div className="stack" style={{ gap: '0.5rem' }}>
              <h1 style={{ margin: 0, fontSize: '2rem' }}>{candidate.alias}</h1>
              <span className="muted">
                {specializationLabel[candidate.specialization]} · {gradeLabel[candidate.grade]}
              </span>
              <div className="row">
                <CategoryBadge
                  specialization={candidate.specialization}
                  grade={candidate.grade}
                  source={candidate.categorySource}
                  confidence={candidate.categoryConfidence}
                />
                <FreshnessChip freshness={candidate.freshness} />
                {candidate.availableForOffers && (
                  <StatusChip tone="neutral" icon="chart">
                    Рассматривает предложения
                  </StatusChip>
                )}
                {candidate.fspVerified && (
                  <StatusChip tone="accent" icon="link">
                    ФСП: подтверждённое участие · демо
                  </StatusChip>
                )}
              </div>
              <span className="small muted">
                Формат: {candidate.workModes.map((mode) => workModeLabel[mode]).join(', ')}
              </span>
            </div>
          </div>
          <div className={styles.needPicker}>
            <SelectField
              id="candidate-need"
              label="Потребность для сравнения"
              value={needId ?? ''}
              onValueChange={(value) => onChange({ needId: value || undefined })}
              options={[
                { value: '', label: 'Без сравнения' },
                ...needs.items.map((item) => ({ value: item.id, label: item.title })),
              ]}
            />
          </div>
        </div>
      </Card>

      <div className="split">
        <Card>
          <Tabs
            items={tabs}
            value={tab}
            onChange={(next) => onChange({ tab: next })}
            label="Разделы профиля"
            idPrefix="candidate"
          />
          <div
            role="tabpanel"
            id={panelId('candidate', tab)}
            aria-labelledby={tabId('candidate', tab)}
            style={{ paddingTop: '1.25rem' }}
          >
            {tab === 'match' && <MatchTab candidateId={candidateId} needId={needId} />}
            {tab === 'skills' && <SkillsTab candidateId={candidateId} />}
            {tab === 'checks' && <ChecksTab candidateId={candidateId} />}
          </div>
        </Card>
        <div className="stack-lg">
          <Card title={`Пригласить в «${company.data?.displayName ?? 'компанию'}»`} tinted>
            <div className="stack">
              {need ? (
                <>
                  <strong>{need.title}</strong>
                  <SalaryBlock salary={need.salary} />
                  <span className="small">{workModeLabel[need.workMode]}</span>
                </>
              ) : (
                <p className="small muted">Выберите потребность — её условия попадут в приглашение.</p>
              )}
              {existing ? (
                <div className="stack">
                  <InvitationStatusChip status={existing.status} audience="employer" />
                  <span className="small muted">
                    Приглашение по «{existing.need.title}», отправлено {formatDate(existing.createdAt)}
                    {isOpenInvitation(existing) ? ' Ждём решения кандидата.' : ''}
                  </span>
                  <ButtonLink to="/employer/invitations" size="sm">
                    Все приглашения
                  </ButtonLink>
                </div>
              ) : (
                <Button
                  variant="primary"
                  iconAfter="arrowRight"
                  disabled={!company.data}
                  onClick={() => setComposing(true)}
                >
                  Пригласить кандидата
                </Button>
              )}
              <BookmarkButton
                withLabel
                alias={candidate.alias}
                shortlistTitle={membership.shortlist.data?.title}
                member={member}
                pending={membership.toggle.isPending || !membership.shortlist.data}
                onToggle={() => membership.toggle.mutate({ candidateId, alias: candidate.alias, member })}
              />
              <DisclosureCard candidateId={candidateId} />
            </div>
          </Card>
          {candidate.github && <GitHubSignalCard signal={candidate.github} />}
        </div>
      </div>
      {company.data && composing && (
        <InvitationComposer
          open={composing}
          onClose={() => setComposing(false)}
          candidate={candidate}
          needs={needs.items}
          defaultNeedId={needId}
          company={company.data}
          onSent={() => setComposing(false)}
        />
      )}
    </>
  );
}
