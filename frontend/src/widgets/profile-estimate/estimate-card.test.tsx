import { useQuery } from '@tanstack/react-query';
import { act, fireEvent, screen, waitFor } from '@testing-library/react';
import { HttpResponse, http } from 'msw';
import { describe, expect, it } from 'vitest';
import { metaQueryOptions } from '@/entities/meta';
import { type Profile, type ProfileEstimate, profileKey } from '@/entities/profile';
import { scopeOf } from '@/entities/session';
import estimates from '../../../../contracts/fixtures/profile-estimates.json' with { type: 'json' };
import { candidateSession, meta, profile } from '../../../test/fixtures';
import { renderWithProviders } from '../../../test/render';
import { API, server } from '../../../test/server';
import { ProfileEstimateCard } from './estimate-card';

const prefill = (client: import('@tanstack/react-query').QueryClient) =>
  client.setQueryData(metaQueryOptions().queryKey, meta);

// The estimates are the Rust rules' serialized output for the shared fixtures.
const github = estimates.github as ProfileEstimate;

const estimated = (overrides: Partial<Profile> = {}): Profile =>
  profile({
    verifiedGrade: null,
    verifiedSpecialization: null,
    gradeConfirmedAt: null,
    estimate: {
      ...github,
      review: {
        specialization: 'backend',
        grade: 'middle',
        rationale: 'Мало ревью чужого кода.',
        agrees: false,
        recommendations: ['Ревьюйте чужие изменения в открытых проектах'],
      },
    },
    category: { specialization: 'backend', grade: 'senior', eligible: true, source: 'estimate', confidence: 'medium' },
    ...overrides,
  });

/** Renders the card from the cached profile, as the pages do, so cache updates reach it. */
function FromCache({ initial }: { initial: Profile }) {
  const { data } = useQuery({
    queryKey: profileKey(scopeOf(candidateSession)),
    queryFn: () => initial,
    initialData: initial,
    staleTime: Number.POSITIVE_INFINITY,
  });
  return <ProfileEstimateCard profile={data} canEstimate />;
}

describe('profile estimate card', () => {
  it('explains the estimated category with every factor and the second opinion', async () => {
    await renderWithProviders(<ProfileEstimateCard profile={estimated()} canEstimate />, {
      session: candidateSession,
      prefill,
    });
    expect(screen.getByText('Бэкенд · Сеньор · оценка, средняя уверенность')).toBeTruthy();
    expect(screen.getByText(`Балл уровня ${github.score} из 100`)).toBeTruthy();
    for (const factor of github.factors) expect(screen.getAllByText(factor.label).length).toBeGreaterThan(0);
    expect(screen.getByText(/Оценивает грейд как «Мидл». Грейд по правилам не меняется/)).toBeTruthy();
    expect(screen.getByRole('link', { name: /Пройти тест/ })).toBeTruthy();
    // The notes say why the grade is what it is and what the next one needs.
    for (const note of github.notes) expect(screen.getByText(note)).toBeTruthy();
    // The growth plan: rule-based steps with points, skills to work on and the AI's advice.
    for (const step of github.growth.filter((item) => item.id !== 'assessment'))
      expect(screen.getByText(step.action)).toBeTruthy();
    expect(screen.getByRole('list', { name: 'Навыки для развития' })).toBeTruthy();
    expect(screen.getByText('Ревьюйте чужие изменения в открытых проектах')).toBeTruthy();
  });

  it('re-estimates with the two answers evidence cannot give', async () => {
    let patch: unknown = null;
    server.use(
      http.patch(`${API}/api/v1/me/profile`, async ({ request }) => {
        patch = await request.json();
        return HttpResponse.json(estimated());
      }),
    );
    const { queryClient } = await renderWithProviders(<FromCache initial={estimated({ experienceMonths: null })} />, {
      session: candidateSession,
      prefill,
    });
    fireEvent.change(screen.getByLabelText('В IT с какого года'), { target: { value: '2016' } });
    // A background update of the profile (for example the AI review) keeps what was typed.
    act(() => {
      queryClient.setQueryData(
        profileKey(scopeOf(candidateSession)),
        estimated({ experienceMonths: null, revision: 99 }),
      );
    });
    expect(screen.getByLabelText('В IT с какого года')).toHaveProperty('value', '2016');
    fireEvent.change(screen.getByLabelText('Роль в команде'), { target: { value: 'Тимлид' } });
    fireEvent.click(screen.getByRole('button', { name: 'Пересчитать с уточнениями' }));
    await waitFor(() => expect(patch).toEqual({ experienceSince: 2016, teamRole: 'Тимлид' }));
  });

  it('rebuilds every answer from the sources in one click', async () => {
    const rebuilt = estimated();
    let mode: string | null = null;
    server.use(
      http.post(`${API}/api/v1/me/profile/estimate`, ({ request }) => {
        mode = new URL(request.url).searchParams.get('answers');
        return HttpResponse.json(rebuilt);
      }),
    );
    const { queryClient } = await renderWithProviders(<ProfileEstimateCard profile={estimated()} canEstimate />, {
      session: candidateSession,
      prefill,
    });
    // One click, no confirmation: edited answers come back from the sources.
    fireEvent.click(screen.getByRole('button', { name: 'Заполнить анкету по источникам' }));
    await waitFor(() => expect(mode).toBe('replace'));
    await waitFor(() => expect(queryClient.getQueryData(profileKey(scopeOf(candidateSession)))).toEqual(rebuilt));
  });

  it('asks for a source without an estimate and recalculates on request', async () => {
    const refreshed = estimated();
    server.use(http.post(`${API}/api/v1/me/profile/estimate`, () => HttpResponse.json(refreshed)));
    const { queryClient } = await renderWithProviders(
      <ProfileEstimateCard profile={profile({ estimate: null })} canEstimate />,
      { session: candidateSession, prefill },
    );
    expect(screen.getByText('Профиль ещё не оценён')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Оценить профиль' }));
    await waitFor(() => expect(queryClient.getQueryData(profileKey(scopeOf(candidateSession)))).toEqual(refreshed));
  });

  it('points to the sources when nothing is connected', async () => {
    await renderWithProviders(<ProfileEstimateCard profile={profile({ estimate: null })} canEstimate={false} />, {
      session: candidateSession,
      prefill,
    });
    expect(screen.getByText('Подключите GitHub или ФСП')).toBeTruthy();
    expect(screen.getByRole('link', { name: /Подключить источники/ })).toBeTruthy();
  });

  it('waits for the background AI review of a fresh estimate', async () => {
    const fresh = estimated({ estimate: { ...github, computedAt: new Date().toISOString(), review: null } });
    server.use(http.get(`${API}/api/v1/me/profile`, () => HttpResponse.json(fresh)));
    await renderWithProviders(<ProfileEstimateCard profile={fresh} canEstimate />, {
      session: candidateSession,
      prefill: (client) =>
        client.setQueryData(metaQueryOptions().queryKey, {
          ...meta,
          capabilities: { ...meta.capabilities, realLlm: true },
        }),
    });
    expect(screen.getByText('ИИ готовит второе мнение об оценке…')).toBeTruthy();
  });
});
