import { expect, test } from '@playwright/test';
import { ALEXEY, actor, COMPANY_A, COMPANY_B, resetMock, signIn } from './support';

test.beforeEach(() => resetMock('baseline'));

test('UI-31 the weekly task is answered once, reviewed by its company and keeps the profile fresh', async ({
  browser,
}) => {
  const candidate = await actor(browser, ALEXEY, 'candidate');
  await candidate.page.goto('/candidate/tasks');
  await expect(candidate.page.getByRole('heading', { name: 'Повторные попытки отправки уведомлений' })).toBeVisible();
  await expect(candidate.page.getByText(/от «Север Тех»/)).toBeVisible();
  await expect(candidate.page.getByText(/Давно не обновлялся · проверка/)).toBeVisible();
  await candidate.page.getByRole('button', { name: 'Отправить ответ' }).click();
  await expect(candidate.page.getByText('Напишите решение или подход')).toBeVisible();
  await candidate.page
    .getByLabel('Ваш подход')
    .fill('Очередь попыток в PostgreSQL, экспоненциальная задержка и предел в пять попыток.');
  await candidate.page.getByRole('button', { name: 'Отправить ответ' }).click();
  await expect(candidate.page.getByText('Ждёт оценки компании')).toBeVisible();
  await expect(candidate.page.getByText(/Профиль актуален · задание/)).toBeVisible();
  await expect(candidate.page.getByRole('button', { name: 'Отправить ответ' })).toHaveCount(0);

  const employer = await actor(browser, COMPANY_A, 'employer');
  await employer.page.goto('/employer/tasks?tab=responses');
  const card = employer.page.getByRole('article').filter({ hasText: 'Кандидат №024' });
  await expect(card.getByText('Очередь попыток в PostgreSQL')).toBeVisible();
  await card.getByLabel('Комментарий кандидату').fill('Хороший план, добавьте журнал попыток.');
  await card.getByRole('button', { name: 'Засчитать' }).click();
  await expect(card.getByText('Засчитано')).toBeVisible();

  await candidate.page.reload();
  await expect(candidate.page.getByText('Засчитано')).toBeVisible();
  await expect(candidate.page.getByText('Хороший план, добавьте журнал попыток.')).toBeVisible();

  const other = await actor(browser, COMPANY_B, 'employer');
  await other.page.goto('/employer/candidates/candidate024');
  await expect(other.page.getByText(/Профиль актуален · задание/)).toBeVisible();
  await other.page.goto('/employer/tasks?tab=responses');
  await expect(other.page.getByText('Кандидат №024')).toHaveCount(0);
});

test('UI-32 companies add and archive tasks; a candidate without a category is told why there is none', async ({
  page,
}) => {
  await signIn(page, COMPANY_A, 'employer');
  await page.goto('/employer/tasks');
  await page.getByRole('button', { name: 'Новое задание' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByRole('button', { name: 'Добавить задание' }).click();
  await expect(dialog.getByText('Укажите название')).toBeVisible();
  await dialog.getByLabel('Название').fill('Индекс для поиска заказов');
  await dialog.getByLabel('Условие').fill('Какой индекс ускорит выборку заказов клиента за последний месяц?');
  await dialog.getByLabel('Навык').selectOption('postgresql');
  await dialog.getByRole('button', { name: 'Добавить задание' }).click();
  await expect(page.getByRole('heading', { name: 'Индекс для поиска заказов' })).toBeVisible();
  const card = page.getByRole('article').filter({ hasText: 'Повтор платежа без двойного списания' });
  await card.getByRole('button', { name: 'В архив' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'В архив' }).click();
  await expect(card.getByText('В архиве')).toBeVisible();

  await resetMock('new_candidate');
  await signIn(page, 'new@example.com', 'candidate');
  await page.goto('/candidate/tasks');
  await expect(page.getByRole('heading', { name: 'Задания на этой неделе нет' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Открыть анкету' })).toBeVisible();
});
