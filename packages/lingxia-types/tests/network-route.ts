import type {
  LxAppDriver,
  NetworkDriver,
  NetworkRouteHandler,
  NetworkRouteRequest,
  Scenario,
  ScenarioCall,
  ScenarioDefinition,
} from '../src/automation/index.js';

const fulfill: NetworkRouteHandler = { status: 501, json: { error: 'not_implemented' } };
const text: NetworkRouteHandler = { status: 404, body: 'missing', contentType: 'text/plain' };
const bytes: NetworkRouteHandler = { body: new Uint8Array([1, 2]), delay: 250 };
const buffer: NetworkRouteHandler = { body: new ArrayBuffer(2) };
const empty: NetworkRouteHandler = { status: 204 };
const abort: NetworkRouteHandler = { abort: 'failed' };
const pass: NetworkRouteHandler = { continue: true };
const patched: NetworkRouteHandler = { continue: true, patchJson: { total: 0, items: [], cursor: null } };
const hang: NetworkRouteHandler = { hang: true };
void [fulfill, text, bytes, buffer, empty, abort, pass, patched, hang];

// @ts-expect-error a handler cannot both fulfill and abort
const mixed: NetworkRouteHandler = { status: 500, abort: 'failed' };
// @ts-expect-error continue excludes fulfill options
const passWithDelay: NetworkRouteHandler = { continue: true, delay: 10 };
// @ts-expect-error abort and continue are exclusive
const abortAndPass: NetworkRouteHandler = { abort: 'failed', continue: true };
// @ts-expect-error abort takes the closed failure union, not free text
const freeText: NetworkRouteHandler = { abort: 'connection reset' };
// @ts-expect-error `abort: true` is not a failure kind
const abortTrue: NetworkRouteHandler = { abort: true };
// @ts-expect-error JSON goes in `json`, not `body`
const objectBody: NetworkRouteHandler = { body: { error: 'x' } };
// @ts-expect-error body and json are exclusive
const bodyAndJson: NetworkRouteHandler = { body: 'x', json: {} };
// @ts-expect-error patchJson patches a real response, so it needs continue
const patchAlone: NetworkRouteHandler = { patchJson: { a: 1 } };
// @ts-expect-error hang never answers, so it takes no fulfill options
const hangWithStatus: NetworkRouteHandler = { hang: true, status: 200 };
// @ts-expect-error hang and continue are exclusive
const hangAndPass: NetworkRouteHandler = { hang: true, continue: true };
// @ts-expect-error hang is `true`, not a duration
const hangFor: NetworkRouteHandler = { hang: 500 };
void [mixed, passWithDelay, abortAndPass, freeText, abortTrue, objectBody, bodyAndJson, patchAlone, hangWithStatus, hangAndPass, hangFor];

declare const request: NetworkRouteRequest;
const sent: { headers: Record<string, string>; body: string | null; bodyTruncated: boolean } = request;
void sent;

const stream: NetworkRouteHandler = {
  sse: [
    { event: 'ready', data: { n: 1 }, id: '1', retry: 500 },
    { comment: 'keepalive' },
    { delayMs: 200 },
    { data: 'bye' },
    { drop: true },
  ],
  headers: { 'x-trace': 't' },
  delay: 50,
};
const sequence: NetworkRouteHandler = {
  sequence: [{ status: 503 }, { json: { up: true, at: '{{now-5m}}' } }, { abort: 'failed' }, { sse: [{ data: 'x' }] }],
};
void [stream, sequence];
// @ts-expect-error an sse answer has status 200 and takes no fulfill body
const sseWithStatus: NetworkRouteHandler = { sse: [], status: 500 };
// @ts-expect-error sequence items are whole answers
const sequenceWithStatus: NetworkRouteHandler = { sequence: [{ status: 200 }], status: 200 };
// @ts-expect-error sequences do not nest
const nested: NetworkRouteHandler = { sequence: [{ sequence: [] }] };
// @ts-expect-error drop is `true`
const dropFalse: NetworkRouteHandler = { sse: [{ drop: false }] };
void [sseWithStatus, sequenceWithStatus, nested, dropFalse];

const outage: ScenarioDefinition = {
  name: 'outage',
  rules: [
    { http: 'GET **/v1/status', times: 2, sequence: [{ status: 503 }, { json: { up: true } }] },
    { http: '* /devices\\/\\w+$/i', status: 404, note: 'gone' },
    { http: 'GET **/icon.png', bodyBase64: 'iVBORw==', contentType: 'image/png' },
    { http: 'GET **/events', sse: [{ data: 'hello' }] },
    { http: 'PATCH **/devices/*', match: { json: { name: 'Office' } }, status: 409, json: { error: 'conflict' } },
    { function: 'coupons.apply', match: { args: { code: 'SPRING' } }, error: { code: 'COUPON_EXPIRED' } },
    { function: 'orders.submit', fault: 'unknown', delay: 200 },
    { function: 'orders.status', sequence: [{ result: { state: 'pending' } }, { result: { state: 'paid' } }] },
  ],
  variants: {
    offline: { description: 'the status API is down', rules: [{ http: 'GET **/v1/status', status: 503 }] },
  },
};
// A JSON import widens literals ('failed' becomes string); scenario() takes it as is.
const imported = { name: 'from-json', rules: [{ http: 'GET **/a', abort: 'failed' as string }] };
const importedVariants = { variants: { a: { rules: [{ http: 'GET **/a', status: 503 }] } } };
declare const app: LxAppDriver;
const handles: Promise<Scenario>[] = [
  app.scenario(outage),
  app.scenario(outage, 'offline'),
  app.scenario(imported),
  app.scenario(importedVariants, 'a'),
];
void handles;
// @ts-expect-error a rule targets http or a function
const noTarget: ScenarioDefinition = { rules: [{ status: 200 }] };
// @ts-expect-error not both
const both: ScenarioDefinition = { rules: [{ http: 'GET x', function: 'f', status: 200 }] };
// @ts-expect-error a function rule answers with result, error or fault
const httpAnswer: ScenarioDefinition = { rules: [{ function: 'f', status: 200 }] };
// @ts-expect-error an http rule matches its body with match.json
const argsOnHttp: ScenarioDefinition = { rules: [{ http: 'GET x', match: { args: {} }, status: 200 }] };
// @ts-expect-error faults are notRun or unknown
const badFault: ScenarioDefinition = { rules: [{ function: 'f', fault: 'timeout' }] };
// @ts-expect-error an error needs a code
const noCode: ScenarioDefinition = { rules: [{ function: 'f', error: {} }] };
void [noTarget, both, httpAnswer, argsOnHttp, badFault, noCode];
// The raw network driver has no scenario() any more.
declare const network: NetworkDriver;
// @ts-expect-error use lxapp().scenario()
void network.scenario;
declare const handle: Scenario;
const name: string | null = handle.name;
const variant: string | null = handle.variant;
const count: Promise<number> = handle.unroute();
const calls: Promise<ScenarioCall[]> = handle.calls({ function: 'orders.submit' });
void [name, variant, count, calls, handle.calls({ http: 'PATCH **/devices/*' }), handle.calls({ rule: 2 }), handle.rules[0].hits];
