import { readFile } from 'node:fs/promises';
import { test as base, expect, type Page } from '@playwright/test';
import { answerKnowledgeTest } from '../knowledge';
import { acceptanceActors } from './actors';

import { testEmail, verificationCode } from './mail';

async function signIn(page: Page, email: string, role: 'candidate' | 'employer') {
  await page.goto('/login');
  if (role === 'employer') await page.getByText('Работодатель', { exact: true }).click();
  await page.getByLabel('Электронная почта').fill(email);
  await page.getByRole('button', { name: 'Получить код' }).click();
  await expect(page.getByRole('heading', { name: 'Подтвердите почту' })).toBeVisible();
  if (process.env.FSP_AUTH_MODE === 'resend') {
    await expect(page.getByText('Код для этого стенда')).toHaveCount(0);
    await expect(page.getByText('Мы отправили шестизначный код', { exact: false })).toBeVisible();
  }
  await page.getByLabel('Код из письма').fill(await verificationCode(email));
  await page.getByRole('button', { name: 'Подтвердить и войти' }).click();
  await page.waitForURL(new RegExp(`/${role}/`));
}

const test = base.extend<{ actors: ReturnType<typeof acceptanceActors> }>({
  actors: [
    async ({ browser }, use) => {
      const actors = acceptanceActors(browser);
      try {
        await use(actors);
      } finally {
        await actors.dispose();
      }
    },
    { timeout: 90_000 },
  ],
});

test('HTTPS stand serves the app and deep links without mock controls', async ({ page, request }) => {
  const home = await request.get('/');
  expect(home.status()).toBe(200);
  expect(home.headers()['content-security-policy']).toContain("default-src 'self'");
  expect(home.headers()['permissions-policy']).toContain('microphone=(self)');
  expect(home.headers()['permissions-policy']).toContain('camera=()');
  expect(
    (
      await request.post('/__mock__/session', {
        data: { actor: 'candidate024' },
      })
    ).status(),
  ).toBe(404);
  expect((await request.get('/__mock__/email-codes')).status()).toBe(404);
  const metaResponse = await request.get('/api/v1/meta');
  expect(metaResponse.status()).toBe(200);
  const meta = await metaResponse.json();
  expect(meta.capabilities.realEmail).toBe(process.env.FSP_AUTH_MODE === 'resend');
  expect(meta.capabilities.graphInterviews).toBe(true);
  expect(metaResponse.headers()['cache-control']).toContain('no-store');
  await page.goto('/employer/candidates?needId=need-001');
  await page.waitForURL(/\/login\?redirect=/);
  await expect(page.getByRole('heading', { name: 'Вход и регистрация' })).toBeVisible();
  expect(
    await page.evaluate(() => {
      const policy = (document as Document & { featurePolicy?: { allowsFeature: (name: string) => boolean } })
        .featurePolicy;
      return policy?.allowsFeature('microphone');
    }),
  ).toBe(true);
});

test('a new candidate and two companies complete the reverse-hiring flow', async ({ actors }) => {
  test.setTimeout(480_000);
  const candidate = await actors.newPage();
  const candidateEmail = testEmail('candidate');
  await signIn(candidate, candidateEmail, 'candidate');
  actors.authenticated(candidate, candidateEmail, 'candidate');
  await candidate.waitForURL('**/candidate/onboarding');
  await candidate.getByLabel('Согласен на обработку данных').click();
  await expect(candidate.getByLabel('Согласен на обработку данных')).toBeChecked();
  const publication = candidate.getByRole('switch', {
    name: 'Показывать профиль работодателям',
  });
  await publication.click();
  await expect(publication).toHaveAttribute('aria-checked', 'true');
  await candidate.getByLabel('Имя и фамилия').fill('Ирина Стендова');
  await candidate.getByLabel('Грейд для проверки').selectOption('junior');
  await candidate.getByRole('combobox', { name: 'Стек' }).fill('Rust');
  await candidate.getByRole('option', { name: /^Rust/ }).click();
  await candidate.getByRole('textbox', { name: 'Опыт', exact: true }).fill('Проекты на Rust.');
  await candidate.getByLabel('Роль в команде').fill('Разработчик');
  await candidate.getByRole('button', { name: 'Сохранить анкету' }).click();
  await expect(candidate.getByText('Профиль сохранён')).toBeVisible();
  await candidate.getByRole('button', { name: 'Пройти тест' }).click();
  await candidate.waitForURL(/\/workspace$/);
  await answerKnowledgeTest(candidate);
  await candidate.getByRole('button', { name: 'Отправить ответы' }).click();
  await candidate.getByRole('dialog').getByRole('button', { name: 'Отправить' }).click();
  await expect(candidate.getByRole('heading', { name: 'Джуниор подтверждён' })).toBeVisible();
  await candidate.goto('/candidate/assessments');
  await candidate.getByRole('button', { name: 'Начать собеседование' }).click();
  await candidate.waitForURL(/\/candidate\/interviews\//);
  await expect(candidate.getByLabel(/Ваш ответ|Расшифровка ответа/)).toBeVisible();
  await candidate
    .getByLabel(/Ваш ответ|Расшифровка ответа/)
    .fill('Сначала соберу логи и попробую воспроизвести проблему.');
  await candidate.getByRole('button', { name: 'Отправить ответ' }).click();
  // The stand evaluates the answer with a real model, which takes seconds: wait until the interview moves on.
  const currentNode = async () =>
    (await (await candidate.request.get('/api/v1/me/interviews')).json()).items[0].currentQuestion?.nodeId;
  await expect.poll(currentNode, { timeout: 120_000 }).not.toBe('backend.idempotency');
  expect(await currentNode()).toBe('backend.unique-operation');
  await candidate.reload();
  await expect(candidate.getByLabel(/Ваш ответ|Расшифровка ответа/)).toBeVisible();
  const profile = await (await candidate.request.get('/api/v1/me/profile')).json();
  const alias: string = profile.alias;
  const candidateId: string = profile.id;

  await candidate.goto('/candidate/overview');
  const downloadPromise = candidate.waitForEvent('download');
  await candidate.getByRole('button', { name: 'Скачать PDF' }).click();
  const download = await downloadPromise;
  expect((await readFile((await download.path()) as string)).subarray(0, 5).toString()).toBe('%PDF-');

  const companyA = await actors.newPage();
  const companyAEmail = testEmail('employer-a');
  await signIn(companyA, companyAEmail, 'employer');
  actors.authenticated(companyA, companyAEmail, 'employer');
  await completeCompany(companyA, 'Проверка ФСП — компания А');
  const needId = await createNeed(companyA);
  await companyA.goto(`/employer/candidates/${candidateId}?needId=${needId}`);
  await expect(companyA.getByRole('heading', { name: alias })).toBeVisible();
  expect(await companyA.content()).not.toContain('Ирина');
  await companyA.getByRole('button', { name: 'Пригласить кандидата' }).click();
  await companyA.getByLabel('Описание предложения').fill('Приглашаем на стажировку в команду заказов.');
  await companyA.getByRole('dialog').getByRole('button', { name: 'Отправить приглашение' }).click();
  await expect(companyA.getByText(/Ждём решения кандидата/)).toBeVisible();

  await candidate.goto('/candidate/invitations');
  await candidate.getByRole('link', { name: /Проверка ФСП — компания А/ }).click();
  await expect(candidate.getByText('До вычета налогов').first()).toBeVisible();
  await candidate.getByRole('button', { name: 'Принять и открыть контакты' }).click();
  await expect(candidate.getByRole('heading', { name: 'Вы приняли приглашение' })).toBeVisible();

  await companyA.reload();
  await expect(companyA.getByText('Ирина Стендова')).toBeVisible();
  const companyB = await actors.newPage();
  const companyBEmail = testEmail('employer-b');
  await signIn(companyB, companyBEmail, 'employer');
  actors.authenticated(companyB, companyBEmail, 'employer');
  await completeCompany(companyB, 'Проверка ФСП — компания Б');
  const disclosure = await (await companyB.request.get(`/api/v1/candidates/${candidateId}/disclosure`)).json();
  expect(disclosure).toMatchObject({ granted: false, fields: {} });
  await companyB.goto(`/employer/candidates/${candidateId}`);
  await expect(companyB.getByRole('heading', { name: alias })).toBeVisible();
  expect(await companyB.content()).not.toContain('Ирина');
});

async function completeCompany(page: Page, name: string) {
  await page.goto('/employer/company');
  await expect(page.getByRole('heading', { name: 'Профиль компании', exact: true })).toBeVisible();
  await page.getByRole('textbox', { name: 'Название', exact: true }).fill(name);
  await page.getByLabel('Сфера деятельности').fill('Разработка ПО');
  await page
    .getByRole('textbox', { name: 'Описание', exact: true })
    .fill('Синтетическая компания для проверки выпуска платформы.');
  await page.getByRole('button', { name: 'Сохранить профиль' }).click();
  await expect(page.getByText('Профиль компании сохранён')).toBeVisible();
}

async function createNeed(page: Page): Promise<string> {
  await page.goto('/employer/needs/new');
  await page.getByRole('textbox', { name: 'Название', exact: true }).fill('Проверка выпуска — Rust');
  await page.getByLabel('Краткое описание').fill('Синтетическая потребность для автоматической проверки.');
  await page.getByRole('combobox', { name: 'Грейд', exact: true }).selectOption('junior');
  await page.getByLabel('От', { exact: true }).fill('80000');
  await page.getByLabel('До', { exact: true }).fill('120000');
  await page.getByLabel('Навык', { exact: true }).selectOption('rust');
  await page.getByLabel('Уровень не ниже').selectOption('junior');
  await page.getByRole('button', { name: 'Сохранить и найти кандидатов' }).click();
  await page.waitForURL(/\/employer\/candidates\?.*mode=recommended/);
  const id = new URL(page.url()).searchParams.get('needId');
  if (!id) throw new Error('Created need has no id');
  return id;
}
