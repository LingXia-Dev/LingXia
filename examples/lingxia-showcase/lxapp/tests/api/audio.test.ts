import { expect, spec } from '@lingxia/test';
import { bindFixture } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

// Output itself is not asserted: these specs observe state, events, and errors.

spec('player lifecycle, queue, and errors through lx.audio', {
  id: 'LOGIC-AUDIO-001',
  covers: [
    'lx.audio.createPlayer',
    'lx.audio.route',
    'AudioPlayer.load',
    'AudioPlayer.setQueue',
    'AudioPlayer.play',
    'AudioPlayer.pause',
    'AudioPlayer.stop',
    'AudioPlayer.seek',
    'AudioPlayer.next',
    'AudioPlayer.previous',
    'AudioPlayer.skipTo',
    'AudioPlayer.setVolume',
    'AudioPlayer.setMuted',
    'AudioPlayer.setRate',
    'AudioPlayer.setRepeat',
    'AudioPlayer.setOutput',
    'AudioPlayer.on',
    'AudioPlayer.state',
    'AudioPlayer.queue',
    'AudioPlayer.dispose',
    'lx.audio.route.get',
    'lx.audio.route.watch',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace } = bindFixture(t, 'LOGIC-AUDIO-001');
  const result = await app.logic.eval(async ({ lx }, namespace) => {
    if (!lx.supports('audio') || !lx.audio) return { supported: false } as const;
    const audio = lx.audio;
    const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
    const root = `${lx.env.USER_CACHE_PATH}/${namespace}`;
    await lx.fs.mkdir(root, { recursive: true });
    const tone = `${root}/tone.wav`;
    const rate = 8000;
    const samples = rate;
    const wav = new Uint8Array(44 + samples * 2);
    const view = new DataView(wav.buffer);
    const ascii = (offset: number, text: string) => {
      for (let i = 0; i < text.length; i++) view.setUint8(offset + i, text.charCodeAt(i));
    };
    ascii(0, 'RIFF'); view.setUint32(4, 36 + samples * 2, true); ascii(8, 'WAVE'); ascii(12, 'fmt ');
    view.setUint32(16, 16, true); view.setUint16(20, 1, true); view.setUint16(22, 1, true);
    view.setUint32(24, rate, true); view.setUint32(28, rate * 2, true); view.setUint16(32, 2, true);
    view.setUint16(34, 16, true); ascii(36, 'data'); view.setUint32(40, samples * 2, true);
    for (let i = 0; i < samples; i++) {
      view.setInt16(44 + i * 2, Math.round(Math.sin((i / rate) * 2 * Math.PI * 440) * 3000), true);
    }
    await lx.fs.write(tone, wav, { overwrite: true });
    const garbage = `${root}/garbage.mp3`;
    await lx.fs.write(garbage, 'not audio at all', { overwrite: true });

    const reason = (error: unknown) => {
      const data = (error as { data?: { bizCode?: number; code?: string } }).data;
      return [data?.bizCode ?? null, data?.code ?? null] as const;
    };
    const out: Record<string, unknown> = { supported: true };
    const routes: string[] = [];
    const unwatch = audio.route.watch(({ route }) => routes.push(route.output));
    out.firstRouteSync = routes.length === 1;
    unwatch();
    out.route = audio.route.get().output;

    const player = audio.createPlayer({ timeUpdateIntervalMs: 100 });
    const statuses: string[] = [];
    const items: string[] = [];
    const ended: number[] = [];
    let times = 0;
    player.on('state', (s) => {
      if (statuses[statuses.length - 1] !== s.status) statuses.push(s.status);
    });
    player.on('item', (e) => items.push(`${e.index}:${e.reason}:${e.item.id ?? ''}`));
    player.on('ended', (e) => ended.push(e.index));
    player.on('time', () => { times++; });
    try {
      out.idle = player.state.status;
      out.loaded = await player.load(tone);
      out.afterLoad = player.state.status;
      await player.play();
      out.afterPlay = player.state.status;
      await wait(400);
      out.advanced = player.state.positionMs > 0;
      await player.pause();
      await player.seek(200);
      out.afterSeek = [player.state.status, Math.abs(player.state.positionMs - 200) < 60, player.state.seeking];
      player.setVolume(0.4);
      player.setMuted(true);
      player.setRate(1.5);
      out.settings = [player.state.volume, player.state.muted, player.state.rate];
      player.setRate(1);
      player.setMuted(false);
      await player.stop();
      out.afterStop = [player.state.status, player.state.positionMs];

      await player.setQueue(
        [{ id: 'a', source: tone }, { id: 'b', source: { uri: tone } }],
        { autoplay: true },
      );
      out.queueLength = player.queue.length;
      await wait(2_800);
      out.queueEnd = [player.state.status, player.state.index];
      await player.skipTo(1);
      await player.previous();
      out.afterPrevious = player.state.index;
      await player.next();
      out.afterNext = player.state.index;
      out.nextBeyondEnd = await player.next().then(() => 'resolved', reason);
      player.setRepeat('all');
      out.wrapNext = await player.next().then(() => player.state.index, reason);
      out.earpiece = lx.supports('audio.output.earpiece')
        ? 'supported'
        : await player.setOutput('earpiece').then(() => 'resolved', reason);
    } finally {
      player.dispose();
    }
    out.disposed = await player.play().then(() => 'resolved', reason);

    // Repeat 'one' restarts the item in place: several ends, still playing.
    const looping = audio.createPlayer({ repeat: 'one', timeUpdateIntervalMs: 0 });
    let loops = 0;
    looping.on('ended', () => { loops++; });
    try {
      await looping.load(tone, { autoplay: true });
      // Restart latency varies by backend (AVPlayer adds ~0.4 s a loop), so
      // wait for the loops instead of a fixed window.
      for (let waited = 0; loops < 2 && waited < 6_000; waited += 100) await wait(100);
      out.repeatOne = [loops >= 2, looping.state.status];
    } finally {
      looping.dispose();
    }
    out.statuses = statuses;
    out.items = items;
    out.ended = ended;
    out.times = times;

    out.idlePlay = await audio.createPlayer().play().then(() => 'resolved', reason);
    out.relative = await audio.createPlayer().load('relative.wav').then(() => 'resolved', reason);
    out.missing = await audio.createPlayer()
      .load(`${root}/missing.wav`).then(() => 'resolved', reason);
    const broken = audio.createPlayer();
    out.garbage = await broken.load(garbage).then(() => broken.state.status, reason);
    out.garbageState = broken.state.error?.code ?? null;
    broken.dispose();
    await lx.fs.remove(root, { recursive: true }).catch(() => {});
    return out;
  }, namespace);

  if (!result.supported) t.skip("lx.supports('audio') is false on this host");
  const r = result as Record<string, unknown>;
  expect(r.firstRouteSync).toBe(true);
  expect(typeof r.route).toBe('string');
  expect(r.idle).toBe('idle');
  expect((r.loaded as { index: number }).index).toBe(0);
  expect(typeof (r.loaded as { durationMs: unknown }).durationMs).toBe('number');
  expect(r.afterLoad).toBe('paused');
  expect(r.afterPlay).toBe('playing');
  expect(r.advanced).toBe(true);
  expect(r.afterSeek).toEqual(['paused', true, false]);
  expect(r.settings).toEqual([0.4, true, 1.5]);
  expect(r.afterStop).toEqual(['stopped', 0]);
  expect(r.queueLength).toBe(2);
  expect(r.queueEnd).toEqual(['ended', 1]);
  expect(r.afterPrevious).toBe(0);
  expect(r.afterNext).toBe(1);
  expect(r.nextBeyondEnd).toEqual([1002, 'invalid_options']);
  expect(r.wrapNext).toBe(0);
  if (r.earpiece !== 'supported') expect(r.earpiece).toEqual([6000, 'output_unsupported']);
  expect(r.disposed).toEqual([4000, 'disposed']);
  expect(r.repeatOne).toEqual([true, 'playing']);
  const statuses = r.statuses as string[];
  for (const status of ['loading', 'paused', 'playing', 'stopped', 'ended', 'disposed']) {
    expect(statuses.includes(status)).toBe(true);
  }
  const items = r.items as string[];
  for (const item of ['0:load:', '0:load:a', '1:auto:b', '1:skip:b']) {
    expect(items.includes(item)).toBe(true);
  }
  const ended = r.ended as number[];
  expect(ended.includes(0) && ended.includes(1)).toBe(true);
  expect(r.times as number).toBeGreaterThan(0);
  expect(r.idlePlay).toEqual([1002, 'invalid_source']);
  expect(r.relative).toEqual([1002, 'invalid_source']);
  expect(r.missing).toEqual([1003, 'source_not_found']);
  expect(r.garbage).toEqual([6000, 'unsupported_format']);
  expect(r.garbageState).toBe('unsupported_format');
});

spec('exclusive focus, mix participants, and sounds', {
  id: 'LOGIC-AUDIO-002',
  covers: [
    'lx.audio.loadSound',
    'lx.audio.onInterruption',
    'Sound.play',
    'Sound.stopAll',
    'Sound.dispose',
    'Sound.durationMs',
    'SoundVoice.stop',
    'SoundVoice.ended',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace } = bindFixture(t, 'LOGIC-AUDIO-002');
  const result = await app.logic.eval(async ({ lx }, namespace) => {
    if (!lx.supports('audio') || !lx.audio) return { supported: false } as const;
    const audio = lx.audio;
    const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
    const root = `${lx.env.USER_CACHE_PATH}/${namespace}`;
    await lx.fs.mkdir(root, { recursive: true });
    const rate = 8000;
    const samples = rate * 3;
    const wav = new Uint8Array(44 + samples * 2);
    const view = new DataView(wav.buffer);
    const ascii = (offset: number, text: string) => {
      for (let i = 0; i < text.length; i++) view.setUint8(offset + i, text.charCodeAt(i));
    };
    ascii(0, 'RIFF'); view.setUint32(4, 36 + samples * 2, true); ascii(8, 'WAVE'); ascii(12, 'fmt ');
    view.setUint32(16, 16, true); view.setUint16(20, 1, true); view.setUint16(22, 1, true);
    view.setUint32(24, rate, true); view.setUint32(28, rate * 2, true); view.setUint16(32, 2, true);
    view.setUint16(34, 16, true); ascii(36, 'data'); view.setUint32(40, samples * 2, true);
    for (let i = 0; i < samples; i++) {
      view.setInt16(44 + i * 2, Math.round(Math.sin((i / rate) * 2 * Math.PI * 330) * 2000), true);
    }
    const tone = `${root}/long.wav`;
    await lx.fs.write(tone, wav, { overwrite: true });

    const out: Record<string, unknown> = { supported: true };
    const unInterrupt = audio.onInterruption(() => {});
    const first = audio.createPlayer();
    const second = audio.createPlayer();
    const ambient = audio.createPlayer({ focus: 'mix' });
    const interruptions: string[] = [];
    first.on('interruption', (e) => interruptions.push(`${e.phase}:${e.reason}:${e.shouldResume}`));
    try {
      await Promise.all([first.load(tone), second.load(tone), ambient.load(tone)]);
      await first.play();
      await ambient.play();
      await second.play();
      await wait(200);
      out.focus = [first.state.status, second.state.status, ambient.state.status];
      out.interruptions = interruptions;
    } finally {
      first.dispose();
      second.dispose();
      ambient.dispose();
      unInterrupt();
    }

    const sound = await audio.loadSound({ bytes: wav.slice(0, 44 + rate * 2), mimeType: 'audio/wav' }, { maxVoices: 2 });
    out.soundDuration = sound.durationMs;
    const started = Date.now();
    await sound.play({ volume: 0.5, pan: -1 }).ended;
    out.voiceMs = Date.now() - started;
    const looping = sound.play({ loop: true });
    looping.stop();
    await looping.ended;
    const a = sound.play({ loop: true });
    const b = sound.play({ loop: true });
    const c = sound.play({ loop: true });
    await a.ended;
    out.stolen = true;
    sound.stopAll();
    await Promise.all([b.ended, c.ended]);
    sound.dispose();
    out.afterDispose = (() => {
      try { sound.play(); return 'played'; } catch (error) {
        return (error as { data?: { code?: string } }).data?.code ?? null;
      }
    })();
    out.exclusiveSound = await audio
      .loadSound(tone, { focus: 'exclusive' as never })
      .then(() => 'resolved', (error) => (error as { data?: { code?: string } }).data?.code ?? null);
    await lx.fs.remove(root, { recursive: true }).catch(() => {});
    return out;
  }, namespace);

  if (!result.supported) t.skip("lx.supports('audio') is false on this host");
  const r = result as Record<string, unknown>;
  expect(r.focus).toEqual(['paused', 'playing', 'playing']);
  expect(r.interruptions).toEqual(['began:focusTaken:false']);
  expect(r.soundDuration as number).toBeGreaterThan(900);
  expect(r.voiceMs as number).toBeGreaterThan(500);
  expect(r.stolen).toBe(true);
  expect(r.afterDispose).toBe('disposed');
  expect(r.exclusiveSound).toBe('invalid_options');
});
