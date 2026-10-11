import { createFileRoute } from '@tanstack/react-router';
import { interviewQuery } from '@/entities/interview';
import { scopeOf, sessionQueryOptions } from '@/entities/session';
import { CandidateInterviewPage } from '@/pages/candidate-interview';

export const Route = createFileRoute('/candidate/interviews/$interviewId')({
  loader: async ({ context: { queryClient }, params }) => {
    const session = await queryClient.ensureQueryData(sessionQueryOptions());
    await queryClient.ensureQueryData(interviewQuery(scopeOf(session), params.interviewId));
  },
  component: InterviewRoute,
});

function InterviewRoute() {
  const { interviewId } = Route.useParams();
  return <CandidateInterviewPage interviewId={interviewId} />;
}
