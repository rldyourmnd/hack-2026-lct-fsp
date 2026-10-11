import { Button } from '@/shared/ui';
import { useRefreshEstimate } from './use-refresh-estimate';

/** Replaces every questionnaire answer but the name with the one the connected sources give. */
export function RebuildProfileButton({ disabled }: { disabled?: boolean }) {
  const rebuild = useRefreshEstimate();
  return (
    <Button
      size="sm"
      icon="sparkles"
      onClick={() => rebuild.mutate('replace')}
      pending={rebuild.isPending}
      pendingLabel="Заполняем…"
      disabled={disabled}
    >
      Заполнить анкету по источникам
    </Button>
  );
}
