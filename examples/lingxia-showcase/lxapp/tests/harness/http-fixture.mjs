// A dependency-free HTTP server the transfer specs run against.
//
// `lx.downloadFile` and `lx.uploadFile` cannot be tested against the public
// internet without importing its flakiness and its outages, and neither can
// their failure contracts — a 500, a truncated body, a slow stream — be
// produced on demand. This serves all of it deterministically.
//
// Start it before `lxdev test` and pass the base URL through:
//   node tests/harness/http-fixture.mjs --port 0 --print-base
//   lxdev test tests/ --arg httpBase=http://127.0.0.1:<port>
import { createServer } from 'node:http';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';

/** Deterministic bytes, so a spec can assert an exact size and digest. */
function body(size) {
  const buffer = Buffer.allocUnsafe(size);
  for (let i = 0; i < size; i += 1) buffer[i] = i % 251;
  return buffer;
}

export function digest(size) {
  return createHash('sha256').update(body(size)).digest('hex');
}

/** A route param a spec can get wrong; `NaN` bytes crashes the allocator. */
function size(url, key, fallback) {
  const raw = Number(url.searchParams.get(key) ?? fallback);
  if (!Number.isFinite(raw) || raw < 0) return fallback;
  return Math.min(Math.floor(raw), 64 * 1024 * 1024);
}

function readAll(request) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    request.on('data', (chunk) => chunks.push(chunk));
    request.on('end', () => resolve(Buffer.concat(chunks)));
    request.on('error', reject);
  });
}

/** Field names and the file part, without pulling in a multipart parser. */
function parseMultipart(buffer, contentType) {
  const match = /boundary=(?:"([^"]+)"|([^;]+))/i.exec(contentType ?? '');
  if (!match) return null;
  const boundary = `--${match[1] ?? match[2]}`;
  const parts = [];
  for (const raw of buffer.toString('latin1').split(boundary)) {
    const split = raw.indexOf('\r\n\r\n');
    if (split === -1) continue;
    const headers = raw.slice(0, split);
    const name = /name="([^"]*)"/i.exec(headers)?.[1];
    if (!name) continue;
    const content = raw.slice(split + 4).replace(/\r\n$/, '');
    parts.push({
      name,
      filename: /filename="([^"]*)"/i.exec(headers)?.[1],
      type: /content-type:\s*([^\r\n]+)/i.exec(headers)?.[1],
      bytes: Buffer.from(content, 'latin1').length,
      text: content,
    });
  }
  return parts;
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export function createFixtureServer() {
  return createServer(async (request, response) => {
    const url = new URL(request.url, 'http://127.0.0.1');
    const route = url.pathname;
    const json = (status, value) => {
      const text = JSON.stringify(value);
      response.writeHead(status, {
        'content-type': 'application/json',
        'content-length': Buffer.byteLength(text),
      });
      response.end(text);
    };

    try {
      // Same bytes, but with the extension in the path. A download client that
      // names its output from the URL needs one, and `/bytes` deliberately has
      // none so a spec can cover both.
      if (route.startsWith('/file/')) {
        const bytes = size(url, 'size', 1024);
        const payload = body(bytes);
        response.writeHead(200, {
          'content-type': url.searchParams.get('type') ?? 'application/octet-stream',
          'content-length': payload.length,
          etag: `"${digest(bytes)}"`,
        });
        response.end(payload);
        return;
      }

      // Fixed-size body with a known digest.
      if (route === '/bytes') {
        const bytes = size(url, 'size', 1024);
        const payload = body(bytes);
        response.writeHead(200, {
          'content-type': url.searchParams.get('type') ?? 'application/octet-stream',
          'content-length': payload.length,
          etag: `"${digest(bytes)}"`,
        });
        response.end(payload);
        return;
      }

      // Trickled in chunks so a spec can observe progress, pause, and cancel.
      if (route === '/slow') {
        const bytes = size(url, 'size', 65536);
        const chunks = Math.max(1, size(url, 'chunks', 8));
        const delay = size(url, 'delayMs', 120);
        const payload = body(bytes);
        const step = Math.ceil(bytes / chunks);
        response.writeHead(200, {
          'content-type': 'application/octet-stream',
          'content-length': payload.length,
        });
        for (let offset = 0; offset < bytes; offset += step) {
          // `writableEnded` only flips after end(); a client that aborts
          // mid-stream sets `destroyed`, and without this the handler keeps
          // sleeping through every remaining chunk — which is precisely the
          // cancel case this route exists to serve.
          if (response.destroyed) return;
          response.write(payload.subarray(offset, Math.min(offset + step, bytes)));
          await sleep(delay);
        }
        response.end();
        return;
      }

      // Promises more than it sends, so the client sees a truncated transfer.
      if (route === '/truncated') {
        const bytes = size(url, 'size', 4096);
        response.writeHead(200, {
          'content-type': 'application/octet-stream',
          'content-length': bytes,
        });
        response.write(body(Math.floor(bytes / 2)));
        response.destroy();
        return;
      }

      if (route === '/status') {
        const raw = Number(url.searchParams.get('code') ?? 500);
        const code = Number.isInteger(raw) && raw >= 100 && raw <= 599 ? raw : 500;
        response.writeHead(code, { 'content-type': 'text/plain' });
        response.end(`status ${code}`);
        return;
      }

      if (route === '/upload' && request.method === 'POST') {
        // Loopback delivers a few MB faster than any client can cancel, so a
        // cancel spec needs the server to hold the request open.
        const holdMs = size(url, 'holdMs', 0);
        if (holdMs > 0) {
          request.pause();
          await sleep(holdMs);
          if (request.destroyed) return;
          request.resume();
        }
        const buffer = await readAll(request);
        const parts = parseMultipart(buffer, request.headers['content-type']);
        if (!parts) {
          json(415, { error: 'expected multipart/form-data' });
          return;
        }
        const file = parts.find((part) => part.filename !== undefined);
        json(200, {
          ok: true,
          received: buffer.length,
          file: file
            ? { field: file.name, filename: file.filename, type: file.type, bytes: file.bytes }
            : null,
          fields: Object.fromEntries(
            parts.filter((part) => part.filename === undefined).map((part) => [part.name, part.text]),
          ),
          headerEcho: request.headers['x-lx-test'] ?? null,
        });
        return;
      }

      if (route === '/upload-raw' && (request.method === 'PUT' || request.method === 'PATCH')) {
        // Answer and hang up without draining the body, the way object storage
        // refuses a signature it disagrees with mid-upload.
        const reject = Number(url.searchParams.get('reject') ?? 0);
        if (Number.isInteger(reject) && reject >= 400 && reject <= 599) {
          response.writeHead(reject, { 'content-type': 'text/plain' });
          response.end(`refused ${reject}`);
          request.socket.destroy();
          return;
        }
        // A presigned-style endpoint: the body is the file, nothing else.
        const buffer = await readAll(request);
        json(200, {
          ok: true,
          method: request.method,
          received: buffer.length,
          contentType: request.headers['content-type'] ?? null,
          contentLength: Number(request.headers['content-length'] ?? -1),
          firstBytes: buffer.subarray(0, 8).toString('hex'),
          headerEcho: request.headers['x-lx-test'] ?? null,
        });
        return;
      }

      if (route === '/health') {
        json(200, { ok: true });
        return;
      }

      json(404, { error: `no fixture route ${route}` });
    } catch (error) {
      // Once headers are out, writeHead throws inside this catch and the
      // rejection is unhandled — which kills the process and takes every
      // remaining spec with it, since one fixture serves the whole suite.
      if (response.headersSent) {
        response.destroy();
        return;
      }
      json(500, { error: String(error) });
    }
  });
}

// argv[1] is a raw path; import.meta.url is percent-encoded.
const invokedDirectly =
  process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href;
if (invokedDirectly) {
  const portFlag = process.argv.indexOf('--port');
  const port = portFlag === -1 ? 0 : Number(process.argv[portFlag + 1]);
  const server = createFixtureServer();
  server.listen(port, '127.0.0.1', () => {
    const base = `http://127.0.0.1:${server.address().port}`;
    process.stdout.write(`${base}\n`);
  });
  for (const signal of ['SIGINT', 'SIGTERM']) {
    process.on(signal, () => server.close(() => process.exit(0)));
  }
}
