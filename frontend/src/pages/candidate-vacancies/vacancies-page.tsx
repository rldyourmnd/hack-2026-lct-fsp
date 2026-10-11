import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { companyInitials } from '@/entities/company';
import { type SkillId, skillLabel, useMeta } from '@/entities/meta';
import { useScope } from '@/entities/session';
import {
  eligibilityView,
  type Vacancy,
  type VacancyFilters,
  VacancyFitChip,
  vacancySearchQuery,
} from '@/entities/vacancy';
import { isApiError } from '@/shared/api';
import {
  formatDate,
  formatSalaryRange,
  gradeLabel,
  gradeOrder,
  skillOptions,
  specializationLabel,
  specializationOptions,
  type specializationOrder,
  taxBasisLabel,
  useDocumentTitle,
  workModeLabel,
  workModeOrder,
} from '@/shared/lib';
import {
  Avatar,
  Button,
  ButtonLink,
  Callout,
  Card,
  EmptyState,
  ErrorState,
  InlineStatus,
  LoadingState,
  PageHeader,
  Pagination,
  SelectField,
  StatusChip,
} from '@/shared/ui';
import styles from './vacancies-page.module.css';

type Grade = (typeof gradeOrder)[number];
type Specialization = (typeof specializationOrder)[number];
type WorkMode = (typeof workModeOrder)[number];

export type VacancySearch = {
  specialization?: Specialization | undefined;
  grade?: Grade | undefined;
  workMode?: WorkMode | undefined;
  skill?: SkillId | undefined;
  page?: number | undefined;
};

const PAGE_SIZE = 10;

/** The API query for a URL search; the route loader prefetches exactly what the page requests. */
export function vacancyFilters(search: VacancySearch): VacancyFilters {
  return {
    page: search.page ?? 1,
    pageSize: PAGE_SIZE,
    ...(search.specialization ? { specialization: search.specialization } : {}),
    ...(search.grade ? { grade: search.grade } : {}),
    ...(search.workMode ? { workMode: search.workMode } : {}),
    ...(search.skill ? { skill: search.skill } : {}),
  };
}

function VacancyCard({ vacancy }: { vacancy: Vacancy }) {
  const meta = useMeta();
  const required = vacancy.requirements.filter((item) => item.criticality === 'required');
  return (
    <Card as="article" headingLevel={3}>
      <div className={styles.card}>
        <Avatar kind="company" text={companyInitials(vacancy.company)} size="md" />
        <div className="stack" style={{ gap: '0.5rem' }}>
          <div className="row" style={{ justifyContent: 'space-between', alignItems: 'flex-start' }}>
            <div className="stack" style={{ gap: '0.25rem' }}>
              <span className="small muted">{vacancy.company.displayName}</span>
              <h3 style={{ margin: 0, fontSize: '1.25rem' }}>
                <Link to="/candidate/vacancies/$vacancyId" params={{ vacancyId: vacancy.id }}>
                  {vacancy.title}
                </Link>
              </h3>
            </div>
            {vacancy.applicationId && (
              <StatusChip tone="info" icon="send">
                Вы откликнулись
              </StatusChip>
            )}
          </div>
          <span>
            <strong>{formatSalaryRange(vacancy.salary)}</strong>{' '}
            <span className="small muted">{taxBasisLabel[vacancy.salary.taxBasis]}</span>
          </span>
          <span className="small">
            {specializationLabel[vacancy.specialization]} · от {gradeLabel[vacancy.grade]} ·{' '}
            {workModeLabel[vacancy.workMode]}
          </span>
          <span className="small muted">
            Обязательно: {required.map((item) => skillLabel(meta, item.skillId)).join(', ')} · опубликована{' '}
            {formatDate(vacancy.publishedAt)}
          </span>
          {vacancy.match && (
            <div>
              <VacancyFitChip status={vacancy.match.status} />
            </div>
          )}
        </div>
      </div>
    </Card>
  );
}

type Props = { search: VacancySearch; onSearchChange: (patch: Partial<VacancySearch>) => void };

export function CandidateVacanciesPage({ search, onSearchChange }: Props) {
  useDocumentTitle('Вакансии');
  const scope = useScope();
  const results = useQuery(vacancySearchQuery(scope, vacancyFilters(search)));
  const filtered = Boolean(search.specialization || search.grade || search.workMode || search.skill);
  const resetFilters = () =>
    onSearchChange({
      specialization: undefined,
      grade: undefined,
      workMode: undefined,
      skill: undefined,
      page: undefined,
    });
  const blockerReason = results.data?.items.find(
    (item) => item.eligibility.reason && item.eligibility.reason !== 'application_exists',
  )?.eligibility.reason;
  const blocker = blockerReason ? eligibilityView[blockerReason] : null;

  return (
    <>
      <PageHeader
        title="Вакансии"
        subtitle="Опубликованные вакансии компаний. Зарплатная вилка видна сразу, отклик — по вашему решению."
        actions={
          <ButtonLink to="/candidate/applications" icon="send">
            Мои отклики
          </ButtonLink>
        }
      />
      {blocker && (
        <Callout
          tone="info"
          title="Как откликаться"
          actions={
            blocker.action && (
              <ButtonLink to={blocker.action.to} variant="primary" size="sm">
                {blocker.action.label}
              </ButtonLink>
            )
          }
        >
          {blocker.text} Основной путь сервиса — приглашения: компании сами находят подтверждённые профили.
        </Callout>
      )}
      <Card className={styles.filters}>
        <div className={styles.filterGrid}>
          <SelectField
            id="vacancy-specialization"
            label="Специализация"
            value={search.specialization ?? ''}
            onValueChange={(value) => onSearchChange({ specialization: value || undefined, page: undefined })}
            options={[{ value: '', label: 'Любая' }, ...specializationOptions]}
          />
          <SelectField
            id="vacancy-grade"
            label="Грейд вакансии"
            value={search.grade ?? ''}
            onValueChange={(value) => onSearchChange({ grade: value || undefined, page: undefined })}
            options={[
              { value: '', label: 'Любой' },
              ...gradeOrder.map((value) => ({ value, label: gradeLabel[value] })),
            ]}
          />
          <SelectField
            id="vacancy-work-mode"
            label="Формат работы"
            value={search.workMode ?? ''}
            onValueChange={(value) => onSearchChange({ workMode: value || undefined, page: undefined })}
            options={[
              { value: '', label: 'Любой' },
              ...workModeOrder.map((value) => ({ value, label: workModeLabel[value] })),
            ]}
          />
          <SelectField
            id="vacancy-skill"
            label="Навык в требованиях"
            value={search.skill ?? ''}
            onValueChange={(value) =>
              onSearchChange({ skill: (value || undefined) as SkillId | undefined, page: undefined })
            }
            options={[{ value: '', label: 'Любой' }, ...skillOptions]}
          />
        </div>
        <Button variant="quiet" icon="x" disabled={!filtered} onClick={resetFilters}>
          Сбросить фильтры
        </Button>
      </Card>
      <section aria-labelledby="vacancy-results" className="stack" style={{ marginTop: '1.5rem' }}>
        <div className="row" style={{ justifyContent: 'space-between' }}>
          <h2 id="vacancy-results" style={{ margin: 0, fontSize: '1.25rem' }}>
            Найденные вакансии
          </h2>
          {results.data && <span className="small muted">Найдено: {results.data.total} · сначала новые</span>}
        </div>
        {results.isPending && <LoadingState label="Загружаем вакансии…" lines={4} />}
        {results.isError && (
          <ErrorState
            error={results.error}
            onRetry={isApiError(results.error, 404) ? undefined : () => results.refetch()}
            retrying={results.isFetching}
          />
        )}
        {results.data && results.isFetching && results.isPlaceholderData && (
          <InlineStatus>Обновляем список…</InlineStatus>
        )}
        {results.data && results.data.items.length === 0 && (
          <Card>
            <EmptyState
              icon="briefcase"
              title={filtered ? 'По этим фильтрам вакансий нет' : 'Опубликованных вакансий пока нет'}
              actions={filtered && <Button onClick={resetFilters}>Сбросить фильтры</Button>}
            >
              Компании могут пригласить вас и без вакансии — приглашения приходят в раздел «Приглашения».
            </EmptyState>
          </Card>
        )}
        {results.data && results.data.items.length > 0 && (
          <>
            <ul className="stack" style={{ margin: 0, padding: 0, listStyle: 'none' }}>
              {results.data.items.map((vacancy) => (
                <li key={vacancy.id}>
                  <VacancyCard vacancy={vacancy} />
                </li>
              ))}
            </ul>
            <Pagination
              page={results.data.page}
              pageSize={results.data.pageSize}
              total={results.data.total}
              onChange={(next) => onSearchChange({ page: next === 1 ? undefined : next })}
            />
          </>
        )}
      </section>
    </>
  );
}
