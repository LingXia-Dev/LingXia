import type {
  AudioPlayer,
  AudioPlayerState,
  AudioQueueItem,
  AudioRepeat,
  Sound,
} from "@lingxia/types";
import { errorMessage } from "../../shared/lib/errors";

const ROOT = `${lx.env.USER_CACHE_PATH}/audio-demo`;
const REMOTE_TRACK =
  "https://interactive-examples.mdn.mozilla.net/media/cc0-audio/t-rex-roar.mp3";
const RATES = [1, 1.5, 2, 0.5];
const REPEATS: AudioRepeat[] = ["off", "all", "one"];

const TRACKS: Array<{ id: string; title: string; notes: number[] }> = [
  { id: "tone-c", title: "Tone · C major arpeggio", notes: [261.63, 329.63, 392, 523.25] },
  { id: "tone-a", title: "Tone · A minor arpeggio", notes: [220, 261.63, 329.63, 440] },
];

// 16-bit mono PCM WAV of `notes` played in sequence, faded to avoid clicks.
function toneWav(notes: number[], noteSeconds: number): Uint8Array {
  const rate = 22050;
  const perNote = Math.round(rate * noteSeconds);
  const samples = perNote * notes.length;
  const wav = new Uint8Array(44 + samples * 2);
  const view = new DataView(wav.buffer);
  const ascii = (offset: number, text: string) => {
    for (let i = 0; i < text.length; i++) view.setUint8(offset + i, text.charCodeAt(i));
  };
  ascii(0, "RIFF"); view.setUint32(4, 36 + samples * 2, true); ascii(8, "WAVE"); ascii(12, "fmt ");
  view.setUint32(16, 16, true); view.setUint16(20, 1, true); view.setUint16(22, 1, true);
  view.setUint32(24, rate, true); view.setUint32(28, rate * 2, true); view.setUint16(32, 2, true);
  view.setUint16(34, 16, true); ascii(36, "data"); view.setUint32(40, samples * 2, true);
  const fade = Math.round(rate * 0.02);
  for (let n = 0; n < notes.length; n++) {
    for (let i = 0; i < perNote; i++) {
      const envelope = Math.min(1, i / fade, (perNote - i) / fade);
      const value = Math.sin((i / rate) * 2 * Math.PI * notes[n]) * 6000 * envelope;
      view.setInt16(44 + (n * perNote + i) * 2, Math.round(value), true);
    }
  }
  return wav;
}

function formatMs(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms)) return "--:--";
  const total = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}

Page({
  data: {
    supported: true,
    earpiece: false,
    status: "idle",
    trackTitle: "Nothing loaded",
    index: -1,
    tracks: [] as Array<{ id: string; title: string }>,
    positionText: "--:--",
    durationText: "--:--",
    progress: 0,
    rate: 1,
    repeat: "off" as AudioRepeat,
    muted: false,
    volume: 1,
    output: "auto",
    routeText: "",
    soundReady: false,
    eventLog: [] as string[],
  },
  player: null as AudioPlayer | null,
  sound: null as Sound | null,
  unwatchRoute: null as (() => void) | null,
  titles: new Map<string, string>(),

  onLoad: async function () {
    const audio = lx.supports("audio") ? lx.audio : undefined;
    if (!audio) {
      this.setData({ supported: false });
      return;
    }
    this.setData({ earpiece: lx.supports("audio.output.earpiece") });
    this.unwatchRoute = audio.route.watch(({ route, reason }) => {
      this.setData({ routeText: route.name ? `${route.output} · ${route.name}` : route.output });
      this._log(`route ${route.output} (${reason})`);
    });

    const player = audio.createPlayer({ signal: this.signal });
    this.player = player;
    player.on("state", (state) => this._render(state));
    player.on("time", ({ positionMs, durationMs }) => {
      this.setData({
        positionText: formatMs(positionMs),
        durationText: formatMs(durationMs),
        progress: durationMs ? Math.min(1, positionMs / durationMs) : 0,
      });
    });
    player.on("item", ({ index, reason }) => this._log(`item ${index} (${reason})`));
    player.on("ended", ({ index }) => this._log(`ended ${index}`));
    player.on("error", ({ code }) => this._log(`error ${code}`));
    player.on("interruption", ({ phase, reason }) => this._log(`interruption ${phase} (${reason})`));

    try {
      await lx.fs.mkdir(ROOT, { recursive: true });
      const queue: AudioQueueItem[] = [];
      for (const track of TRACKS) {
        const path = `${ROOT}/${track.id}.wav`;
        await lx.fs.write(path, toneWav(track.notes, 0.6), { overwrite: true });
        queue.push({ source: path, id: track.id });
        this.titles.set(track.id, track.title);
      }
      queue.push({ source: REMOTE_TRACK, id: "remote" });
      this.titles.set("remote", "Remote · T-Rex roar (MDN, CC0)");
      this.setData({
        tracks: queue.map((item) => ({ id: item.id!, title: this.titles.get(item.id!)! })),
      });
      await player.setQueue(queue);

      this.sound = await audio.loadSound(
        { bytes: toneWav([880], 0.15), mimeType: "audio/wav" },
        { maxVoices: 4, signal: this.signal },
      );
      this.setData({ soundReady: true });
    } catch (error) {
      this._fail(error, "Audio setup failed");
    }
  },

  onUnload() {
    // The player and sound follow `this.signal`; the route watch does not.
    this.unwatchRoute?.();
    this.unwatchRoute = null;
  },

  _render(state: AudioPlayerState) {
    const id = state.item?.id;
    this.setData({
      status: state.status,
      index: state.index ?? -1,
      trackTitle: id ? this.titles.get(id) ?? id : "Nothing loaded",
      positionText: formatMs(state.positionMs),
      durationText: formatMs(state.durationMs),
      progress: state.durationMs ? Math.min(1, state.positionMs / state.durationMs) : 0,
      rate: state.rate,
      repeat: state.repeat,
      muted: state.muted,
      volume: state.volume,
      output: state.output,
    });
  },

  _log(line: string) {
    this.setData({ eventLog: [line, ...this.data.eventLog].slice(0, 8) });
  },

  _fail(error: unknown, fallback: string) {
    const message = errorMessage(error, fallback);
    this._log(message);
    lx.showToast({ title: message, icon: "none" });
  },

  async _run(label: string, command: (player: AudioPlayer) => Promise<unknown> | void) {
    if (!this.player) return;
    try {
      await command(this.player);
    } catch (error) {
      this._fail(error, `${label} failed`);
    }
  },

  togglePlay: async function () {
    await this._run("play", (p) => (p.state.status === "playing" ? p.pause() : p.play()));
  },

  stop: async function () {
    await this._run("stop", (p) => p.stop());
  },

  previous: async function () {
    await this._run("previous", (p) => p.previous());
  },

  next: async function () {
    await this._run("next", (p) => p.next());
  },

  skipTo: async function (params?: { index?: number }) {
    await this._run("skipTo", (p) => p.skipTo(params?.index ?? 0));
  },

  seekBy: async function (params?: { deltaMs?: number }) {
    await this._run("seek", (p) => p.seek(Math.max(0, p.state.positionMs + (params?.deltaMs ?? 0))));
  },

  cycleRate: async function () {
    await this._run("setRate", (p) => {
      p.setRate(RATES[(RATES.indexOf(p.state.rate) + 1) % RATES.length]);
    });
  },

  cycleRepeat: async function () {
    await this._run("setRepeat", (p) => {
      p.setRepeat(REPEATS[(REPEATS.indexOf(p.state.repeat) + 1) % REPEATS.length]);
    });
  },

  toggleMute: async function () {
    await this._run("setMuted", (p) => p.setMuted(!p.state.muted));
  },

  setVolume: async function (params?: { volume?: number }) {
    await this._run("setVolume", (p) => p.setVolume(params?.volume ?? 1));
  },

  toggleOutput: async function () {
    await this._run("setOutput", (p) => p.setOutput(p.state.output === "earpiece" ? "auto" : "earpiece"));
  },

  playSound: function (params?: { pan?: number }) {
    if (!this.sound) return;
    this.sound.play({ pan: params?.pan ?? 0 });
  },

  stopSounds: function () {
    this.sound?.stopAll();
  },
});
