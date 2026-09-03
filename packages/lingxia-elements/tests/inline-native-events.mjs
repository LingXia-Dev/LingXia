import assert from "node:assert/strict";
import { applyIslandHostEvent } from "../dist/inline-native/events.js";
import { compileInlineNativeRoot } from "../dist/inline-native/structure.js";

const compiled = compileInlineNativeRoot({
  type: "LxNativeRoot",
  children: [
    { type: "LxVideo", authorId: "hero", props: { src: "https://cdn.example.com/a.mp4", controls: false } },
    {
      type: "LxNativeCover",
      authorId: "chrome",
      props: { scrim: "bottom", scrimOpacity: 0.6 },
      children: [
        {
          type: "LxNativeButton",
          authorId: "play",
          props: { icon: "play", "aria-label": "Play" },
        },
        {
          type: "LxNativeButton",
          authorId: "more",
          props: { icon: "more", "aria-label": "More" },
        },
      ],
    },
  ],
});
assert.equal(compiled.ok, true);
assert.equal(compiled.root.children[1].props.scrimPaint.scrim, "bottom");
assert.equal(compiled.root.children[1].props.pointerEvents, "box-none");
assert.deepEqual(compiled.root.children[1].children[0].props.content.icon, {
  kind: "semantic",
  name: "play",
});
assert.equal(compiled.root.children[1].children[1].kind, "tappable");

class FakeElement {
  events = [];
  dispatchEvent(event) {
    this.events.push(event);
    return true;
  }
}

const button = new FakeElement();
assert.equal(
  applyIslandHostEvent(button, { event: "press", detail: { source: "pointer" } }),
  true
);
assert.equal(button.events.length, 1);
assert.equal(button.events[0].type, "press");
assert.deepEqual(button.events[0].detail, { source: "pointer" });

assert.equal(applyIslandHostEvent(button, { event: "playing" }), false);
assert.equal(button.events.length, 1);
