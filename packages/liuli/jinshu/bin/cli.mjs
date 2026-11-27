// `jinshu check|outline|diff|apply`: the schema's own readers, checks and writers, for scripts and CI.
import { readFileSync, writeFileSync } from 'node:fs';
import { extname } from 'node:path';
import {
  JINSHU_DOC_VERSION,
  applyOps,
  checkDocument,
  diffDocuments,
  documentToMarkdown,
  extractOutline,
  markdownToDocument,
  normalizeDocument,
  parseAgentMarkup,
  plainText,
  toAgentView,
} from '../src/schema/index.ts';

const USAGE = `Usage: jinshu <command> [options]

  check <file>                   Validate a document; exit 1 on errors
  outline <file>                 List its headings
  diff <a> <b>                   Block-level changes from a to b; exit 1 when they differ
  apply <file> <ops.json|->      Apply an op batch atomically; exit 1 if refused
      -o, --out <file>           Write the result there (format by its extension)
      -w, --write                Write the result back to <file>
                                 (default: print it in <file>'s format)

Options:
  --json                         Machine-readable output
  --format json|markdown|markup  Read the file in this format, whatever its extension
  --profile markdown             Hold a JSON or markup document to what Markdown can say
  -h, --help                     Show this help

Formats by extension: .md/.markdown Markdown, .json Jinshu JSON, .xml/.jinshu Jinshu Markup.
Exit codes: 0 ok, 1 problems (or differences, for diff), 2 usage or I/O error.`;

class UsageError extends Error {}

const FORMATS = { '.md': 'markdown', '.markdown': 'markdown', '.json': 'json', '.xml': 'markup', '.jinshu': 'markup' };

function formatOf(path, override) {
  if (override) {
    if (!['json', 'markdown', 'markup'].includes(override)) throw new UsageError(`Unknown format "${override}".`);
    return override;
  }
  const format = FORMATS[extname(path).toLowerCase()];
  if (!format) throw new UsageError(`Cannot tell the format of "${path}"; use .md, .json or .xml, or --format.`);
  return format;
}

function readText(path, stdin) {
  if (path === '-') return stdin();
  try {
    return readFileSync(path, 'utf8');
  } catch (err) {
    throw new UsageError(`Cannot read "${path}": ${err.code ?? err.message}`);
  }
}

/** Reads a document; `problems` are what reading and checking found. `previous` keeps Markdown ids stable. */
function readDocument(path, format, io, previous, profile) {
  const text = readText(path, io.stdin);
  const checkOptions = profile === 'markdown' ? { format: 'markdown' } : undefined;
  if (format === 'markdown') {
    const read = markdownToDocument(text, previous ? { previous } : undefined);
    const problems = [...read.problems];
    for (const p of checkDocument(read.doc, undefined, { format: 'markdown' })) {
      if (!problems.some(q => q.message === p.message && q.id === p.id)) problems.push(p);
    }
    return { doc: read.doc, problems, source: read.source };
  }
  if (format === 'markup') {
    const parsed = parseAgentMarkup(text);
    const doc = {
      version: parsed.version ?? JINSHU_DOC_VERSION,
      ...(parsed.id !== undefined ? { id: parsed.id } : {}),
      ...(parsed.title !== undefined ? { title: parsed.title } : {}),
      ...(parsed.lang !== undefined ? { lang: parsed.lang } : {}),
      ...(parsed.meta !== undefined ? { meta: parsed.meta } : {}),
      blocks: [...parsed.blocks],
    };
    return { doc, problems: [...parsed.problems, ...checkDocument(doc, undefined, checkOptions)] };
  }
  let doc;
  try {
    doc = JSON.parse(text);
  } catch (err) {
    return { doc: undefined, problems: [{ code: 'PARSE', severity: 'error', message: `Not JSON: ${err.message}` }] };
  }
  return { doc, problems: checkDocument(doc, undefined, checkOptions) };
}

function serialize(doc, format, source) {
  if (format === 'markdown') return documentToMarkdown(doc, source ? { source } : undefined);
  if (format === 'markup') return { text: `${toAgentView(normalizeDocument(doc))}\n`, problems: [] };
  return { text: `${JSON.stringify(normalizeDocument(doc), null, 2)}\n`, problems: [] };
}

const hasErrors = problems => problems.some(p => p.severity === 'error');

function problemLine(file, p) {
  const where = [p.opIndex !== undefined ? `op ${p.opIndex}` : '', p.id ? `#${p.id}` : ''].filter(Boolean).join(' ');
  return `${file}: ${p.severity} ${p.code}${where ? ` ${where}` : ''}: ${p.message}`;
}

function printProblems(io, file, problems) {
  for (const p of problems) io.err(problemLine(file, p));
}

function parseArgs(argv) {
  const opts = { json: false, write: false, out: undefined, format: undefined, profile: undefined, help: false };
  const positional = [];
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const value = () => {
      if (i + 1 >= argv.length) throw new UsageError(`${arg} needs a value.`);
      return argv[++i];
    };
    if (arg === '--json') opts.json = true;
    else if (arg === '-w' || arg === '--write') opts.write = true;
    else if (arg === '-o' || arg === '--out') opts.out = value();
    else if (arg === '--format') opts.format = value();
    else if (arg === '--profile') {
      opts.profile = value();
      if (opts.profile !== 'markdown') throw new UsageError(`Unknown profile "${opts.profile}"; the only one is markdown.`);
    }
    else if (arg === '-h' || arg === '--help') opts.help = true;
    else if (arg.startsWith('-') && arg !== '-') throw new UsageError(`Unknown option ${arg}.`);
    else positional.push(arg);
  }
  return { opts, positional };
}

function arity(positional, n, command) {
  if (positional.length !== n) throw new UsageError(`${command} takes ${n} argument${n === 1 ? '' : 's'}.`);
}

function check([file], opts, io) {
  const { problems } = readDocument(file, formatOf(file, opts.format), io, undefined, opts.profile);
  const ok = !hasErrors(problems);
  if (opts.json) io.out(JSON.stringify({ file, ok, problems }, null, 2));
  else {
    printProblems(io, file, problems);
    const errors = problems.filter(p => p.severity === 'error').length;
    io.out(`${file}: ${ok ? 'ok' : 'invalid'} (${errors} error${errors === 1 ? '' : 's'}, ${problems.length - errors} warning${problems.length - errors === 1 ? '' : 's'})`);
  }
  return ok ? 0 : 1;
}

function outline([file], opts, io) {
  const { doc, problems } = readDocument(file, formatOf(file, opts.format), io);
  if (!doc || hasErrors(problems)) {
    printProblems(io, file, problems);
    return 1;
  }
  const items = extractOutline(doc);
  if (opts.json) io.out(JSON.stringify({ title: doc.title, headings: items }, null, 2));
  else {
    if (doc.title) io.out(doc.title);
    for (const h of items) io.out(`${'  '.repeat(h.level - 1)}- ${h.text}  #${h.id}`);
  }
  return 0;
}

const clip = (text, n = 60) => (text.length > n ? `${text.slice(0, n - 1)}…` : text);

/** One line of words for a block, for the text summary. */
function blockText(block) {
  if (Array.isArray(block.runs)) return plainText(block.runs);
  if (typeof block.code === 'string') return block.code.split('\n')[0];
  if (typeof block.latex === 'string') return block.latex;
  if (typeof block.src === 'string') return block.src;
  return '';
}

function wordDiffText(segments) {
  return segments.map(s => (s.kind === 'equal' ? s.text : s.kind === 'insert' ? `{+${s.text}+}` : `[-${s.text}-]`)).join('');
}

function entryLine(e) {
  switch (e.kind) {
    case 'added':
      return `+ ${e.block.type} #${e.id} at ${e.toIndex}: ${clip(blockText(e.block))}`;
    case 'removed':
      return `- ${e.block.type} #${e.id} from ${e.fromIndex}: ${clip(blockText(e.block))}`;
    case 'moved':
      return `> ${e.block.type} #${e.id} ${e.fromIndex} -> ${e.toIndex}${e.changed ? ' (changed)' : ''}`;
    case 'modified': {
      const type = e.typeChanged ? `${e.before.type} -> ${e.after.type}` : e.after.type;
      const parts = [];
      if (e.wordDiff && !e.fullBlockReplace) parts.push(clip(wordDiffText(e.wordDiff), 120));
      else if (e.fullBlockReplace) parts.push('replaced');
      if (e.marksOnly) parts.push('marks only');
      if (e.attrChanges.length) parts.push(`fields: ${e.attrChanges.map(c => c.key).join(', ')}`);
      if (e.moved) parts.push(`moved ${e.moved.fromIndex} -> ${e.moved.toIndex}`);
      return `~ ${type} #${e.id}: ${parts.join('; ')}`;
    }
    default:
      return null;
  }
}

function diff([fileA, fileB], opts, io) {
  const a = readDocument(fileA, formatOf(fileA, opts.format), io);
  const b = readDocument(fileB, formatOf(fileB, opts.format), io, a.doc);
  if (!a.doc || !b.doc || hasErrors(a.problems) || hasErrors(b.problems)) {
    printProblems(io, fileA, a.problems.filter(p => p.severity === 'error'));
    printProblems(io, fileB, b.problems.filter(p => p.severity === 'error'));
    return 2;
  }
  const result = diffDocuments(normalizeDocument(a.doc), normalizeDocument(b.doc));
  const changed = result.stats.added + result.stats.removed + result.stats.modified + result.stats.moved > 0;
  if (opts.json) {
    const entries = result.entries.filter(e => e.kind !== 'unchanged');
    io.out(JSON.stringify({ a: fileA, b: fileB, stats: result.stats, entries }, null, 2));
  } else {
    for (const e of result.entries) {
      const line = entryLine(e);
      if (line) io.out(line);
    }
    const s = result.stats;
    io.out(`${s.added} added, ${s.removed} removed, ${s.modified} modified, ${s.moved} moved, ${s.unchanged} unchanged`);
  }
  return changed ? 1 : 0;
}

function readOps(path, io) {
  let parsed;
  try {
    parsed = JSON.parse(readText(path, io.stdin));
  } catch (err) {
    if (err instanceof UsageError) throw err;
    throw new UsageError(`Ops "${path}" are not JSON: ${err.message}`);
  }
  // A bare array, or a batch object with `ops`.
  const ops = Array.isArray(parsed) ? parsed : parsed?.ops;
  if (!Array.isArray(ops)) throw new UsageError('Ops must be a JSON array of ops, or an object with an "ops" array.');
  return ops;
}

function apply([file, opsPath], opts, io) {
  if (opts.write && opts.out) throw new UsageError('Use --write or --out, not both.');
  if (file === '-' && opsPath === '-') throw new UsageError('Only one of <file> and <ops> can be stdin.');
  const format = formatOf(file, opts.format);
  const ops = readOps(opsPath, io);
  const read = readDocument(file, format, io, undefined, opts.profile);
  if (!read.doc || hasErrors(read.problems)) {
    printProblems(io, file, read.problems);
    return 1;
  }
  const markdown = format === 'markdown' || opts.profile === 'markdown';
  const result = applyOps(read.doc, ops, undefined, markdown ? { format: 'markdown' } : undefined);
  const refused = !result.ops;
  const target = opts.write ? file : opts.out;
  let written;
  if (!refused) {
    const outFormat = target && target !== '-' && target !== file ? formatOf(target) : format;
    written = serialize(result.doc, outFormat, outFormat === format ? read.source : undefined);
  }
  const problems = [...result.problems, ...(written?.problems ?? [])];
  const failed = refused || hasErrors(problems);
  if (opts.json) {
    io.out(JSON.stringify({ file, ok: !failed, problems, ...(failed || target ? {} : { text: written.text }) }, null, 2));
  } else {
    printProblems(io, file, problems);
  }
  if (failed) {
    if (!opts.json) io.err(`${file}: refused; nothing written`);
    return 1;
  }
  if (target && target !== '-') {
    try {
      writeFileSync(target, written.text);
    } catch (err) {
      throw new UsageError(`Cannot write "${target}": ${err.code ?? err.message}`);
    }
  } else if (!opts.json) {
    io.write(written.text);
  }
  return 0;
}

const COMMANDS = { check: [check, 1], outline: [outline, 1], diff: [diff, 2], apply: [apply, 2] };

const defaultIo = {
  out: line => process.stdout.write(`${line}\n`),
  err: line => process.stderr.write(`${line}\n`),
  write: text => process.stdout.write(text),
  stdin: () => readFileSync(0, 'utf8'),
};

/** Runs one command; resolves to the exit code. */
export async function main(argv, io = defaultIo) {
  try {
    const [command, ...rest] = argv;
    const { opts, positional } = parseArgs(rest);
    if (!command || command === '-h' || command === '--help' || command === 'help' || opts.help) {
      io.out(USAGE);
      return command ? 0 : 2;
    }
    const entry = COMMANDS[command];
    if (!entry) throw new UsageError(`Unknown command "${command}".`);
    arity(positional, entry[1], command);
    return entry[0](positional, opts, io);
  } catch (err) {
    if (!(err instanceof UsageError)) throw err;
    io.err(`jinshu: ${err.message}`);
    io.err('Run "jinshu --help" for usage.');
    return 2;
  }
}
