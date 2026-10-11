import { useSuspenseQuery } from '@tanstack/react-query';
import { hiringFlowAnalyticsQuery } from '@/entities/hiring-analytics';
import { useScope } from '@/entities/session';
import { useDocumentTitle } from '@/shared/lib';
import { Button, Card, PageHeader, StatusChip, TableWrap } from '@/shared/ui';
import styles from './analytics-page.module.css';

function duration(hours: number | null) {
  if (hours === null) return 'Нет данных';
  if (hours < 24) return `${hours} ч`;
  return `${Math.round((hours / 24) * 10) / 10} дн`;
}

export function EmployerAnalyticsPage() {
  useDocumentTitle('Аналитика найма');
  const scope = useScope();
  const analytics = useSuspenseQuery({ ...hiringFlowAnalyticsQuery(scope), refetchOnWindowFocus: 'always' });
  const data = analytics.data;

  return (
    <>
      <PageHeader
        title="Аналитика найма"
        subtitle="Где кандидаты задерживаются и что можно улучшить прямо сейчас."
        actions={
          <Button
            icon="refresh"
            pending={analytics.isFetching}
            pendingLabel="Обновляем…"
            onClick={() => analytics.refetch()}
          >
            Обновить
          </Button>
        }
      />
      <section className={styles.metrics} aria-label="Ключевые показатели">
        <Card className={styles.metric}>
          <span>Откликов</span>
          <strong>{data.total}</strong>
          <small>во всех вакансиях</small>
        </Card>
        <Card className={styles.metric}>
          <span>До просмотра</span>
          <strong>{duration(data.medianTimeToViewHours)}</strong>
          <small>медианное время</small>
        </Card>
        <Card className={styles.metric}>
          <span>До решения</span>
          <strong>{duration(data.medianTimeToDecisionHours)}</strong>
          <small>медианное время</small>
        </Card>
        <Card className={styles.metric}>
          <span>Принято</span>
          <strong>{data.acceptanceRatePercent}%</strong>
          <small>от принятых решений</small>
        </Card>
      </section>

      <div className={styles.mainGrid}>
        <Card title="Воронка откликов" subtitle="Каждый следующий этап считается от общего числа откликов.">
          <ol className={styles.funnel}>
            {data.stages.map((stage) => (
              <li key={stage.id}>
                <div className={styles.funnelLabel}>
                  <span>{stage.label}</span>
                  <strong>{stage.count}</strong>
                </div>
                <div className={styles.track}>
                  <span style={{ width: `${stage.sharePercent}%` }} />
                </div>
                <small>{stage.sharePercent}% от всех откликов</small>
              </li>
            ))}
          </ol>
        </Card>
        <Card className={styles.assistant} title="Помощник по потоку" tinted>
          <p className={styles.summary}>{data.assistant.summary}</p>
          <ul className={styles.actions}>
            {data.assistant.actions.map((action) => (
              <li key={action}>{action}</li>
            ))}
          </ul>
          {data.bottlenecks.map((item) => (
            <article className={styles.bottleneck} key={`${item.kind}-${item.title}`}>
              <StatusChip tone={item.severity === 'high' ? 'danger' : item.severity === 'medium' ? 'warning' : 'info'}>
                {item.title}
              </StatusChip>
              <p>{item.evidence}</p>
              <strong>{item.recommendation}</strong>
            </article>
          ))}
        </Card>
      </div>

      <Card title="Вакансии" subtitle="Очередь и качество соответствия по каждой позиции.">
        <TableWrap label="Аналитика по вакансиям">
          <table className="table">
            <thead>
              <tr>
                <th scope="col">Вакансия</th>
                <th scope="col">Отклики</th>
                <th scope="col">Ждут просмотра</th>
                <th scope="col">Ждут решения</th>
                <th scope="col">Принято</th>
                <th scope="col">Обязательные навыки</th>
                <th scope="col">Грейд совпадает</th>
              </tr>
            </thead>
            <tbody>
              {data.vacancies.map((vacancy) => (
                <tr key={vacancy.vacancyId}>
                  <td>
                    <strong>{vacancy.title}</strong>
                  </td>
                  <td>{vacancy.total}</td>
                  <td>{vacancy.awaitingReview}</td>
                  <td>{vacancy.awaitingDecision}</td>
                  <td>{vacancy.accepted}</td>
                  <td>{vacancy.requiredCoveragePercent}%</td>
                  <td>{vacancy.gradeMatchPercent}%</td>
                </tr>
              ))}
            </tbody>
          </table>
        </TableWrap>
      </Card>
    </>
  );
}
