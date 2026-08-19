import assert from "node:assert/strict";
import { compileInlineNativeForest, compileInlineNativeRoot } from "../dist/inline-native/structure.js";
import { INLINE_NATIVE_SCHEMA } from "../dist/inline-native/schema.js";

assert.deepEqual(
  [...INLINE_NATIVE_SCHEMA.publicComponents],
  [
    "LxNativeRoot",
    "LxNativeView",
    "LxNativeCover",
    "LxNativeText",
    "LxNativeButton",
    "LxNativeSlider",
    "LxVideo",
  ]
);
assert.deepEqual([...INLINE_NATIVE_SCHEMA.coreKinds], ["root", "view", "text", "tappable", "slider"]);
assert.ok(INLINE_NATIVE_SCHEMA.hostFactoryKinds.includes("video"));
assert.equal(INLINE_NATIVE_SCHEMA.recipes.LxNativeCover.expandsTo, "view");
assert.equal(INLINE_NATIVE_SCHEMA.recipes.LxNativeButton.expandsTo, "tappable");

const valid = compileInlineNativeRoot({
  type: "LxNativeRoot",
  authorId: "player",
  props: { fullscreenScope: "root" },
  children: [
    {
      type: "LxVideo",
      authorId: "hero",
      props: { src: "https://cdn.example.com/a.mp4", controls: false, "aria-label": "Demo" },
    },
    {
      type: "LxNativeCover",
      props: { scrim: "bottom", scrimOpacity: 0.6 },
      children: [
        { type: "LxNativeText", children: "Title" },
        {
          type: "LxNativeView",
          children: [
            {
              type: "LxNativeButton",
              props: { icon: "play", "aria-label": "Play" },
            },
            {
              type: "LxNativeSlider",
              props: { value: 12, max: 100, "aria-label": "Playback position", valueLabel: "time" },
            },
          ],
        },
      ],
    },
  ],
});

assert.equal(valid.ok, true, valid.ok ? "" : valid.error.message);
assert.equal(valid.root.kind, "root");
assert.equal(valid.root.children.length, 2);
assert.equal(valid.root.children[0].kind, "video");
assert.equal(valid.root.children[0].authorType, "LxVideo");
assert.equal(valid.root.children[1].kind, "view");
assert.equal(valid.root.children[1].authorType, "LxNativeCover");
assert.deepEqual(valid.root.children[1].props.scrimPaint, { scrim: "bottom", opacity: 0.6 });
assert.equal(valid.root.children[1].props.pointerEvents, "box-none");
assert.deepEqual(valid.root.children[1].props.coverPreset, { position: "absolute", inset: 0 });

const coverKids = valid.root.children[1].children;
assert.equal(coverKids[0].kind, "text");
assert.equal(coverKids[0].text, "Title");
assert.equal(coverKids[1].kind, "view");
assert.equal(coverKids[1].authorType, "LxNativeView");
const bar = coverKids[1].children;
assert.equal(bar[0].kind, "tappable");
assert.equal(bar[0].authorType, "LxNativeButton");
assert.deepEqual(bar[0].props.content.icon, { kind: "semantic", name: "play" });
assert.equal(bar[1].kind, "slider");
assert.equal(bar[1].authorType, "LxNativeSlider");
assert.equal(bar[1].props.valueLabel, "time");

const bareVideo = compileInlineNativeRoot({
  type: "LxVideo",
  props: { src: "https://cdn.example.com/a.mp4" },
});
assert.equal(bareVideo.ok, false);
assert.equal(bareVideo.error.code, "NATIVE_ROOT_INVALID_STRUCTURE");
assert.match(bareVideo.error.message, /explicit LxNativeRoot/);

const forestBare = compileInlineNativeForest([
  { type: "LxVideo", props: { src: "x" } },
]);
assert.equal(forestBare.ok, false);
assert.equal(forestBare.error.code, "NATIVE_ROOT_INVALID_STRUCTURE");

const nestedRoot = compileInlineNativeRoot({
  type: "LxNativeRoot",
  children: [{ type: "LxNativeRoot", children: [] }],
});
assert.equal(nestedRoot.ok, false);
assert.equal(nestedRoot.error.code, "NATIVE_ROOT_INVALID_STRUCTURE");
assert.match(nestedRoot.error.message, /cannot nest/);

const videoInCover = compileInlineNativeRoot({
  type: "LxNativeRoot",
  children: [
    {
      type: "LxNativeCover",
      children: [{ type: "LxVideo", props: { src: "x", controls: false } }],
    },
  ],
});
assert.equal(videoInCover.ok, false);
assert.equal(videoInCover.error.code, "NATIVE_ROOT_INVALID_STRUCTURE");
assert.match(videoInCover.error.message, /direct child/);

const videoInView = compileInlineNativeRoot({
  type: "LxNativeRoot",
  children: [
    {
      type: "LxNativeView",
      children: [{ type: "LxVideo", props: { src: "x", controls: false } }],
    },
  ],
});
assert.equal(videoInView.ok, false);
assert.equal(videoInView.error.code, "NATIVE_ROOT_INVALID_STRUCTURE");

const domChild = compileInlineNativeRoot({
  type: "LxNativeRoot",
  children: [{ type: "div", children: "nope" }],
});
assert.equal(domChild.ok, false);
assert.equal(domChild.error.code, "NATIVE_ROOT_INVALID_STRUCTURE");
assert.match(domChild.error.message, /DOM or unregistered/);

const controlsPlusSlider = compileInlineNativeRoot({
  type: "LxNativeRoot",
  children: [
    { type: "LxVideo", props: { src: "x" } },
    {
      type: "LxNativeCover",
      children: [{ type: "LxNativeSlider", props: { "aria-label": "Seek" } }],
    },
  ],
});
assert.equal(controlsPlusSlider.ok, false);
assert.equal(controlsPlusSlider.error.code, "NATIVE_ROOT_INVALID_STRUCTURE");
assert.match(controlsPlusSlider.error.message, /Slider/);

const controlsPlusPlay = compileInlineNativeRoot({
  type: "LxNativeRoot",
  children: [
    { type: "LxVideo", props: { src: "x", controls: true } },
    {
      type: "LxNativeButton",
      props: { icon: "play", "aria-label": "Play" },
    },
  ],
});
assert.equal(controlsPlusPlay.ok, false);
assert.equal(controlsPlusPlay.error.code, "NATIVE_ROOT_INVALID_STRUCTURE");
assert.match(controlsPlusPlay.error.message, /play/);

const controlsPlusMore = compileInlineNativeRoot({
  type: "LxNativeRoot",
  children: [
    { type: "LxVideo", props: { src: "x", controls: true } },
    {
      type: "LxNativeButton",
      props: { icon: "more", "aria-label": "More" },
    },
  ],
});
assert.equal(controlsPlusMore.ok, true, controlsPlusMore.ok ? "" : controlsPlusMore.error.message);
assert.equal(controlsPlusMore.root.children[1].kind, "tappable");

const defaultControlsIsTrue = compileInlineNativeRoot({
  type: "LxNativeRoot",
  children: [{ type: "lx-video", props: { src: "x" } }],
});
assert.equal(defaultControlsIsTrue.ok, true);
assert.equal(defaultControlsIsTrue.root.children[0].props.controls, true);

const falseLiteral = compileInlineNativeRoot({
  type: "LxNativeRoot",
  children: [{ type: "lx-video", props: { src: "x", controls: "false" } }],
});
assert.equal(falseLiteral.ok, true);
assert.equal(falseLiteral.root.children[0].props.controls, false);

const htmlBooleanVideo = compileInlineNativeRoot({
  type: "LxNativeRoot",
  children: [{
    type: "LxVideo",
    props: {
      src: "https://cdn.example.com/a.mp4",
      controls: false,
      autoplay: "",
      volume: "0.8",
      qualities: JSON.stringify([{ label: "HD", url: "https://cdn.example.com/hd.mp4" }]),
    },
  }],
});
assert.equal(htmlBooleanVideo.ok, true, htmlBooleanVideo.ok ? "" : htmlBooleanVideo.error.message);
assert.equal(htmlBooleanVideo.root.children[0].props.autoplay, true);
assert.equal(htmlBooleanVideo.root.children[0].props.volume, 0.8);
assert.deepEqual(htmlBooleanVideo.root.children[0].props.qualities, [
  { label: "HD", url: "https://cdn.example.com/hd.mp4" },
]);
