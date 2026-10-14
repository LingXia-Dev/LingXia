import type { AudioPlayer, AudioPlayerStatus, Lx, Sound } from "../src/index.js";

declare const lx: Lx;

// `lx.audio` is optional: its presence and `lx.supports('audio')` agree.
// @ts-expect-error audio may be absent
lx.audio.createPlayer();

const audio = lx.audio!;
const player: AudioPlayer = audio.createPlayer({ focus: "duck", output: "auto", proximity: true });

// Events are typed through the event map and return their unsubscribe.
const off: () => void = player.on("state", (state) => {
  const status: AudioPlayerStatus = state.status;
  const position: number = state.positionMs;
  const duration: number | null = state.durationMs;
  void status, position, duration;
});
player.on("item", ({ index, item, reason }) => {
  const id: string | undefined = item.id;
  void index, id, reason;
});
player.on("interruption", ({ phase, reason, shouldResume, resumed }) => {
  void phase, reason, shouldResume, resumed;
});
// @ts-expect-error unknown events are rejected
player.on("canplay", () => {});
void off;

async function commands() {
  const info = await player.load("lx://lxapp/assets/intro.mp3", { autoplay: true });
  const index: number = info.index;
  await player.setQueue(
    [{ id: "a", source: { uri: "https://cdn.example.com/a.m3u8", mimeType: "application/x-mpegURL" } }],
    { startIndex: 0 },
  );
  await player.play();
  await player.seek(1_000);
  player.setVolume(0.5);
  player.setRepeat("all");
  // @ts-expect-error repeat modes are a closed set
  player.setRepeat("shuffle");
  // @ts-expect-error a sound cannot take exclusive focus
  await audio.loadSound("lx://lxapp/tap.wav", { focus: "exclusive" });
  const sound: Sound = await audio.loadSound({ bytes: new Uint8Array(0), mimeType: "audio/wav" });
  const voice = sound.play({ pan: -1, volume: 0.6 });
  await voice.ended;
  void index;
}
void commands;

const unwatch: () => void = audio.route.watch(({ route, reason }) => {
  void route.output, reason;
});
const unInterrupt: () => void = audio.onInterruption((event) => {
  const scope: "host" = event.scope;
  void scope;
});
void unwatch, unInterrupt;
