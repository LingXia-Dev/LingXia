import {
  LX_RETURNED_OBJECT_SURFACES,
  LX_RUNTIME_SURFACES,
} from "@lingxia/types/testing";

export type CapabilityLayer = "logic" | "automation" | "object";

export interface Capability {
  /** Canonical cover tag, e.g. `lx.fs.readFile` or `DownloadTask.wait`. */
  name: string;
  layer: CapabilityLayer;
  /** Display bucket — the owning surface. */
  group: string;
}

export const LAYER_TITLE: Record<CapabilityLayer, string> = {
  logic: "Logic API (lx.*)",
  object: "Objects returned by lx APIs",
  automation: "Automation drivers",
};

/**
 * The layers a coverage report is about. The automation drivers are the
 * harness — how a suite tests — not the cross-platform API an lxapp calls, so
 * they are deliberately not measured as product coverage.
 */
export const MEASURED_LAYERS: readonly CapabilityLayer[] = ["logic", "object"];

export function isMeasured(layer: CapabilityLayer): boolean {
  return MEASURED_LAYERS.includes(layer);
}

function build(): Capability[] {
  const out: Capability[] = [];
  for (const surface of LX_RUNTIME_SURFACES) {
    const layer: CapabilityLayer = surface.layer === "logic" ? "logic" : "automation";
    for (const member of surface.members) {
      out.push({ name: `${surface.name}.${member}`, layer, group: surface.name });
    }
  }
  for (const surface of LX_RETURNED_OBJECT_SURFACES) {
    for (const member of surface.members) {
      out.push({ name: `${surface.name}.${member}`, layer: "object", group: surface.name });
    }
  }
  return out;
}

/**
 * The published public surface, straight from `@lingxia/types`. The report
 * measures a run against this rather than against the tags a suite happens to
 * declare, so a capability nobody wrote a spec for still shows as a hole.
 */
export const PUBLIC_CAPABILITIES: Capability[] = build();

export const CAPABILITY_INDEX: Map<string, Capability> = new Map(
  PUBLIC_CAPABILITIES.map((entry) => [entry.name, entry]),
);
