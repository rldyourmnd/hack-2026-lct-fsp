import type { Browser, BrowserContext, Page } from '@playwright/test';
import type { components } from '../../src/shared/api/generated/schema';

type Actor = { page: Page; email: string; role: 'candidate' | 'employer' };

/** Own only fresh acceptance actors; cleanup uses their ordinary authenticated API. */
export function acceptanceActors(browser: Browser) {
  const contexts: BrowserContext[] = [];
  const actors: Actor[] = [];
  return {
    async newPage() {
      const context = await browser.newContext({ locale: 'ru-RU', viewport: { width: 1440, height: 900 } });
      contexts.push(context);
      return context.newPage();
    },
    authenticated(page: Page, email: string, role: Actor['role']) {
      if (!contexts.includes(page.context()) || !/^fsp-ci-[a-z0-9-]+@/i.test(email))
        throw new Error('Cleanup is restricted to fresh acceptance actors in owned contexts');
      actors.push({ page, email, role });
    },
    async dispose() {
      const cleanups = await Promise.allSettled(actors.map(cleanupActor));
      const closures = await Promise.allSettled(contexts.map((context) => context.close()));
      const failures = [...cleanups, ...closures].filter((result) => result.status === 'rejected');
      if (failures.length)
        throw new AggregateError(
          failures.map((result) => result.reason),
          'Acceptance cleanup failed',
        );
    },
  };
}

async function cleanupActor({ page, email, role }: Actor) {
  if (role === 'candidate') {
    const response = await page.request.get('/api/v1/me/profile', { timeout: 10_000 });
    if (!response.ok()) throw new Error(`Candidate cleanup read failed: HTTP ${response.status()}`);
    const profile: components['schemas']['Profile'] = await response.json();
    if (profile.email !== email) throw new Error('Candidate cleanup identity mismatch');
    const updated = await page.request.patch('/api/v1/me/profile', {
      data: { consents: { ...profile.consents, publication: false } },
      timeout: 10_000,
    });
    if (!updated.ok()) throw new Error(`Candidate cleanup write failed: HTTP ${updated.status()}`);
    const hidden: components['schemas']['Profile'] = await updated.json();
    if (hidden.consents.publication || hidden.onboarding.visibleToEmployers)
      throw new Error('Acceptance candidate is still published');
  } else {
    // Discover all needs of this fresh company, including one created before a UI assertion failed.
    const response = await page.request.get('/api/v1/needs', { timeout: 10_000 });
    if (!response.ok()) throw new Error(`Company cleanup read failed: HTTP ${response.status()}`);
    const needs: components['schemas']['Needs'] = await response.json();
    const results = await Promise.allSettled(
      needs.items.map(async (need) => {
        const removed = await page.request.delete(`/api/v1/needs/${encodeURIComponent(need.id)}`, { timeout: 10_000 });
        if (removed.status() !== 204 && removed.status() !== 404)
          throw new Error(`Company cleanup write failed: HTTP ${removed.status()}`);
      }),
    );
    const failures = results.filter((result) => result.status === 'rejected');
    if (failures.length)
      throw new AggregateError(
        failures.map((result) => result.reason),
        'Company cleanup failed',
      );
  }
}
