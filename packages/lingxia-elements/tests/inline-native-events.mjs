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
          type: "LxNativeSlider",
          authorId: "seek",
          props: { value: 12, max: 100, "aria-label": "Position", valueLabel: "time" },
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
assert.equal(compiled.root.children[1].children[1].kind, "slider");

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

const slider = new FakeElement();
assert.equal(
  applyIslandHostEvent(slider, { event: "valuechange", detail: { value: 40 } }),
  true
);
assert.equal(
  applyIslandHostEvent(slider, { event: "valuecommit", detail: { value: 45 } }),
  true
);
assert.equal(slider.events.map((event) => event.type).join(","), "valuechange,valuecommit");
assert.equal(slider.events[1].detail.value, 45);
assert.equal(applyIslandHostEvent(slider, { event: "playing" }), false);
assert.equal(slider.events.length, 2);
