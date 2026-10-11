import { useQuery, useSuspenseQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useMeta } from '@/entities/meta';
import { blockerLabel, type Profile, profileQuery } from '@/entities/profile';
import { CategoryBadge, estimateConfidenceLabel } from '@/entities/qualification';
import { useScope } from '@/entities/session';
import { mySkillsQuery } from '@/entities/skill';
import { ConsentControls } from '@/features/consents';
import { ProfileForm } from '@/features/edit-profile';
import { ImportProfileDialog } from '@/features/profile-pdf';
import { formatDate, gradeLabel, specializationLabel, useDocumentTitle } from '@/shared/lib';
import { Avatar, Button, Card, Icon, PageHeader, TagList } from '@/shared/ui';
import styles from './profile-page.module.css';

function EmployerPreview({ profile }: { profile: Profile }) {
  const scope = useScope();
  const skills = useQuery(mySkillsQuery(scope));
  const visibleSkills = skills.data?.items
    .filter((skill) => skill.status !== 'insufficient')
    .map((skill) => skill.label);
  return (
    <Card title="Для работодателей" subtitle="Так компании видят профиль до вашего согласия">
      <div className="stack">
        <div className={styles.preview}>
          <Avatar kind="anonymous" size="md" />
          <div className={styles.previewBody}>
            <strong>{profile.alias}</strong>
            <span className="row small">
              <CategoryBadge {...profile.category} />
            </span>
            {visibleSkills && visibleSkills.length > 0 && <TagList label="Навыки" items={visibleSkills} />}
          </div>
        </div>
        <p className="small muted" style={{ display: 'flex', gap: '0.5rem' }}>
          <Icon name="lock" size={18} />
          Контакты откроются, когда вы примете приглашение — и только компании из этого приглашения.
        </p>
        {!profile.onboarding.visibleToEmployers && (
          <p className="small">
            Сейчас профиль не показывается в поиске:{' '}
            {profile.onboarding.blockers.map((item) => blockerLabel[item]).join(', ')}.
          </p>
        )}
      </div>
    </Card>
  );
}

function CurrentStatus({ profile }: { profile: Profile }) {
  return (
    <Card title="Текущий статус">
      <div className={styles.status}>
        {profile.verifiedGrade && profile.verifiedSpecialization ? (
          <div className={`${styles.statusItem} ${styles.verified}`}>
            <Icon name="checkCircle" />
            <div>
              <strong>
                {specializationLabel[profile.verifiedSpecialization]} · {gradeLabel[profile.verifiedGrade]} —
                подтверждён
              </strong>
              <p>
                На основе проведённой проверки
                {profile.gradeConfirmedAt ? `, ${formatDate(profile.gradeConfirmedAt)}` : ''}
              </p>
            </div>
          </div>
        ) : profile.category.source === 'estimate' && profile.category.specialization && profile.category.grade ? (
          <div className={`${styles.statusItem} ${styles.target}`}>
            <Icon name="chart" />
            <div>
              <strong>
                {specializationLabel[profile.category.specialization]} · {gradeLabel[profile.category.grade]} — оценка
                по открытым данным
              </strong>
              <p>
                {profile.category.confidence
                  ? `${estimateConfidenceLabel[profile.category.confidence][0]?.toUpperCase()}${estimateConfidenceLabel[profile.category.confidence].slice(1)}. `
                  : ''}
                Категория видна работодателям; тест знаний её подтверждает.
              </p>
            </div>
          </div>
        ) : (
          <div className={`${styles.statusItem} ${styles.pending}`}>
            <Icon name="minusCircle" />
            <div>
              <strong>Категории пока нет</strong>
              <p>Она появится после автоматической оценки по GitHub или ФСП либо после теста знаний.</p>
            </div>
          </div>
        )}
        {profile.selectedGrade !== profile.category.grade && (
          <div className={`${styles.statusItem} ${styles.target}`}>
            <Icon name="target" />
            <div>
              <strong>{gradeLabel[profile.selectedGrade]} — цель следующего теста</strong>
              <p>Подтверждённый грейд меняется только по результату теста.</p>
            </div>
          </div>
        )}
      </div>
    </Card>
  );
}

export function CandidateProfilePage() {
  useDocumentTitle('Профиль и цели');
  const meta = useMeta();
  const scope = useScope();
  const { data: profile } = useSuspenseQuery(profileQuery(scope));
  const [importOpen, setImportOpen] = useState(false);
  return (
    <>
      <PageHeader
        title="Профиль и цели"
        subtitle="Контактные данные и направление следующей проверки"
        actions={
          meta.capabilities.profilePdfImport && (
            <Button icon="upload" onClick={() => setImportOpen(true)} disabled={!profile.consents.processing}>
              Импортировать PDF
            </Button>
          )
        }
      />
      <div className="split">
        <Card>
          <ProfileForm profile={profile} submitLabel="Сохранить профиль" />
        </Card>
        <div className="stack-lg">
          <EmployerPreview profile={profile} />
          <Card title="Настройки приватности">
            <ConsentControls profile={profile} />
          </Card>
          <CurrentStatus profile={profile} />
        </div>
      </div>
      <ImportProfileDialog open={importOpen} onClose={() => setImportOpen(false)} />
    </>
  );
}
