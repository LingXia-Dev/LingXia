import type { NetworkRouteHandler, NetworkRouteRequest } from '../src/automation/index.js';

const fulfill: NetworkRouteHandler = { status: 501, json: { error: 'not_implemented' } };
const text: NetworkRouteHandler = { status: 404, body: 'missing', contentType: 'text/plain' };
const bytes: NetworkRouteHandler = { body: new Uint8Array([1, 2]), delay: 250 };
const buffer: NetworkRouteHandler = { body: new ArrayBuffer(2) };
const empty: NetworkRouteHandler = { status: 204 };
const abort: NetworkRouteHandler = { abort: 'failed' };
const pass: NetworkRouteHandler = { continue: true };
void [fulfill, text, bytes, buffer, empty, abort, pass];

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
void [mixed, passWithDelay, abortAndPass, freeText, abortTrue, objectBody, bodyAndJson];

declare const request: NetworkRouteRequest;
const sent: { headers: Record<string, string>; body: string | null; bodyTruncated: boolean } = request;
void sent;
