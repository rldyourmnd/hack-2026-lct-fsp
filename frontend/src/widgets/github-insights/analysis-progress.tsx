import type { CSSProperties } from 'react';
import type { GitHubAnalysis } from '@/entities/github-insights';
import styles from './github-insights.module.css';

/** The stage and progress of a background GitHub analysis. */
export function AnalysisProgress({ analysis }: { analysis: GitHubAnalysis }) {
  if (analysis.state === 'failed') return null;
  return (
    <div className="stack" style={{ gap: '0.5rem' }}>
      <div className="row small">
        <strong>{analysis.state === 'queued' ? 'Анализ в очереди' : 'Идёт анализ'}</strong>
        <span className="muted">
          {analysis.stage} · {analysis.progress}%
        </span>
      </div>
      <div
        className={styles.progress}
        role="progressbar"
        aria-label="Анализ GitHub"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={analysis.progress}
        aria-valuetext={`${analysis.stage}, ${analysis.progress}%`}
      >
        <span style={{ '--value': String(analysis.progress) } as CSSProperties} />
      </div>
      <p className="small muted" style={{ margin: 0 }}>
        Без токена GitHub ограничивает публичные запросы, поэтому анализ идёт несколько минут. Страницу можно закрыть:
        отчёт сохранится, когда анализ завершится.
      </p>
    </div>
  );
}
