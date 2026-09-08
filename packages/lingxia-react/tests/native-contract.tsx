import { LxVideo, LxNativeButton, LxNativeView } from "../dist/index.js";
import type { LxVideoProps as VueVideoProps, LxNativeButtonProps as VueButtonProps, LxNativeViewProps as VueViewProps } from "../../lingxia-vue/src/types.js";

const video = <LxVideo onTimeUpdate={({ currentTime }) => currentTime.toFixed(1)}
  onError={({ code, message }) => `${code}: ${message}`}
  onFullscreenChange={({ fullscreen }) => Boolean(fullscreen)}
  onVolumeChange={({ volume }) => volume.toFixed(2)} />;
const button = <LxNativeButton icon="play" label="Custom playback" />;
const view = <LxNativeView style={{ display: "flex", gap: 8, borderRadius: 12, backgroundColor: "black" }} />;
// @ts-expect-error Native video callbacks receive payloads, not DOM events.
const oldEvent = <LxVideo onTimeUpdate={(event: Event) => event.type} />;
// @ts-expect-error Resource icons have no cross-platform rendering contract yet.
const resource = <LxNativeButton icon={{ resource: "logo" }} />;
// @ts-expect-error Native surfaces do not paint CSS shadows.
const shadow = <LxNativeView style={{ boxShadow: "0 2px 4px black" }} />;
// @ts-expect-error Native video uses the same constrained style contract.
const videoShadow = <LxVideo style={{ boxShadow: "0 2px 4px black" }} />;

const vueVideo: VueVideoProps = { onTimeUpdate: ({ currentTime }) => currentTime.toFixed(1), onFullscreenChange: ({ fullscreen }) => Boolean(fullscreen) };
const vueButton: VueButtonProps = { icon: "play", disabled: true, hitSlop: 8, tabIndex: -1 };
// @ts-expect-error Vue shares the same icon contract.
const vueResource: VueButtonProps = { icon: { resource: "logo" } };
// @ts-expect-error Vue shares the same style contract.
const vueShadow: VueViewProps = { style: { boxShadow: "0 2px 4px black" } };
void [video, button, view, oldEvent, resource, shadow, videoShadow, vueVideo, vueButton, vueResource, vueShadow];

// @ts-expect-error Native focus is not a cross-platform framework event yet.
const nativeFocus = <LxNativeButton onFocus={() => {}} label="Focus" />;
void nativeFocus;

// @ts-expect-error Root-wide fullscreen is not an implemented contract.
<LxNativeRoot fullscreenScope="root" />;
// @ts-expect-error Visibility transitions are not an implemented contract.
<LxNativeRoot hiddenTransition="fade" />;
