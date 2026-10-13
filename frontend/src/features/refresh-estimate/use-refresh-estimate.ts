import { useMutation, useQueryClient } from '@tanstack/react-query';
import { type EstimateAnswers, profileKey, refreshEstimate } from '@/entities/profile';
import { useScope } from '@/entities/session';
import { sourcesKey } from '@/entities/source';
import { vacanciesKey } from '@/entities/vacancy';
import { useToast } from '@/shared/ui';

/**
 * Recomputes the profile estimate from the connected sources and stores the updated profile. The
 * server may also re-analyze an outdated GitHub report, so the sources are refetched too.
 */
export function useRefreshEstimate() {
  const scope = useScope();
  const queryClient = useQueryClient();
  const toast = useToast();
  return useMutation({
    mutationFn: (answers: EstimateAnswers = 'keep') => refreshEstimate(answers),
    onSuccess: async (profile, answers) => {
      queryClient.setQueryData(profileKey(scope), profile);
      void queryClient.invalidateQueries({ queryKey: vacanciesKey(scope) });
      await queryClient.invalidateQueries({ queryKey: sourcesKey(scope) });
      toast(answers === 'replace' ? 'Анкета заполнена по источникам' : 'Оценка обновлена');
    },
  });
}
