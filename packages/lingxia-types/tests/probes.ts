import type { Lx } from "../src/index.js";

declare const lx: Lx;
declare const url: string;

// Wrong keys name the right one.
// @ts-expect-error durationMs, not duration
lx.ui.toast("x", { duration: 1 });
// @ts-expect-error isHighAccuracy, not highAccuracy
lx.device.location({ highAccuracy: true });
// @ts-expect-error fileName, not fileNmae
lx.network.upload(url, { file: "a", fileNmae: "x" });

// Wrong literals list the allowed ones.
// @ts-expect-error not a haptic style
lx.device.haptics("medium");
// @ts-expect-error not a code format
lx.pick.code({ formats: ["qr"] });
// @ts-expect-error not a toast tone
lx.ui.toast("x", { tone: "warning" });

// Missing fields and exclusive options.
// @ts-expect-error confirm needs a title
lx.ui.confirm({});
// @ts-expect-error upload needs its options
lx.network.upload(url);
// @ts-expect-error quality excludes fps
lx.media.compressVideo("a.mp4", { quality: "low", fps: 3 });
// @ts-expect-error files exclude text
lx.share({ files: ["a"], text: "hello" });

// A picked result narrows on status before its payload.
async function picked() {
  const result = await lx.pick.file();
  // @ts-expect-error the file exists only once status is checked
  result.file;
  if (result.status === "ok") void result.file;
}
void picked;
