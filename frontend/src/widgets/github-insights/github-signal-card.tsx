import type { GitHubSignal } from '@/entities/github-insights';
import { Card, StatusChip } from '@/shared/ui';
import styles from './github-insights.module.css';
import {
  Archetypes,
  DimensionList,
  Flags,
  IndexSummary,
  LanguageBar,
  Numbers,
  plainListClass,
  StackGroups,
} from './parts';

/**
 * The anonymous employer projection: rounded values, no names, links or calendars. `wide` lays it out in two
 * columns for a full-width preview.
 */
export function GitHubSignalView({ signal, wide = false }: { signal: GitHubSignal; wide?: boolean }) {
  const portrait = (
    <div className="stack">
      <IndexSummary index={signal.index}>
        <div className="row">
          {signal.ownershipVerified ? (
            <StatusChip tone="success">Владение аккаунтом подтверждено</StatusChip>
          ) : (
            <StatusChip tone="warning">Владение аккаунтом не подтверждено</StatusChip>
          )}
        </div>
      </IndexSummary>
      <p style={{ margin: 0 }}>{signal.summary}</p>
      <p className="small muted" style={{ margin: 0 }}>
        Сигнал публичной активности по правилам {signal.algorithmVersion} на {signal.generatedAt}. Он не подтверждает
        навыки и грейд и не влияет на порядок выдачи.
      </p>
      <Flags flags={signal.flags} />
      <Archetypes archetypes={signal.archetypes} />
      <LanguageBar languages={signal.languages} />
      <StackGroups compact stack={signal.stack.map((item) => ({ name: item.technology, category: item.category }))} />
    </div>
  );
  const evidence = (
    <div className="stack">
      {signal.highlights.length > 0 && (
        <ul className={`${plainListClass} small`}>
          {signal.highlights.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      )}
      {signal.rhythm && (
        <Numbers
          items={[
            ['время в активный день', signal.rhythm.hoursPerActiveDay],
            ['в активную неделю', signal.rhythm.hoursPerActiveWeek ?? '—'],
            ['будни 9:00–19:00', `≈${signal.rhythm.officeHoursPercent}%`],
            ['выходные', `≈${signal.rhythm.weekendPercent}%`],
          ]}
        />
      )}
      <DimensionList dimensions={signal.dimensions} collapsible />
    </div>
  );
  return (
    <div className={wide ? styles.columns2 : 'stack'}>
      {portrait}
      {evidence}
    </div>
  );
}

type Props = { signal: GitHubSignal; title?: string; subtitle?: string };

export function GitHubSignalCard({
  signal,
  title = 'GitHub: публичная активность',
  subtitle = 'Обезличено: без ника, ссылок, названий и точных чисел',
}: Props) {
  return (
    <Card title={title} subtitle={subtitle}>
      <GitHubSignalView signal={signal} />
    </Card>
  );
}
