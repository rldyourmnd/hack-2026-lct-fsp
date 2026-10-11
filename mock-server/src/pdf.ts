import { fileURLToPath } from 'node:url';
import PDFDocument from 'pdfkit';
import { gradeLabel, specializationShort } from './catalog.ts';
import type { components } from './generated/schema.d.ts';

type Profile = components['schemas']['Profile'];

const font = fileURLToPath(new URL('../fixtures/DejaVuSans.ttf', import.meta.url));

export function profilePdf(profile: Profile): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    const document = new PDFDocument({
      size: 'A4',
      margin: 48,
      info: { Title: 'ФСП / Карьера: профиль участника', Author: 'Демонстрационный мок-сервер' },
    });
    const chunks: Buffer[] = [];
    document.on('data', (chunk: Buffer) => chunks.push(chunk));
    document.on('end', () => resolve(Buffer.concat(chunks)));
    document.on('error', reject);
    document.font(font);
    document.fontSize(10).fillColor('#6B5278').text('ФСП / Карьера · Демонстрационный профиль');
    document.moveDown().fontSize(23).fillColor('#281036').text(profile.name);
    document.fontSize(12).fillColor('#4B4651').text(profile.email);
    document.moveDown();
    const field = (label: string, value: string) => {
      document.fontSize(10).fillColor('#6B5278').text(label);
      document
        .fontSize(12)
        .fillColor('#29242E')
        .text(value || 'Не указано');
      document.moveDown(0.7);
    };
    field('Специализация', specializationShort(profile.specialization));
    field(
      'Подтверждённый уровень',
      profile.verifiedGrade && profile.verifiedSpecialization
        ? `${gradeLabel(profile.verifiedGrade)} · ${specializationShort(profile.verifiedSpecialization)}`
        : 'Пока не подтверждён',
    );
    field('Цель проверки', gradeLabel(profile.selectedGrade));
    field('Технологии', profile.stack.join(', '));
    field('Роль в команде', profile.teamRole);
    field(
      'Продолжительность опыта',
      profile.experienceMonths === null ? 'Не указана' : `${profile.experienceMonths} мес.`,
    );
    field('Опыт', profile.experience);
    field('Самооценка навыков взаимодействия', profile.softSkills.join(', '));
    document
      .moveDown()
      .fontSize(9)
      .fillColor('#6B5278')
      .text(
        'Данные и результаты этой версии симулированы для демонстрации интерфейса. PDF экспортирован из текущего профиля. Он не является официальным документом Федерации.',
      );
    document.end();
  });
}
