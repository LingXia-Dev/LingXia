#!/usr/bin/env node
// Liuli's visual baseline: every gallery page on the macOS Runner at fixed sizes, light and dark,
// compared with pictures a person accepted.
//
//   node scripts/visual-baseline.mjs capture   run the capture spec per preset into gallery/.baseline/current
//   node scripts/visual-baseline.mjs check     compare current with reference; exit 1 past the threshold
//   node scripts/visual-baseline.mjs accept    make current the reference and rewrite the manifest;
//                                              it needs a second capture, `--into confirm`, that agrees
//
//   --into <name>       capture into gallery/.baseline/<name> instead of current
//   --against <name>    check current (or --into) against gallery/.baseline/<name>, a capture kept
//                       aside, with no manifest: a before and an after of one change
//   --tolerance <n>, --share <x>   with --against: a finer or coarser comparison than the gate's
//   --gallery <dir>     another copy of the gallery, such as one scripts/gallery-with-pack.mjs wrote
//   --pack <name>       with --gallery: the copy is that pack's; its pictures go under
//                       gallery/.baseline/<name> and its manifest is gallery/baseline.<name>.manifest.json
//   --presets <a,b>     only these size presets, for a capture and what is accepted from it
//   --pages <a,b>       only these gallery pages, by name: a capture shoots them, a check compares
//                       them, and an accept replaces them in the reference and keeps every other picture
//   --except <a,b>      every gallery page but these, as if the rest were named with --pages
//   --confirm <name>    with accept: the second capture to hold current against (default confirm)
//   --changed <rev>     only the pages the changes since <rev> (or in <a>..<b>) reach through their imports
//                       (scripts/reach.mjs); a change it cannot place, such as a theme or a token, names every page
//   --differing         with capture: only the pages whose pictures in current differ from the reference, so
//                       `capture --into confirm --differing` re-shoots what moved; with accept: a picture that
//                       matches its reference counts as steady and keeps the reference
//
// The pictures stay out of git (gallery/.baseline); gallery/baseline.manifest.json, which is
// committed, records what was accepted: each picture's size and hash, and the host that drew it.
// `capture` needs a Runner session (`LXDEV_SESSION`, by default the macOS one) with the gallery
// open, and takes the window for a few minutes: it sets the Runner's size preset and puts it back.
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { changedSince, pagesReaching } from './reach.mjs';
import { deflateSync, inflateSync } from 'node:zlib';

const liuli = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const option = name => { const at = process.argv.indexOf(`--${name}`); return at > 0 ? process.argv[at + 1] : undefined; };
const gallery = resolve(option('gallery') ?? join(liuli, 'gallery'));
const into = option('into') ?? 'current';
// A pack's gallery is a copy outside the repository; its pictures and manifest stay with Liuli's gallery.
const pack = option('pack');
if (pack && !option('gallery')) throw Error('--pack needs --gallery: the copy scripts/gallery-with-pack.mjs wrote');
const root = pack ? join(liuli, 'gallery', '.baseline', pack) : join(gallery, '.baseline');
const manifestPath = pack ? join(liuli, 'gallery', `baseline.${pack}.manifest.json`) : join(gallery, 'baseline.manifest.json');
// A phone and a desk: the two forms every page has.
const presets = option('presets')?.split(',') ?? ['iphone-15-pro', 'desktop-1280'];
// `--except` names the pages left out, so every other page is named.
const except = option('except')?.split(',');
let named = option('pages')?.split(',')
  ?? (except && JSON.parse(readFileSync(join(gallery, 'lxapp.json'), 'utf8')).pages.map(page => page.name).filter(page => !except.includes(page)));
if (option('changed')) {
  const { pages, why } = pagesReaching(changedSince(option('changed')), gallery);
  console.log(why ? `every page: ${why}` : `${pages.length} pages reached: ${pages.join(', ')}`);
  named = why ? named : pages.filter(page => !named || named.includes(page));
}
const differingOnly = process.argv.includes('--differing');
// A picture is `<page>-<theme>.png` under its preset.
const ofNamed = name => !named || named.some(page => new RegExp(`/${page}-(light|dark)\\.png$`).test(name));
// A pixel differs when any channel is further than this from the reference; a picture fails when
// more than this share of its pixels differ. Text edges move by a shade between runs; layout does not.
const channelTolerance = 12;
const failShare = 0.001;

const lxdev = process.env.LXDEV ?? 'lxdev';
const session = process.env.LXDEV_SESSION ?? 'macos';
const run = (args, options = {}) => execFileSync(lxdev, ['--session', session, ...args], { cwd: gallery, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'], ...options });

// A picture of current that differs from its accepted reference past the gate's limits, or has none.
function moved(name) {
  const accepted = existsSync(manifestPath) ? JSON.parse(readFileSync(manifestPath, 'utf8')).pictures[name] : undefined;
  const referencePath = join(root, 'reference', name);
  if (!accepted || !existsSync(referencePath)) return true;
  if (hash(join(root, 'current', name)) === accepted.sha256) return false;
  return differing(decode(readFileSync(referencePath)), decode(readFileSync(join(root, 'current', name))), channelTolerance) > failShare;
}

function capture() {
  if (differingOnly) {
    if (into === 'current') throw Error('--differing re-shoots what current shows moved: give --into confirm');
    const pages = [...new Set(pictures('current').filter(moved).map(name => /\/(.+)-(light|dark)\.png$/.exec(name)[1]))];
    console.log(pages.length ? `${pages.length} pages moved: ${pages.join(', ')}` : 'nothing moved: no second capture needed');
    rmSync(join(root, into), { recursive: true, force: true });
    if (!pages.length) return;
    named = pages;
  }
  const appId = JSON.parse(readFileSync(join(gallery, 'lxapp.json'), 'utf8')).appId;
  const before = run(['runner', 'get']).trim();
  const [, previous] = /\(([\w-]+)\)/.exec(before) ?? [];
  const wasLandscape = /landscape/.test(before);
  rmSync(join(root, into), { recursive: true, force: true });
  const started = Date.now();
  // A host that has lost its GPU draws on the CPU renderer, whose pictures are not the device's: none of them is kept.
  const onGpu = () => {
    const fallen = run(['logs', '--grep', 'CPU renderer', '--limit', '20', '--jsonl']).split('\n').filter(Boolean).some(line => JSON.parse(line).timestamp_ms >= started);
    if (fallen) { rmSync(join(root, into), { recursive: true, force: true }); throw Error('the host fell back to the CPU renderer during the capture; nothing was captured'); }
  };
  try {
    for (const preset of presets) {
      run(['runner', 'set', '--id', preset, preset.startsWith('desktop') ? '--landscape' : '--portrait']);
      const out = join(root, into, preset);
      mkdirSync(out, { recursive: true });
      // Fresh pages: one left scrolled by an earlier run or by a person would be shot where it was left.
      run(['lxapp', 'restart', '--app', appId]);
      // A run that did not start or did not pass leaves the last run's pictures behind: copy nothing from it.
      const only = named ? ['--grep', `^baseline: (${named.join('|')})$`] : [];
      try { run(['test', 'zz-tests/baseline.test.ts', ...only], { stdio: 'inherit' }); } catch { onGpu(); throw Error(`${preset}: the capture spec did not run or did not pass; nothing was captured`); }
      onGpu();
      const attachments = join(gallery, 'test-results', 'latest', 'attachments');
      for (const file of walk(attachments)) if (file.endsWith('.png') && !file.endsWith('failure.png')) cpSync(file, join(out, file.slice(file.lastIndexOf('/') + 1)));
      console.log(`${preset}: ${readdirSync(out).length} pictures`);
    }
  } finally {
    if (previous) run(['runner', 'set', '--id', previous, wasLandscape ? '--landscape' : '--portrait']);
  }
}

function* walk(dir) {
  if (!existsSync(dir)) return;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* walk(path); else yield path;
  }
}

const pictures = which => presets.flatMap(preset => (existsSync(join(root, which, preset)) ? readdirSync(join(root, which, preset)).filter(name => name.endsWith('.png')).sort().map(name => `${preset}/${name}`) : [])).filter(ofNamed);
const hash = path => createHash('sha256').update(readFileSync(path)).digest('hex');

// The share of two pictures' pixels that differ by more than `tolerance` in a channel; 1 when their sizes differ.
function differing(a, b, tolerance, mark) {
  if (a.width !== b.width || a.height !== b.height) return 1;
  let count = 0;
  for (let i = 0; i < a.data.length; i += 4) {
    const off = Math.max(Math.abs(a.data[i] - b.data[i]), Math.abs(a.data[i + 1] - b.data[i + 1]), Math.abs(a.data[i + 2] - b.data[i + 2]));
    if (off > tolerance) { count += 1; mark?.(i); }
  }
  return count / (a.width * a.height);
}

function accept() {
  const current = pictures(into);
  if (!current.length) throw Error('nothing captured: run `capture` first');
  // A reference is a picture the Runner draws every time. One capture cannot show that, so a second
  // one, taken afterwards, has to agree with it within the gate's own limits.
  const confirm = option('confirm') ?? 'confirm';
  const again = new Set(pictures(confirm));
  if (!again.size && !differingOnly) throw Error(`no second capture: run \`capture --into ${confirm}\` and accept again`);
  // With `--differing` only what moved was shot again; a picture that matches its reference was steady when accepted.
  const steady = new Set(differingOnly ? current.filter(name => !again.has(name) && !moved(name)) : []);
  const unsteady = current.filter(name => {
    if (steady.has(name)) return false;
    if (!again.has(name)) return true;
    if (hash(join(root, into, name)) === hash(join(root, confirm, name))) return false;
    return differing(decode(readFileSync(join(root, into, name))), decode(readFileSync(join(root, confirm, name))), channelTolerance) > failShare;
  });
  if (unsteady.length) {
    console.error(unsteady.map(name => `${name}: differs between the two captures`).join('\n'));
    console.error(`\n${unsteady.length} of ${current.length} pictures are not drawn the same twice; nothing accepted`);
    process.exit(1);
  }
  // Named pages replace their own pictures; every other accepted picture stays as it was.
  const kept = named || steady.size ? JSON.parse(readFileSync(manifestPath, 'utf8')) : null;
  if (!kept) rmSync(join(root, 'reference'), { recursive: true, force: true });
  const replaced = current.filter(name => !steady.has(name));
  for (const name of replaced) { mkdirSync(dirname(join(root, 'reference', name)), { recursive: true }); cpSync(join(root, into, name), join(root, 'reference', name)); }
  const fresh = Object.fromEntries(replaced.map(name => {
    const { width, height } = decode(readFileSync(join(root, 'reference', name)));
    return [name, { width, height, sha256: hash(join(root, 'reference', name)) }];
  }));
  // In the manifest's own order, so an accept of a few pages changes only their lines.
  const pictureSet = kept ? { ...Object.fromEntries(Object.entries(kept.pictures).filter(([name]) => !ofNamed(name) || fresh[name] || steady.has(name)).map(([name, entry]) => [name, fresh[name] ?? entry])), ...fresh } : fresh;
  const manifest = {
    accepted: new Date().toISOString().slice(0, 10),
    presets: kept?.presets ?? presets,
    channelTolerance,
    failShare,
    pictures: pictureSet,
  };
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`accepted ${replaced.length} pictures, each drawn the same in two captures${steady.size ? `; ${steady.size} matched their reference and kept it` : ''}`);
}

function check() {
  const against = option('against');
  if (!against && !existsSync(manifestPath)) throw Error('no baseline.manifest.json: capture, look, then `accept`');
  const reference = against ?? 'reference';
  // Against a capture kept aside there is no manifest: every picture in it counts, by its own hash.
  const manifest = against
    ? { channelTolerance: Number(option('tolerance') ?? channelTolerance), failShare: Number(option('share') ?? failShare), pictures: Object.fromEntries(pictures(against).map(name => [name, { sha256: hash(join(root, against, name)) }])) }
    : JSON.parse(readFileSync(manifestPath, 'utf8'));
  const accepted = Object.keys(manifest.pictures).filter(name => presets.includes(name.split('/')[0]) && ofNamed(name));
  const current = new Set(pictures(into));
  if (!current.size) throw Error('nothing captured: run `capture` first');
  rmSync(join(root, 'diff'), { recursive: true, force: true });
  const problems = [];
  for (const name of accepted) {
    const referencePath = join(root, reference, name);
    if (!current.has(name)) { problems.push(`${name}: not captured`); continue; }
    if (!existsSync(referencePath)) { problems.push(`${name}: no reference picture on this machine; \`accept\` after looking, or copy gallery/.baseline/reference`); continue; }
    if (hash(referencePath) !== manifest.pictures[name].sha256) { problems.push(`${name}: the reference picture is not the one the manifest accepted`); continue; }
    if (hash(join(root, into, name)) === manifest.pictures[name].sha256) continue;
    const [a, b] = [decode(readFileSync(referencePath)), decode(readFileSync(join(root, into, name)))];
    if (a.width !== b.width || a.height !== b.height) { problems.push(`${name}: ${b.width}x${b.height}, was ${a.width}x${a.height}`); continue; }
    const marked = Buffer.from(b.data);
    const share = differing(a, b, manifest.channelTolerance, i => { marked[i] = 255; marked[i + 1] = 0; marked[i + 2] = 96; marked[i + 3] = 255; });
    if (share > manifest.failShare) {
      // Reference, current, and current with the differing pixels marked, side by side.
      const out = join(root, 'diff', name);
      mkdirSync(dirname(out), { recursive: true });
      writeFileSync(out, encode(sideBySide([a, b, { ...b, data: marked }])));
      problems.push(`${name}: ${(share * 100).toFixed(2)}% of pixels differ (see .baseline/diff/${name})`);
    }
  }
  for (const name of current) if (!accepted.includes(name)) problems.push(`${name}: new, not in the baseline; look at it and \`accept\``);
  if (problems.length) { console.error(problems.join('\n')); console.error(`\n${problems.length} of ${accepted.length} pictures differ from the baseline`); process.exit(1); }
  console.log(`${accepted.length} pictures match the baseline`);
}

// A PNG reader and writer for what a screenshot is: 8-bit RGB or RGBA, not interlaced.
function decode(file) {
  let offset = 8, width = 0, height = 0, channels = 4;
  const chunks = [];
  while (offset < file.length) {
    const length = file.readUInt32BE(offset);
    const type = file.toString('latin1', offset + 4, offset + 8);
    const body = file.subarray(offset + 8, offset + 8 + length);
    if (type === 'IHDR') {
      width = body.readUInt32BE(0); height = body.readUInt32BE(4);
      if (body[8] !== 8 || ![2, 6].includes(body[9]) || body[12] !== 0) throw Error('a baseline picture is an 8-bit RGB or RGBA PNG, not interlaced');
      channels = body[9] === 6 ? 4 : 3;
    } else if (type === 'IDAT') chunks.push(body);
    offset += 12 + length;
  }
  const raw = inflateSync(Buffer.concat(chunks));
  const stride = width * channels;
  const data = Buffer.alloc(width * height * 4);
  let previous = Buffer.alloc(stride);
  for (let y = 0; y < height; y += 1) {
    const filter = raw[y * (stride + 1)];
    const line = Buffer.from(raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1)));
    for (let x = 0; x < stride; x += 1) {
      const left = x >= channels ? line[x - channels] : 0, up = previous[x], upLeft = x >= channels ? previous[x - channels] : 0;
      const p = left + up - upLeft, pa = Math.abs(p - left), pb = Math.abs(p - up), pc = Math.abs(p - upLeft);
      const predicted = filter === 1 ? left : filter === 2 ? up : filter === 3 ? (left + up) >> 1 : filter === 4 ? (pa <= pb && pa <= pc ? left : pb <= pc ? up : upLeft) : 0;
      line[x] = (line[x] + predicted) & 255;
    }
    for (let x = 0; x < width; x += 1) {
      data[(y * width + x) * 4] = line[x * channels]; data[(y * width + x) * 4 + 1] = line[x * channels + 1];
      data[(y * width + x) * 4 + 2] = line[x * channels + 2]; data[(y * width + x) * 4 + 3] = channels === 4 ? line[x * channels + 3] : 255;
    }
    previous = line;
  }
  return { width, height, data };
}

function sideBySide(images) {
  const gap = 16, height = Math.max(...images.map(image => image.height));
  const width = images.reduce((sum, image) => sum + image.width, 0) + gap * (images.length - 1);
  const data = Buffer.alloc(width * height * 4, 255);
  let left = 0;
  for (const image of images) {
    for (let y = 0; y < image.height; y += 1) image.data.copy(data, (y * width + left) * 4, y * image.width * 4, (y + 1) * image.width * 4);
    left += image.width + gap;
  }
  return { width, height, data };
}

const crcTable = Array.from({ length: 256 }, (_, n) => { let c = n; for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
const crc = buffer => { let c = 0xffffffff; for (const byte of buffer) c = crcTable[(c ^ byte) & 255] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
function encode({ width, height, data }) {
  const chunk = (type, body) => {
    const out = Buffer.alloc(12 + body.length);
    out.writeUInt32BE(body.length, 0); out.write(type, 4, 'latin1'); body.copy(out, 8);
    out.writeUInt32BE(crc(out.subarray(4, 8 + body.length)), 8 + body.length);
    return out;
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0); header.writeUInt32BE(height, 4); header[8] = 8; header[9] = 6;
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y += 1) data.copy(raw, y * (width * 4 + 1) + 1, y * width * 4, (y + 1) * width * 4);
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', header), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}

const command = { capture, accept, check }[process.argv[2]];
if (!command) { console.error('usage: visual-baseline.mjs capture | check | accept'); process.exit(2); }
command();
