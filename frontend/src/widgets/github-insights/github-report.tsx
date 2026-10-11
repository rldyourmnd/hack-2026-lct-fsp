import { type ReactNode, useState } from 'react';
import {
  formatMinutes,
  formatOffset,
  type GitHubReport,
  type GitHubSignal,
  hygieneLabel,
} from '@/entities/github-insights';
import { skillLabel, useMeta } from '@/entities/meta';
import { isDemoReference } from '@/entities/source';
import { formatDate, pluralRu } from '@/shared/lib';
import { Card, Pagination, panelId, StatusChip, type TabItem, TableWrap, Tabs, TagList } from '@/shared/ui';
import styles from './github-insights.module.css';
import { GitHubSignalView } from './github-signal-card';
import {
  ActivityCalendar,
  Archetypes,
  DimensionList,
  Flags,
  HourChart,
  IndexSummary,
  LanguageBar,
  Numbers,
  plainListClass,
  StackGroups,
  WeekdayChart,
} from './parts';

export const githubReportTabs = ['overview', 'activity', 'work', 'stack', 'repositories', 'score', 'employer'] as const;
export type GitHubReportTab = (typeof githubReportTabs)[number];

const TAB_PREFIX = 'github-report';
const REPOSITORIES_PAGE = 10;
const STACK_SHOWN = 5;
const count = (value: number) => value.toLocaleString('ru-RU');
const share = (value: number | null | undefined) => (value === null || value === undefined ? '—' : `${value}%`);
const repositories = (value: number) => `${value} ${pluralRu(value, 'репозиторий', 'репозитория', 'репозиториев')}`;
const commits = (value: number) => `${count(value)} ${pluralRu(value, 'коммит', 'коммита', 'коммитов')}`;

/** «+35%», or «×20» from threefold growth, where a percentage stops being readable. */
function formatTrend(percent: number | null) {
  if (percent === null) return '—';
  if (percent >= 200) return `×${Math.round(1 + percent / 100)}`;
  return `${percent > 0 ? '+' : ''}${count(percent)}%`;
}

/** Fixture links identify synthetic data and are not hosted pages. */
function ExternalLink({ href, children }: { href: string; children: ReactNode }) {
  if (isDemoReference(href)) return <span>{children}</span>;
  return (
    <a href={href} target="_blank" rel="noreferrer noopener">
      {children}
    </a>
  );
}

function Section({ title, subtitle, children }: { title: string; subtitle?: string; children: ReactNode }) {
  return (
    <section className="stack" style={{ gap: '0.75rem' }}>
      <div>
        <h3 className={styles.sectionTitle}>{title}</h3>
        {subtitle && (
          <p className="small muted" style={{ margin: 0 }}>
            {subtitle}
          </p>
        )}
      </div>
      {children}
    </section>
  );
}

function Overview({ report }: { report: GitHubReport }) {
  const { coverage, profile } = report;
  return (
    <div className="stack-lg">
      <IndexSummary index={report.index}>
        <span className="small">
          Индекс публичной активности: регулярность, вовлечённость, собственные проекты, совместная работа и инженерная
          культура. Это не оценка навыков: грейд подтверждает только тест знаний.
        </span>
      </IndexSummary>
      <p className="small muted" style={{ margin: 0 }}>
        Публичных коммитов за год: {count(coverage.commitsLastYearTotal)}
        {coverage.coveragePercent === null ? '' : ` (в анализе — ${coverage.coveragePercent}%)`}, за всё время —{' '}
        {count(coverage.commitsPublicTotal)}. Разобрано {commits(coverage.commitsAnalyzed)} в{' '}
        {repositories(coverage.repositoriesAnalyzed)}. Отчёт от {formatDate(report.generatedAt)}
      </p>
      {coverage.notes.length > 0 && (
        <ul className={`${plainListClass} small muted`}>
          {coverage.notes.map((note) => (
            <li key={note}>{note}</li>
          ))}
        </ul>
      )}
      <Flags flags={report.flags} />
      <Section title="Портрет разработчика" subtitle="По вашим собственным изменениям в публичных репозиториях">
        <p style={{ margin: 0 }}>{profile.summary}</p>
        {report.highlights.length > 0 && (
          <ul className={`${plainListClass} small`}>
            {report.highlights.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        )}
      </Section>
      <div className={styles.columns2}>
        <Section title="Направления" subtitle="Доля изменённых вами файлов по областям">
          <Archetypes archetypes={profile.archetypes} />
        </Section>
        <div className="stack-lg">
          <Section title="Языки" subtitle="По изменённым вами файлам">
            <LanguageBar languages={report.languages} />
          </Section>
          {profile.focusTopics.length > 0 && (
            <Section title="Темы проектов">
              <TagList items={profile.focusTopics} label="Темы проектов" />
            </Section>
          )}
        </div>
      </div>
      {profile.topProjects.length > 0 && (
        <Section title="Главные проекты" subtitle="Проекты, которые вы ведёте, по числу ваших коммитов">
          <ul className={styles.projects}>
            {profile.topProjects.map((project) => (
              <li key={project.nameWithOwner}>
                <ExternalLink href={project.url}>{project.nameWithOwner}</ExternalLink>
                {project.description && <span className={`small ${styles.clamp}`}>{project.description}</span>}
                <span className="small muted">
                  {project.primaryLanguage ?? 'язык не указан'} · ★ {count(project.stars)} ·{' '}
                  {commits(project.userCommits)} ({project.userSharePercent}% истории)
                  {project.lastCommitAt ? ` · последний ${formatDate(project.lastCommitAt)}` : ''}
                </span>
              </li>
            ))}
          </ul>
        </Section>
      )}
    </div>
  );
}

function Activity({ report }: { report: GitHubReport }) {
  const { activity, rhythm } = report;
  return (
    <div className="stack-lg">
      <Numbers
        items={[
          ['коммитов за год', count(activity.commitsLastYear)],
          ['PR за год', count(activity.pullRequestsLastYear)],
          ['активных дней за год', count(activity.activeDaysLastYear)],
          ['активных недель из 52', activity.activeWeeksLastYear],
          ['дней подряд, рекорд', activity.longestDailyStreak],
          ['дней подряд, сейчас', activity.currentDailyStreak],
          ['недель подряд, рекорд', activity.longestWeeklyStreak],
          ['самый долгий перерыв, дней', activity.longestGapDaysLastYear],
          ['динамика за полгода', formatTrend(activity.trendPercent)],
        ]}
      />
      <ActivityCalendar days={activity.calendar} />
      <div className="table-wrap">
        <table className="table">
          <caption className="visually-hidden">Публичные коммиты по годам</caption>
          <thead>
            <tr>
              <th scope="col">Год</th>
              <th scope="col">Коммиты</th>
              <th scope="col">Активных дней</th>
            </tr>
          </thead>
          <tbody>
            {activity.yearly.map((year) => (
              <tr key={year.year}>
                <td>{year.year}</td>
                <td>{count(year.commits)}</td>
                <td>{year.activeDays}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="small muted" style={{ margin: 0 }}>
        Календарь и активные дни строятся по разобранным репозиториям; коммиты по годам — по поиску GitHub во всех
        публичных репозиториях.
      </p>
      {rhythm ? (
        <Section title="Ритм работы" subtitle={`По ${commits(rhythm.sampleSize)} в местном времени автора`}>
          <Numbers
            items={[
              ['в активный день, медиана', formatMinutes(rhythm.medianMinutesPerActiveDay)],
              ['в напряжённые дни (90-й перцентиль)', formatMinutes(rhythm.p90MinutesPerActiveDay)],
              [
                'в активную неделю, медиана',
                rhythm.medianMinutesPerActiveWeek === null ? '—' : formatMinutes(rhythm.medianMinutesPerActiveWeek),
              ],
              ['сессий в активный день', (rhythm.sessionsPerActiveDayTenths / 10).toLocaleString('ru-RU')],
              [
                'самая длинная сессия',
                rhythm.longestSessionMinutes >= 24 * 60 ? 'сутки и более' : formatMinutes(rhythm.longestSessionMinutes),
              ],
              ['будни 9:00–19:00', share(rhythm.officeHoursPercent)],
              ['вечер после 19:00', share(rhythm.eveningPercent)],
              ['ночь до 6:00', share(rhythm.nightPercent)],
              ['выходные', share(rhythm.weekendPercent)],
            ]}
          />
          <div className="split-even">
            <div className="stack" style={{ gap: '0.5rem' }}>
              <strong className="small">Коммиты по часам</strong>
              <HourChart hourly={rhythm.hourlyPercent} />
            </div>
            <div className="stack" style={{ gap: '0.5rem' }}>
              <strong className="small">Коммиты по дням недели</strong>
              <WeekdayChart weekdays={rhythm.weekdayPercent} />
            </div>
          </div>
          <p className="small muted" style={{ margin: 0 }}>
            Часовой пояс — из отметок времени самих коммитов:{' '}
            {rhythm.timezoneOffsets.map((item) => `${formatOffset(item.offsetMinutes)} (${item.percent}%)`).join(', ')}.
            Сессия — коммиты с перерывами не больше двух часов плюс 30 минут до первого коммита; сессия через полночь
            делится между днями. Это оценка по отметкам времени, а не учёт рабочего времени.
          </p>
        </Section>
      ) : (
        <p className="small muted">Слишком мало публичных коммитов для оценки ритма работы.</p>
      )}
    </div>
  );
}

function Work({ report }: { report: GitHubReport }) {
  const { collaboration, ownership, organizations } = report;
  return (
    <div className="stack-lg">
      <Section title="Собственные проекты" subtitle="Проекты, где у вас не меньше половины истории, и ваши организации">
        <Numbers
          items={[
            ['ведёте проектов', ownership.ledRepositories],
            ['из них с коммитами за год', ownership.maintainedRepositories],
            ['своих организаций', ownership.ownOrganizations],
            ['репозиториев в своих организациях', ownership.ownOrganizationRepositories],
            ['репозиториев с прямыми пушами', ownership.directPushRepositories],
          ]}
        />
      </Section>
      <Section
        title="Организации"
        subtitle="Свои организации определяются по коммитам, прямым пушам и принятию своих PR: GitHub по умолчанию скрывает членство"
      >
        {organizations.contributed.length ? (
          <ul className={styles.projects}>
            {organizations.contributed.map((organization) => (
              <li key={organization.login}>
                <span className="row">
                  <ExternalLink href={organization.url}>{organization.login}</ExternalLink>
                  {organization.affiliated && (
                    <StatusChip tone="neutral" icon={null}>
                      своя
                    </StatusChip>
                  )}
                </span>
                <span className="small muted">
                  {commits(organization.commits)} · {repositories(organization.repositories)} в анализе
                </span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="small muted">Вклада в репозитории организаций не найдено.</p>
        )}
        {organizations.memberships.length > 0 && (
          <p className="small muted" style={{ margin: 0 }}>
            Публичное членство: {organizations.memberships.map((organization) => organization.login).join(', ')}.
          </p>
        )}
      </Section>
      <Section
        title="Совместная работа"
        subtitle="Засчитывается работа, принятая или проверенная другими людьми вне ваших организаций"
      >
        <Numbers
          items={[
            ['PR всего', count(collaboration.pullRequestsTotal)],
            ['PR одобрено другими', count(collaboration.approvedPullRequests)],
            ['ревью чужих PR', count(collaboration.reviewsOfOthers)],
            ['ревью в независимых проектах', count(collaboration.independentReviews)],
            ['issues', count(collaboration.issuesTotal)],
            ['PR в независимые проекты', count(collaboration.externalPullRequests)],
            ['из них принято', count(collaboration.externalMerged)],
            ['доля принятых', share(collaboration.externalMergeRatePercent)],
            ['независимых репозиториев', collaboration.externalRepositories],
          ]}
        />
        {collaboration.notableContributions.length > 0 && (
          <>
            <strong className="small">Вклад в заметные независимые проекты</strong>
            <ul className={styles.projects}>
              {collaboration.notableContributions.map((item) => (
                <li key={item.repository}>
                  <ExternalLink href={item.url}>{item.repository}</ExternalLink>
                  <span className="small muted">
                    ★ {count(item.stars)} · принято PR: {item.mergedPullRequests}
                    {item.commits ? ` · ${commits(item.commits)} в анализе` : ''}
                  </span>
                </li>
              ))}
            </ul>
          </>
        )}
      </Section>
    </div>
  );
}

function Stack({ report }: { report: GitHubReport }) {
  const meta = useMeta();
  const { craft } = report;
  return (
    <div className="stack-lg">
      <Section title="Технологии" subtitle="Из манифестов, файлов и истории проектов, где вы работаете">
        <StackGroups stack={report.stack} />
        {report.technologies.length > 0 && (
          <details>
            <summary className="small">Навыки каталога с доказательствами</summary>
            <ul className={plainListClass}>
              {report.technologies.map((technology) => (
                <li key={technology.skillId}>
                  <strong>{skillLabel(meta, technology.skillId)}</strong>{' '}
                  <span className="small muted">
                    · {repositories(technology.repositories)}: {technology.signals.join('; ')}
                  </span>
                </li>
              ))}
            </ul>
          </details>
        )}
        <p className="small muted" style={{ margin: 0 }}>
          Технология засчитывается только по артефакту: зависимости в манифесте, файлу конфигурации или вашим изменениям
          кода. Это свидетельство использования, а не подтверждённый навык.
        </p>
      </Section>
      <Section
        title="Инженерная культура"
        subtitle={`Доля основных проектов (${craft.substantiveRepositories}) с каждой практикой`}
      >
        <Numbers
          items={[
            ['README', share(craft.readmePercent)],
            ['лицензия', share(craft.licensePercent)],
            ['CI', share(craft.ciPercent)],
            ['тесты', share(craft.testsPercent)],
            ['lock-файл', share(craft.lockfilePercent)],
            ['линтеры', share(craft.lintPercent)],
            ['обновление зависимостей', share(craft.dependencyUpdatesPercent)],
            ['контейнеры', share(craft.containerPercent)],
            ['содержательные коммиты', share(craft.descriptiveMessagesPercent)],
            ['Conventional Commits', share(craft.conventionalCommitsPercent)],
            [
              'файлов в коммите, медиана',
              craft.medianFilesPerCommit === null ? '—' : count(craft.medianFilesPerCommit),
            ],
            ['крупные коммиты', share(craft.largeCommitsPercent)],
            ['merge-коммиты', share(craft.mergeCommitsPercent)],
          ]}
        />
      </Section>
    </div>
  );
}

function Repositories({ report }: { report: GitHubReport }) {
  const [page, setPage] = useState(1);
  const items = report.repositories;
  if (!items.length) return <p className="small muted">Репозитории не разобраны.</p>;
  const visible = items.slice((page - 1) * REPOSITORIES_PAGE, page * REPOSITORIES_PAGE);
  return (
    <div className="stack">
      <p className="small muted" style={{ margin: 0 }}>
        {repositories(items.length)} по числу ваших коммитов. «Ведёте» — у вас не меньше половины истории.
      </p>
      <TableWrap label="Разобранные репозитории">
        <table className={`table ${styles.repositoryTable}`}>
          <thead>
            <tr>
              <th scope="col">Репозиторий</th>
              <th scope="col">Ваши коммиты</th>
              <th scope="col">Стек и практики</th>
            </tr>
          </thead>
          <tbody>
            {visible.map((repository) => {
              const extra = repository.stack.length - STACK_SHOWN;
              return (
                <tr key={repository.nameWithOwner}>
                  <td className={styles.repository}>
                    <ExternalLink href={repository.url}>{repository.nameWithOwner}</ExternalLink>
                    <div className="small muted">
                      {repository.primaryLanguage ?? 'язык не указан'} · ★ {count(repository.stars)}
                      {repository.owned ? ' · ваш' : repository.led ? ' · ведёте' : ''}
                      {repository.archived ? ' · в архиве' : ''}
                    </div>
                  </td>
                  <td>
                    {commits(repository.userCommits)}
                    <div className="small muted">
                      {repository.userSharePercent}% из {count(repository.totalCommits)}
                    </div>
                  </td>
                  <td>
                    <div className="small">
                      {repository.stack.slice(0, STACK_SHOWN).join(', ') || '—'}
                      {extra > 0 && <span className="muted"> и ещё {extra}</span>}
                    </div>
                    <div className="small muted">
                      {repository.hygiene.map((signal) => hygieneLabel[signal]).join(', ') || 'практики не найдены'}
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </TableWrap>
      <Pagination page={page} pageSize={REPOSITORIES_PAGE} total={items.length} onChange={setPage} />
    </div>
  );
}

function Score({ report }: { report: GitHubReport }) {
  return (
    <div className="stack">
      <p className="small muted" style={{ margin: 0 }}>
        Пороговые правила, одинаковые для всех кандидатов. Каждый показатель насыщается на своём пороге, поэтому объём
        сверх порога не добавляет баллов; флаги только снижают уверенность. Индекс не влияет на грейд и порядок выдачи.
      </p>
      <DimensionList dimensions={report.dimensions} />
    </div>
  );
}

type Props = {
  report: GitHubReport;
  /** Exactly what employers see; adds the «Для работодателя» tab. */
  preview?: GitHubSignal | null;
  tab?: GitHubReportTab;
  onTabChange?: (tab: GitHubReportTab) => void;
};

/** The candidate's full report in tabs: exact values, names and links stay with the candidate. */
export function GitHubReportView({ report, preview, tab, onTabChange }: Props) {
  const [ownTab, setOwnTab] = useState<GitHubReportTab>('overview');
  const current = tab === 'employer' && !preview ? 'overview' : (tab ?? ownTab);
  const change = onTabChange ?? setOwnTab;
  const tabs: TabItem<GitHubReportTab>[] = [
    { id: 'overview', label: 'Обзор' },
    { id: 'activity', label: 'Активность' },
    { id: 'work', label: 'Проекты и вклад' },
    { id: 'stack', label: 'Технологии' },
    { id: 'repositories', label: 'Репозитории', count: report.repositories.length },
    { id: 'score', label: 'Оценка' },
    ...(preview ? [{ id: 'employer' as const, label: 'Для работодателя' }] : []),
  ];
  return (
    <Card title="Отчёт GitHub" subtitle={`Публичные данные · алгоритм ${report.algorithmVersion}`}>
      <Tabs items={tabs} value={current} onChange={change} label="Разделы отчёта GitHub" idPrefix={TAB_PREFIX} />
      <div
        role="tabpanel"
        id={panelId(TAB_PREFIX, current)}
        aria-labelledby={`${TAB_PREFIX}-tab-${current}`}
        style={{ paddingTop: '1.25rem' }}
      >
        {current === 'overview' && <Overview report={report} />}
        {current === 'activity' && <Activity report={report} />}
        {current === 'work' && <Work report={report} />}
        {current === 'stack' && <Stack report={report} />}
        {current === 'repositories' && <Repositories report={report} />}
        {current === 'score' && <Score report={report} />}
        {current === 'employer' && preview && (
          <div className="stack">
            <p className="small muted" style={{ margin: 0 }}>
              Этот обезличенный блок работодатель видит в карточке кандидата: без ника, ссылок, названий проектов и
              организаций, с округлёнными вниз числами.
            </p>
            <GitHubSignalView signal={preview} wide />
          </div>
        )}
      </div>
    </Card>
  );
}
