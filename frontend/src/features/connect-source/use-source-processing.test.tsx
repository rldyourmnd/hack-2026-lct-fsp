import { fireEvent, screen, waitFor } from '@testing-library/react';
import { HttpResponse, http } from 'msw';
import { describe, expect, it } from 'vitest';
import type { Job } from '@/entities/job';
import { profileKey } from '@/entities/profile';
import { scopeOf } from '@/entities/session';
import { graphKey, skillsKey } from '@/entities/skill';
import { type Source, sourcesKey } from '@/entities/source';
import { candidateSession } from '../../../test/fixtures';
import { renderWithProviders } from '../../../test/render';
import { API, server } from '../../../test/server';
import { useSourceProcessing } from './use-source-processing';

const source: Source = {
  id: 'source-fsp',
  provider: 'fsp',
  status: 'connected',
  synthetic: true,
  activeJobId: null,
  accountLabel: 'FSP-DEMO',
  lastSyncedAt: null,
  counts: { found: 0, processed: 0, queued: 0, unavailable: 0 },
  repositories: [],
  activeDays: null,
  observationPeriod: null,
  verifiedParticipation: false,
  competitionIds: [],
  notice: '',
  github: null,
};

function Probe() {
  const { sync, processing, lastFailure } = useSourceProcessing(source);
  return (
    <>
      <button type="button" onClick={() => sync.mutate()}>
        Sync
      </button>
      <span>{sync.isSuccess ? 'done' : 'waiting'}</span>
      <span>{processing ? 'processing' : 'idle'}</span>
      {lastFailure && <span>{lastFailure.message}</span>}
    </>
  );
}

describe('source synchronization', () => {
  it.each(['succeeded', 'failed'] as const)(
    'observes an inline %s job without a source activeJobId',
    async (status) => {
      const job: Job = {
        id: 'job-sync',
        kind: 'source_sync',
        status,
        createdAt: '2026-10-08T00:00:00Z',
        updatedAt: '2026-10-08T00:00:00Z',
        result: status === 'succeeded' ? { kind: 'source_sync', source } : null,
        error:
          status === 'failed' ? { code: 'source_unavailable', message: 'Источник недоступен', retryable: true } : null,
        pollAfterMs: 0,
        simulation: true,
      };
      server.use(
        http.post(`${API}/api/v1/me/sources/source-fsp/sync`, () => HttpResponse.json(job)),
        http.get(`${API}/api/v1/jobs/job-sync`, () => HttpResponse.json(job)),
      );
      const scope = scopeOf(candidateSession);
      const keys = [sourcesKey(scope), skillsKey(scope), graphKey(scope), profileKey(scope)];
      const { queryClient } = await renderWithProviders(<Probe />, {
        session: candidateSession,
        prefill: (client) => {
          for (const key of keys) client.setQueryData(key, {});
        },
      });
      fireEvent.click(screen.getByRole('button', { name: 'Sync' }));
      await screen.findByText('done');
      await waitFor(() => expect(screen.getByText('idle')).toBeTruthy());
      for (const key of keys) expect(queryClient.getQueryState(key)?.isInvalidated).toBe(true);
      if (status === 'failed') expect(await screen.findByText('Источник недоступен')).toBeTruthy();
    },
  );
});
