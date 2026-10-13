import { QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider } from '@tanstack/react-router';
import { useState } from 'react';
import { ToastProvider } from '@/shared/ui';
import { createQueryClient } from './query-client';
import { createAppRouter } from './router';
import { useObservability } from './use-observability';

function createApp() {
  let expiring = false;
  const queryClient = createQueryClient(() => {
    // A 401 anywhere ends the session: drop every private cache entry, then ask for sign-in again.
    if (expiring || router.state.location.pathname === '/login') return;
    expiring = true;
    void queryClient.cancelQueries();
    queryClient.clear();
    void router
      .navigate({ to: '/login', search: { redirect: router.state.location.href, reason: 'expired' } })
      .finally(() => {
        expiring = false;
      });
  });
  const router = createAppRouter(queryClient);
  return { queryClient, router };
}

export function App() {
  const [{ queryClient, router }] = useState(createApp);
  useObservability(router);
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <RouterProvider router={router} />
      </ToastProvider>
    </QueryClientProvider>
  );
}
