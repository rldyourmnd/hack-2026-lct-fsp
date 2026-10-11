import { useQuery } from '@tanstack/react-query';
import { interviewQuery } from '@/entities/interview';
import { useScope } from '@/entities/session';
import { useDocumentTitle } from '@/shared/lib';
import { ButtonLink, ErrorState, LoadingState } from '@/shared/ui';
import { InterviewWorkspace } from '@/widgets/graph-interview';

export function CandidateInterviewPage({ interviewId }: { interviewId: string }) {
  useDocumentTitle('Собеседование');
  const scope = useScope();
  const interview = useQuery(interviewQuery(scope, interviewId));
  return (
    <>
      <div style={{ marginBottom: '1rem' }}>
        <ButtonLink to="/candidate/assessments" variant="quiet" icon="arrowLeft">
          К проверкам
        </ButtonLink>
      </div>
      {interview.isPending && <LoadingState label="Открываем собеседование…" />}
      {interview.isError && <ErrorState error={interview.error} onRetry={() => interview.refetch()} />}
      {interview.data && <InterviewWorkspace interview={interview.data} />}
    </>
  );
}
