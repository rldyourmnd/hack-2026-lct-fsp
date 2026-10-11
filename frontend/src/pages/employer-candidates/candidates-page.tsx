import { useQuery, useSuspenseQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  type AnonymousCandidate,
  aliasShape,
  type CandidateFilters,
  candidateSearchQuery,
  matchGroupView,
  RequirementStatusChip,
} from '@/entities/candidate';
import { bandLabel } from '@/entities/github-insights';
import { type SkillId, skillLabel, useMeta } from '@/entities/meta';
import { needsQuery } from '@/entities/need';
import { estimateConfidenceLabel } from '@/entities/qualification';
import { useScope } from '@/entities/session';
import { shortlistsQuery } from '@/entities/shortlist';
import { BookmarkButton, useShortlistMembership } from '@/features/shortlist-membership';
import { isApiError } from '@/shared/api';
import {
  gradeLabel,
  gradeOrder,
  skillOptions,
  specializationLabel,
  specializationOptions,
  type specializationOrder,
  useDocumentTitle,
} from '@/shared/lib';
import {
  Button,
  ButtonLink,
  Card,
  EmptyState,
  ErrorState,
  Icon,
  InlineStatus,
  LoadingState,
  PageHeader,
  Pagination,
  Segmented,
  SelectField,
  StatusChip,
  TableWrap,
} from '@/shared/ui';
import { ShortlistPanel } from '@/widgets/shortlist-panel';
import styles from './candidates-page.module.css';

type Grade = (typeof gradeOrder)[number];
type Specialization = (typeof specializationOrder)[number];

export type CandidateSearch = {
  needId?: string | undefined;
  mode?: 'recommended' | 'bank' | undefined;
  specialization?: Specialization | undefined;
  grade?: Grade | undefined;
  categorySource?: 'assessment' | 'estimate' | undefined;
  skill?: SkillId | undefined;
  fsp?: 'any' | 'verified' | undefined;
  page?: number | undefined;
  shortlistId?: string | undefined;
};

const PAGE_SIZE = 10;
const groupOrder = ['meets_required', 'needs_verification', 'missing_required'] as const;

type Group = {
  key: string;
  title: string;
  icon: 'checkCircle' | 'alert' | 'minusCircle' | 'users';
  items: AnonymousCandidate[];
};

function groupResults(items: AnonymousCandidate[], withNeed: boolean): Group[] {
  if (withNeed) {
    return groupOrder
      .map((status) => ({
        key: status,
        title: matchGroupView[status].label,
        icon: matchGroupView[status].icon as Group['icon'],
        items: items.filter((item) => item.match?.status === status),
      }))
      .filter((group) => group.items.length > 0);
  }
  const groups = new Map<string, Group>();
  for (const item of items) {
    const key = `${item.specialization}:${item.grade}`;
    const group = groups.get(key) ?? {
      key,
      title: `${specializationLabel[item.specialization]} · ${gradeLabel[item.grade]}`,
      icon: 'users' as const,
      items: [],
    };
    group.items.push(item);
    groups.set(key, group);
  }
  return [...groups.values()];
}

type Props = { search: CandidateSearch; onSearchChange: (patch: Partial<CandidateSearch>) => void };

export function EmployerCandidatesPage({ search, onSearchChange }: Props) {
  useDocumentTitle('Кандидаты');
  const scope = useScope();
  const meta = useMeta();
  const { data: needs } = useSuspenseQuery(needsQuery(scope));
  const { data: shortlists } = useSuspenseQuery(shortlistsQuery(scope));
  const need = needs.items.find((item) => item.id === search.needId);
  const mode = search.mode ?? (search.needId ? 'recommended' : 'bank');
  const page = search.page ?? 1;
  const filters: CandidateFilters = {
    mode,
    page,
    pageSize: PAGE_SIZE,
    fsp: search.fsp ?? 'any',
    ...(search.needId ? { needId: search.needId } : {}),
    ...(search.specialization ? { specialization: search.specialization } : {}),
    ...(search.grade ? { grade: search.grade } : {}),
    ...(search.categorySource ? { categorySource: search.categorySource } : {}),
    ...(search.skill ? { skill: search.skill } : {}),
  };
  const canQuery = mode === 'bank' || Boolean(search.needId);
  const results = useQuery({ ...candidateSearchQuery(scope, filters), enabled: canQuery });
  const shortlistId = search.shortlistId ?? shortlists.items[0]?.id ?? null;
  const membership = useShortlistMembership(shortlistId);
  const filtered = Boolean(
    search.specialization ||
      search.grade ||
      search.categorySource ||
      search.skill ||
      (search.fsp && search.fsp !== 'any'),
  );
  const resetFilters = () =>
    onSearchChange({
      specialization: undefined,
      grade: undefined,
      categorySource: undefined,
      skill: undefined,
      fsp: undefined,
      page: undefined,
    });

  const toggle = (candidate: AnonymousCandidate) =>
    membership.toggle.mutate({
      candidateId: candidate.id,
      alias: candidate.alias,
      member: membership.isMember(candidate.id),
    });

  return (
    <>
      <PageHeader title="Кандидаты" subtitle="Подбор и оценка кандидатов для ваших потребностей" />
      <Card className={styles.needBar}>
        <div className={styles.needSelect}>
          <SelectField
            id="search-need"
            label="Для потребности"
            value={search.needId ?? ''}
            onValueChange={(needId) =>
              onSearchChange({ needId: needId || undefined, mode: needId ? 'recommended' : 'bank', page: undefined })
            }
            options={[
              { value: '', label: 'Без потребности — весь банк' },
              ...needs.items.map((item) => ({ value: item.id, label: item.title })),
            ]}
          />
        </div>
        {need && (
          <ButtonLink to="/employer/needs/$needId" params={{ needId: need.id }} variant="quiet" icon="edit">
            Изменить потребность
          </ButtonLink>
        )}
        {needs.items.length === 0 && (
          <ButtonLink to="/employer/needs/new" variant="primary" icon="plus">
            Описать потребность
          </ButtonLink>
        )}
      </Card>

      <div className={styles.controls}>
        <Segmented<'recommended' | 'bank'>
          label="Режим поиска"
          value={mode}
          onChange={(next) => onSearchChange({ mode: next, page: undefined })}
          options={[
            {
              value: 'recommended',
              label: 'Рекомендации',
              disabled: !search.needId,
              title: search.needId ? undefined : 'Выберите потребность, чтобы получить рекомендации',
            },
            { value: 'bank', label: 'Весь банк' },
          ]}
        />
        {!search.needId && <span className="small muted">Рекомендации строятся по сохранённой потребности.</span>}
      </div>

      <Card className={styles.filters}>
        <div className={styles.filterGrid}>
          <SelectField
            id="filter-specialization"
            label="Специализация"
            value={search.specialization ?? ''}
            onValueChange={(value) => onSearchChange({ specialization: value || undefined, page: undefined })}
            options={[{ value: '', label: 'Любая' }, ...specializationOptions]}
          />
          <SelectField
            id="filter-grade"
            label="Грейд"
            value={search.grade ?? ''}
            onValueChange={(value) => onSearchChange({ grade: value || undefined, page: undefined })}
            options={[
              { value: '', label: 'Любой' },
              ...gradeOrder.map((value) => ({ value, label: gradeLabel[value] })),
            ]}
          />
          <SelectField
            id="filter-category-source"
            label="Подтверждение категории"
            value={search.categorySource ?? ''}
            onValueChange={(value) => onSearchChange({ categorySource: value || undefined, page: undefined })}
            options={[
              { value: '', label: 'Любое' },
              { value: 'assessment', label: 'Тест знаний' },
              { value: 'estimate', label: 'Автооценка' },
            ]}
          />
          <SelectField
            id="filter-skill"
            label="Стек"
            value={search.skill ?? ''}
            onValueChange={(value) =>
              onSearchChange({ skill: (value || undefined) as SkillId | undefined, page: undefined })
            }
            options={[{ value: '', label: 'Любой' }, ...skillOptions]}
          />
          <SelectField
            id="filter-fsp"
            label="ФСП"
            value={search.fsp ?? 'any'}
            onValueChange={(value) => onSearchChange({ fsp: value === 'any' ? undefined : value, page: undefined })}
            options={[
              { value: 'any', label: 'Неважно' },
              { value: 'verified', label: 'Подтверждённое участие' },
            ]}
          />
        </div>
        <Button variant="quiet" icon="x" disabled={!filtered} onClick={resetFilters}>
          Сбросить фильтры
        </Button>
        <p className="small muted" style={{ flexBasis: '100%' }}>
          Категорию даёт тест знаний или автоматическая оценка по GitHub и ФСП; подтверждённые тестом стоят выше в своей
          категории. Фильтр по стеку учитывает и наблюдения в работе; подтверждение навыка показано в столбце
          соответствия.
        </p>
      </Card>

      <div className={styles.layout}>
        <Card
          title="Найденные кандидаты"
          actions={
            <span className="row small muted">
              {results.data && <span>Найдено: {results.data.total}</span>}
              <span>Порядок: {need ? 'по соответствию потребности' : 'по категории'} · задаёт сервер</span>
            </span>
          }
        >
          {!canQuery && (
            <EmptyState icon="briefcase" title="Выберите потребность">
              Рекомендации строятся по потребности. Или переключитесь на весь банк кандидатов.
            </EmptyState>
          )}
          {canQuery && results.isPending && <LoadingState label="Ищем кандидатов…" lines={5} />}
          {canQuery && results.isError && (
            <ErrorState
              error={results.error}
              title={isApiError(results.error, 404) ? 'Потребность недоступна' : undefined}
              onRetry={isApiError(results.error, 404) ? undefined : () => results.refetch()}
              retrying={results.isFetching}
            />
          )}
          {results.data && results.isFetching && results.isPlaceholderData && (
            <InlineStatus>Обновляем результаты…</InlineStatus>
          )}
          {results.data && results.data.items.length === 0 && (
            <EmptyState
              icon="search"
              title="Кандидаты не найдены"
              actions={
                <div className="row">
                  {filtered && <Button onClick={resetFilters}>Сбросить фильтры</Button>}
                  {mode === 'recommended' && (
                    <Button onClick={() => onSearchChange({ mode: 'bank', page: undefined })}>Весь банк</Button>
                  )}
                  {need && (
                    <ButtonLink to="/employer/needs/$needId" params={{ needId: need.id }}>
                      Изменить потребность
                    </ButtonLink>
                  )}
                </div>
              }
            >
              В банке видны только кандидаты с категорией — по тесту или по оценке открытых данных, — согласившиеся на
              показ профиля. Сохранённая подборка не меняется.
            </EmptyState>
          )}
          {results.data && results.data.items.length > 0 && (
            <TableWrap label="Найденные кандидаты">
              <table className="table">
                <thead>
                  <tr>
                    <th scope="col">Кандидат</th>
                    <th scope="col">Квалификация</th>
                    <th scope="col">Стек</th>
                    <th scope="col">Соответствие</th>
                    <th scope="col">
                      <span className="visually-hidden">Подборка</span>
                    </th>
                  </tr>
                </thead>
                {groupResults(results.data.items, Boolean(need)).map((group) => (
                  <tbody key={group.key}>
                    <tr className={styles.groupRow}>
                      <th scope="colgroup" colSpan={5}>
                        <span className="row">
                          <Icon name={group.icon} />
                          {group.title}
                          <span className={styles.count}>{group.items.length}</span>
                        </span>
                      </th>
                    </tr>
                    {group.items.map((candidate) => {
                      const member = membership.isMember(candidate.id);
                      const desiredGaps = candidate.match?.requirements.filter(
                        (item) => item.criticality === 'desired' && item.status !== 'meets',
                      );
                      const requiredGap = candidate.match?.requirements.find(
                        (item) => item.criticality === 'required' && item.status !== 'meets',
                      );
                      return (
                        <tr key={candidate.id} className={member ? styles.saved : undefined}>
                          <td>
                            <span className="row">
                              <span className={styles.shape} aria-hidden="true">
                                {aliasShape(candidate.alias)}
                              </span>
                              <Link
                                to="/employer/candidates/$candidateId"
                                params={{ candidateId: candidate.id }}
                                search={search.needId ? { needId: search.needId } : {}}
                              >
                                <strong>{candidate.alias}</strong>
                              </Link>
                            </span>
                          </td>
                          <td>
                            {specializationLabel[candidate.specialization]} · {gradeLabel[candidate.grade]}
                            <div>
                              {candidate.categorySource === 'assessment' ? (
                                <StatusChip tone="success" plain>
                                  Подтверждено тестом
                                </StatusChip>
                              ) : (
                                <StatusChip tone="info" icon="chart" plain>
                                  Оценка
                                  {candidate.categoryConfidence
                                    ? `, ${estimateConfidenceLabel[candidate.categoryConfidence]}`
                                    : ''}
                                </StatusChip>
                              )}
                            </div>
                            {candidate.estimate && (
                              <div className="small muted">Балл уровня: {candidate.estimate.score} из 100</div>
                            )}
                            {candidate.fspVerified && <div className="small muted">ФСП: подтверждённое участие</div>}
                            {candidate.github && (
                              <div className="small muted">
                                GitHub: {candidate.github.index.score} из 100,{' '}
                                {bandLabel[candidate.github.index.band].toLowerCase()}
                                {candidate.github.ownershipVerified ? '' : ' · владение не подтверждено'}
                              </div>
                            )}
                            <div className="small muted">
                              {candidate.freshness.fresh ? 'Профиль актуален' : 'Давно не обновлялся'}
                            </div>
                          </td>
                          <td>
                            {candidate.skills
                              .filter((skill) => skill.status !== 'insufficient')
                              .map((skill) => skillLabel(meta, skill.skillId))
                              .join(', ') || '—'}
                          </td>
                          <td>
                            {candidate.match ? (
                              <div className="stack" style={{ gap: '0.25rem' }}>
                                {requiredGap ? (
                                  <RequirementStatusChip status={requiredGap.status} plain />
                                ) : (
                                  <StatusChip tone="success" plain>
                                    {candidate.match.requiredMet} из {candidate.match.requiredTotal} обязательных
                                  </StatusChip>
                                )}
                                {requiredGap && (
                                  <span className="small">
                                    {skillLabel(meta, requiredGap.skillId)}: {requiredGap.reason}
                                  </span>
                                )}
                                {!candidate.match.gradeMatches && (
                                  <span className="small">{candidate.match.explanation}</span>
                                )}
                                {desiredGaps && desiredGaps.length > 0 && (
                                  <span className="small muted">
                                    Желательно проверить:{' '}
                                    {desiredGaps.map((item) => skillLabel(meta, item.skillId)).join(', ')}
                                  </span>
                                )}
                              </div>
                            ) : (
                              <span className="small muted">Выберите потребность для сравнения</span>
                            )}
                          </td>
                          <td>
                            <BookmarkButton
                              alias={candidate.alias}
                              shortlistTitle={membership.shortlist.data?.title}
                              member={member}
                              pending={membership.toggle.isPending || !shortlistId}
                              onToggle={() => toggle(candidate)}
                            />
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                ))}
              </table>
            </TableWrap>
          )}
          {results.data && (
            <Pagination
              page={page}
              pageSize={PAGE_SIZE}
              total={results.data.total}
              onChange={(next) => onSearchChange({ page: next })}
            />
          )}
          {results.data?.items.some((item) => item.simulation) && (
            <p className="small muted" style={{ marginTop: '0.75rem' }}>
              Соответствие рассчитано демонстрационным алгоритмом по правилам (
              {results.data.items[0]?.match?.algorithmVersion ?? 'без потребности'}); это не вероятность успеха найма.
            </p>
          )}
        </Card>
        <ShortlistPanel
          shortlists={shortlists.items}
          selectedId={shortlistId}
          onSelect={(id) => onSearchChange({ shortlistId: id })}
          membership={membership}
          needId={search.needId}
        />
      </div>
    </>
  );
}
