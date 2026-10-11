// Generates the frontend's contract types and the operation index from ../openapi.json.
// The declarations follow the openapi-typescript layout that openapi-fetch expects, for the JSON Schema subset the
// contract uses; anything outside that subset fails the run instead of being guessed.
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';

type Json = string | number | boolean | null | Json[] | { [key: string]: Json };

interface Schema {
  $ref?: string;
  type?: 'object' | 'array' | 'string' | 'integer' | 'number' | 'boolean' | 'null';
  enum?: Json[];
  const?: Json;
  format?: string;
  title?: string;
  description?: string;
  default?: Json;
  deprecated?: boolean;
  properties?: Record<string, Schema>;
  required?: string[];
  additionalProperties?: boolean | Schema;
  items?: Schema;
  anyOf?: Schema[];
  oneOf?: Schema[];
  allOf?: Schema[];
}

interface Parameter {
  name: string;
  in: 'query' | 'header' | 'path' | 'cookie';
  required?: boolean;
  description?: string;
  schema: Schema;
}

interface MediaType {
  schema?: Schema;
}

interface Operation {
  operationId: string;
  summary?: string;
  description?: string;
  parameters?: Parameter[];
  requestBody?: { required?: boolean; description?: string; content: Record<string, MediaType> };
  responses: Record<string, { description?: string; content?: Record<string, MediaType> }>;
}

interface Contract {
  info: { version: string };
  paths: Record<string, Partial<Record<Method, Operation>>>;
  components: { schemas: Record<string, Schema> };
}

const methods = ['get', 'put', 'post', 'delete', 'options', 'head', 'patch', 'trace'] as const;
type Method = (typeof methods)[number];

const contractUrl = new URL('../openapi.json', import.meta.url);
const typesUrl = new URL('../../frontend/src/shared/api/generated/', import.meta.url);
const mockTypesUrl = new URL('../../mock-server/src/generated/', import.meta.url);
const indexUrl = new URL('../API_OPERATIONS.md', import.meta.url);

const source = await readFile(contractUrl, 'utf8');
const contract = JSON.parse(source) as Contract;

// ---- Type model and printer ------------------------------------------------------------------------------------

type Type =
  | { kind: 'text'; text: string; primitive: boolean }
  | { kind: 'array'; element: Type }
  | { kind: 'union'; members: Type[] }
  | { kind: 'object'; members: Member[] };

interface Member {
  key: string;
  optional: boolean;
  type: Type;
  doc: string[];
}

const text = (value: string, primitive = false): Type => ({ kind: 'text', text: value, primitive });
const STRING = text('string', true);
const NUMBER = text('number', true);
const BOOLEAN = text('boolean', true);
const NULL = text('null', true);
const NEVER = text('never', true);
const UNKNOWN = text('unknown');

function union(types: Type[]): Type {
  const seen = new Set<string>();
  const members = types.filter((type) => {
    if (type.kind !== 'text' || !type.primitive || type.text.startsWith('"')) return true;
    if (seen.has(type.text)) return false;
    seen.add(type.text);
    return true;
  });
  if (members.length === 0) return NEVER;
  return members.length === 1 ? (members[0] as Type) : { kind: 'union', members };
}

const indent = (level: number) => '    '.repeat(level);

function print(type: Type, level: number): string {
  switch (type.kind) {
    case 'text':
      return type.text;
    case 'array': {
      const element = print(type.element, level);
      return type.element.kind === 'union' ? `(${element})[]` : `${element}[]`;
    }
    case 'union':
      return type.members
        .map((member) => (member.kind === 'union' ? `(${print(member, level)})` : print(member, level)))
        .join(' | ');
    case 'object':
      return `{\n${type.members.map((member) => printMember(member, level + 1)).join('')}${indent(level)}}`;
  }
}

function printMember(member: Member, level: number): string {
  const pad = indent(level);
  let comment = '';
  if (member.doc.length === 1 && !member.doc[0]?.includes('\n')) comment = `${pad}/** ${member.doc[0]} */\n`;
  else if (member.doc.length > 0) {
    comment = `${pad}/**\n${member.doc.map((line) => `${pad} * ${line.replaceAll('\n', `\n${pad} *     `)}\n`).join('')}${pad} */\n`;
  }
  return `${comment}${pad}${member.key}${member.optional ? '?' : ''}: ${print(member.type, level)};\n`;
}

function propertyKey(name: string): string {
  if (/^(0|[1-9]\d*)$/.test(name)) return name;
  return /^[A-Za-z_$][A-Za-z_$0-9]*$/.test(name) ? name : JSON.stringify(name);
}

const member = (name: string, type: Type, optional = false, doc: string[] = []): Member => ({
  key: propertyKey(name),
  optional,
  type,
  doc,
});

interface Documented {
  title?: string;
  summary?: string;
  format?: string;
  deprecated?: boolean;
  description?: string;
  default?: Json;
  const?: Json;
  enum?: Json[];
  type?: string;
}

/** JSDoc lines in openapi-typescript's order: title, summary, format, deprecation, description, default, const, enum. */
function docFor(value: Documented): string[] {
  const lines: string[] = [];
  const clean = (line: string) => line.trim().replaceAll('*/', '*\\/');
  if (value.title) lines.push(clean(value.title));
  if (value.summary) lines.push(clean(value.summary));
  if (value.format) lines.push(`Format: ${value.format}`);
  if (value.deprecated) lines.push('@deprecated');
  if (value.description) lines.push(`@description ${clean(value.description)}`);
  if (value.default !== undefined) {
    const serialized =
      typeof value.default === 'object' ? JSON.stringify(value.default, null, 2) : String(value.default);
    lines.push(`@default ${clean(serialized)}`);
  }
  if ('const' in value) lines.push('@constant');
  if (value.enum) lines.push(`@enum {${typeof value.type === 'string' ? value.type : 'unknown'}}`);
  return lines;
}

// ---- Schemas ---------------------------------------------------------------------------------------------------

function literal(value: Json): Type {
  if (typeof value === 'string') return text(JSON.stringify(value), true);
  if (typeof value === 'number' || typeof value === 'boolean') return text(String(value), true);
  if (value === null) return NULL;
  throw new Error(`Unsupported literal: ${JSON.stringify(value)}`);
}

function reference(ref: string): Type {
  const name = /^#\/components\/schemas\/([^/]+)$/.exec(ref)?.[1];
  if (!name || !Object.hasOwn(contract.components.schemas, name)) throw new Error(`Unresolved reference: ${ref}`);
  return text(`components["schemas"][${JSON.stringify(name)}]`);
}

function schemaType(schema: Schema, at: string): Type {
  if (schema.allOf) throw new Error(`${at}: allOf is not supported`);
  if (schema.$ref) return reference(schema.$ref);
  if (schema.type === 'string' && schema.format === 'binary') return text('Blob');
  if (schema.const !== undefined && schema.const !== null) return literal(schema.const);
  if (schema.enum && schema.type !== 'object') return union(schema.enum.map(literal));
  if (schema.anyOf) return union(schema.anyOf.map((item, index) => schemaType(item, `${at}/anyOf/${index}`)));
  if (schema.oneOf) return union(schema.oneOf.map((item, index) => schemaType(item, `${at}/oneOf/${index}`)));
  switch (schema.type) {
    case 'null':
      return NULL;
    case 'string':
      return STRING;
    case 'integer':
    case 'number':
      return NUMBER;
    case 'boolean':
      return BOOLEAN;
    case 'array':
      return { kind: 'array', element: schema.items ? schemaType(schema.items, `${at}/items`) : UNKNOWN };
    case 'object':
      return objectType(schema, at);
    default:
      throw new Error(`${at}: unsupported schema ${JSON.stringify(schema)}`);
  }
}

function objectType(schema: Schema, at: string): Type {
  if (schema.additionalProperties !== undefined && schema.additionalProperties !== false) {
    throw new Error(`${at}: additionalProperties other than false is not supported`);
  }
  const entries = Object.entries(schema.properties ?? {});
  if (entries.length === 0) return text('Record<string, never>');
  return {
    kind: 'object',
    members: entries.map(([name, property]) =>
      member(name, schemaType(property, `${at}/${name}`), !schema.required?.includes(name), docFor(property)),
    ),
  };
}

// ---- Paths and operations --------------------------------------------------------------------------------------

function parametersType(parameters: Parameter[]): Type {
  return {
    kind: 'object',
    members: (['query', 'header', 'path', 'cookie'] as const).map((location) => {
      const members = parameters
        .filter((parameter) => parameter.in === location)
        .map((parameter) =>
          member(
            parameter.name,
            schemaType(parameter.schema, `parameter ${parameter.name}`),
            location !== 'path' && !parameter.required,
            docFor(parameter),
          ),
        );
      const optional = members.length === 0 || members.every((item) => item.optional);
      return member(location, members.length ? { kind: 'object', members } : NEVER, optional);
    }),
  };
}

function contentType(content: Record<string, MediaType>, at: string): Type {
  return {
    kind: 'object',
    members: Object.entries(content).map(([media, value]) =>
      member(media, value.schema ? schemaType(value.schema, `${at}/${media}`) : UNKNOWN),
    ),
  };
}

/** Responses declare no headers, so each allows any header, as openapi-typescript emits it. */
const responseHeaders: Type = {
  kind: 'object',
  members: [{ key: '[name: string]', optional: false, type: UNKNOWN, doc: [] }],
};

function operationType(operation: Operation): Type {
  const at = operation.operationId;
  const members = [member('parameters', parametersType(operation.parameters ?? []))];
  const body = operation.requestBody;
  members.push(
    body
      ? member(
          'requestBody',
          { kind: 'object', members: [member('content', contentType(body.content, `${at}/requestBody`))] },
          !body.required,
          docFor(body),
        )
      : member('requestBody', NEVER, true),
  );
  const responses = Object.entries(operation.responses).map(([code, response]) => {
    const content = response.content
      ? member('content', contentType(response.content, `${at}/${code}`))
      : member('content', NEVER, true);
    return member(
      code,
      { kind: 'object', members: [member('headers', responseHeaders), content] },
      false,
      docFor(response),
    );
  });
  members.push(member('responses', responses.length ? { kind: 'object', members: responses } : NEVER));
  return { kind: 'object', members };
}

const noParameters = parametersType([]);
const pathMembers: Member[] = [];
const operationMembers: Member[] = [];
for (const [path, item] of Object.entries(contract.paths)) {
  const members = [member('parameters', noParameters)];
  for (const method of methods) {
    const operation = item[method];
    if (!operation) {
      members.push(member(method, NEVER, true));
      continue;
    }
    members.push(
      member(method, text(`operations[${JSON.stringify(operation.operationId)}]`), false, docFor(operation)),
    );
    operationMembers.push(member(operation.operationId, operationType(operation)));
  }
  pathMembers.push(member(path, { kind: 'object', members }));
}

const schemaMembers = Object.entries(contract.components.schemas).map(([name, schema]) =>
  member(name, schemaType(schema, `#/components/schemas/${name}`), false, docFor(schema)),
);
const componentMembers = [
  member('schemas', { kind: 'object', members: schemaMembers }),
  ...['responses', 'parameters', 'requestBodies', 'headers', 'pathItems'].map((name) => member(name, NEVER)),
];

const block = (members: Member[]) => members.map((item) => printMember(item, 1)).join('');
const declarations = [
  `export interface paths {\n${block(pathMembers)}}`,
  'export type webhooks = Record<string, never>;',
  `export interface components {\n${block(componentMembers)}}`,
  'export type $defs = Record<string, never>;',
  `export interface operations {\n${block(operationMembers)}}`,
].join('\n');
const generated = `/** Generated by contracts/codegen from openapi.json. Do not edit. */\n\n${declarations}\n`;

// ---- Operation index -------------------------------------------------------------------------------------------

function operationIndex(): string {
  const rows: string[] = [];
  for (const [path, item] of Object.entries(contract.paths)) {
    for (const [method, operation] of Object.entries(item)) {
      const scope = path.startsWith('/__mock__/') ? 'Development only' : path === '/health' ? 'Health' : 'Application';
      const success = Object.keys(operation.responses)
        .filter((code) => /^2\d\d$/.test(code))
        .join(', ');
      rows.push(`| \`${operation.operationId}\` | ${method.toUpperCase()} | \`${path}\` | ${success} | ${scope} |`);
    }
  }
  return `# API operation index\n\nGenerated from [\`openapi.json\`](openapi.json) by \`codegen/generate.ts\`. Do not edit the table independently. Contract version: ${contract.info.version}. The exact bodies, error schemas, authorization descriptions and parameters remain in OpenAPI.\n\n| Operation | Method | Path | Success | Scope |\n| --- | --- | --- | --- | --- |\n${rows.join('\n')}\n\nTotal documented operations: ${rows.length}. Development controls are not production application endpoints.\n`;
}

// ---- Output ----------------------------------------------------------------------------------------------------

const sha256 = (value: string) => createHash('sha256').update(value).digest('hex');
const manifest = `${JSON.stringify(
  {
    contract: 'contracts/openapi.json',
    contractVersion: contract.info.version,
    contractSha256: sha256(source),
    generator: 'contracts/codegen',
    binaryRepresentation: 'Blob',
    outputSha256: sha256(generated),
  },
  null,
  2,
)}\n`;

// The shared catalog as constants: ordered ids for route schemas and selects, labels for display.
const catalogUrl = new URL('../../backend/crates/domain/data/catalog.json', import.meta.url);
const catalogSource = JSON.parse(await readFile(catalogUrl, 'utf8')) as {
  version: string;
  grades: { id: string; label: string; description: string }[];
  specializations: { id: string; label: string; short: string; role: string; group: string; testTopics: string[] }[];
  specializationGroups: { id: string; label: string }[];
  skillCategories: { id: string; label: string }[];
  skills: { id: string; label: string; category: string }[];
};
const constant = (name: string, value: unknown) =>
  `export const ${name} = ${JSON.stringify(value, null, 2)} as const;\n`;
const catalogModule = [
  '// Generated from backend/crates/domain/data/catalog.json by contracts/codegen. Do not edit.\n',
  constant('CATALOG_VERSION', catalogSource.version),
  constant('GRADES', catalogSource.grades),
  constant(
    'SPECIALIZATIONS',
    catalogSource.specializations.map(({ id, label, short, role, group, testTopics }) => ({
      id,
      label,
      short,
      role,
      group,
      testTopics,
    })),
  ),
  constant('SPECIALIZATION_GROUPS', catalogSource.specializationGroups),
  constant('SKILL_CATEGORIES', catalogSource.skillCategories),
  constant('SKILLS', catalogSource.skills),
  constant(
    'GRADE_IDS',
    catalogSource.grades.map((item) => item.id),
  ),
  constant(
    'SPECIALIZATION_IDS',
    catalogSource.specializations.map((item) => item.id),
  ),
  constant(
    'SKILL_IDS',
    catalogSource.skills.map((item) => item.id),
  ),
].join('\n');

const outputs: Array<[URL, string]> = [
  [new URL('schema.d.ts', typesUrl), generated],
  [new URL('catalog.ts', typesUrl), catalogModule],
  [new URL('manifest.json', typesUrl), manifest],
  [new URL('schema.d.ts', mockTypesUrl), generated],
  [indexUrl, operationIndex()],
];
const check = process.argv.includes('--check');
for (const [destination, expected] of outputs) {
  if (check) {
    const actual = await readFile(destination, 'utf8').catch(() => '');
    if (actual !== expected) throw new Error(`Generated output is stale: ${destination.pathname}`);
  } else {
    await mkdir(new URL('./', destination), { recursive: true });
    await writeFile(destination, expected);
  }
}
console.log(
  check ? 'Generated files match the source contract.' : 'Generated types, manifest and operation index written.',
);
