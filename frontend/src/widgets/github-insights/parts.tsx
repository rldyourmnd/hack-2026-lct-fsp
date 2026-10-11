import type { CSSProperties, ReactNode } from 'react';
import {
  bandLabel,
  bandTone,
  byStackCategory,
  confidenceLabel,
  type GitHubFlag,
  type GitHubIndex,
  type StackCategory,
  stackCategoryLabel,
  weekdayShortLabel,
} from '@/entities/github-insights';
import { formatShortDate } from '@/shared/lib';
import { Callout, StatusChip, TagList } from '@/shared/ui';
import styles from './github-insights.module.css';

const vars = (values: Record<string, number>) =>
  Object.fromEntries(Object.entries(values).map(([key, value]) => [`--${key}`, String(value)])) as CSSProperties;

export function IndexSummary({ index, children }: { index: GitHubIndex; children?: ReactNode }) {
  return (
    <div className={styles.summary}>
      <div className={styles.score} style={vars({ score: index.score })}>
        <span className={styles.scoreValue}>
          <span aria-hidden="true">{index.score}</span>
          <span className="visually-hidden">Индекс {index.score} из 100</span>
        </span>
      </div>
      <div className="stack" style={{ gap: '0.5rem' }}>
        <div className="row">
          <StatusChip tone={bandTone[index.band]}>{bandLabel[index.band]}</StatusChip>
          <StatusChip tone="neutral" icon="info">
            Уверенность: {confidenceLabel[index.confidence]}
          </StatusChip>
        </div>
        {children}
      </div>
    </div>
  );
}

type DimensionItem = {
  id: string;
  label: string;
  score: number;
  summary: string;
  weight?: number;
  metrics: { id: string; label: string; display: string }[];
};

/** Dimensions with their scores; `collapsible` hides each dimension's metrics behind a disclosure. */
export function DimensionList({
  dimensions,
  collapsible = false,
}: {
  dimensions: DimensionItem[];
  collapsible?: boolean;
}) {
  return (
    <ul className={styles.dimensions}>
      {dimensions.map((dimension) => (
        <li key={dimension.id}>
          <div className={styles.dimensionHead}>
            <span>{dimension.label}</span>
            <span>
              {dimension.score}
              <span className="small muted"> / 100</span>
            </span>
          </div>
          <div className={styles.bar} aria-hidden="true">
            <span className={styles.barFill} style={vars({ value: dimension.score })} />
          </div>
          <span className="small muted">
            {dimension.summary}
            {dimension.weight !== undefined && ` · вес в индексе ${dimension.weight}%`}
          </span>
          {collapsible ? (
            <details className={styles.details}>
              <summary>Показатели: {dimension.metrics.length}</summary>
              <Metrics metrics={dimension.metrics} />
            </details>
          ) : (
            <Metrics metrics={dimension.metrics} />
          )}
        </li>
      ))}
    </ul>
  );
}

function Metrics({ metrics }: { metrics: DimensionItem['metrics'] }) {
  return (
    <ul className={styles.metrics}>
      {metrics.map((metric) => (
        <li key={metric.id}>
          <span className="muted">{metric.label}</span>
          <span>{metric.display}</span>
        </li>
      ))}
    </ul>
  );
}

export function Numbers({ items }: { items: [string, ReactNode][] }) {
  return (
    <dl className={styles.numbers}>
      {items.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd>{value}</dd>
        </div>
      ))}
    </dl>
  );
}

/** Commit heatmap, Monday first; screen readers get the summary and the monthly table. */
export function ActivityCalendar({ days }: { days: { date: string; count: number }[] }) {
  if (!days.length) return null;
  const nonZero = days
    .map((day) => day.count)
    .filter(Boolean)
    .sort((a, b) => a - b);
  const quartile = (fraction: number) => nonZero[Math.floor((nonZero.length - 1) * fraction)] ?? 1;
  const thresholds = [quartile(0.25), quartile(0.5), quartile(0.75)];
  const level = (count: number) => (count === 0 ? 0 : 1 + thresholds.filter((threshold) => count > threshold).length);
  const first = new Date(`${days[0]?.date}T00:00:00Z`);
  const leading = (first.getUTCDay() + 6) % 7;
  const months = new Map<string, number>();
  for (const day of days) months.set(day.date.slice(0, 7), (months.get(day.date.slice(0, 7)) ?? 0) + day.count);
  const total = nonZero.reduce((sum, value) => sum + value, 0);
  return (
    <div className="stack" style={{ gap: '0.5rem' }}>
      <div
        className={styles.calendar}
        role="img"
        aria-label={`Коммиты за год по дням: ${total}, активных дней ${nonZero.length}`}
      >
        {Array.from({ length: leading }, (_, index) => (
          <span key={`empty-${String(index)}`} className={styles.cell} data-empty="" />
        ))}
        {days.map((day) => (
          <span
            key={day.date}
            className={styles.cell}
            data-level={level(day.count)}
            title={`${formatShortDate(day.date)}: ${day.count}`}
          />
        ))}
      </div>
      <details>
        <summary className="small">Коммиты по месяцам таблицей</summary>
        <div className="table-wrap">
          <table className="table">
            <thead>
              <tr>
                <th scope="col">Месяц</th>
                <th scope="col">Коммиты</th>
              </tr>
            </thead>
            <tbody>
              {[...months].map(([month, count]) => (
                <tr key={month}>
                  <td>{month}</td>
                  <td>{count}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </details>
    </div>
  );
}

/** Column chart drawn as a list: each item states its own value. */
export function Columns({ values, labels, unit }: { values: number[]; labels: string[]; unit: string }) {
  const max = Math.max(1, ...values);
  return (
    <ol className={styles.columns} style={vars({ count: values.length })}>
      {values.map((value, index) => (
        <li
          key={labels[index]}
          aria-label={`${labels[index]}: ${value}${unit}`}
          title={`${labels[index]}: ${value}${unit}`}
        >
          <span className={styles.columnFill} style={vars({ value: (value / max) * 100 })} />
        </li>
      ))}
    </ol>
  );
}

export function HourChart({ hourly }: { hourly: number[] }) {
  return (
    <div className="stack" style={{ gap: '0.25rem' }}>
      <Columns values={hourly} labels={hourly.map((_, hour) => `${String(hour).padStart(2, '0')}:00`)} unit="%" />
      <div className={styles.axis} aria-hidden="true">
        <span>00</span>
        <span>06</span>
        <span>12</span>
        <span>18</span>
        <span>23</span>
      </div>
    </div>
  );
}

export function WeekdayChart({ weekdays }: { weekdays: number[] }) {
  return (
    <div className="stack" style={{ gap: '0.25rem' }}>
      <Columns values={weekdays} labels={[...weekdayShortLabel]} unit="%" />
      <div className={styles.axis} aria-hidden="true">
        {weekdayShortLabel.map((day) => (
          <span key={day}>{day}</span>
        ))}
      </div>
    </div>
  );
}

type LanguageShare = { language: string; percent: number; filesChanged?: number };

export function LanguageBar({ languages }: { languages: LanguageShare[] }) {
  if (!languages.length) return <p className="small muted">Языки не определены.</p>;
  return (
    <div className="stack" style={{ gap: '0.5rem' }}>
      <div className={styles.languages} aria-hidden="true">
        {languages.map((item) => (
          <span key={item.language} className={styles.segment} style={vars({ value: item.percent })} />
        ))}
      </div>
      <ul className="row small" style={{ margin: 0, padding: 0, listStyle: 'none' }}>
        {languages.map((item) => (
          <li key={item.language}>
            {item.language} — {item.percent}%
            {item.filesChanged !== undefined && (
              <span className="muted"> ({item.filesChanged.toLocaleString('ru-RU')} изм. файлов)</span>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

type ArchetypeShare = { id: string; label: string; percent: number; evidence?: string[] };

/** Areas of work by share of the candidate's own changed files, with the evidence behind each. */
export function Archetypes({ archetypes }: { archetypes: ArchetypeShare[] }) {
  if (!archetypes.length) return null;
  return (
    <ul className={styles.dimensions}>
      {archetypes.map((archetype) => (
        <li key={archetype.id}>
          <div className={styles.dimensionHead}>
            <span>{archetype.label}</span>
            <span>{archetype.percent}%</span>
          </div>
          <div className={styles.bar} aria-hidden="true">
            <span className={styles.barFill} style={vars({ value: archetype.percent })} />
          </div>
          {archetype.evidence && archetype.evidence.length > 0 && (
            <span className="small muted">{archetype.evidence.join(' · ')}</span>
          )}
        </li>
      ))}
    </ul>
  );
}

/**
 * Technologies grouped by category; the language category is shown by the language bar instead.
 * `compact` puts each category above its technologies for narrow columns.
 */
export function StackGroups({
  stack,
  compact = false,
}: {
  stack: { name: string; category: StackCategory }[];
  compact?: boolean;
}) {
  const groups = byStackCategory(stack.filter((item) => item.category !== 'language'));
  if (!groups.length) return <p className="small muted">Технологии не определены.</p>;
  return (
    <dl className={styles.stack} data-compact={compact || undefined}>
      {groups.map((group) => (
        <div key={group.category}>
          <dt>{stackCategoryLabel[group.category]}</dt>
          <dd>
            <TagList items={group.items.map((item) => item.name)} />
          </dd>
        </div>
      ))}
    </dl>
  );
}

export function Flags({ flags }: { flags: GitHubFlag[] }) {
  if (!flags.length) return null;
  return (
    <div className="stack">
      {flags.map((flag) => (
        <Callout key={flag.id} tone={flag.severity === 'warning' ? 'warning' : 'info'} title={flag.title}>
          {flag.detail}
        </Callout>
      ))}
    </div>
  );
}

export const codeClass = styles.code;
export const plainListClass = styles.plain;
