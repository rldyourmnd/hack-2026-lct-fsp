import { expect, test } from '@playwright/test';
import { ALEXEY, actor, COMPANY_A, COMPANY_B, PRIVATE_MARKERS, recordApiBodies, resetMock } from './support';

test.beforeEach(() => resetMock('qualified'));

async function sendInvitation(page: import('@playwright/test').Page, needId: string, message: string) {
  await page.goto(`/employer/candidates/candidate024?needId=${needId}`);
  await page.getByRole('button', { name: 'Пригласить кандидата' }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByText('До вычета налогов')).toBeVisible();
  await dialog.getByLabel('Описание предложения').fill(message);
  await dialog.getByRole('button', { name: 'Отправить приглашение' }).dblclick();
  await expect(page.getByText(/Ждём решения кандидата|Отправлено|Просмотрено/).first()).toBeVisible();
}

test('UI-20 UI-21 UI-23 anonymous until acceptance, then only chosen fields for the inviting company', async ({
  browser,
}) => {
  const companyA = await actor(browser, COMPANY_A, 'employer');
  const bodies = recordApiBodies(companyA.page);
  await companyA.page.goto('/employer/candidates?needId=need-001&mode=recommended');
  await companyA.page.goto('/employer/candidates/candidate024?needId=need-001');
  await companyA.page.getByRole('tab', { name: 'Навыки' }).click();
  await companyA.page.getByRole('button', { name: 'Граф' }).click();
  await expect(companyA.page.getByRole('button', { name: /Технология: PostgreSQL/ })).toBeVisible();
  const html = await companyA.page.content();
  for (const marker of PRIVATE_MARKERS) {
    expect(html).not.toContain(marker);
    for (const body of bodies) expect(body).not.toContain(marker);
  }
  await expect(companyA.page.getByText('Контакты и ссылки скрыты до согласия кандидата.')).toBeVisible();

  await sendInvitation(companyA.page, 'need-001', 'Приглашаем обсудить роль в команде заказов.');
  const invitations = await companyA.page.request.get('/api/v1/invitations');
  expect((await invitations.json()).items).toHaveLength(1);

  const candidate = await actor(browser, ALEXEY, 'candidate');
  await candidate.page.goto('/candidate/invitations');
  await candidate.page.getByRole('link', { name: /Север Тех: Бэкенд-разработчик/ }).click();
  await expect(candidate.page.getByText('200 000 – 260 000 ₽/мес').first()).toBeVisible();
  await expect(candidate.page.getByText('До вычета налогов').first()).toBeVisible();
  await expect(candidate.page.getByRole('link', { name: 'hr@sever-tech.example' })).toBeVisible();
  await candidate.page.getByRole('checkbox', { name: /^Email/ }).uncheck();
  await candidate.page.getByRole('button', { name: 'Принять и открыть контакты' }).click();
  await expect(candidate.page.getByRole('heading', { name: 'Вы приняли приглашение' })).toBeVisible();

  await companyA.page.reload();
  await expect(companyA.page.getByText('Алексей Морозов')).toBeVisible();
  await expect(companyA.page.getByText('alexey@example.com')).toHaveCount(0);
  const disclosure = await (await companyA.page.request.get('/api/v1/candidates/candidate024/disclosure')).json();
  expect(disclosure).toMatchObject({ granted: true, fields: { name: 'Алексей Морозов' } });
  expect(disclosure.fields.email).toBeUndefined();

  const companyB = await actor(browser, COMPANY_B, 'employer');
  await companyB.page.goto('/employer/candidates/candidate024');
  await expect(companyB.page.getByRole('heading', { name: 'Кандидат №024' })).toBeVisible();
  await expect(companyB.page.getByText('Алексей')).toHaveCount(0);
  const other = await (await companyB.page.request.get('/api/v1/candidates/candidate024/disclosure')).json();
  expect(other).toMatchObject({ granted: false, fields: {} });
});

test('UI-22 viewing changes only the status; decline grants nothing and both sides agree', async ({ browser }) => {
  const companyB = await actor(browser, COMPANY_B, 'employer');
  await sendInvitation(companyB.page, 'need-b-001', 'Предложение от второй компании.');
  const candidate = await actor(browser, ALEXEY, 'candidate');
  await candidate.page.goto('/candidate/invitations');
  await expect(candidate.page.getByText('Новое')).toBeVisible();
  await candidate.page.getByRole('link', { name: /Вторая компания/ }).click();
  await expect(candidate.page.getByText('Просмотрено · ждёт решения')).toBeVisible();
  await companyB.page.goto('/employer/invitations');
  await expect(companyB.page.getByText('Просмотрено', { exact: true })).toBeVisible();
  expect((await (await companyB.page.request.get('/api/v1/candidates/candidate024/disclosure')).json()).granted).toBe(
    false,
  );

  await candidate.page.getByRole('button', { name: 'Отклонить' }).click();
  await candidate.page.getByRole('dialog').getByRole('button', { name: 'Отклонить' }).click();
  await expect(candidate.page.getByRole('heading', { name: 'Вы отклонили приглашение' })).toBeVisible();
  await companyB.page.getByRole('button', { name: 'Обновить статусы' }).click();
  await expect(companyB.page.getByText('Отклонено кандидатом')).toBeVisible();
  expect((await (await companyB.page.request.get('/api/v1/candidates/candidate024/disclosure')).json()).granted).toBe(
    false,
  );
});

test('UI-24 a stale tab sees the canonical decision and a withdrawn offer cannot be accepted', async ({ browser }) => {
  const companyA = await actor(browser, COMPANY_A, 'employer');
  await sendInvitation(companyA.page, 'need-001', 'Предложение для проверки конфликтов.');
  const candidate = await actor(browser, ALEXEY, 'candidate');
  await candidate.page.goto('/candidate/invitations');
  await candidate.page.getByRole('link', { name: /Север Тех/ }).click();
  const second = await candidate.context.newPage();
  await second.goto(candidate.page.url());
  await candidate.page.getByRole('button', { name: 'Принять и открыть контакты' }).click();
  await expect(candidate.page.getByRole('heading', { name: 'Вы приняли приглашение' })).toBeVisible();
  await second.getByRole('button', { name: 'Отклонить' }).click();
  await second.getByRole('dialog').getByRole('button', { name: 'Отклонить' }).click();
  await expect(second.getByRole('heading', { name: 'Вы приняли приглашение' })).toBeVisible();

  await resetMock('qualified');
  const freshA = await actor(browser, COMPANY_A, 'employer');
  await sendInvitation(freshA.page, 'need-001', 'Предложение, которое будет отозвано.');
  const freshCandidate = await actor(browser, ALEXEY, 'candidate');
  await freshCandidate.page.goto('/candidate/invitations');
  await freshCandidate.page.getByRole('link', { name: /Север Тех/ }).click();
  await freshA.page.goto('/employer/invitations');
  await freshA.page.getByRole('button', { name: 'Отозвать' }).click();
  await freshA.page.getByRole('dialog').getByRole('button', { name: 'Отозвать' }).click();
  await expect(freshA.page.getByText('Отозвано', { exact: true })).toBeVisible();
  await freshCandidate.page.getByRole('button', { name: 'Принять и открыть контакты' }).click();
  await expect(freshCandidate.page.getByRole('heading', { name: 'Приглашение отозвано' })).toBeVisible();
});
