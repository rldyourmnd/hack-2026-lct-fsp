import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readdirSync, readFileSync } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import { request as rawHttpRequest } from 'node:http';
import test, { after, type TestContext } from 'node:test';
import { fileURLToPath } from 'node:url';
import bank from '../../backend/crates/domain/data/knowledge-v1.json';
import additions from '../../backend/crates/domain/data/knowledge-v2-additions.json';
import { buildOpenApi, routes, routeTable } from '../src/contract.ts';
import { createMockServer, type MockServerOptions } from '../src/server.ts';
import { validators } from '../src/validation.ts';

// Response bodies are untyped JSON here; every body is validated against its contract schema in request().
// biome-ignore lint/suspicious/noExplicitAny: assertions read arbitrary fields of contract-validated JSON
type Body = any;
interface Client {
  cookie: string;
}
interface RequestOptions {
  actor?: Client;
  body?: unknown;
  raw?: BodyInit;
  contentType?: string;
  key?: string | null;
  status?: number;
  headers?: Record<string, string>;
}

const coverage = new Map<string, number[]>();
const operationFor = (method: string, path: string) =>
  routeTable.find(
    (route) =>
      route.method === method.toLowerCase() &&
      new RegExp(`^${route.path.replace(/\{[^}]+\}/g, '[^/]+')}$`).test(path.split('?')[0] ?? ''),
  );
async function setup(t: TestContext, options: MockServerOptions = {}) {
  const app = createMockServer({ port: 0, ...options });
  const url = await app.listen();
  t.after(() => app.close());
  const client = (cookie = ''): Client => ({ cookie });
  async function request(
    method: string,
    path: string,
    { actor = client(), body, raw, contentType, key, status = 200, headers = {} }: RequestOptions = {},
  ) {
    const operation = operationFor(method, path);
    if (operation?.idempotent && key !== null) headers['Idempotency-Key'] = key ?? randomUUID();
    if (actor.cookie) headers.Cookie = actor.cookie;
    if (body !== undefined) headers['Content-Type'] = 'application/json';
    if (contentType) headers['Content-Type'] = contentType;
    const response = await fetch(`${url}${path}`, {
      method,
      headers,
      body: body === undefined ? (raw ?? null) : JSON.stringify(body),
    });
    const cookie = response.headers.get('set-cookie');
    if (cookie) actor.cookie = cookie.split(';')[0] ?? '';
    const buffer = Buffer.from(await response.arrayBuffer());
    let data: Body = null;
    if (response.headers.get('content-type')?.includes('application/json')) data = JSON.parse(buffer.toString());
    assert.equal(response.status, status, `${method} ${path}: ${JSON.stringify(data)}`);
    if (operation) {
      coverage.set(operation.operationId, [...new Set([...(coverage.get(operation.operationId) ?? []), status])]);
      if (status >= 400) assert.ok(validators.Error(data), JSON.stringify(validators.Error.errors));
      else if (operation.response)
        assert.ok(validators[operation.response](data), JSON.stringify(validators[operation.response].errors));
    }
    return { body: data, response, buffer, actor };
  }
  async function login(actorName: string) {
    const actor = client();
    await request('POST', '/__mock__/session', { actor, body: { actor: actorName } });
    return actor;
  }
  async function job(id: string, actor: Client): Promise<Body> {
    let value: Body;
    for (let attempt = 0; attempt < 30; attempt += 1) {
      value = (await request('GET', `/api/v1/jobs/${id}`, { actor })).body;
      if (['succeeded', 'failed'].includes(value.status)) return value;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert.fail(`Job ${id} did not finish: ${JSON.stringify(value)}`);
  }
  return { app, url, request, login, job, client };
}
const dataDirectory = new URL('../../backend/crates/domain/data/', import.meta.url);
// The topic banks (knowledge-v3-*.json) serve every specialization tested on their topics.
const topicQuestions = readdirSync(dataDirectory)
  .filter((name) => name.startsWith('knowledge-v3-'))
  .flatMap(
    (name) =>
      JSON.parse(readFileSync(new URL(name, dataDirectory), 'utf8')).questions as {
        id: string;
        correctOptionId: string;
      }[],
  );
const answerKey = new Map(
  [...bank.questions, ...additions.questions, ...topicQuestions].map((question) => [
    question.id,
    question.correctOptionId,
  ]),
);
const assessmentAnswerFor = (task: Body) => ({
  code: '',
  explanation: '',
  externalAiUsed: true,
  responses: task.questions.map((question: Body) => ({
    questionId: question.id,
    optionId: answerKey.get(question.id),
  })),
});
const forbiddenKeys = new Set([
  'name',
  'email',
  'initials',
  'avatarUrl',
  'accountLabel',
  'handle',
  'githubHandle',
  'url',
  'reference',
  'sha',
  'sourceLinks',
]);
function assertAnonymous(value: unknown) {
  if (!value || typeof value !== 'object') return;
  for (const [key, child] of Object.entries(value)) {
    assert.ok(!forbiddenKeys.has(key), `Private key leaked: ${key}`);
    assertAnonymous(child);
  }
  const serialized = JSON.stringify(value);
  for (const privateText of ['Алексей', 'Морозов', 'alexey@', 'PR #17', 'order-api', 'https://'])
    assert.ok(!serialized.includes(privateText), `Private text leaked: ${privateText}`);
}
function assertGraph(graph: Body) {
  const ids = new Set(graph.nodes.map((item: Body) => item.id));
  assert.equal(ids.size, graph.nodes.length);
  for (const edge of graph.edges) {
    assert.ok(ids.has(edge.from));
    assert.ok(ids.has(edge.to));
  }
}

test('canonical contract matches generator and all declared routes have explicit operation IDs', async () => {
  const file: Body = JSON.parse(await readFile(new URL('../../contracts/openapi.json', import.meta.url), 'utf8'));
  assert.deepEqual(file, buildOpenApi());
  assert.equal(new Set(routes.map((route) => route.operationId)).size, routes.length);
  assert.equal(file.openapi, '3.1.0');
  for (const route of routeTable.filter((entry) => entry.dev))
    assert.equal(file.paths[route.path]?.[route.method]?.['x-mock-only'], true);
});

test('the Rust GitHub report and employer projection match the contract schemas', async () => {
  for (const [file, schema] of [
    ['github-report.json', 'GitHubReport'],
    ['github-signal.json', 'GitHubSignal'],
  ] as const) {
    const value = JSON.parse(await readFile(new URL(`../../contracts/fixtures/${file}`, import.meta.url), 'utf8'));
    const validate = validators[schema];
    assert.ok(validate(value), `${file}: ${JSON.stringify(validate.errors)}`);
  }
});

test('error-codes.json lists every code, status and literal message the mock raises', async () => {
  const catalog: Body = JSON.parse(
    await readFile(new URL('../../contracts/error-codes.json', import.meta.url), 'utf8'),
  );
  const documented = new Map<string, Body>(catalog.http.map((entry: Body) => [entry.code, entry]));
  for (const file of ['domain.ts', 'server.ts']) {
    const source = await readFile(new URL(`../src/${file}`, import.meta.url), 'utf8');
    for (const [, status, code = '', message] of source.matchAll(
      /(?:fail|new ApiError)\(\s*(\d{3}),\s*'([a-z_]+)'(?:,\s*'([^']*)')?/g,
    )) {
      const entry = documented.get(code);
      assert.ok(entry, `${code} is missing from error-codes.json`);
      assert.ok(entry.statuses.includes(Number(status)), `${code} lacks status ${status}`);
      if (message !== undefined) assert.ok(entry.messages.includes(message), `${code} lacks «${message}»`);
    }
  }
});

test('error-codes.json catalogues every code, status and literal message the Rust backend raises', async () => {
  const catalog: Body = JSON.parse(
    await readFile(new URL('../../contracts/error-codes.json', import.meta.url), 'utf8'),
  );
  const documented = new Map<string, Body>(catalog.http.map((entry: Body) => [entry.code, entry]));
  const jobCodes = new Set<string>(catalog.job.map((entry: Body) => entry.code));
  const statusByName: Record<string, number> = {
    BAD_REQUEST: 400,
    UNAUTHORIZED: 401,
    FORBIDDEN: 403,
    NOT_FOUND: 404,
    CONFLICT: 409,
    PAYLOAD_TOO_LARGE: 413,
    UNSUPPORTED_MEDIA_TYPE: 415,
    UNPROCESSABLE_ENTITY: 422,
    TOO_MANY_REQUESTS: 429,
    INTERNAL_SERVER_ERROR: 500,
    BAD_GATEWAY: 502,
    SERVICE_UNAVAILABLE: 503,
  };
  const assertCode = (status: number | undefined, code: string, message: string | undefined) => {
    const entry = documented.get(code);
    assert.ok(entry || jobCodes.has(code), `${code} is missing from error-codes.json`);
    if (!entry) return;
    if (status !== undefined) assert.ok(entry.statuses.includes(status), `${code} lacks status ${status}`);
    if (message !== undefined && entry.messages.length)
      assert.ok(entry.messages.includes(message), `${code} lacks «${message}»`);
  };
  const rustDir = fileURLToPath(new URL('../../backend/crates/', import.meta.url));
  const files: string[] = [];
  const walk = (dir: string) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = `${dir}/${entry.name}`;
      if (entry.isDirectory()) walk(path);
      else if (entry.name.endsWith('.rs')) files.push(path);
    }
  };
  walk(rustDir);
  for (const file of files) {
    const source = await readFile(file, 'utf8');
    const triples: { status: number | undefined; code: string; message: string | undefined }[] = [
      ...source.matchAll(/(?:ApplicationError::new|Self::new)\(\s*(\d{3})\s*,\s*"([a-z_]+)"(?:\s*,\s*"([^"]*)")?/g),
    ].map((m) => ({ status: Number(m[1]), code: m[2] ?? '', message: m[3] }));
    for (const m of source.matchAll(/ApiError::new\(\s*StatusCode::([A-Z_]+)\s*,\s*"([a-z_]+)"(?:\s*,\s*"([^"]*)")?/g))
      triples.push({ status: statusByName[m[1] ?? ''] ?? -1, code: m[2] ?? '', message: m[3] });
    for (const m of source.matchAll(/ApiError::forbidden\(\s*"([a-z_]+)"\s*,\s*"([^"]*)"/g))
      triples.push({ status: 403, code: m[1] ?? '', message: m[2] });
    for (const m of source.matchAll(/ApiError::not_found\(\s*"([^"]*)"/g))
      triples.push({ status: 404, code: 'not_found', message: m[1] });
    for (const m of source.matchAll(/validation\(\s*"([^"]+)"\s*\)/g))
      triples.push({ status: 400, code: 'validation_error', message: m[1] });
    for (const m of source.matchAll(/\bcode:\s*"([a-z_]+)"/g))
      triples.push({ status: undefined, code: m[1] ?? '', message: undefined });
    for (const { status, code, message } of triples) {
      try {
        assertCode(status, code, message);
      } catch (error) {
        throw new assert.AssertionError({
          message: `${file}: ${error instanceof Error ? error.message : error}`,
        });
      }
    }
  }
});

test('health, request validation, session, role, origin and host boundaries', async (t) => {
  const { request, login, url } = await setup(t);
  await request('GET', '/health');
  const meta = (await request('GET', '/api/v1/meta')).body;
  assert.equal(meta.mode, 'mock');
  assert.equal(meta.capabilities.chat, false);
  assert.equal(meta.capabilities.realCodeExecution, false);
  await request('GET', '/api/v1/session', { status: 401 });
  await request('GET', '/api/v1/not-real', { status: 404 });
  const candidate = await login('candidate024');
  const employer = await login('employer-a');
  await request('GET', '/api/v1/session', { actor: candidate });
  await request('GET', '/api/v1/candidates', { actor: candidate, status: 403 });
  await request('GET', '/api/v1/me/profile', { actor: employer, status: 403 });
  await request('PATCH', '/api/v1/me/profile', { actor: candidate, body: { verifiedGrade: 'senior' }, status: 400 });
  await request('PATCH', '/api/v1/me/profile', {
    actor: candidate,
    raw: '{bad',
    contentType: 'application/json',
    status: 400,
  });
  await request('PATCH', '/api/v1/me/profile', { actor: candidate, raw: '{}', contentType: 'text/plain', status: 415 });
  await request('GET', '/api/v1/candidates?page=-1', { actor: employer, status: 400 });
  await request('GET', '/api/v1/candidates?grade=middle&grade=junior', { actor: employer, status: 400 });
  await request('GET', '/api/v1/candidates?companyId=company-b', { actor: employer, status: 400 });
  await request('POST', '/__mock__/session', {
    body: { actor: 'candidate024' },
    headers: { Origin: 'https://untrusted.example' },
    status: 403,
  });
  await new Promise((resolve, reject) => {
    const raw = rawHttpRequest(`${url}/health`, { headers: { Host: 'untrusted.example' } }, (response) => {
      try {
        assert.equal(response.statusCode, 403);
        response.resume();
        response.on('end', resolve);
      } catch (error) {
        reject(error);
      }
    });
    raw.on('error', reject);
    raw.end();
  });
  await request('DELETE', '/api/v1/auth/session', { actor: candidate, status: 204 });
  await request('GET', '/api/v1/session', { actor: candidate, status: 401 });
});

test('deployment configuration admits only the configured origin and removes development routes', async (t) => {
  const { request } = await setup(t, { allowedOrigins: ['https://demo.example'], devRoutes: false });
  await request('POST', '/__mock__/session', { body: { actor: 'candidate024' }, status: 404 });
  await request('GET', '/__mock__/email-codes', { status: 404 });
  await request('POST', '/__mock__/reset', { body: { scenario: 'baseline' }, status: 404 });
  await request('POST', '/api/v1/auth/email/challenges', {
    body: { email: 'alexey@example.com', role: 'candidate' },
    headers: { Origin: 'https://demo.example' },
    status: 201,
  });
  await request('POST', '/api/v1/auth/email/challenges', {
    body: { email: 'alexey@example.com', role: 'candidate' },
    headers: { Origin: 'http://localhost:5173' },
    status: 403,
  });
});

test('verification attempt budget spans reissued live challenges', async (t) => {
  const { request } = await setup(t);
  const body = { email: 'locked@example.com', role: 'candidate' };
  const first = (await request('POST', '/api/v1/auth/email/challenges', { body, status: 201 })).body;
  const second = (await request('POST', '/api/v1/auth/email/challenges', { body, status: 201 })).body;
  for (let attempt = 0; attempt < 5; attempt += 1) {
    await request('POST', '/api/v1/auth/email/verify', {
      body: { challengeId: (attempt < 3 ? first : second).challengeId, code: '000000' },
      status: 400,
    });
  }
  for (const challenge of [first, second]) {
    const result = await request('POST', '/api/v1/auth/email/verify', {
      body: { challengeId: challenge.challengeId, code: '123456' },
      status: 429,
    });
    assert.equal(result.body.code, 'challenge_locked');
  }
  await request('POST', '/api/v1/auth/email/challenges', { body, status: 429 });
});

test('email verification creates role-bound candidate and employer accounts; expiry is authoritative', async (t) => {
  const { request, client } = await setup(t);
  let challenge = (
    await request('POST', '/api/v1/auth/email/challenges', {
      body: { email: 'new-person@example.com', role: 'candidate' },
      status: 201,
    })
  ).body;
  const mailbox = (await request('GET', '/__mock__/email-codes')).body;
  assert.equal(mailbox.items[0].code, '123456');
  await request('POST', '/api/v1/auth/email/verify', {
    body: { challengeId: challenge.challengeId, code: '000000' },
    status: 400,
  });
  const candidate = client();
  await request('POST', '/api/v1/auth/email/verify', {
    actor: candidate,
    body: { challengeId: challenge.challengeId, code: '123456' },
  });
  const profile = (await request('GET', '/api/v1/me/profile', { actor: candidate })).body;
  assert.equal(profile.emailVerified, true);
  assert.equal(profile.verifiedGrade, null);
  assert.equal(profile.category.eligible, false);
  await request('POST', '/api/v1/auth/email/challenges', {
    body: { email: 'new-person@example.com', role: 'employer' },
    status: 409,
  });
  await request('POST', '/api/v1/auth/email/verify', {
    body: { challengeId: challenge.challengeId, code: '123456' },
    status: 409,
  });
  challenge = (
    await request('POST', '/api/v1/auth/email/challenges', {
      body: { email: 'new-hr@example.com', role: 'employer' },
      status: 201,
    })
  ).body;
  const employer = client();
  await request('POST', '/api/v1/auth/email/verify', {
    actor: employer,
    body: { challengeId: challenge.challengeId, code: '123456' },
  });
  let company = (await request('GET', '/api/v1/company', { actor: employer })).body;
  assert.equal(company.readyToInvite, false);
  await request('POST', '/api/v1/invitations', {
    actor: employer,
    body: { candidateId: 'candidate024', needId: 'need-001', message: 'Приглашение' },
    status: 422,
  });
  company = (
    await request('PATCH', '/api/v1/company', {
      actor: employer,
      body: { displayName: 'Новая компания', businessArea: 'Разработка ПО', description: 'Разрабатываем веб-сервисы.' },
    })
  ).body;
  assert.equal(company.readyToInvite, true);
  assert.notEqual(company.id, 'company-a');
  await request('GET', '/api/v1/needs/need-001', { actor: employer, status: 404 });
  challenge = (
    await request('POST', '/api/v1/auth/email/challenges', {
      body: { email: 'expire@example.com', role: 'candidate' },
      status: 201,
    })
  ).body;
  await request('POST', '/__mock__/clock', { body: { advanceSeconds: 901 } });
  await request('POST', '/api/v1/auth/email/verify', {
    body: { challengeId: challenge.challengeId, code: '123456' },
    status: 400,
  });
});

test('profile category is server-owned and publication consent does not replace qualification', async (t) => {
  const { request, login } = await setup(t);
  const candidate = await login('candidate024');
  const employer = await login('employer-a');
  let profile = (
    await request('PATCH', '/api/v1/me/profile', {
      actor: candidate,
      body: { selectedGrade: 'senior', experienceMonths: null },
    })
  ).body;
  assert.equal(profile.selectedGrade, 'senior');
  assert.equal(profile.verifiedGrade, 'junior');
  assert.equal(profile.category.grade, 'junior');
  profile = (
    await request('PATCH', '/api/v1/me/profile', {
      actor: candidate,
      body: { consents: { processing: true, publication: false } },
    })
  ).body;
  assert.equal(profile.onboarding.visibleToEmployers, false);
  await request('GET', '/api/v1/candidates/candidate024', { actor: employer, status: 404 });
  const fresh = await login('candidate-new');
  await request('PATCH', '/api/v1/me/profile', {
    actor: fresh,
    body: {
      name: 'Новый участник',
      industry: 'it',
      specialization: 'backend',
      selectedGrade: 'middle',
      stack: ['rust'],
      teamRole: 'Разработчик',
      experienceMonths: 0,
      experience: 'Учебные проекты',
      softSkills: [],
      consents: { processing: true, publication: true },
    },
  });
  await request('POST', '/api/v1/me/assessments', {
    actor: fresh,
    body: { specialization: 'backend', grade: 'middle' },
    status: 409,
  });
  await request('GET', '/api/v1/candidates/candidate-new', { actor: employer, status: 404 });
});

test('source enrichment is automatic, asynchronous, private and never infers grades', async (t) => {
  const { request, login, job } = await setup(t);
  const candidate = await login('candidate024');
  const other = await login('candidate-new');
  const employer = await login('employer-a');
  const authorization = await request('GET', '/api/v1/integrations/github/authorize', { actor: candidate });
  assert.match(authorization.body.authorizationUrl, /^https:\/\/github\.com\/login\/oauth\/authorize/);
  const sources = (await request('GET', '/api/v1/me/sources', { actor: candidate })).body.items;
  const github = sources.find((source: Body) => source.provider === 'github');
  assert.deepEqual(github.counts, { found: 12, processed: 8, queued: 3, unavailable: 1 });
  assert.ok(github.activeJobId);
  // Daily time is only an explicitly labelled estimate inside the GitHub report.
  for (const key of Object.keys(github)) assert.ok(!/hours/i.test(key));
  assert.ok(github.github.report.rhythm.medianMinutesPerActiveDay > 0);
  await request('GET', `/api/v1/jobs/${github.activeJobId}`, { actor: other, status: 404 });
  await request('GET', `/api/v1/jobs/${github.activeJobId}`, { actor: employer, status: 404 });
  const completed = await job(github.activeJobId, candidate);
  assert.equal(completed.status, 'succeeded');
  assert.deepEqual(completed.result.source.counts, { found: 12, processed: 11, queued: 0, unavailable: 1 });
  const refreshed = (await request('POST', `/api/v1/me/sources/${github.id}/sync`, { actor: candidate, status: 202 }))
    .body;
  await job(refreshed.id, candidate);
  const connected = (
    await request('POST', '/api/v1/me/sources/fsp/connect', { actor: candidate, body: { account: 'FSP-DEMO-024' } })
  ).body;
  assert.equal(connected.synthetic, true);
  assert.ok(connected.activeJobId);
  await job(connected.activeJobId, candidate);
  await request('POST', '/api/v1/me/sources/not-provider/connect', {
    actor: candidate,
    body: { account: 'x' },
    status: 404,
  });
  const graph = (await request('GET', '/api/v1/me/graph', { actor: candidate })).body;
  assertGraph(graph);
  // Category → concepts → technologies with levels; GitHub and FSP as sources.
  const kinds = (kind: string) => graph.nodes.filter((node: Body) => node.kind === kind);
  assert.equal(kinds('role').length, 1);
  assert.ok(kinds('concept').length > 0);
  assert.ok(kinds('skill').every((node: Body) => node.detail));
  const sourceIds = kinds('source').map((node: Body) => node.id);
  assert.ok(sourceIds.includes('source-github') && sourceIds.includes('source-fsp'), sourceIds.join());
  for (const skill of kinds('skill'))
    assert.ok(graph.edges.some((edge: Body) => edge.to === skill.id && edge.from.startsWith('concept-')));
  const skills = (await request('GET', '/api/v1/me/skills', { actor: candidate })).body.items;
  assert.ok(skills.some((skill: Body) => skill.evidence.some((evidence: Body) => evidence.reference === 'PR #17')));
  assert.equal((await request('GET', '/api/v1/me/profile', { actor: candidate })).body.verifiedGrade, 'junior');
});

test('graph interview follows prerequisites and rejects a stale answer', async (t) => {
  const { request, login } = await setup(t);
  const candidate = await login('candidate024');
  const empty = (await request('GET', '/api/v1/me/interviews', { actor: candidate })).body;
  assert.deepEqual(empty.items, []);

  const started = (
    await request('POST', '/api/v1/me/interviews', {
      actor: candidate,
      body: { blueprintId: 'backend-middle-v1', maxTurns: 10 },
      status: 201,
    })
  ).body;
  assert.equal(started.currentQuestion.nodeId, 'backend.idempotency');
  await request('GET', `/api/v1/me/interviews/${started.id}`, { actor: candidate });
  await request('POST', `/api/v1/me/interviews/${started.id}/speech`, {
    actor: candidate,
    body: { questionId: started.currentQuestion.id },
    status: 503,
  });
  await request('POST', `/api/v1/me/interviews/${started.id}/transcription`, {
    actor: candidate,
    body: { questionId: started.currentQuestion.id, audioBase64: 'YXVkaW8=', mimeType: 'audio/webm' },
    status: 503,
  });

  const answered = (
    await request('POST', `/api/v1/me/interviews/${started.id}/answers`, {
      actor: candidate,
      body: {
        questionId: started.currentQuestion.id,
        answer: 'Сначала уточню ожидаемое поведение повторного запроса.',
        responseMode: 'voice',
        revision: started.revision,
      },
    })
  ).body;
  assert.equal(answered.currentQuestion.nodeId, 'backend.unique-operation');
  assert.equal(answered.evidence[0].state, 'observed');
  assert.equal(answered.evidence[0].responseMode, 'voice');
  assert.equal(answered.evidence[0].signalStrength, 2);

  await request('POST', `/api/v1/me/interviews/${started.id}/answers`, {
    actor: candidate,
    body: {
      questionId: started.currentQuestion.id,
      answer: 'Устаревший ответ',
      responseMode: 'text',
      revision: started.revision,
    },
    status: 409,
  });
});

test('assessment workflow uses jobs, preserves unrelated evidence, and applies a successful grade change', async (t) => {
  const { request, login, job } = await setup(t);
  const candidate = await login('candidate024');
  const before = (await request('GET', '/api/v1/me/skills', { actor: candidate })).body.items;
  const key = 'assessment-start-key';
  const item = (
    await request('POST', '/api/v1/me/assessments', {
      actor: candidate,
      body: { specialization: 'backend', grade: 'middle' },
      key,
      status: 201,
    })
  ).body;
  const replay = await request('POST', '/api/v1/me/assessments', {
    actor: candidate,
    body: { grade: 'middle', specialization: 'backend' },
    key,
    status: 201,
  });
  assert.equal(replay.body.id, item.id);
  assert.equal(replay.response.headers.get('x-idempotent-replay'), 'true');
  await request('POST', '/api/v1/me/assessments', {
    actor: candidate,
    body: { specialization: 'backend', grade: 'senior' },
    key,
    status: 409,
  });
  await request('GET', `/api/v1/me/assessments/${item.id}`, { actor: candidate });
  await request('POST', `/api/v1/me/assessments/${item.id}/submit`, { actor: candidate, status: 422 });
  await request('PUT', `/api/v1/me/assessments/${item.id}/answer`, {
    actor: candidate,
    body: assessmentAnswerFor(item.task),
  });
  const reply = (
    await request('POST', `/api/v1/me/assessments/${item.id}/assistant`, {
      actor: candidate,
      body: { message: 'Как обработать конкуренцию?' },
    })
  ).body;
  assert.equal(reply.simulation, true);
  assert.equal(reply.externalAiAllowed, true);
  const checksJob = (
    await request('POST', `/api/v1/me/assessments/${item.id}/checks`, { actor: candidate, status: 202 })
  ).body;
  assert.equal(checksJob.status, 'queued');
  assert.equal((await job(checksJob.id, candidate)).result.checks.length, 10);
  const submitted = (
    await request('POST', `/api/v1/me/assessments/${item.id}/submit`, {
      actor: candidate,
      key: 'submit-immutable-key',
      status: 202,
    })
  ).body;
  const evaluated = await job(submitted.id, candidate);
  assert.equal(evaluated.status, 'succeeded');
  assert.equal(evaluated.result.assessment.status, 'passed');
  const result = evaluated.result.assessment.result;
  assert.equal(result.gradeChanged, true);
  assert.equal(result.nextEligibleAt, '2027-01-03T12:00:00.000Z');
  const profile = (await request('GET', '/api/v1/me/profile', { actor: candidate })).body;
  assert.equal(profile.verifiedGrade, 'middle');
  assert.equal(profile.assessmentEligibility.canStartUpgrade, false);
  assert.equal(profile.assessmentEligibility.canStartSelected, true);
  const afterSkills = (await request('GET', '/api/v1/me/skills', { actor: candidate })).body.items;
  for (const id of ['docker', 'redis'])
    assert.deepEqual(
      afterSkills.find((skill: Body) => skill.skillId === id),
      before.find((skill: Body) => skill.skillId === id),
    );
  const postgres = afterSkills.find((skill: Body) => skill.skillId === 'postgresql');
  assert.equal(postgres.status, 'confirmed');
  assert.ok(postgres.evidence.some((evidence: Body) => evidence.reference === 'PR #17'));
  await request('POST', '/api/v1/me/assessments', {
    actor: candidate,
    body: { specialization: 'backend', grade: 'senior' },
    status: 409,
  });
  await request('PUT', `/api/v1/me/assessments/${item.id}/answer`, {
    actor: candidate,
    body: assessmentAnswerFor(item.task),
    status: 409,
  });
  const history = (await request('GET', '/api/v1/me/assessments', { actor: candidate })).body.items;
  assert.equal(history.length, 2);
});

test('frontend assessment confirms TypeScript and React for a supported grade', async (t) => {
  const { request, login, job } = await setup(t);
  const candidate = await login('candidate024');
  await request('PATCH', '/api/v1/me/profile', {
    actor: candidate,
    body: {
      specialization: 'frontend',
      selectedGrade: 'middle',
      stack: ['typescript', 'react'],
    },
  });
  const item = (
    await request('POST', '/api/v1/me/assessments', {
      actor: candidate,
      body: { specialization: 'frontend', grade: 'middle' },
      status: 201,
    })
  ).body;
  assert.deepEqual([...new Set(item.task.questions.map((question: Body) => question.skillId))].sort(), [
    'react',
    'typescript',
  ]);
  await request('PUT', `/api/v1/me/assessments/${item.id}/answer`, {
    actor: candidate,
    body: assessmentAnswerFor(item.task),
  });
  const submitted = (
    await request('POST', `/api/v1/me/assessments/${item.id}/submit`, {
      actor: candidate,
      status: 202,
    })
  ).body;
  const result = await job(submitted.id, candidate);
  assert.equal(result.result.assessment.status, 'passed');
  const profile = (await request('GET', '/api/v1/me/profile', { actor: candidate })).body;
  assert.equal(profile.verifiedSpecialization, 'frontend');
  assert.equal(profile.verifiedGrade, 'middle');
  const skills = (await request('GET', '/api/v1/me/skills', { actor: candidate })).body.items;
  for (const skillId of ['typescript', 'react'])
    assert.equal(skills.find((skill: Body) => skill.skillId === skillId).status, 'confirmed');
});

test('failed and unavailable assessment scenarios preserve grades and allow retry without a penalty', async (t) => {
  for (const scenario of ['assessment_failure', 'service_failure'] as const)
    await t.test(scenario, async (subtest) => {
      const { request, login, job } = await setup(subtest, { scenario });
      const candidate = await login('candidate024');
      const item = (
        await request('POST', '/api/v1/me/assessments', {
          actor: candidate,
          body: { specialization: 'backend', grade: 'middle' },
          status: 201,
        })
      ).body;
      await request('PUT', `/api/v1/me/assessments/${item.id}/answer`, {
        actor: candidate,
        body: assessmentAnswerFor(item.task),
      });
      const submitted = (
        await request('POST', `/api/v1/me/assessments/${item.id}/submit`, { actor: candidate, status: 202 })
      ).body;
      const outcome = await job(submitted.id, candidate);
      const result = outcome.result.assessment.result;
      assert.equal(result.confirmedGrade, 'junior');
      assert.equal(result.gradeChanged, false);
      assert.equal(result.nextEligibleAt, '2026-08-30T12:00:00.000Z');
      if (scenario === 'service_failure') {
        assert.equal(outcome.status, 'failed');
        assert.equal(result.rubric.length, 0);
        assert.equal(result.checks.length, 0);
      }
      assert.equal(
        (await request('GET', '/api/v1/me/profile', { actor: candidate })).body.assessmentEligibility.canStartSelected,
        true,
      );
      await request('POST', '/api/v1/me/assessments', {
        actor: candidate,
        body: { specialization: 'backend', grade: 'middle' },
        status: 201,
      });
    });
});

test('same-grade retry is allowed; downgrade is refused; cooldown configuration is server-owned', async (t) => {
  const { request, login } = await setup(t, { scenario: 'qualified', gradeChangeCooldownDays: 7 });
  const candidate = await login('candidate024');
  let profile = (await request('GET', '/api/v1/me/profile', { actor: candidate })).body;
  assert.equal(profile.assessmentEligibility.cooldownDays, 7);
  assert.equal(profile.assessmentEligibility.nextEligibleAt, '2026-10-12T12:00:00.000Z');
  assert.equal(profile.assessmentEligibility.canStartSelected, true);
  await request('POST', '/api/v1/me/assessments', {
    actor: candidate,
    body: { specialization: 'backend', grade: 'junior' },
    status: 422,
  });
  profile = (await request('PATCH', '/api/v1/me/profile', { actor: candidate, body: { selectedGrade: 'senior' } }))
    .body;
  assert.equal(profile.assessmentEligibility.canStartSelected, false);
  await request('POST', '/__mock__/clock', { body: { advanceSeconds: 7 * 86400 } });
  await request('POST', '/api/v1/me/assessments', {
    actor: candidate,
    body: { specialization: 'backend', grade: 'senior' },
    status: 201,
  });
});

test('stale check jobs and revoked processing consent cannot write grades or outdated results', async (t) => {
  const { request, login, job } = await setup(t);
  const candidate = await login('candidate024');
  const item = (
    await request('POST', '/api/v1/me/assessments', {
      actor: candidate,
      body: { specialization: 'backend', grade: 'middle' },
      status: 201,
    })
  ).body;
  const pending = (await request('POST', `/api/v1/me/assessments/${item.id}/checks`, { actor: candidate, status: 202 }))
    .body;
  await request('PUT', `/api/v1/me/assessments/${item.id}/answer`, {
    actor: candidate,
    body: assessmentAnswerFor(item.task),
  });
  assert.equal((await job(pending.id, candidate)).error.code, 'stale_input');
  const submitted = (
    await request('POST', `/api/v1/me/assessments/${item.id}/submit`, { actor: candidate, status: 202 })
  ).body;
  await request('PATCH', '/api/v1/me/profile', {
    actor: candidate,
    body: { consents: { processing: false, publication: true } },
  });
  assert.equal((await job(submitted.id, candidate)).error.code, 'processing_consent_revoked');
  assert.equal((await request('GET', '/api/v1/me/profile', { actor: candidate })).body.verifiedGrade, 'junior');
});

test('company validation and nullable profile patches match the persisted API', async (t) => {
  const { request, login } = await setup(t);
  const employer = await login('employer-a');
  const before = (await request('GET', '/api/v1/company', { actor: employer })).body;
  for (const body of [
    { displayName: 'x'.repeat(121) },
    { businessArea: 'x'.repeat(201) },
    { description: 'x'.repeat(4001) },
    { contactEmail: 'x@' },
    { website: 'javascript:alert(1)' },
    { website: 'https://' },
    { website: `https://example.com/${'x'.repeat(2048)}` },
    { displayName: null, description: 'must not be stored' },
  ])
    await request('PATCH', '/api/v1/company', { actor: employer, body, status: 400 });
  assert.deepEqual((await request('GET', '/api/v1/company', { actor: employer })).body, before);
  await request('PATCH', '/api/v1/company', { actor: employer, body: { website: 'https://example.com' } });
  assert.equal(
    (await request('PATCH', '/api/v1/company', { actor: employer, body: { description: 'Updated' } })).body.website,
    'https://example.com',
  );
  assert.equal(
    (await request('PATCH', '/api/v1/company', { actor: employer, body: { website: null } })).body.website,
    null,
  );
  const candidate = await login('candidate024');
  await request('PATCH', '/api/v1/me/profile', { actor: candidate, body: { experienceMonths: 24 } });
  assert.equal(
    (await request('PATCH', '/api/v1/me/profile', { actor: candidate, body: { teamRole: 'Developer' } })).body
      .experienceMonths,
    24,
  );
  assert.equal(
    (await request('PATCH', '/api/v1/me/profile', { actor: candidate, body: { experienceMonths: null } })).body
      .experienceMonths,
    null,
  );
  await request('PATCH', '/api/v1/me/profile', {
    actor: candidate,
    body: { name: null, teamRole: 'Ignored' },
    status: 400,
  });
  await request('PATCH', '/api/v1/needs/need-001', {
    actor: employer,
    body: { title: 'Ignored', salary: null },
    status: 400,
  });
});

test('need CRUD validates salary and evidence policies without tenant crossover or ID collisions', async (t) => {
  const { request, login } = await setup(t);
  const a = await login('employer-a');
  const b = await login('employer-b');
  const original = (await request('GET', '/api/v1/needs/need-001', { actor: a })).body;
  const input = Object.fromEntries(
    ['title', 'specialization', 'grade', 'workMode', 'salary', 'description', 'requirements'].map((key) => [
      key,
      original[key],
    ]),
  );
  await request('GET', '/api/v1/needs/need-001', { actor: b, status: 404 });
  await request('POST', '/api/v1/needs', {
    actor: a,
    body: { ...input, salary: { ...input.salary, from: 300000, to: 200000 } },
    status: 422,
  });
  await request('POST', '/api/v1/needs', {
    actor: a,
    body: { ...input, salary: { ...input.salary, currency: 'USD' } },
    status: 400,
  });
  await request('POST', '/api/v1/needs', {
    actor: a,
    body: { ...input, salary: { ...input.salary, from: 0 } },
    status: 400,
  });
  await request('POST', '/api/v1/needs', {
    actor: a,
    body: { ...input, requirements: [input.requirements[0], input.requirements[0]] },
    status: 422,
  });
  const item = (await request('POST', '/api/v1/needs', { actor: b, body: input, status: 201 })).body;
  assert.notEqual(item.id, 'need-001');
  assert.equal(item.published, false);
  assert.deepEqual((await request('GET', '/api/v1/needs/need-001', { actor: a })).body, original);
  await request('PATCH', `/api/v1/needs/${item.id}`, { actor: b, body: { title: 'Новая потребность' } });
  const graph = (await request('GET', `/api/v1/needs/${item.id}/graph`, { actor: b })).body;
  assertGraph(graph);
  await request('GET', '/api/v1/needs', { actor: b });
  await request('DELETE', `/api/v1/needs/${item.id}`, { actor: b, status: 204 });
  await request('GET', `/api/v1/needs/${item.id}`, { actor: b, status: 404 });
});

test('GitHub connects by login or profile link, stays unverified without proof and reaches employers anonymously', async (t) => {
  const { request, login, job } = await setup(t, { scenario: 'qualified' });
  const candidate = await login('candidate024');
  const employer = await login('employer-a');
  for (const account of ['https://github.com/octo-cat/repo', 'octo_cat', 'https://gitlab.com/octo-cat'])
    assert.equal(
      (
        await request('POST', '/api/v1/me/sources/github/connect', {
          actor: candidate,
          body: { account },
          status: 422,
        })
      ).body.code,
      'github_login_invalid',
    );
  const source = (
    await request('POST', '/api/v1/me/sources/github/connect', {
      actor: candidate,
      body: { account: 'https://github.com/Octo-Cat/' },
    })
  ).body;
  assert.equal(source.accountLabel, 'Octo-Cat');
  assert.equal(source.github.ownership.verified, false);
  assert.match(source.github.ownership.code, /^fsp-/);
  // The analysis runs in the background; the report arrives with its job.
  assert.equal(source.github.report, null);
  assert.equal(source.github.analysis.state, 'running');
  await job(source.activeJobId, candidate);
  const analyzed = (await request('GET', '/api/v1/me/sources', { actor: candidate })).body.items.find(
    (item: Body) => item.provider === 'github',
  );
  assert.equal(analyzed.activeJobId, null);
  assert.equal(analyzed.github.analysis, null);
  assert.equal(analyzed.github.report.login, 'Octo-Cat');
  assert.equal(analyzed.github.report.activity.calendar.length, 365);
  assert.equal(analyzed.github.employerPreview.ownershipVerified, false);
  const seen = (await request('GET', '/api/v1/candidates/candidate024', { actor: employer })).body;
  assertAnonymous(seen);
  assert.equal(seen.github.ownershipVerified, false);
  assert.deepEqual(seen.github, analyzed.github.employerPreview);
  const serialized = JSON.stringify(seen.github);
  for (const hidden of ['Octo-Cat', 'demo-company', 'calendar', 'login', 'url'])
    assert.ok(!serialized.includes(hidden), `${hidden} leaked to the employer`);
  assert.equal((await request('GET', '/api/v1/me/profile', { actor: candidate })).body.verifiedGrade, 'middle');
});

test('connecting GitHub builds the profile and an estimated category; edited answers stay', async (t) => {
  const { request, client, login, job } = await setup(t);
  const challenge = (
    await request('POST', '/api/v1/auth/email/challenges', {
      body: { email: 'auto-profile@example.com', role: 'candidate' },
      status: 201,
    })
  ).body;
  const candidate = client();
  await request('POST', '/api/v1/auth/email/verify', {
    actor: candidate,
    body: { challengeId: challenge.challengeId, code: '123456' },
  });
  let profile = (
    await request('PATCH', '/api/v1/me/profile', {
      actor: candidate,
      body: { consents: { processing: true, publication: true } },
    })
  ).body;
  assert.equal(profile.name, 'Новый кандидат');
  assert.deepEqual(profile.onboarding.blockers, ['questionnaire', 'category']);
  assert.equal(profile.estimate, null);
  assert.equal(
    (await request('POST', '/api/v1/me/profile/estimate', { actor: candidate, status: 409 })).body.code,
    'source_not_connected',
  );
  const source = (
    await request('POST', '/api/v1/me/sources/github/connect', { actor: candidate, body: { account: 'auto-dev' } })
  ).body;
  await job(source.activeJobId, candidate);
  profile = (await request('GET', '/api/v1/me/profile', { actor: candidate })).body;
  assert.equal(profile.name, 'Иван Петров');
  assert.equal(profile.specialization, 'backend');
  assert.equal(profile.selectedGrade, 'middle');
  assert.equal(profile.stack.length, 12);
  // Public tenure is evidence for the estimate, never written in as the candidate's experience.
  assert.equal(profile.experienceMonths, null);
  assert.equal(profile.experienceSince, null);
  assert.equal(profile.estimate.version, 'estimate-v6');
  assert.ok(profile.softSkills.length > 0);
  assert.ok(profile.estimate.notes.length > 0);
  // The growth plan names concrete steps; focus skills are catalog skills the evidence lacks.
  assert.ok(profile.estimate.growth.some((step: Body) => step.id === 'github_scope' && step.points > 0));
  assert.equal(profile.estimate.growth.at(-1).id, 'assessment');
  assert.ok(profile.estimate.focusSkills.length > 0);
  assert.deepEqual([...profile.autoFields].sort(), [
    'experience',
    'name',
    'selectedGrade',
    'softSkills',
    'specialization',
    'stack',
    'teamRole',
  ]);
  assert.deepEqual(profile.category, {
    specialization: 'backend',
    grade: 'middle',
    eligible: true,
    source: 'estimate',
    confidence: 'high',
  });
  assert.deepEqual(profile.onboarding.blockers, []);
  // An edited answer is the candidate's from then on; an unchanged one stays automatic.
  profile = (
    await request('PATCH', '/api/v1/me/profile', {
      actor: candidate,
      body: { teamRole: 'Тимлид', name: profile.name },
    })
  ).body;
  assert.ok(!profile.autoFields.includes('teamRole'));
  assert.ok(profile.autoFields.includes('name'));
  // A start year in IT counts whole years up to now; typed months replace it.
  profile = (await request('PATCH', '/api/v1/me/profile', { actor: candidate, body: { experienceSince: 2016 } })).body;
  assert.equal(profile.experienceSince, 2016);
  assert.equal(profile.experienceMonths, (Number(profile.updatedAt.slice(0, 4)) - 2016) * 12);
  await request('PATCH', '/api/v1/me/profile', { actor: candidate, body: { experienceSince: 1900 }, status: 400 });
  profile = (await request('POST', '/api/v1/me/profile/estimate', { actor: candidate })).body;
  assert.equal(profile.teamRole, 'Тимлид');
  assert.equal(profile.selectedGrade, 'middle');
  // Employers see the estimated category, its source and factors; no names or the experience text.
  const employer = await login('employer-a');
  const estimated = (
    await request('GET', '/api/v1/candidates?categorySource=estimate&specialization=backend', { actor: employer })
  ).body;
  assertAnonymous(estimated);
  assert.ok(!JSON.stringify(estimated).includes('Иван'));
  const card = estimated.items.find((item: Body) => item.id === profile.id);
  assert.equal(card.grade, 'middle');
  assert.equal(card.categorySource, 'estimate');
  assert.equal(card.categoryConfidence, 'high');
  assert.equal(card.verifiedGrade, null);
  assert.equal(card.estimate.factors.length, 8);
  const confirmed = (await request('GET', '/api/v1/candidates?categorySource=assessment', { actor: employer })).body;
  assert.ok(confirmed.items.length > 0);
  assert.ok(confirmed.items.every((item: Body) => item.categorySource === 'assessment' && item.estimate === null));
  // The candidate may pick another specialization the evidence supports; the category follows it.
  const supported = profile.estimate.specializations[1].id;
  profile = (await request('PATCH', '/api/v1/me/profile', { actor: candidate, body: { specialization: supported } }))
    .body;
  assert.equal(profile.category.specialization, supported);
  profile = (await request('PATCH', '/api/v1/me/profile', { actor: candidate, body: { specialization: 'design' } }))
    .body;
  assert.equal(profile.category.specialization, 'backend');
  // Rebuilding hands every answer back to the evidence; an unknown mode is a contract error.
  profile = (await request('POST', '/api/v1/me/profile/estimate?answers=replace', { actor: candidate })).body;
  assert.notEqual(profile.teamRole, 'Тимлид');
  assert.equal(profile.specialization, 'backend');
  assert.ok(profile.autoFields.includes('teamRole') && profile.autoFields.includes('specialization'));
  await request('POST', '/api/v1/me/profile/estimate?answers=all', { actor: candidate, status: 400 });
});

test('every catalog specialization is testable; a 1C test confirms the category', async (t) => {
  const { request, client, job } = await setup(t);
  const meta = (await request('GET', '/api/v1/meta')).body;
  assert.equal(meta.assessmentPolicy.supportedScopes.length, 25);
  for (const scope of meta.assessmentPolicy.supportedScopes)
    assert.deepEqual(scope.grades, ['intern', 'junior', 'middle', 'senior', 'lead']);
  const challenge = (
    await request('POST', '/api/v1/auth/email/challenges', {
      body: { email: 'onec-developer@example.com', role: 'candidate' },
      status: 201,
    })
  ).body;
  const candidate = client();
  await request('POST', '/api/v1/auth/email/verify', {
    actor: candidate,
    body: { challengeId: challenge.challengeId, code: '123456' },
  });
  await request('PATCH', '/api/v1/me/profile', {
    actor: candidate,
    body: {
      name: 'Ольга Смирнова',
      specialization: 'onec',
      selectedGrade: 'middle',
      stack: ['onec_language', 'onec_queries'],
      teamRole: 'Разработчик 1С',
      experience: 'Доработки 1С:ERP и обмены данными.',
      consents: { processing: true, publication: true },
    },
  });
  const item = (
    await request('POST', '/api/v1/me/assessments', {
      actor: candidate,
      body: { specialization: 'onec', grade: 'middle' },
      status: 201,
    })
  ).body;
  assert.equal(item.task.title, 'Тест знаний: Встроенный язык 1С и Язык запросов 1С');
  assert.equal(item.task.questions.length, 10);
  await request('PUT', `/api/v1/me/assessments/${item.id}/answer`, {
    actor: candidate,
    body: assessmentAnswerFor(item.task),
  });
  const submitted = (
    await request('POST', `/api/v1/me/assessments/${item.id}/submit`, { actor: candidate, status: 202 })
  ).body;
  assert.equal((await job(submitted.id, candidate)).result.assessment.status, 'passed');
  const profile = (await request('GET', '/api/v1/me/profile', { actor: candidate })).body;
  assert.equal(profile.verifiedSpecialization, 'onec');
  assert.deepEqual(
    [profile.category.specialization, profile.category.grade, profile.category.source],
    ['onec', 'middle', 'assessment'],
  );
});

test('candidate bank is anonymous, deterministic and distinguishes qualification from skill evidence', async (t) => {
  const { request, login } = await setup(t, { scenario: 'qualified' });
  const employer = await login('employer-a');
  const page = (
    await request('GET', '/api/v1/candidates?mode=recommended&needId=need-001&grade=middle&specialization=backend', {
      actor: employer,
    })
  ).body;
  assertAnonymous(page);
  assert.deepEqual(
    page.items.map((item: Body) => item.id),
    ['candidate031', 'candidate024', 'candidate018'],
  );
  assert.equal(page.items[2].verifiedGrade, 'middle');
  assert.equal(
    page.items[2].match.requirements.find((item: Body) => item.skillId === 'postgresql').status,
    'not_evidenced',
  );
  const fsp = (await request('GET', '/api/v1/candidates?fsp=verified', { actor: employer })).body;
  assert.deepEqual(
    fsp.items.map((item: Body) => item.id),
    ['candidate031'],
  );
  const first = (await request('GET', '/api/v1/candidates?page=1&pageSize=2', { actor: employer })).body;
  const second = (await request('GET', '/api/v1/candidates?page=2&pageSize=2', { actor: employer })).body;
  assert.equal(new Set([...first.items, ...second.items].map((item) => item.id)).size, 4);
  const candidate = (await request('GET', '/api/v1/candidates/candidate024?needId=need-001', { actor: employer })).body;
  assertAnonymous(candidate);
  assert.equal(candidate.match.requiredMet, 2);
  const graph = (await request('GET', '/api/v1/candidates/candidate024/graph', { actor: employer })).body;
  assertAnonymous(graph);
  assertGraph(graph);
  const plan = (await request('GET', '/api/v1/candidates/candidate018/gap-plan?needId=need-001', { actor: employer }))
    .body;
  assert.ok(
    plan.items.some(
      (item: Body) => item.skillId === 'postgresql' && item.acceptedCompetitionIds.includes('competition-demo-api'),
    ),
  );
  await request('GET', '/api/v1/candidates?mode=recommended', { actor: employer, status: 422 });
});

test('shortlists persist across filters and preserve each company membership', async (t) => {
  const { request, login } = await setup(t);
  const a = await login('employer-a');
  const b = await login('employer-b');
  let list = (await request('GET', '/api/v1/shortlists/shortlist-001', { actor: a })).body;
  assert.equal(list.count, 3);
  await request('GET', '/api/v1/candidates?specialization=backend&grade=middle', { actor: a });
  assert.deepEqual((await request('GET', '/api/v1/shortlists/shortlist-001', { actor: a })).body.members, list.members);
  await request('GET', '/api/v1/shortlists/shortlist-001', { actor: b, status: 404 });
  list = (await request('PUT', '/api/v1/shortlists/shortlist-001/members/candidate031', { actor: a })).body;
  assert.equal(list.count, 4);
  list = (await request('PUT', '/api/v1/shortlists/shortlist-001/members/candidate031', { actor: a })).body;
  assert.equal(list.count, 4);
  list = (await request('DELETE', '/api/v1/shortlists/shortlist-001/members/candidate031', { actor: a })).body;
  assert.equal(list.count, 3);
  const other = (
    await request('POST', '/api/v1/shortlists', { actor: b, body: { title: 'Отдельный список' }, status: 201 })
  ).body;
  assert.notEqual(other.id, 'shortlist-001');
  await request('PATCH', `/api/v1/shortlists/${other.id}`, { actor: b, body: { title: 'Новый заголовок' } });
  await request('GET', '/api/v1/shortlists', { actor: b });
  await request('DELETE', `/api/v1/shortlists/${other.id}`, { actor: b, status: 204 });
  assert.equal((await request('GET', '/api/v1/shortlists/shortlist-001', { actor: a })).body.count, 3);
});

test('direct invitations freeze terms, are idempotent, and disclose only selected fields to the accepting company', async (t) => {
  const { request, login } = await setup(t);
  const a = await login('employer-a');
  const b = await login('employer-b');
  const candidate = await login('candidate024');
  const other = await login('candidate-new');
  await request('GET', '/api/v1/me/gap-plan?needId=need-001', { actor: candidate, status: 404 });
  let disclosure = (await request('GET', '/api/v1/candidates/candidate024/disclosure', { actor: a })).body;
  assert.equal(disclosure.granted, false);
  const input = { candidateId: 'candidate024', needId: 'need-001', message: 'Приглашаем обсудить разработку API.' };
  await request('POST', '/api/v1/invitations', { actor: a, body: input, key: null, status: 400 });
  const invite = (
    await request('POST', '/api/v1/invitations', { actor: a, body: input, key: 'invite-key-a-001', status: 201 })
  ).body;
  const replay = (
    await request('POST', '/api/v1/invitations', { actor: a, body: input, key: 'invite-key-a-001', status: 201 })
  ).body;
  assert.equal(replay.id, invite.id);
  await request('POST', '/api/v1/invitations', {
    actor: a,
    body: { ...input, message: 'Изменено' },
    key: 'invite-key-a-001',
    status: 409,
  });
  await request('POST', '/api/v1/invitations', { actor: a, body: input, status: 409 });
  await request('PATCH', '/api/v1/needs/need-001', {
    actor: a,
    body: { salary: { from: 300000, to: 400000, currency: 'RUB', period: 'month', taxBasis: 'gross' } },
  });
  assert.equal((await request('GET', `/api/v1/invitations/${invite.id}`, { actor: a })).body.need.salary.from, 200000);
  await request('GET', `/api/v1/invitations/${invite.id}`, { actor: b, status: 404 });
  await request('GET', `/api/v1/me/invitations/${invite.id}`, { actor: other, status: 404 });
  await request('GET', `/api/v1/me/invitations/${invite.id}`, { actor: candidate });
  assert.equal(
    (await request('POST', `/api/v1/me/invitations/${invite.id}/view`, { actor: candidate })).body.status,
    'viewed',
  );
  await request('GET', '/api/v1/me/gap-plan?needId=need-001', { actor: candidate });
  await request('GET', '/api/v1/me/invitations', { actor: candidate });
  await request('GET', '/api/v1/invitations', { actor: a });
  const accepted = (
    await request('POST', `/api/v1/me/invitations/${invite.id}/accept`, {
      actor: candidate,
      body: { disclose: ['name', 'email'] },
    })
  ).body;
  assert.equal(accepted.status, 'accepted');
  await request('POST', `/api/v1/me/invitations/${invite.id}/accept`, {
    actor: candidate,
    body: { disclose: ['email', 'name'] },
  });
  await request('POST', `/api/v1/me/invitations/${invite.id}/accept`, {
    actor: candidate,
    body: { disclose: ['sourceLinks'] },
    status: 409,
  });
  await request('POST', `/api/v1/invitations/${invite.id}/withdraw`, { actor: a, status: 409 });
  await request('POST', `/api/v1/me/invitations/${invite.id}/decline`, { actor: candidate, status: 409 });
  disclosure = (await request('GET', '/api/v1/candidates/candidate024/disclosure', { actor: a })).body;
  assert.deepEqual(disclosure.fields, { name: 'Алексей Морозов', email: 'alexey@example.com' });
  const hidden = (await request('GET', '/api/v1/candidates/candidate024/disclosure', { actor: b })).body;
  assert.deepEqual(hidden.fields, {});
  assertAnonymous((await request('GET', '/api/v1/candidates/candidate024', { actor: a })).body);
});

test('declining and withdrawing never grant contacts and terminal transitions remain stable', async (t) => {
  const { request, login } = await setup(t);
  const a = await login('employer-a');
  const candidate = await login('candidate024');
  const input = { candidateId: 'candidate024', needId: 'need-001', message: 'Обсудим вакансию.' };
  let invite = (await request('POST', '/api/v1/invitations', { actor: a, body: input, status: 201 })).body;
  await request('POST', `/api/v1/me/invitations/${invite.id}/decline`, { actor: candidate });
  await request('POST', `/api/v1/me/invitations/${invite.id}/decline`, { actor: candidate });
  assert.equal((await request('GET', '/api/v1/candidates/candidate024/disclosure', { actor: a })).body.granted, false);
  await request('POST', `/api/v1/me/invitations/${invite.id}/accept`, {
    actor: candidate,
    body: { disclose: ['email'] },
    status: 409,
  });
  invite = (await request('POST', '/api/v1/invitations', { actor: a, body: input, status: 201 })).body;
  await request('POST', `/api/v1/invitations/${invite.id}/withdraw`, { actor: a });
  await request('POST', `/api/v1/invitations/${invite.id}/withdraw`, { actor: a });
  await request('POST', `/api/v1/me/invitations/${invite.id}/accept`, {
    actor: candidate,
    body: { disclose: ['email'] },
    status: 409,
  });
});

test('vacancy publication needs a complete company and shows candidates only published vacancies', async (t) => {
  const { request, login, client } = await setup(t);
  const a = await login('employer-a');
  const b = await login('employer-b');
  const candidate = await login('candidate024');
  const fresh = await login('candidate-new');
  let page = (await request('GET', '/api/v1/vacancies', { actor: candidate })).body;
  assert.deepEqual(
    page.items.map((item: Body) => item.id),
    ['need-002', 'need-b-002', 'need-b-001'],
  );
  await request('POST', '/api/v1/needs/need-001/publish', { actor: b, status: 404 });
  const published = (await request('POST', '/api/v1/needs/need-001/publish', { actor: a })).body;
  assert.equal(published.published, true);
  assert.equal(published.publishedAt, '2026-10-05T12:00:00.000Z');
  assert.equal(
    (await request('POST', '/api/v1/needs/need-001/publish', { actor: a })).body.publishedAt,
    published.publishedAt,
  );
  page = (await request('GET', '/api/v1/vacancies?pageSize=2', { actor: candidate })).body;
  assert.deepEqual(
    page.items.map((item: Body) => item.id),
    ['need-002', 'need-b-002'],
  );
  assert.equal(page.total, 4);
  assert.equal(page.hasNextPage, true);
  for (const [query, ids] of [
    ['specialization=frontend', ['need-b-002']],
    ['grade=junior', ['need-002']],
    ['workMode=office', ['need-b-002']],
    ['skill=redis', ['need-001', 'need-b-001']],
  ] as const) {
    const filtered = (await request('GET', `/api/v1/vacancies?${query}`, { actor: candidate })).body;
    assert.deepEqual(
      filtered.items.map((item: Body) => item.id),
      ids,
    );
  }
  const vacancy = (await request('GET', '/api/v1/vacancies/need-002', { actor: candidate })).body;
  assert.equal(vacancy.company.displayName, 'Север Тех');
  assert.deepEqual(vacancy.eligibility, { canApply: true, reason: null });
  assert.equal(vacancy.applicationId, null);
  assert.equal(vacancy.match.status, 'meets_required');
  assert.ok(!('evidencePolicy' in vacancy.requirements[0]));
  const blocked = (await request('GET', '/api/v1/vacancies/need-002', { actor: fresh })).body;
  assert.equal(blocked.match, null);
  assert.deepEqual(blocked.eligibility, { canApply: false, reason: 'email' });
  await request('GET', '/api/v1/vacancies', { actor: a, status: 403 });
  await request('POST', '/api/v1/needs/need-001/unpublish', { actor: a });
  assert.equal((await request('POST', '/api/v1/needs/need-001/unpublish', { actor: a })).body.publishedAt, null);
  await request('GET', '/api/v1/vacancies/need-001', { actor: candidate, status: 404 });

  const challenge = (
    await request('POST', '/api/v1/auth/email/challenges', {
      body: { email: 'vacancy-hr@example.com', role: 'employer' },
      status: 201,
    })
  ).body;
  const employer = client();
  await request('POST', '/api/v1/auth/email/verify', {
    actor: employer,
    body: { challengeId: challenge.challengeId, code: '123456' },
  });
  const input = Object.fromEntries(
    ['title', 'specialization', 'grade', 'workMode', 'salary', 'description', 'requirements'].map((key) => [
      key,
      published[key],
    ]),
  );
  const own = (await request('POST', '/api/v1/needs', { actor: employer, body: input, status: 201 })).body;
  assert.equal(own.published, false);
  await request('POST', `/api/v1/needs/${own.id}/publish`, { actor: employer, status: 422 });
  await request('PATCH', '/api/v1/company', {
    actor: employer,
    body: { displayName: 'Третья компания', businessArea: 'Финтех', description: 'Платёжные сервисы.' },
  });
  await request('POST', `/api/v1/needs/${own.id}/publish`, { actor: employer });
  assert.ok(
    (await request('GET', '/api/v1/vacancies', { actor: candidate })).body.items.some(
      (item: Body) => item.id === own.id,
    ),
  );
});

test('applications disclose the chosen fields, keep vacancy terms and follow employer decisions', async (t) => {
  const { request, login } = await setup(t);
  const a = await login('employer-a');
  const b = await login('employer-b');
  const candidate = await login('candidate024');
  const other = await login('candidate-new');
  const input = { vacancyId: 'need-002', message: 'Хочу работать над доставкой уведомлений.', disclose: ['email'] };
  await request('POST', '/api/v1/me/applications', { actor: other, body: input, status: 403 });
  await request('POST', '/api/v1/me/applications', {
    actor: candidate,
    body: { ...input, vacancyId: 'need-001' },
    status: 404,
  });
  await request('POST', '/api/v1/me/applications', { actor: candidate, body: { ...input, disclose: [] }, status: 400 });
  const application = (
    await request('POST', '/api/v1/me/applications', {
      actor: candidate,
      body: input,
      key: 'application-key-001',
      status: 201,
    })
  ).body;
  assert.equal(application.status, 'sent');
  assert.equal(application.vacancy.salary.from, 120000);
  const replay = (
    await request('POST', '/api/v1/me/applications', {
      actor: candidate,
      body: input,
      key: 'application-key-001',
      status: 201,
    })
  ).body;
  assert.equal(replay.id, application.id);
  await request('POST', '/api/v1/me/applications', { actor: candidate, body: input, status: 409 });
  const vacancy = (await request('GET', '/api/v1/vacancies/need-002', { actor: candidate })).body;
  assert.equal(vacancy.applicationId, application.id);
  assert.deepEqual(vacancy.eligibility, { canApply: false, reason: 'application_exists' });

  await request('PATCH', '/api/v1/needs/need-002', {
    actor: a,
    body: { salary: { from: 150000, to: 190000, currency: 'RUB', period: 'month', taxBasis: 'gross' } },
  });
  const seen = (await request('GET', `/api/v1/applications/${application.id}`, { actor: a })).body;
  assert.equal(seen.vacancy.salary.from, 120000);
  assertAnonymous(seen);
  await request('GET', `/api/v1/applications/${application.id}`, { actor: b, status: 404 });
  await request('GET', `/api/v1/me/applications/${application.id}`, { actor: other, status: 404 });
  assert.equal((await request('GET', '/api/v1/applications?needId=need-002', { actor: a })).body.items.length, 1);
  assert.equal((await request('GET', '/api/v1/applications?needId=need-001', { actor: a })).body.items.length, 0);
  assert.equal((await request('GET', '/api/v1/applications', { actor: b })).body.items.length, 0);
  const analyticsA = (await request('GET', '/api/v1/analytics/hiring-flow', { actor: a })).body;
  const analyticsB = (await request('GET', '/api/v1/analytics/hiring-flow', { actor: b })).body;
  assert.equal(analyticsA.total, 1);
  assert.equal(analyticsA.stages[0].count, 1);
  assert.equal(analyticsB.total, 0);
  assert.deepEqual(analyticsB.vacancies, []);
  let disclosure = (await request('GET', '/api/v1/candidates/candidate024/disclosure', { actor: a })).body;
  assert.deepEqual(disclosure.fields, { email: 'alexey@example.com' });
  assert.equal(disclosure.applicationId, application.id);
  assert.equal(disclosure.invitationId, null);
  assert.equal((await request('GET', '/api/v1/candidates/candidate024/disclosure', { actor: b })).body.granted, false);

  const viewed = (await request('POST', `/api/v1/applications/${application.id}/view`, { actor: a })).body;
  assert.equal(viewed.status, 'viewed');
  assert.equal(
    (await request('GET', `/api/v1/me/applications/${application.id}`, { actor: candidate })).body.status,
    'viewed',
  );
  assert.equal(
    (await request('POST', `/api/v1/applications/${application.id}/accept`, { actor: a })).body.status,
    'accepted',
  );
  await request('POST', `/api/v1/applications/${application.id}/accept`, { actor: a });
  await request('POST', `/api/v1/applications/${application.id}/decline`, { actor: a, status: 409 });
  await request('POST', `/api/v1/me/applications/${application.id}/withdraw`, { actor: candidate, status: 409 });

  const second = (
    await request('POST', '/api/v1/me/applications', {
      actor: candidate,
      body: { vacancyId: 'need-b-001', message: 'Откликаюсь на роль бэкенд-разработчика.', disclose: ['name'] },
      status: 201,
    })
  ).body;
  await request('PATCH', '/api/v1/me/profile', {
    actor: candidate,
    body: { consents: { processing: true, publication: false } },
  });
  await request('GET', '/api/v1/candidates/candidate024', { actor: a });
  await request('GET', '/api/v1/candidates/candidate024', { actor: b });
  const withdrawn = (await request('POST', `/api/v1/me/applications/${second.id}/withdraw`, { actor: candidate })).body;
  assert.equal(withdrawn.status, 'withdrawn');
  await request('POST', `/api/v1/me/applications/${second.id}/withdraw`, { actor: candidate });
  await request('GET', '/api/v1/candidates/candidate024', { actor: b, status: 404 });
  await request('GET', '/api/v1/candidates/candidate024/disclosure', { actor: b, status: 404 });
  disclosure = (await request('GET', '/api/v1/candidates/candidate024/disclosure', { actor: a })).body;
  assert.deepEqual(disclosure.fields, { email: 'alexey@example.com' });

  const again = (
    await request('POST', '/api/v1/me/applications', {
      actor: candidate,
      body: { vacancyId: 'need-b-001', message: 'Повторный отклик после отзыва.', disclose: ['email'] },
      status: 201,
    })
  ).body;
  assert.equal(
    (await request('POST', `/api/v1/applications/${again.id}/decline`, { actor: b })).body.status,
    'declined',
  );
  await request('POST', `/api/v1/applications/${again.id}/decline`, { actor: b });
  const mine = (await request('GET', '/api/v1/me/applications', { actor: candidate })).body.items;
  assert.deepEqual(
    mine.map((item: Body) => item.status),
    ['declined', 'withdrawn', 'accepted'],
  );
});

test('weekly tasks come from evidenced skills, are answered once a week and keep the profile fresh', async (t) => {
  const { request, login } = await setup(t);
  const a = await login('employer-a');
  const b = await login('employer-b');
  const candidate = await login('candidate024');
  const fresh = await login('candidate-new');
  let mine = (await request('GET', '/api/v1/me/tasks', { actor: candidate })).body;
  assert.deepEqual(mine.period, { from: '2026-10-05T00:00:00.000Z', to: '2026-10-12T00:00:00.000Z' });
  assert.equal(mine.current.id, 'task-002');
  assert.equal(mine.current.company.displayName, 'Север Тех');
  assert.equal(mine.currentResponse, null);
  assert.deepEqual(mine.freshness, {
    lastSignalAt: '2026-06-01T12:00:00.000Z',
    lastSignal: 'assessment',
    fresh: false,
    windowDays: 90,
  });
  assert.equal((await request('GET', '/api/v1/me/tasks', { actor: fresh })).body.reason, 'category');
  const answer = { mode: 'approach', answer: 'Очередь попыток с экспоненциальной задержкой и пределом попыток.' };
  await request('POST', '/api/v1/me/tasks/task-001/responses', { actor: candidate, body: answer, status: 409 });
  const response = (
    await request('POST', '/api/v1/me/tasks/task-002/responses', {
      actor: candidate,
      body: answer,
      key: 'task-response-key-001',
      status: 201,
    })
  ).body;
  assert.equal(response.status, 'submitted');
  assert.equal(
    (
      await request('POST', '/api/v1/me/tasks/task-002/responses', {
        actor: candidate,
        body: answer,
        key: 'task-response-key-001',
        status: 201,
      })
    ).body.id,
    response.id,
  );
  await request('POST', '/api/v1/me/tasks/task-002/responses', { actor: candidate, body: answer, status: 409 });
  mine = (await request('GET', '/api/v1/me/tasks', { actor: candidate })).body;
  assert.equal(mine.currentResponse.id, response.id);
  assert.equal(mine.freshness.lastSignal, 'task');
  assert.equal(mine.freshness.fresh, true);
  const seen = (await request('GET', '/api/v1/candidates/candidate024', { actor: b })).body;
  assert.equal(seen.freshness.fresh, true);
  assertAnonymous(seen);

  assert.deepEqual(
    (await request('GET', '/api/v1/task-responses', { actor: b })).body.items.map((item: Body) => item.id),
    ['task-response-seed-041'],
  );
  await request('POST', `/api/v1/task-responses/${response.id}/review`, {
    actor: b,
    body: { outcome: 'accepted', feedback: '' },
    status: 404,
  });
  const listed = (await request('GET', '/api/v1/task-responses?taskId=task-002', { actor: a })).body.items;
  assert.deepEqual(
    listed.map((item: Body) => item.id),
    [response.id, 'task-response-seed-031'],
  );
  assertAnonymous(listed[0]);
  const reviewed = (
    await request('POST', `/api/v1/task-responses/${response.id}/review`, {
      actor: a,
      body: { outcome: 'needs_work', feedback: 'Не хватает условия прекращения попыток.' },
    })
  ).body;
  assert.equal(reviewed.status, 'needs_work');
  await request('POST', `/api/v1/task-responses/${response.id}/review`, {
    actor: a,
    body: { outcome: 'needs_work', feedback: 'Не хватает условия прекращения попыток.' },
  });
  await request('POST', `/api/v1/task-responses/${response.id}/review`, {
    actor: a,
    body: { outcome: 'accepted', feedback: '' },
    status: 409,
  });
  assert.equal((await request('GET', '/api/v1/me/profile', { actor: candidate })).body.freshness.fresh, false);

  await request('POST', '/__mock__/clock', { body: { advanceSeconds: 7 * 86400 } });
  mine = (await request('GET', '/api/v1/me/tasks', { actor: candidate })).body;
  assert.equal(mine.current.id, 'task-001');
  assert.equal(mine.history.length, 1);
  const next = (
    await request('POST', '/api/v1/me/tasks/task-001/responses', { actor: candidate, body: answer, status: 201 })
  ).body;
  await request('POST', `/api/v1/task-responses/${next.id}/review`, {
    actor: a,
    body: { outcome: 'accepted', feedback: 'Спасибо, разбор подходит.' },
  });
  mine = (await request('GET', '/api/v1/me/tasks', { actor: candidate })).body;
  assert.equal(mine.currentResponse.status, 'accepted');
  assert.equal(mine.freshness.fresh, true);
  await request('PATCH', '/api/v1/me/profile', {
    actor: candidate,
    body: { consents: { processing: false, publication: false } },
  });
  await request('POST', '/__mock__/clock', { body: { advanceSeconds: 7 * 86400 } });
  assert.equal((await request('GET', '/api/v1/me/tasks', { actor: candidate })).body.reason, 'processing_consent');
  await request('POST', '/api/v1/me/tasks/task-001/responses', { actor: candidate, body: answer, status: 403 });
});

test('companies author and archive short tasks; archived tasks are no longer offered', async (t) => {
  const { request, login, client } = await setup(t);
  const a = await login('employer-a');
  const b = await login('employer-b');
  const candidate = await login('candidate024');
  const input = {
    title: 'Индекс для поиска заказов',
    prompt: 'Какой индекс ускорит выборку заказов клиента за последний месяц и почему?',
    skillId: 'postgresql',
    grade: 'junior',
    estimateMinutes: 20,
  };
  const created = (await request('POST', '/api/v1/tasks', { actor: a, body: input, status: 201 })).body;
  assert.equal(created.status, 'active');
  assert.equal(created.responseCount, 0);
  await request('POST', '/api/v1/tasks', { actor: a, body: { ...input, estimateMinutes: 500 }, status: 400 });
  const tasks = (await request('GET', '/api/v1/tasks', { actor: a })).body.items;
  assert.deepEqual(
    tasks.map((item: Body) => item.id),
    [created.id, 'task-002', 'task-001'],
  );
  assert.equal(tasks[1].responseCount, 1);
  await request('POST', '/api/v1/tasks/task-002/archive', { actor: b, status: 404 });
  assert.equal((await request('POST', '/api/v1/tasks/task-002/archive', { actor: a })).body.status, 'archived');
  assert.equal((await request('POST', '/api/v1/tasks/task-002/archive', { actor: a })).body.status, 'archived');
  const offer = (await request('GET', '/api/v1/me/tasks', { actor: candidate })).body.current;
  assert.notEqual(offer.id, 'task-002');
  assert.ok([created.id, 'task-001'].includes(offer.id));

  const challenge = (
    await request('POST', '/api/v1/auth/email/challenges', {
      body: { email: 'tasks-hr@example.com', role: 'employer' },
      status: 201,
    })
  ).body;
  const employer = client();
  await request('POST', '/api/v1/auth/email/verify', {
    actor: employer,
    body: { challengeId: challenge.challengeId, code: '123456' },
  });
  await request('POST', '/api/v1/tasks', { actor: employer, body: input, status: 422 });
});

test('PDF export is a real current-profile PDF; fixture import is asynchronous and explicitly confirmed', async (t) => {
  const { request, login, job } = await setup(t);
  const candidate = await login('candidate024');
  // The seeded source sync changes the profile revision when it finishes; an import starts after it.
  for (const source of (await request('GET', '/api/v1/me/sources', { actor: candidate })).body.items)
    if (source.activeJobId) await job(source.activeJobId, candidate);
  const exported = await request('GET', '/api/v1/me/profile/export', { actor: candidate });
  assert.ok(exported.buffer.subarray(0, 5).equals(Buffer.from('%PDF-')));
  assert.ok(exported.buffer.subarray(-1024).includes(Buffer.from('%%EOF')));
  assert.equal(exported.response.headers.get('content-type'), 'application/pdf');
  // The PDF embeds its creation time, so the sample is rewritten only on request.
  if (process.env.UPDATE_FIXTURES === '1')
    await writeFile(new URL('../fixtures/profile-export.pdf', import.meta.url), exported.buffer);
  const form = new FormData();
  form.set('file', new Blob([exported.buffer], { type: 'application/pdf' }), 'profile.pdf');
  const imported = (await request('POST', '/api/v1/me/profile/imports', { actor: candidate, raw: form, status: 202 }))
    .body;
  await request('POST', `/api/v1/me/profile/imports/${imported.id}/confirm`, {
    actor: candidate,
    body: { profile: { name: 'Имя' } },
    status: 409,
  });
  const result = await job(imported.id, candidate);
  assert.equal(result.result.parser, 'fixture_preview');
  assert.equal(result.result.preview.experienceMonths, null);
  await request('POST', `/api/v1/me/profile/imports/${imported.id}/confirm`, {
    actor: candidate,
    body: { profile: result.result.preview },
  });
  await request('POST', `/api/v1/me/profile/imports/${imported.id}/confirm`, {
    actor: candidate,
    body: { profile: { name: 'Повтор' } },
    status: 409,
  });
  const invalid = new FormData();
  invalid.set('file', new Blob(['not-a-pdf'], { type: 'application/pdf' }), 'bad.pdf');
  await request('POST', '/api/v1/me/profile/imports', { actor: candidate, raw: invalid, status: 422 });
});

test('a PDF preview cannot overwrite a newer profile revision', async (t) => {
  const { request, login, job } = await setup(t);
  const candidate = await login('candidate024');
  const exported = await request('GET', '/api/v1/me/profile/export', { actor: candidate });
  const form = new FormData();
  form.set('file', new Blob([exported.buffer], { type: 'application/pdf' }), 'profile.pdf');
  const imported = (await request('POST', '/api/v1/me/profile/imports', { actor: candidate, raw: form, status: 202 }))
    .body;
  await job(imported.id, candidate);
  await request('PATCH', '/api/v1/me/profile', { actor: candidate, body: { name: 'Актуальное имя' } });
  const conflict = await request('POST', `/api/v1/me/profile/imports/${imported.id}/confirm`, {
    actor: candidate,
    body: { profile: { name: 'Старое имя' } },
    status: 409,
  });
  assert.equal(conflict.body.code, 'import_profile_changed');
  assert.equal((await request('GET', '/api/v1/me/profile', { actor: candidate })).body.name, 'Актуальное имя');
});

test('scenario reset clears sessions and supports no-FSP, empty and error states', async (t) => {
  const { request, login } = await setup(t);
  let employer = await login('employer-a');
  await request('POST', '/__mock__/reset', { body: { scenario: 'no_fsp' } });
  await request('GET', '/api/v1/session', { actor: employer, status: 401 });
  employer = await login('employer-a');
  assert.equal((await request('GET', '/api/v1/candidates?fsp=verified', { actor: employer })).body.total, 0);
  assert.ok((await request('GET', '/api/v1/candidates', { actor: employer })).body.total > 0);
  await request('POST', '/__mock__/reset', { body: { scenario: 'empty_bank' } });
  employer = await login('employer-a');
  assert.equal((await request('GET', '/api/v1/candidates', { actor: employer })).body.total, 0);
  assert.equal((await request('GET', '/api/v1/shortlists/shortlist-001', { actor: employer })).body.count, 3);
  await request('POST', '/__mock__/reset', { body: { scenario: 'api_error' } });
  employer = await login('employer-a');
  await request('GET', '/api/v1/candidates', { actor: employer, status: 503 });
});

after(async () => {
  const report = {
    contractVersion: '0.1.0',
    routeCount: routes.length,
    coveredOperationCount: coverage.size,
    operations: routes.map((route) => ({
      operationId: route.operationId,
      method: route.method.toUpperCase(),
      path: route.path,
      observedStatuses: (coverage.get(route.operationId) ?? []).sort((a, b) => a - b),
    })),
  };
  await writeFile(new URL('contract-coverage.json', import.meta.url), `${JSON.stringify(report, null, 2)}\n`);
  const missing = report.operations.filter((entry) => !entry.observedStatuses.length);
  assert.deepEqual(
    missing.map((entry) => entry.operationId),
    [],
    'Every declared operation must be exercised',
  );
});

test('candidate and employer see identical policy-aware fit and safe source projections', async (t) => {
  const { request, login } = await setup(t, { scenario: 'qualified' });
  const candidate = await login('candidate024');
  const employer = await login('employer-a');
  await request('POST', '/api/v1/needs/need-001/publish', { actor: employer });
  const original = (await request('GET', '/api/v1/needs/need-001', { actor: employer })).body;
  for (const acceptAssessments of [true, false]) {
    await request('PATCH', '/api/v1/needs/need-001', {
      actor: employer,
      body: {
        requirements: original.requirements.map((item: Body) => ({
          ...item,
          evidencePolicy: { ...item.evidencePolicy, acceptAssessments, acceptGithub: true },
        })),
      },
    });
    const own = (await request('GET', '/api/v1/vacancies/need-001', { actor: candidate })).body;
    const seen = (await request('GET', '/api/v1/candidates/candidate024?needId=need-001', { actor: employer })).body;
    assert.deepEqual(own.match, seen.match);
    assertAnonymous(seen);
    assert.equal(
      seen.match.requiredMet,
      seen.match.requirements.filter((item: Body) => item.criticality === 'required' && item.status === 'meets').length,
    );
    assert.equal(
      seen.match.requiredEvidenced,
      seen.match.requirements.filter(
        (item: Body) =>
          item.criticality === 'required' && item.evidenceStatus !== 'insufficient' && item.acceptedSources.length > 0,
      ).length,
    );
    if (!acceptAssessments) assert.equal(seen.match.requiredMet, 0);
    for (const item of seen.match.requirements) {
      if (!acceptAssessments) assert.ok(!item.acceptedSources.includes('assessment'));
      assert.ok(item.acceptedSources.every((source: string) => item.evidenceSources.includes(source)));
    }
    assert.ok(seen.fsp === null || (!('teamName' in seen.fsp) && !('accountLabel' in seen.fsp)));
    assert.ok(seen.review === null || !('rationale' in seen.review));
  }
});
