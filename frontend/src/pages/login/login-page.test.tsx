import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { HttpResponse, http } from 'msw';
import { describe, expect, it } from 'vitest';
import { metaQueryOptions } from '@/entities/meta';
import { meta, problem } from '../../../test/fixtures';
import { renderWithProviders } from '../../../test/render';
import { API, server } from '../../../test/server';
import { LoginPage } from './login-page';

const unauthenticated = http.get(`${API}/api/v1/session`, () =>
  HttpResponse.json(problem('unauthenticated', 'Войдите в аккаунт', 401).body, { status: 401 }),
);

function renderLogin(realEmail = false) {
  return renderWithProviders(<LoginPage redirectTo={undefined} expired={false} />, {
    prefill: (client) =>
      client.setQueryData(metaQueryOptions().queryKey, { ...meta, capabilities: { ...meta.capabilities, realEmail } }),
  });
}

describe('email sign-in', () => {
  it('shows real mail guidance and keeps demo accounts available when the server sends email', async () => {
    server.use(
      unauthenticated,
      http.post(`${API}/api/v1/auth/email/challenges`, () =>
        HttpResponse.json(
          { challengeId: 'challenge-real', expiresAt: '2026-10-05T12:15:00.000Z', delivery: 'email' },
          { status: 201 },
        ),
      ),
    );
    await renderLogin(true);
    expect(screen.getByText('Демонстрационные аккаунты')).toBeTruthy();
    await userEvent.type(screen.getByLabelText(/Электронная почта/), 'real@example.com');
    await userEvent.click(screen.getByRole('button', { name: 'Получить код' }));
    expect(await screen.findByText('Проверьте почту')).toBeTruthy();
    expect(screen.queryByText('Демонстрационная почта')).toBeNull();
    expect(screen.queryByText(/Код для этого стенда/)).toBeNull();
  });
  it('validates the address before contacting the server', async () => {
    server.use(unauthenticated);
    await renderLogin();
    await userEvent.type(screen.getByLabelText(/Электронная почта/), 'not-an-email');
    await userEvent.click(screen.getByRole('button', { name: 'Получить код' }));
    expect(await screen.findByText(/Проверьте адрес/)).toBeTruthy();
  });

  it('explains a wrong code and lets the user request a new one', async () => {
    server.use(
      unauthenticated,
      http.post(`${API}/api/v1/auth/email/challenges`, () =>
        HttpResponse.json(
          { challengeId: 'challenge-001', expiresAt: '2026-10-05T12:15:00.000Z', delivery: 'mock_mailbox' },
          { status: 201 },
        ),
      ),
      http.post(`${API}/api/v1/auth/email/verify`, () =>
        HttpResponse.json(problem('invalid_code', 'Неверный код').body, { status: 400 }),
      ),
    );
    await renderLogin();
    await userEvent.type(screen.getByLabelText(/Электронная почта/), 'new@example.com');
    await userEvent.click(screen.getByRole('button', { name: 'Получить код' }));
    expect(await screen.findByText(/Демонстрационная почта/)).toBeTruthy();
    expect(screen.getByText('Код действует 15 минут.')).toBeTruthy();
    await userEvent.type(screen.getByLabelText('Код из письма'), '000000');
    await userEvent.click(screen.getByRole('button', { name: 'Подтвердить и войти' }));
    expect(await screen.findByText(/Неверный код. Проверьте цифры/)).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Отправить новый код' })).toBeTruthy();
  });

  it('shows the fixed code only for a seeded demo account', async () => {
    server.use(
      unauthenticated,
      http.post(`${API}/api/v1/auth/email/challenges`, () =>
        HttpResponse.json(
          { challengeId: 'challenge-demo', expiresAt: '2026-10-05T12:15:00.000Z', delivery: 'mock_mailbox' },
          { status: 201 },
        ),
      ),
    );
    await renderLogin(true);
    await userEvent.click(screen.getByRole('button', { name: 'Подставить alexey@example.com' }));
    await userEvent.click(screen.getByRole('button', { name: 'Получить код' }));
    expect(await screen.findByText('123456')).toBeTruthy();
  });

  it('tells the user that an existing account cannot change its role', async () => {
    server.use(
      unauthenticated,
      http.post(`${API}/api/v1/auth/email/challenges`, () =>
        HttpResponse.json(problem('role_mismatch', 'Роль существующего аккаунта изменить нельзя', 409).body, {
          status: 409,
        }),
      ),
    );
    await renderLogin();
    await userEvent.click(screen.getByText('Работодатель'));
    await userEvent.type(screen.getByLabelText(/Электронная почта/), 'alexey@example.com');
    await userEvent.click(screen.getByRole('button', { name: 'Получить код' }));
    expect(await screen.findByText('Роль аккаунта изменить нельзя')).toBeTruthy();
  });
});
