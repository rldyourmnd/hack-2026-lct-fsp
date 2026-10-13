import { useEffect } from 'react';
import type { startObservability } from '@/shared/observability';
import type { createAppRouter } from './router';

export function useObservability(router: ReturnType<typeof createAppRouter>) {
  useEffect(() => {
    if (import.meta.env.VITE_OBSERVABILITY !== '1') return;
    let active = true;
    let observer: ReturnType<typeof startObservability> | undefined;
    const stop = () => observer?.stopReplay();
    // Subscribe before SDK initialization: navigation must stop recording before the next DOM snapshot.
    const before = router.subscribe('onBeforeNavigate', stop);
    const resolved = router.subscribe('onResolved', () => observer?.view());
    window.addEventListener('popstate', stop, true);
    window.addEventListener('hashchange', stop, true);
    void import('@/shared/observability')
      .then(({ startObservability }) => {
        if (!active) return;
        observer = startObservability();
        observer.view();
      })
      .catch(() => {
        // Diagnostic delivery must not break the user's workflow or expose import errors.
        stop();
      });
    return () => {
      active = false;
      stop();
      before();
      resolved();
      window.removeEventListener('popstate', stop, true);
      window.removeEventListener('hashchange', stop, true);
    };
  }, [router]);
}
