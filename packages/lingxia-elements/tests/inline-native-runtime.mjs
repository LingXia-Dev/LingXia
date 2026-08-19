import assert from "node:assert/strict";
import { compileInlineNativeRoot } from "../dist/inline-native/structure.js";
import { identifyCompiledRoot } from "../dist/inline-native/identity.js";
import { buildRootCommit } from "../dist/inline-native/commit.js";
import { buildGeometrySnapshot } from "../dist/inline-native/geometry.js";
import {
  viewAcceptLease,
  viewApplyGrant,
  viewCanShowFallback,
  emptyViewLease,
} from "../dist/inline-native/lease.js";
import {
  applyHostLeaseMessage,
  createRootRuntimeState,
  publishCompiledRoot,
} from "../dist/inline-native/runtime.js";

const compiled = compileInlineNativeRoot({
  type: "LxNativeRoot",
  children: [
    { type: "LxVideo", authorId: "hero", props: { src: "https://cdn.example.com/a.mp4", controls: false } },
    {
      type: "LxNativeCover",
      authorId: "chrome",
      children: [{ type: "LxNativeText", authorId: "title", children: "Hi" }],
    },
  ],
});
assert.equal(compiled.ok, true);

const rootRef = {
  surfaceInstanceId: "s",
  pageInstanceId: "p",
  documentInstanceId: "d",
  rootKey: "player",
  rootEpoch: 1,
};

const identified = identifyCompiledRoot(compiled.root, rootRef, null);
const commit = buildRootCommit(identified, null, 1);
assert.equal(commit.action, "root.commit");
assert.equal(commit.baseRevision, 0);
assert.equal(commit.revision, 1);
assert.ok(commit.operations.every((op) => op.op === "mount"));
assert.equal(commit.operations.length, 3);
assert.equal(commit.operations[0].node.kind, "video");
assert.equal(commit.operations[1].node.kind, "view");
assert.equal(commit.operations[2].node.parent.nodeKey, commit.operations[1].node.ref.nodeKey);

const geometry = buildGeometrySnapshot({
  identified,
  basisTreeRevision: commit.revision,
  geometryRevision: 7,
  rootOrder: 0,
  rootRect: { x: 0, y: 0, width: 320, height: 180 },
  nodeRects: {},
});
assert.equal(geometry.coordinateSpace, "page-unscrolled-css-px");
assert.equal(geometry.revision, 7);
assert.equal(geometry.roots[0].basisTreeRevision, 1);
assert.notEqual(geometry.revision, commit.revision);

const updated = compileInlineNativeRoot({
  type: "LxNativeRoot",
  children: [
    { type: "LxVideo", authorId: "hero", props: { src: "https://cdn.example.com/b.mp4", controls: false } },
    {
      type: "LxNativeCover",
      authorId: "chrome",
      children: [{ type: "LxNativeText", authorId: "title", children: "Hi" }],
    },
  ],
});
assert.equal(updated.ok, true);
const identified2 = identifyCompiledRoot(updated.root, rootRef, identified);
const commit2 = buildRootCommit(identified2, identified, 2);
assert.equal(commit2.baseRevision, 1);
assert.equal(commit2.operations.length, 1);
assert.equal(commit2.operations[0].op, "update");
assert.equal(commit2.operations[0].patch.src, "https://cdn.example.com/b.mp4");

import {
  buildVideoCommandRequest,
  collectVideoResourceUrls,
  validateControlsSnapshot,
} from "../dist/inline-native/video.js";

const videoUrls = collectVideoResourceUrls({
  src: "https://cdn.example.com/a.mp4",
  poster: "https://cdn.example.com/p.jpg",
  qualities: [{ id: "hd", label: "HD", url: "https://cdn.example.com/hd.mp4" }],
});
assert.deepEqual(videoUrls, [
  "https://cdn.example.com/a.mp4",
  "https://cdn.example.com/p.jpg",
  "https://cdn.example.com/hd.mp4",
]);

const command = buildVideoCommandRequest(
  identified.children[0].nodeRef,
  { name: "seek", seconds: 12 },
  "req-1"
);
assert.equal(command.action, "video.command");
assert.equal(command.command.name, "seek");

const snapshotOk = validateControlsSnapshot(
  {
    action: "video.controlsSemanticSnapshot",
    owner: identified.children[0].nodeRef,
    revision: 1,
    controls: [
      {
        controlId: "play",
        label: "Play",
        frame: { x: 0, y: 0, width: 24, height: 24 },
        visible: true,
        role: "button",
        actions: ["activate"],
      },
    ],
  },
  0
);
assert.equal(snapshotOk.ok, true);

const published = publishCompiledRoot({
  compiled: compiled.root,
  rootRef,
  state: createRootRuntimeState(),
  rootRect: { x: 0, y: 10, width: 320, height: 180 },
  nodeRects: { hero: { x: 0, y: 10, width: 320, height: 180 } },
});
assert.ok(published.messages.commit, "first tick must send root.commit");
assert.equal(published.messages.geometry.action, "geometry.snapshot");
assert.equal(published.messages.geometry.coordinateSpace, "page-unscrolled-css-px");
assert.equal(published.messages.geometry.roots[0].basisTreeRevision, published.messages.commit.revision);
assert.equal(published.messages.geometry.nodes[0].contentRect.width, 320);
const scrolled = publishCompiledRoot({
  compiled: compiled.root,
  rootRef,
  state: published.state,
  rootRect: { x: 0, y: 40, width: 320, height: 180 },
  nodeRects: { hero: { x: 0, y: 40, width: 320, height: 180 } },
});
assert.equal(scrolled.messages.commit, undefined);
assert.equal(scrolled.messages.geometry.revision, published.messages.geometry.revision + 1);
assert.equal(scrolled.messages.geometry.roots[0].basisTreeRevision, published.messages.commit.revision);
assert.equal(scrolled.messages.geometry.nodes[0].contentRect.y, 40);

const leased = applyHostLeaseMessage(
  published.state,
  { action: "root.leaseGranted", leaseId: "lease-1", sequence: 1, leaseDurationMs: 8000 },
  0
);
assert.ok(leased.leaseAccept);
assert.equal(leased.leaseAccept.action, "root.leaseAccept");
assert.equal(leased.ready, false);
const activated = applyHostLeaseMessage(leased.state, { action: "root.leaseActive" }, 0);
assert.equal(activated.ready, true);

const none = emptyViewLease();
assert.equal(viewCanShowFallback(none, 0), true);
const granted = viewApplyGrant("lease-1", 1, 8000, 0);
assert.equal(viewCanShowFallback(granted, 0), true);
const accepted = viewAcceptLease(granted);
assert.ok(accepted);
assert.equal(viewCanShowFallback(accepted, 0), false);
assert.equal(viewCanShowFallback(accepted, 8000), true);
