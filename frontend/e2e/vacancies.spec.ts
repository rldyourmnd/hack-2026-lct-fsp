import { expect, test } from '@playwright/test';
import { ALEXEY, actor, COMPANY_A, COMPANY_B, PRIVATE_MARKERS, recordApiBodies, resetMock, signIn } from './support';

test.beforeEach(() => resetMock('baseline'));

test('UI-27 an employer publishes a need and candidates browse it with salary, filters and their fit', async ({
  browser,
}) => {
  const employer = await actor(browser, COMPANY_A, 'employer');
  await employer.page.goto('/employer/needs');
  const card = employer.page
    .getByRole('article')
    .filter({ has: employer.page.getByRole('heading', { name: 'Бэкенд-разработчик', exact: true }) });
  await card.getByRole('button', { name: 'Опубликовать вакансию' }).click();
  await employer.page.getByRole('dialog').getByRole('button', { name: 'Опубликовать' }).click();
  await expect(card.getByText(/Опубликована \d/)).toBeVisible();
  await expect(card.getByRole('link', { name: 'Отклики: 0' })).toBeVisible();

  const candidate = await actor(browser, ALEXEY, 'candidate');
  await candidate.page.goto('/candidate/vacancies');
  await expect(candidate.page.getByRole('heading', { name: 'Вакансии', level: 1 })).toBeVisible();
  await expect(candidate.page.getByText('Найдено: 4')).toBeVisible();
  await expect(candidate.page.getByRole('link', { name: 'Бэкенд-разработчик сервиса уведомлений' })).toBeVisible();
  await candidate.page.getByLabel('Формат работы').selectOption('office');
  await expect(candidate.page).toHaveURL(/workMode=office/);
  await expect(candidate.page.getByText('Найдено: 1')).toBeVisible();
  await expect(candidate.page.getByRole('link', { name: 'Фронтенд-разработчик (React)' })).toBeVisible();
  await candidate.page.getByRole('button', { name: 'Сбросить фильтры' }).click();
  await candidate.page.getByRole('link', { name: 'Бэкенд-разработчик сервиса уведомлений' }).click();
  await expect(candidate.page.getByText('120 000 – 160 000 ₽/мес').first()).toBeVisible();
  await expect(candidate.page.getByText('Вы подходите по обязательным требованиям').first()).toBeVisible();
  await expect(candidate.page.getByRole('link', { name: 'hr@sever-tech.example' })).toBeVisible();

  await card.getByRole('button', { name: 'Снять с публикации' }).click();
  await employer.page.getByRole('dialog').getByRole('button', { name: 'Снять с публикации' }).click();
  await expect(card.getByText('Не опубликована — приглашать можно без публикации')).toBeVisible();
  await candidate.page.goto('/candidate/vacancies');
  await expect(candidate.page.getByText('Найдено: 3')).toBeVisible();
});

test('UI-28 applying opens only the chosen contacts to that company, which reviews and accepts', async ({
  browser,
}) => {
  const candidate = await actor(browser, ALEXEY, 'candidate');
  await candidate.page.goto('/candidate/vacancies/need-002');
  await candidate.page.getByLabel('Сопроводительное письмо').fill('Хочу развивать доставку уведомлений на Rust.');
  await candidate.page.getByRole('checkbox', { name: /^Имя и фамилия/ }).uncheck();
  await candidate.page.getByRole('button', { name: 'Откликнуться и открыть контакты' }).click();
  await candidate.page.waitForURL(/\/candidate\/applications\/application-/);
  await expect(candidate.page.getByRole('heading', { name: 'Отклик в «Север Тех»' })).toBeVisible();
  await expect(candidate.page.getByText('Отправлен', { exact: true })).toBeVisible();

  const employer = await actor(browser, COMPANY_A, 'employer');
  const bodies = recordApiBodies(employer.page);
  await employer.page.goto('/employer/needs');
  await expect(employer.page.getByRole('link', { name: 'Отклики: 1 новых' })).toBeVisible();
  await employer.page.getByRole('link', { name: 'Отклики: 1 новых' }).click();
  await employer.page.getByRole('link', { name: 'Кандидат №024' }).click();
  await expect(employer.page.getByRole('heading', { name: 'Отклик: Кандидат №024' })).toBeVisible();
  await expect(employer.page.getByText('Хочу развивать доставку уведомлений на Rust.')).toBeVisible();
  await expect(employer.page.getByRole('link', { name: 'alexey@example.com' })).toBeVisible();
  await expect(employer.page.getByText('Алексей Морозов')).toHaveCount(0);
  for (const marker of PRIVATE_MARKERS.filter((item) => item !== 'alexey@'))
    for (const body of bodies) expect(body).not.toContain(marker);

  await candidate.page.reload();
  await expect(candidate.page.getByText('Просмотрен компанией')).toBeVisible();
  await employer.page.getByRole('button', { name: 'Принять и перейти к разговору' }).click();
  await expect(employer.page.getByRole('heading', { name: 'Вы приняли отклик' })).toBeVisible();
  await candidate.page.reload();
  await expect(candidate.page.getByText('Компания готова к разговору').first()).toBeVisible();
  await expect(candidate.page.getByRole('button', { name: 'Отозвать отклик' })).toHaveCount(0);

  const companyB = await actor(browser, COMPANY_B, 'employer');
  const other = await (await companyB.page.request.get('/api/v1/candidates/candidate024/disclosure')).json();
  expect(other).toMatchObject({ granted: false, fields: {} });
});

test('UI-29 withdrawing an application takes back its contacts; a repeat is refused while it is open', async ({
  browser,
}) => {
  const candidate = await actor(browser, ALEXEY, 'candidate');
  await candidate.page.goto('/candidate/vacancies/need-b-001');
  await candidate.page.getByLabel('Сопроводительное письмо').fill('Интересна роль в вашей команде.');
  await candidate.page.getByRole('button', { name: 'Откликнуться и открыть контакты' }).click();
  await candidate.page.waitForURL(/\/candidate\/applications\/application-/);
  await candidate.page.goto('/candidate/vacancies/need-b-001');
  await expect(candidate.page.getByRole('heading', { name: 'Вы откликнулись' })).toBeVisible();
  await candidate.page.getByRole('link', { name: 'Статус отклика' }).click();

  const companyB = await actor(browser, COMPANY_B, 'employer');
  let disclosure = await (await companyB.page.request.get('/api/v1/candidates/candidate024/disclosure')).json();
  expect(disclosure).toMatchObject({ granted: true, fields: { name: 'Алексей Морозов', email: 'alexey@example.com' } });

  await candidate.page.getByRole('button', { name: 'Отозвать отклик' }).click();
  await candidate.page.getByRole('dialog').getByRole('button', { name: 'Отозвать' }).click();
  await expect(candidate.page.getByText('Отозван вами')).toBeVisible();
  disclosure = await (await companyB.page.request.get('/api/v1/candidates/candidate024/disclosure')).json();
  expect(disclosure.granted).toBe(false);
  await companyB.page.goto('/employer/applications');
  await expect(companyB.page.getByText('Отозван кандидатом')).toBeVisible();
  await candidate.page.goto('/candidate/applications?status=closed');
  await expect(candidate.page.getByRole('link', { name: /Вторая компания: Бэкенд-разработчик/ })).toBeVisible();
});

test('UI-30 a candidate without a category sees why applying is unavailable and where to fix it', async ({ page }) => {
  await resetMock('new_candidate');
  await signIn(page, 'new@example.com', 'candidate');
  await page.goto('/candidate/vacancies/need-002');
  await expect(page.getByRole('heading', { name: 'Отклик пока недоступен' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Откликнуться и открыть контакты' })).toHaveCount(0);
  await expect(page.getByText(/Соответствие появится, когда у профиля будет категория/)).toBeVisible();
});
