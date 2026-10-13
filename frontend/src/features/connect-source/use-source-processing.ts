import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { isTerminalJob, type Job, useJob } from '@/entities/job';
import { profileKey } from '@/entities/profile';
import { useScope } from '@/entities/session';
import { graphKey, skillsKey } from '@/entities/skill';
import { type Source, sourcesKey, syncSource } from '@/entities/source';
import { vacanciesKey } from '@/entities/vacancy';

/** Follows the source's active job (resumed after reload) and refreshes everything enrichment can change. */
export function useSourceProcessing(source: Source | undefined) {
  const scope = useScope();
  const queryClient = useQueryClient();
  const [startedJobId, setStartedJobId] = useState<string | null>(null);
  const refresh = () =>
    Promise.all(
      [sourcesKey(scope), skillsKey(scope), graphKey(scope), profileKey(scope), vacanciesKey(scope)].map((queryKey) =>
        queryClient.invalidateQueries({ queryKey }),
      ),
    );
  const jobId = source?.activeJobId ?? startedJobId;
  const job = useJob(scope, jobId, () => void refresh());
  const sync = useMutation({
    mutationFn: () => {
      if (!source) throw new Error('Источник не найден');
      return syncSource(source.id);
    },
    onSuccess: async (result) => {
      setStartedJobId(result.id);
      // Inline adapters can return a terminal job without setting source.activeJobId.
      if (isTerminalJob(result)) await refresh();
      else await queryClient.invalidateQueries({ queryKey: sourcesKey(scope) });
    },
  });
  const lastFailure: Job['error'] = job.data?.status === 'failed' ? job.data.error : null;
  return {
    job: job.data,
    processing: Boolean(jobId) && !isTerminalJob(job.data),
    lastFailure,
    sync,
  };
}
