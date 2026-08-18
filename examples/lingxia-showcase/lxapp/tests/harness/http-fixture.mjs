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
      // Fixed-size body with a known digest.
      if (route === '/bytes') {
        const size = Number(url.searchParams.get('size') ?? 1024);
        const payload = body(size);
        response.writeHead(200, {
          'content-type': url.searchParams.get('type') ?? 'application/octet-stream',
          'content-length': payload.length,
          etag: `"${digest(size)}"`,
        });
        response.end(payload);
        return;
      }

      // Trickled in chunks so a spec can observe progress, pause, and cancel.
      if (route === '/slow') {
        const size = Number(url.searchParams.get('size') ?? 65536);
        const chunks = Number(url.searchParams.get('chunks') ?? 8);
        const delay = Number(url.searchParams.get('delayMs') ?? 120);
        const payload = body(size);
        const step = Math.ceil(size / chunks);
        response.writeHead(200, {
          'content-type': 'application/octet-stream',
          'content-length': payload.length,
        });
        for (let offset = 0; offset < size; offset += step) {
          if (response.writableEnded) return;
          response.write(payload.subarray(offset, Math.min(offset + step, size)));
          await sleep(delay);
        }
        response.end();
        return;
      }

      // Promises more than it sends, so the client sees a truncated transfer.
      if (route === '/truncated') {
        const size = Number(url.searchParams.get('size') ?? 4096);
        response.writeHead(200, {
          'content-type': 'application/octet-stream',
          'content-length': size,
        });
        response.write(body(Math.floor(size / 2)));
        response.destroy();
        return;
      }

      if (route === '/status') {
        const code = Number(url.searchParams.get('code') ?? 500);
        response.writeHead(code, { 'content-type': 'text/plain' });
        response.end(`status ${code}`);
        return;
      }

      if (route === '/upload' && request.method === 'POST') {
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

      if (route === '/health') {
        json(200, { ok: true });
        return;
      }

      json(404, { error: `no fixture route ${route}` });
    } catch (error) {
      json(500, { error: String(error) });
    }
  });
}

const invokedDirectly = import.meta.url === `file://${process.argv[1]}`;
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
