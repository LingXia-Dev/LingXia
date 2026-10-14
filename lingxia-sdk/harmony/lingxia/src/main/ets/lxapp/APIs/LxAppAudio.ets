import { audio } from '@kit.AudioKit';
import { BusinessError, runningLock } from '@kit.BasicServicesKit';
import { fileIo } from '@kit.CoreFileKit';
import { media } from '@kit.MediaKit';
import { sensor } from '@kit.SensorServiceKit';
import liblingxia from 'liblingxia.so';
import { LxLog } from '../../app/LxLog';

const TAG = 'LingXia.Audio';

/** One coordinator command (`{"op": ...}`), see lingxia-media `audio::protocol`. */
interface AudioCommand {
  op: string;
  id?: number;
  req?: number;
  uri?: string;
  headers?: Record<string, string>;
  mimeType?: string | null;
  startMs?: number;
  tickMs?: number;
  positionMs?: number;
  volume?: number;
  rate?: number;
  pan?: number;
  loop?: boolean;
  voice?: number;
  path?: string;
  active?: boolean;
  focus?: string | null;
  obeySilentSwitch?: boolean;
  output?: string;
  proximity?: boolean;
}

/** One native event (`{"event": ...}`); undefined fields are dropped by JSON.stringify. */
interface AudioEvent {
  event: string;
  id?: number;
  req?: number;
  durationMs?: number | null;
  positionMs?: number;
  bufferedMs?: number;
  buffering?: boolean;
  code?: string;
  message?: string;
  voice?: number;
  output?: string;
  name?: string;
  reason?: string;
  lost?: boolean;
  phase?: string;
  shouldResume?: boolean;
  near?: boolean;
}

interface PlayerSource {
  uri: string;
  headers?: Record<string, string>;
  mimeType?: string | null;
}

function errorCode(error: BusinessError, remote: boolean): string {
  const code = error.code;
  if (code === 5400106) {
    return 'unsupported_format';
  }
  if ((code >= 5411001 && code <= 5411011) || (remote && (code === 5400103 || code === 5400104))) {
    return 'network';
  }
  if (!remote && code === 5400103) {
    return 'source_not_found';
  }
  return 'decode_failed';
}

const SEEK_TIMEOUT_MS = 3000;

function speedFor(rate: number): media.PlaybackSpeed {
  const speeds: [number, media.PlaybackSpeed][] = [
    [0.5, media.PlaybackSpeed.SPEED_FORWARD_0_50_X],
    [0.75, media.PlaybackSpeed.SPEED_FORWARD_0_75_X],
    [1.0, media.PlaybackSpeed.SPEED_FORWARD_1_00_X],
    [1.25, media.PlaybackSpeed.SPEED_FORWARD_1_25_X],
    [1.5, media.PlaybackSpeed.SPEED_FORWARD_1_50_X],
    [1.75, media.PlaybackSpeed.SPEED_FORWARD_1_75_X],
    [2.0, media.PlaybackSpeed.SPEED_FORWARD_2_00_X],
  ];
  let best = speeds[2];
  for (const entry of speeds) {
    if (Math.abs(entry[0] - rate) < Math.abs(best[0] - rate)) {
      best = entry;
    }
  }
  return best[1];
}

function routeOutput(type: audio.DeviceType): string {
  switch (type) {
    case audio.DeviceType.EARPIECE:
      return 'earpiece';
    case audio.DeviceType.SPEAKER:
      return 'speaker';
    case audio.DeviceType.WIRED_HEADSET:
    case audio.DeviceType.WIRED_HEADPHONES:
      return 'headphones';
    case audio.DeviceType.BLUETOOTH_SCO:
    case audio.DeviceType.BLUETOOTH_A2DP:
      return 'bluetooth';
    case audio.DeviceType.USB_HEADSET:
      return 'usb';
    case audio.DeviceType.DISPLAY_PORT:
      return 'hdmi';
    default:
      return 'other';
  }
}

class AudioPlayerSlot {
  readonly id: number;
  private readonly tickMs: number;
  private chain: Promise<void> = Promise.resolve();
  private player: media.AVPlayer | null = null;
  private file: fileIo.File | null = null;
  private source: PlayerSource | null = null;
  private loadReq: number = -1;
  private seekReq: number = -1;
  private lastState: string = 'idle';
  private preparedUsage: audio.StreamUsage = audio.StreamUsage.STREAM_USAGE_MUSIC;
  private volume: number = 1;
  private rate: number = 1;
  private bufferedAheadMs: number = 0;
  private timer: number = -1;
  private stateWaiter: ((state: string) => void) | null = null;
  private seekWaiter: (() => void) | null = null;
  private disposed: boolean = false;

  constructor(id: number, tickMs: number) {
    this.id = id;
    this.tickMs = tickMs;
    this.enqueue(async () => {
      const player = await media.createAVPlayer();
      this.player = player;
      this.observe(player);
    });
  }

  /** Commands for one player run strictly in arrival order. */
  enqueue(op: () => Promise<void>): void {
    this.chain = this.chain.then(op).catch((error: BusinessError) => {
      LxLog.w(TAG, `player ${this.id} command failed: code=${error.code} msg=${error.message}`);
    });
  }

  private observe(player: media.AVPlayer): void {
    player.on('stateChange', (state: string) => {
      const previous = this.lastState;
      this.lastState = state;
      const waiter = this.stateWaiter;
      if (waiter) {
        waiter(state);
      }
      if (state === 'playing') {
        LxAppAudio.emit({ event: 'player.playing', id: this.id, positionMs: player.currentTime });
        this.startTicking();
      } else if (state === 'paused') {
        this.stopTicking();
        // Seeking out of 'completed' lands in 'paused'; that is a restart, not a pause.
        if (previous !== 'completed') {
          LxAppAudio.emit({ event: 'player.paused', id: this.id, positionMs: player.currentTime });
        }
      } else if (state === 'completed') {
        this.stopTicking();
        LxAppAudio.emit({ event: 'player.ended', id: this.id });
      }
    });
    player.on('error', (error: BusinessError) => {
      this.stopTicking();
      const waiter = this.stateWaiter;
      if (waiter) {
        waiter('error');
      }
      const remote = this.source !== null && /^https?:/i.test(this.source.uri);
      const event: AudioEvent = {
        event: 'player.error',
        id: this.id,
        code: errorCode(error, remote),
        message: `${error.code}: ${error.message}`,
      };
      if (this.loadReq >= 0) {
        event.req = this.loadReq;
      }
      LxAppAudio.emit(event);
    });
    player.on('seekDone', (time: number) => {
      if (this.seekReq >= 0) {
        LxAppAudio.emit({ event: 'player.seeked', id: this.id, req: this.seekReq, positionMs: time });
        this.seekReq = -1;
      }
      const waiter = this.seekWaiter;
      if (waiter) {
        this.seekWaiter = null;
        waiter();
      }
    });
    player.on('bufferingUpdate', (type: media.BufferingInfoType, value: number) => {
      if (type === media.BufferingInfoType.BUFFERING_START) {
        LxAppAudio.emit({ event: 'player.buffering', id: this.id, buffering: true });
      } else if (type === media.BufferingInfoType.BUFFERING_END) {
        LxAppAudio.emit({ event: 'player.buffering', id: this.id, buffering: false });
      } else if (type === media.BufferingInfoType.CACHED_DURATION) {
        this.bufferedAheadMs = value;
      }
    });
    player.on('audioInterrupt', (info: audio.InterruptEvent) => LxAppAudio.onInterrupt(info));
  }

  private startTicking(): void {
    this.stopTicking();
    this.timer = setInterval(() => {
      const player = this.player;
      if (!player) {
        return;
      }
      const position = player.currentTime;
      LxAppAudio.emit({
        event: 'player.progress',
        id: this.id,
        positionMs: position,
        bufferedMs: position + this.bufferedAheadMs,
        durationMs: player.duration > 0 ? player.duration : null,
      });
    }, this.tickMs);
  }

  private stopTicking(): void {
    if (this.timer >= 0) {
      clearInterval(this.timer);
      this.timer = -1;
    }
  }

  private waitState(targets: string[]): Promise<string> {
    return new Promise<string>((resolve) => {
      this.stateWaiter = (state: string) => {
        if (targets.indexOf(state) >= 0 || state === 'error') {
          this.stateWaiter = null;
          resolve(state);
        }
      };
    });
  }

  private closeFile(): void {
    if (this.file) {
      try {
        fileIo.closeSync(this.file);
      } catch (error) {
        LxLog.w(TAG, `close failed: ${error}`);
      }
      this.file = null;
    }
  }

  private async resetPlayer(): Promise<void> {
    const player = this.player;
    if (!player) {
      return;
    }
    this.stopTicking();
    if (player.state !== 'idle') {
      const idle = this.waitState(['idle']);
      await player.reset();
      const after: string = player.state;
      if (after !== 'idle') {
        await idle;
      }
      this.stateWaiter = null;
    }
    this.closeFile();
  }

  /** Attach the source, fix the stream usage, and prepare; false on failure (already reported). */
  private async prepare(source: PlayerSource, usage: audio.StreamUsage, startMs: number): Promise<boolean> {
    const player = this.player;
    if (!player) {
      return false;
    }
    await this.resetPlayer();
    const initialized = this.waitState(['initialized']);
    const remote = /^https?:/i.test(source.uri);
    if (remote && (source.headers || source.mimeType)) {
      const mediaSource = media.createMediaSourceWithUrl(source.uri, source.headers ?? {});
      if (source.mimeType && source.mimeType.toLowerCase().indexOf('mpegurl') >= 0) {
        mediaSource.setMimeType(media.AVMimeTypes.APPLICATION_M3U8);
      }
      await player.setMediaSource(mediaSource);
    } else if (remote) {
      player.url = source.uri;
    } else {
      try {
        this.file = fileIo.openSync(source.uri, fileIo.OpenMode.READ_ONLY);
      } catch (error) {
        this.stateWaiter = null;
        const event: AudioEvent = { event: 'player.error', id: this.id, code: 'source_not_found', message: `${error}` };
        if (this.loadReq >= 0) {
          event.req = this.loadReq;
        }
        LxAppAudio.emit(event);
        return false;
      }
      player.fdSrc = { fd: this.file.fd };
    }
    if ((await initialized) !== 'initialized') {
      return false;
    }
    player.audioRendererInfo = { usage: usage, rendererFlags: 0 };
    this.preparedUsage = usage;
    // A remote source's prepare() can resolve before the state reaches 'prepared';
    // setVolume/setSpeed in between drive the AVPlayer into 'error'.
    const prepared = this.waitState(['prepared']);
    await player.prepare();
    const reached: string = player.state;
    if (reached !== 'prepared' && (await prepared) !== 'prepared') {
      return false;
    }
    this.stateWaiter = null;
    player.setVolume(this.volume);
    if (this.rate !== 1) {
      player.setSpeed(speedFor(this.rate));
    }
    if (startMs > 0) {
      const sought = new Promise<void>((resolve) => {
        this.seekWaiter = resolve;
      });
      player.seek(startMs, media.SeekMode.SEEK_CLOSEST);
      await sought;
    }
    return true;
  }

  load(command: AudioCommand): void {
    const req = command.req ?? -1;
    this.loadReq = req;
    const source: PlayerSource = { uri: command.uri ?? '', headers: command.headers, mimeType: command.mimeType };
    this.source = source;
    const usage = LxAppAudio.usage();
    this.enqueue(async () => {
      if (this.loadReq !== req || this.disposed) {
        return;
      }
      if (!(await this.prepare(source, usage, command.startMs ?? 0)) || this.loadReq !== req) {
        return;
      }
      const duration = this.player?.duration ?? -1;
      LxAppAudio.emit({ event: 'player.loaded', id: this.id, req: req, durationMs: duration > 0 ? duration : null });
      this.loadReq = -1;
    });
  }

  play(): void {
    this.enqueue(async () => {
      const player = this.player;
      const source = this.source;
      if (!player || !source) {
        return;
      }
      const usage = LxAppAudio.usage();
      if (usage !== this.preparedUsage) {
        // Stream usage is fixed at prepare; switching focus or output means re-preparing in place.
        if (!(await this.prepare(source, usage, player.currentTime))) {
          return;
        }
      }
      await player.play();
    });
  }

  /** Session output changed under a playing stream: move it to the new usage. */
  retarget(usage: audio.StreamUsage): void {
    this.enqueue(async () => {
      const player = this.player;
      const source = this.source;
      if (!player || !source || player.state !== 'playing' || usage === this.preparedUsage) {
        return;
      }
      if (await this.prepare(source, usage, player.currentTime)) {
        await player.play();
      }
    });
  }

  pause(): void {
    this.enqueue(async () => {
      const player = this.player;
      if (player && player.state === 'playing') {
        await player.pause();
      }
    });
  }

  seek(command: AudioCommand): void {
    const req = command.req ?? -1;
    this.enqueue(async () => {
      const player = this.player;
      if (!player) {
        return;
      }
      this.seekReq = req;
      // Hold the queue until the seek lands: a play() issued mid-seek is dropped
      // (repeat-one restarts with seek 0 then play).
      const done = new Promise<void>((resolve) => {
        this.seekWaiter = resolve;
        setTimeout(resolve, SEEK_TIMEOUT_MS);
      });
      player.seek(command.positionMs ?? 0, media.SeekMode.SEEK_CLOSEST);
      await done;
      this.seekWaiter = null;
    });
  }

  unload(): void {
    this.loadReq = -1;
    this.source = null;
    this.enqueue(() => this.resetPlayer());
  }

  setVolume(volume: number): void {
    this.volume = volume;
    this.enqueue(async () => {
      const player = this.player;
      if (player && ['prepared', 'playing', 'paused', 'completed'].indexOf(player.state) >= 0) {
        player.setVolume(volume);
      }
    });
  }

  setRate(rate: number): void {
    this.rate = rate;
    this.enqueue(async () => {
      const player = this.player;
      if (player && ['prepared', 'playing', 'paused', 'completed'].indexOf(player.state) >= 0) {
        player.setSpeed(speedFor(rate));
      }
    });
  }

  isPlaying(): boolean {
    return this.player?.state === 'playing';
  }

  dispose(): void {
    this.disposed = true;
    this.loadReq = -1;
    this.enqueue(async () => {
      this.stopTicking();
      const player = this.player;
      this.player = null;
      if (player) {
        await player.release();
      }
      this.closeFile();
    });
  }
}

class SoundSlot {
  readonly id: number;
  private pool: media.SoundPool | null = null;
  private soundId: number = -1;
  private file: fileIo.File | null = null;
  private chain: Promise<void> = Promise.resolve();
  private readonly voices: Map<number, number> = new Map();

  constructor(id: number) {
    this.id = id;
  }

  private enqueue(op: () => Promise<void>): void {
    this.chain = this.chain.then(op).catch((error: BusinessError) => {
      LxLog.w(TAG, `sound ${this.id} command failed: code=${error.code} msg=${error.message}`);
    });
  }

  load(req: number, path: string): void {
    this.enqueue(async () => {
      try {
        this.file = fileIo.openSync(path, fileIo.OpenMode.READ_ONLY);
      } catch (error) {
        LxAppAudio.emit({ event: 'sound.error', id: this.id, req: req, code: 'source_not_found', message: `${error}` });
        return;
      }
      const length = fileIo.statSync(this.file.fd).size;
      try {
        const durationMs = await SoundSlot.duration(this.file.fd, length);
        const pool = await media.createSoundPool(16, {
          usage: audio.StreamUsage.STREAM_USAGE_GAME,
          rendererFlags: 1,
        });
        this.pool = pool;
        pool.on('playFinishedWithStreamId', (streamId: number) => {
          const voice = this.voices.get(streamId);
          if (voice !== undefined) {
            this.voices.delete(streamId);
            LxAppAudio.emit({ event: 'sound.ended', id: this.id, voice: voice });
          }
        });
        const loaded = new Promise<void>((resolve) => {
          pool.on('loadComplete', () => resolve());
        });
        this.soundId = await pool.load(this.file.fd, 0, length);
        await loaded;
        LxAppAudio.emit({ event: 'sound.loaded', id: this.id, req: req, durationMs: durationMs });
      } catch (error) {
        const failure = error as BusinessError;
        LxAppAudio.emit({
          event: 'sound.error',
          id: this.id,
          req: req,
          code: errorCode(failure, false),
          message: `${failure.code}: ${failure.message}`,
        });
      }
    });
  }

  private static async duration(fd: number, length: number): Promise<number> {
    const extractor = await media.createAVMetadataExtractor();
    try {
      extractor.fdSrc = { fd: fd, offset: 0, length: length };
      const metadata = await extractor.fetchMetadata();
      return Number.parseInt(metadata.duration ?? '0') || 0;
    } finally {
      await extractor.release();
    }
  }

  play(command: AudioCommand): void {
    this.enqueue(async () => {
      const pool = this.pool;
      if (!pool || this.soundId < 0) {
        return;
      }
      const volume = command.volume ?? 1;
      const pan = command.pan ?? 0;
      const rate = command.rate ?? 1;
      // SoundPool offers only half, normal, and double speed.
      const renderRate = rate <= 0.75 ? audio.AudioRendererRate.RENDER_RATE_HALF
        : rate >= 1.5 ? audio.AudioRendererRate.RENDER_RATE_DOUBLE : audio.AudioRendererRate.RENDER_RATE_NORMAL;
      const streamId = await pool.play(this.soundId, {
        loop: command.loop ? -1 : 0,
        rate: renderRate,
        leftVolume: volume * Math.min(1, 1 - pan),
        rightVolume: volume * Math.min(1, 1 + pan),
        priority: 0,
      });
      this.voices.set(streamId, command.voice ?? -1);
    });
  }

  stop(voice: number): void {
    this.enqueue(async () => {
      const pool = this.pool;
      for (const entry of this.voices) {
        if (entry[1] === voice) {
          this.voices.delete(entry[0]);
          if (pool) {
            await pool.stop(entry[0]);
          }
          return;
        }
      }
    });
  }

  dispose(): void {
    this.enqueue(async () => {
      const pool = this.pool;
      this.pool = null;
      this.voices.clear();
      if (pool) {
        await pool.release();
      }
      if (this.file) {
        fileIo.closeSync(this.file);
        this.file = null;
      }
    });
  }
}

/**
 * Native backend of the host audio coordinator. Rust owns focus, queue, and
 * state; this applies commands in order and reports what the platform did.
 */
export class LxAppAudio {
  private static callbackId: string = '';
  private static players: Map<number, AudioPlayerSlot> = new Map();
  private static sounds: Map<number, SoundSlot> = new Map();
  private static focus: string = 'exclusive';
  private static earpiece: boolean = false;
  private static interrupted: boolean = false;
  private static lastRoute: string = '';
  private static proximityLock: runningLock.RunningLock | null = null;
  private static proximityOn: boolean = false;

  static emit(event: AudioEvent): void {
    if (LxAppAudio.callbackId) {
      liblingxia.onCallback(LxAppAudio.callbackId, true, JSON.stringify(event));
    }
  }

  static attach(callbackId: string): void {
    LxAppAudio.callbackId = callbackId;
    try {
      const routing = audio.getAudioManager().getRoutingManager();
      routing.on('deviceChange', audio.DeviceFlag.OUTPUT_DEVICES_FLAG, (action: audio.DeviceChangeAction) => {
        const removed = action.type === audio.DeviceChangeType.DISCONNECT;
        const previous = LxAppAudio.lastRoute;
        const lost = removed && previous !== '' && previous !== 'speaker' && previous !== 'earpiece'
          && action.deviceDescriptors.some((d: audio.AudioDeviceDescriptor) => routeOutput(d.deviceType) === previous);
        LxAppAudio.emitRoute(removed ? 'deviceRemoved' : 'deviceAdded', lost);
      });
    } catch (error) {
      LxLog.w(TAG, `route observation unavailable: ${error}`);
    }
    LxAppAudio.emitRoute('other', false);
  }

  private static emitRoute(reason: string, lost: boolean): void {
    let output = 'speaker';
    let name: string | undefined = undefined;
    try {
      const routing = audio.getAudioManager().getRoutingManager();
      const devices = routing.getPreferredOutputDeviceForRendererInfoSync({
        usage: LxAppAudio.usage(),
        rendererFlags: 0,
      });
      if (devices.length > 0) {
        output = routeOutput(devices[0].deviceType);
        name = devices[0].displayName || devices[0].name || undefined;
      }
    } catch (error) {
      LxLog.w(TAG, `route query failed: ${error}`);
    }
    LxAppAudio.lastRoute = output;
    LxAppAudio.emit({ event: 'route', output: output, name: name, reason: reason, lost: lost });
  }

  /** One interruption per external event, however many players saw it. */
  static onInterrupt(info: audio.InterruptEvent): void {
    if (info.eventType === audio.InterruptType.INTERRUPT_TYPE_BEGIN) {
      if (info.hintType === audio.InterruptHint.INTERRUPT_HINT_PAUSE
        || info.hintType === audio.InterruptHint.INTERRUPT_HINT_STOP) {
        if (!LxAppAudio.interrupted) {
          LxAppAudio.interrupted = true;
          LxAppAudio.emit({ event: 'interruption', phase: 'began', reason: 'otherApp', shouldResume: false });
        }
      }
    } else if (info.eventType === audio.InterruptType.INTERRUPT_TYPE_END && LxAppAudio.interrupted) {
      LxAppAudio.interrupted = false;
      LxAppAudio.emit({
        event: 'interruption',
        phase: 'ended',
        reason: 'otherApp',
        shouldResume: info.hintType === audio.InterruptHint.INTERRUPT_HINT_RESUME,
      });
    }
  }

  /** Stream usage carries focus on HarmonyOS: the system ducks, mixes, or pauses by usage. */
  static usage(): audio.StreamUsage {
    if (LxAppAudio.earpiece) {
      return audio.StreamUsage.STREAM_USAGE_VOICE_COMMUNICATION;
    }
    if (LxAppAudio.focus === 'duck') {
      return audio.StreamUsage.STREAM_USAGE_NAVIGATION;
    }
    if (LxAppAudio.focus === 'mix') {
      return audio.StreamUsage.STREAM_USAGE_GAME;
    }
    return audio.StreamUsage.STREAM_USAGE_MUSIC;
  }

  static send(json: string): void {
    let command: AudioCommand;
    try {
      command = JSON.parse(json) as AudioCommand;
    } catch (error) {
      LxLog.w(TAG, `bad audio command: ${json}`);
      return;
    }
    const id = command.id ?? -1;
    switch (command.op) {
      case 'player.create':
        LxAppAudio.players.set(id, new AudioPlayerSlot(id, command.tickMs ?? 250));
        break;
      case 'player.load':
        LxAppAudio.players.get(id)?.load(command);
        break;
      case 'player.play':
        LxAppAudio.players.get(id)?.play();
        break;
      case 'player.pause':
        LxAppAudio.players.get(id)?.pause();
        break;
      case 'player.seek':
        LxAppAudio.players.get(id)?.seek(command);
        break;
      case 'player.unload':
        LxAppAudio.players.get(id)?.unload();
        break;
      case 'player.volume':
        LxAppAudio.players.get(id)?.setVolume(command.volume ?? 1);
        break;
      case 'player.rate':
        LxAppAudio.players.get(id)?.setRate(command.rate ?? 1);
        break;
      case 'player.dispose':
        LxAppAudio.players.get(id)?.dispose();
        LxAppAudio.players.delete(id);
        break;
      case 'sound.load': {
        const slot = new SoundSlot(id);
        LxAppAudio.sounds.set(id, slot);
        slot.load(command.req ?? -1, command.path ?? '');
        break;
      }
      case 'sound.play':
        LxAppAudio.sounds.get(id)?.play(command);
        break;
      case 'sound.stop':
        LxAppAudio.sounds.get(id)?.stop(command.voice ?? -1);
        break;
      case 'sound.dispose':
        LxAppAudio.sounds.get(id)?.dispose();
        LxAppAudio.sounds.delete(id);
        break;
      case 'session':
        LxAppAudio.applySession(command);
        break;
      default:
        LxLog.w(TAG, `unknown audio command: ${command.op}`);
    }
  }

  private static applySession(command: AudioCommand): void {
    if (command.focus) {
      LxAppAudio.focus = command.focus;
    }
    const earpiece = command.active === true && command.output === 'earpiece';
    if (earpiece !== LxAppAudio.earpiece) {
      LxAppAudio.earpiece = earpiece;
      try {
        // Only voice-communication streams follow the communication device.
        audio.getAudioManager().getRoutingManager()
          .setCommunicationDevice(audio.CommunicationDeviceType.SPEAKER, !earpiece)
          .catch((error: BusinessError) => LxLog.w(TAG, `communication device: ${error.message}`))
          .finally(() => LxAppAudio.emitRoute('override', false));
      } catch (error) {
        LxLog.w(TAG, `communication device unavailable: ${error}`);
      }
      const usage = LxAppAudio.usage();
      LxAppAudio.players.forEach((slot: AudioPlayerSlot) => {
        if (slot.isPlaying()) {
          slot.retarget(usage);
        }
      });
    }
    LxAppAudio.setProximity(command.active === true && command.proximity === true);
  }

  private static setProximity(on: boolean): void {
    if (on === LxAppAudio.proximityOn) {
      return;
    }
    LxAppAudio.proximityOn = on;
    if (on) {
      try {
        sensor.on(sensor.SensorId.PROXIMITY, (data: sensor.ProximityResponse) => {
          LxAppAudio.emit({ event: 'proximity', near: data.distance === 0 });
        });
      } catch (error) {
        LxLog.w(TAG, `proximity sensor unavailable: ${error}`);
      }
      let supported = false;
      try {
        supported = runningLock.isSupported(runningLock.RunningLockType.PROXIMITY_SCREEN_CONTROL);
      } catch (error) {
        LxLog.w(TAG, `proximity lock unsupported: ${error}`);
      }
      if (supported) {
        runningLock.create('lx-audio-proximity', runningLock.RunningLockType.PROXIMITY_SCREEN_CONTROL)
          .then((lock: runningLock.RunningLock) => {
            if (!LxAppAudio.proximityOn) {
              return;
            }
            LxAppAudio.proximityLock = lock;
            // The lock turns the screen off while near; the timeout only bounds a leak.
            try {
              lock.hold(60 * 60 * 1000);
            } catch (error) {
              LxLog.w(TAG, `proximity hold: ${error}`);
            }
          })
          .catch((error: BusinessError) => LxLog.w(TAG, `proximity lock: ${error.message}`));
      }
    } else {
      try {
        sensor.off(sensor.SensorId.PROXIMITY);
      } catch (error) {
        LxLog.w(TAG, `proximity sensor off: ${error}`);
      }
      const lock = LxAppAudio.proximityLock;
      LxAppAudio.proximityLock = null;
      try {
        lock?.unhold();
      } catch (error) {
        LxLog.w(TAG, `proximity unhold: ${error}`);
      }
    }
  }
}
