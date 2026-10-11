# API operation index

Generated from [`openapi.json`](openapi.json) by `codegen/generate.ts`. Do not edit the table independently. Contract version: 0.1.0. The exact bodies, error schemas, authorization descriptions and parameters remain in OpenAPI.

| Operation | Method | Path | Success | Scope |
| --- | --- | --- | --- | --- |
| `getHealth` | GET | `/health` | 200 | Health |
| `getMeta` | GET | `/api/v1/meta` | 200 | Application |
| `createEmailChallenge` | POST | `/api/v1/auth/email/challenges` | 201 | Application |
| `verifyEmail` | POST | `/api/v1/auth/email/verify` | 200 | Application |
| `logout` | DELETE | `/api/v1/auth/session` | 204 | Application |
| `getSession` | GET | `/api/v1/session` | 200 | Application |
| `getMyProfile` | GET | `/api/v1/me/profile` | 200 | Application |
| `updateMyProfile` | PATCH | `/api/v1/me/profile` | 200 | Application |
| `importMyProfile` | POST | `/api/v1/me/profile/imports` | 202 | Application |
| `confirmProfileImport` | POST | `/api/v1/me/profile/imports/{jobId}/confirm` | 200 | Application |
| `refreshMyEstimate` | POST | `/api/v1/me/profile/estimate` | 200 | Application |
| `exportMyProfile` | GET | `/api/v1/me/profile/export` | 200 | Application |
| `authorizeGitHub` | GET | `/api/v1/integrations/github/authorize` | 200 | Application |
| `getMySources` | GET | `/api/v1/me/sources` | 200 | Application |
| `connectMySource` | POST | `/api/v1/me/sources/{provider}/connect` | 200 | Application |
| `syncMySource` | POST | `/api/v1/me/sources/{sourceId}/sync` | 202 | Application |
| `getMySkills` | GET | `/api/v1/me/skills` | 200 | Application |
| `getMyGraph` | GET | `/api/v1/me/graph` | 200 | Application |
| `getMyGapPlan` | GET | `/api/v1/me/gap-plan` | 200 | Application |
| `listMyAssessments` | GET | `/api/v1/me/assessments` | 200 | Application |
| `startAssessment` | POST | `/api/v1/me/assessments` | 201 | Application |
| `getAssessment` | GET | `/api/v1/me/assessments/{assessmentId}` | 200 | Application |
| `saveAssessmentAnswer` | PUT | `/api/v1/me/assessments/{assessmentId}/answer` | 200 | Application |
| `runAssessmentChecks` | POST | `/api/v1/me/assessments/{assessmentId}/checks` | 202 | Application |
| `askAssessmentAssistant` | POST | `/api/v1/me/assessments/{assessmentId}/assistant` | 200 | Application |
| `submitAssessment` | POST | `/api/v1/me/assessments/{assessmentId}/submit` | 202 | Application |
| `listMyInterviews` | GET | `/api/v1/me/interviews` | 200 | Application |
| `startInterview` | POST | `/api/v1/me/interviews` | 201 | Application |
| `getInterview` | GET | `/api/v1/me/interviews/{interviewId}` | 200 | Application |
| `answerInterview` | POST | `/api/v1/me/interviews/{interviewId}/answers` | 200 | Application |
| `synthesizeInterviewSpeech` | POST | `/api/v1/me/interviews/{interviewId}/speech` | 200 | Application |
| `getJob` | GET | `/api/v1/jobs/{jobId}` | 200 | Application |
| `getMyTasks` | GET | `/api/v1/me/tasks` | 200 | Application |
| `respondToTask` | POST | `/api/v1/me/tasks/{taskId}/responses` | 201 | Application |
| `listMyInvitations` | GET | `/api/v1/me/invitations` | 200 | Application |
| `getMyInvitation` | GET | `/api/v1/me/invitations/{invitationId}` | 200 | Application |
| `viewMyInvitation` | POST | `/api/v1/me/invitations/{invitationId}/view` | 200 | Application |
| `acceptMyInvitation` | POST | `/api/v1/me/invitations/{invitationId}/accept` | 200 | Application |
| `declineMyInvitation` | POST | `/api/v1/me/invitations/{invitationId}/decline` | 200 | Application |
| `searchVacancies` | GET | `/api/v1/vacancies` | 200 | Application |
| `getVacancy` | GET | `/api/v1/vacancies/{vacancyId}` | 200 | Application |
| `listMyApplications` | GET | `/api/v1/me/applications` | 200 | Application |
| `createApplication` | POST | `/api/v1/me/applications` | 201 | Application |
| `getMyApplication` | GET | `/api/v1/me/applications/{applicationId}` | 200 | Application |
| `withdrawMyApplication` | POST | `/api/v1/me/applications/{applicationId}/withdraw` | 200 | Application |
| `getCompany` | GET | `/api/v1/company` | 200 | Application |
| `updateCompany` | PATCH | `/api/v1/company` | 200 | Application |
| `listNeeds` | GET | `/api/v1/needs` | 200 | Application |
| `createNeed` | POST | `/api/v1/needs` | 201 | Application |
| `getNeed` | GET | `/api/v1/needs/{needId}` | 200 | Application |
| `updateNeed` | PATCH | `/api/v1/needs/{needId}` | 200 | Application |
| `deleteNeed` | DELETE | `/api/v1/needs/{needId}` | 204 | Application |
| `publishNeed` | POST | `/api/v1/needs/{needId}/publish` | 200 | Application |
| `unpublishNeed` | POST | `/api/v1/needs/{needId}/unpublish` | 200 | Application |
| `getNeedGraph` | GET | `/api/v1/needs/{needId}/graph` | 200 | Application |
| `searchCandidates` | GET | `/api/v1/candidates` | 200 | Application |
| `getCandidate` | GET | `/api/v1/candidates/{candidateId}` | 200 | Application |
| `getCandidateGraph` | GET | `/api/v1/candidates/{candidateId}/graph` | 200 | Application |
| `getCandidateGapPlan` | GET | `/api/v1/candidates/{candidateId}/gap-plan` | 200 | Application |
| `getCandidateDisclosure` | GET | `/api/v1/candidates/{candidateId}/disclosure` | 200 | Application |
| `listShortlists` | GET | `/api/v1/shortlists` | 200 | Application |
| `createShortlist` | POST | `/api/v1/shortlists` | 201 | Application |
| `getShortlist` | GET | `/api/v1/shortlists/{shortlistId}` | 200 | Application |
| `updateShortlist` | PATCH | `/api/v1/shortlists/{shortlistId}` | 200 | Application |
| `deleteShortlist` | DELETE | `/api/v1/shortlists/{shortlistId}` | 204 | Application |
| `addShortlistMember` | PUT | `/api/v1/shortlists/{shortlistId}/members/{candidateId}` | 200 | Application |
| `removeShortlistMember` | DELETE | `/api/v1/shortlists/{shortlistId}/members/{candidateId}` | 200 | Application |
| `listInvitations` | GET | `/api/v1/invitations` | 200 | Application |
| `createInvitation` | POST | `/api/v1/invitations` | 201 | Application |
| `getInvitation` | GET | `/api/v1/invitations/{invitationId}` | 200 | Application |
| `withdrawInvitation` | POST | `/api/v1/invitations/{invitationId}/withdraw` | 200 | Application |
| `listTasks` | GET | `/api/v1/tasks` | 200 | Application |
| `createTask` | POST | `/api/v1/tasks` | 201 | Application |
| `archiveTask` | POST | `/api/v1/tasks/{taskId}/archive` | 200 | Application |
| `listTaskResponses` | GET | `/api/v1/task-responses` | 200 | Application |
| `reviewTaskResponse` | POST | `/api/v1/task-responses/{responseId}/review` | 200 | Application |
| `listApplications` | GET | `/api/v1/applications` | 200 | Application |
| `getHiringFlowAnalytics` | GET | `/api/v1/analytics/hiring-flow` | 200 | Application |
| `getApplication` | GET | `/api/v1/applications/{applicationId}` | 200 | Application |
| `viewApplication` | POST | `/api/v1/applications/{applicationId}/view` | 200 | Application |
| `acceptApplication` | POST | `/api/v1/applications/{applicationId}/accept` | 200 | Application |
| `declineApplication` | POST | `/api/v1/applications/{applicationId}/decline` | 200 | Application |
| `mockSelectSession` | POST | `/__mock__/session` | 200 | Development only |
| `mockReset` | POST | `/__mock__/reset` | 200 | Development only |
| `mockEmailCodes` | GET | `/__mock__/email-codes` | 200 | Development only |
| `mockAdvanceClock` | POST | `/__mock__/clock` | 200 | Development only |

Total documented operations: 86. Development controls are not production application endpoints.
