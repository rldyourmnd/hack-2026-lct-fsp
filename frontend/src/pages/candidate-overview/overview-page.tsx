import { useQuery, useSuspenseQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { AssessmentStatusChip, assessmentsQuery } from '@/entities/assessment';
import { bandLabel } from '@/entities/github-insights';
import { skillLabel, useMeta } from '@/entities/meta';
import { blockerLabel, type Profile, profileQuery } from '@/entities/profile';
import { CategoryBadge, FreshnessChip, TargetGradeBadge } from '@/entities/qualification';
import { useScope } from '@/entities/session';
import { mySkillsQuery, SkillStatusChip } from '@/entities/skill';
import { mySourcesQuery, type Source } from '@/entities/source';
import { ExportProfileButton } from '@/features/profile-pdf';
import { evidenceSourceLabel, formatShortDate, gradeLabel, initials, roleTitle, useDocumentTitle } from '@/shared/lib';
import {
  Avatar,
  ButtonLink,
  Card,
  EmptyState,
  ErrorState,
  Icon,
  type IconName,
  LoadingState,
  PageHeader,
  StatusChip,
  TableWrap,
  TagList,
} from '@/shared/ui';
import { AssessmentNextStep } from '@/widgets/assessment-next-step';
import { ProfileEstimateCard } from '@/widgets/profile-estimate';
import styles from './overview-page.module.css';

function VisibilityRow({ profile }: { profile: Profile }) {
  const visible = profile.onboarding.visibleToEmployers;
  return (
    <div className={styles.visibility}>
      <Icon name={visible ? 'lock' : 'eye'} />
      {visible ? (
        <span>
          <strong>Для работодателей:</strong> {profile.alias} · контакты скрыты до вашего согласия
        </span>
      ) : (
        <span>
          <strong>Работодатели пока не видят профиль:</strong>{' '}
          {profile.onboarding.blockers.map((blocker) => blockerLabel[blocker]).join(', ')}
        </span>
      )}
      <Link to="/candidate/settings">Настроить видимость →</Link>
    </div>
  );
}

function SkillsCard() {
  const scope = useScope();
  const skills = useQuery(mySkillsQuery(scope));
  return (
    <Card
      title="Мои компетенции"
      actions={
        <Link to="/candidate/skills" search={{ view: 'graph' }}>
          Открыть граф →
        </Link>
      }
    >
      {skills.isPending && <LoadingState lines={4} />}
      {skills.isError && <ErrorState error={skills.error} onRetry={() => skills.refetch()} />}
      {skills.data && skills.data.items.length === 0 && (
        <EmptyState icon="chart" title="Навыков пока нет">
          Они появятся после анализа GitHub или теста знаний.
        </EmptyState>
      )}
      {skills.data && skills.data.items.length > 0 && (
        <TableWrap label="Навыки">
          <table className="table">
            <thead>
              <tr>
                <th scope="col">Навык</th>
                <th scope="col">Подтверждение</th>
                <th scope="col">Основания</th>
              </tr>
            </thead>
            <tbody>
              {skills.data.items.map((skill) => (
                <tr key={skill.skillId}>
                  <td>
                    <strong>{skill.label}</strong>
                  </td>
                  <td>
                    <SkillStatusChip status={skill.status} plain />
                    {skill.status === 'confirmed' && skill.evidenceGrade && (
                      <div className="small muted">уровень «{gradeLabel[skill.evidenceGrade]}»</div>
                    )}
                  </td>
                  <td>
                    {skill.evidenceSources.length
                      ? skill.evidenceSources.map((source) => evidenceSourceLabel[source]).join(', ')
                      : 'Нужна проверка'}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </TableWrap>
      )}
      <SourcesSummary />
    </Card>
  );
}

function sourceSummary(source: Source): { icon: IconName; text: string; busy: boolean } {
  if (source.provider === 'github') {
    if (source.status !== 'connected')
      return { icon: 'minusCircle', text: 'Не подключён · добавьте ник, чтобы показать публичную работу', busy: false };
    const { found, processed, unavailable } = source.counts;
    if (source.activeJobId) {
      const analysis = source.github?.analysis;
      return {
        icon: 'refresh',
        text: analysis
          ? `Идёт анализ · ${analysis.stage} · ${analysis.progress}%`
          : `Анализ продолжается · разобрано ${processed} из ${found} репозиториев`,
        busy: true,
      };
    }
    const report = source.github?.report;
    if (report) {
      return {
        icon: 'checkCircle',
        text: `Индекс ${report.index.score} из 100 · ${bandLabel[report.index.band].toLowerCase()}${
          source.github?.ownership.verified ? '' : ' · владение не подтверждено'
        }`,
        busy: false,
      };
    }
    return {
      icon: 'checkCircle',
      text: `Разобрано ${processed} из ${found} репозиториев${unavailable ? ` · недоступно ${unavailable}` : ''}`,
      busy: false,
    };
  }
  if (source.status !== 'connected') return { icon: 'minusCircle', text: 'Не подключена · необязательно', busy: false };
  return source.verifiedParticipation
    ? { icon: 'checkCircle', text: 'Подтверждённое участие · демо-источник', busy: false }
    : { icon: 'info', text: 'Подключена · истории участия нет', busy: false };
}

function SourcesSummary() {
  const scope = useScope();
  const sources = useQuery(mySourcesQuery(scope));
  return (
    <section style={{ marginTop: '1.5rem' }} aria-labelledby="overview-sources">
      <h3 id="overview-sources" style={{ margin: 0, fontSize: '1rem' }}>
        Подключённые источники
      </h3>
      {sources.isPending && <LoadingState lines={2} />}
      {sources.isError && <ErrorState error={sources.error} onRetry={() => sources.refetch()} />}
      {sources.data && (
        <ul className={styles.sources}>
          {sources.data.items.map((source) => {
            const summary = sourceSummary(source);
            return (
              <li key={source.id}>
                <Link to="/candidate/sources" className={styles.sourceRow}>
                  <Icon name={source.provider === 'github' ? 'branch' : 'link'} size={22} />
                  <strong>{source.provider === 'github' ? 'GitHub' : 'ФСП'}</strong>
                  <span className="small muted" style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
                    <Icon name={summary.icon} size={16} />
                    {summary.text}
                  </span>
                  <Icon name="chevronRight" />
                </Link>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

function HistoryCard() {
  const scope = useScope();
  const assessments = useQuery(assessmentsQuery(scope));
  const items = assessments.data?.items.slice(0, 3) ?? [];
  return (
    <Card title="История проверок" actions={<Link to="/candidate/assessments">Вся история →</Link>}>
      {assessments.isPending && <LoadingState lines={2} />}
      {assessments.isError && <ErrorState error={assessments.error} onRetry={() => assessments.refetch()} />}
      {assessments.data && items.length === 0 && <p className="muted">Проверок пока не было.</p>}
      {items.length > 0 && (
        <ul className={styles.history}>
          {items.map((item) => (
            <li key={item.id} className={styles.historyItem}>
              <span className="stack" style={{ gap: '0.25rem' }}>
                <Link to="/candidate/assessments/$assessmentId/result" params={{ assessmentId: item.id }}>
                  {gradeLabel[item.grade]} · проверка
                </Link>
                <AssessmentStatusChip status={item.status} />
              </span>
              <span className="small muted">{formatShortDate(item.updatedAt)}</span>
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}

export function CandidateOverviewPage() {
  useDocumentTitle('Мой профиль');
  const scope = useScope();
  const meta = useMeta();
  const { data: profile } = useSuspenseQuery(profileQuery(scope));
  const sources = useQuery(mySourcesQuery(scope));
  const canEstimate = (sources.data?.items ?? []).some((item) => item.status === 'connected' && !item.activeJobId);
  const showTarget = profile.selectedGrade !== profile.category.grade;

  return (
    <>
      <PageHeader title="Мой профиль" subtitle="Навыки, подтверждения и карьерные возможности" />
      <Card>
        <div className={styles.hero}>
          <div className={styles.identity}>
            <Avatar kind="initials" text={initials(profile.name)} size="lg" />
            <div className={styles.identityText}>
              <h2 className={styles.name}>{profile.name}</h2>
              <p className="muted">{roleTitle[profile.category.specialization ?? profile.specialization]}</p>
              <div className="row">
                <CategoryBadge {...profile.category} />
                {showTarget && <TargetGradeBadge grade={profile.selectedGrade} />}
                {profile.verifiedGrade && <FreshnessChip freshness={profile.freshness} />}
                {!profile.onboarding.questionnaireComplete && (
                  <StatusChip tone="warning">Анкета не заполнена</StatusChip>
                )}
              </div>
              {profile.stack.length > 0 && (
                <TagList label="Стек" items={profile.stack.map((id) => skillLabel(meta, id))} />
              )}
            </div>
            <ExportProfileButton />
          </div>
          {!profile.category.source && (
            <p className="small muted">
              Пока нет категории, профиль не попадает в банк кандидатов: подключите GitHub или ФСП для автоматической
              оценки либо пройдите тест знаний.
            </p>
          )}
          <VisibilityRow profile={profile} />
        </div>
      </Card>
      {!profile.onboarding.questionnaireComplete && (
        <Card title="Соберите профиль" style={{ marginTop: '1.5rem' }}>
          <div className="row" style={{ justifyContent: 'space-between' }}>
            <p className="muted">
              Подключите GitHub или ФСП — система сама заполнит анкету и оценит специализацию и грейд. Можно заполнить и
              вручную.
            </p>
            <ButtonLink to="/candidate/onboarding" variant="primary" iconAfter="arrowRight">
              Продолжить
            </ButtonLink>
          </div>
        </Card>
      )}
      <div style={{ marginTop: '1.5rem' }}>
        <ProfileEstimateCard profile={profile} canEstimate={canEstimate} />
      </div>
      <div className={styles.grid}>
        <SkillsCard />
        <div className="stack-lg">
          <AssessmentNextStep profile={profile} />
          <HistoryCard />
        </div>
      </div>
    </>
  );
}
