import { spec } from "@lingxia/test";
import stubs from "./backlog-stubs.mjs";

const platform = globalThis.__LINGXIA_AUTOMATION_HOST__?.args?.platform;
for (const stub of stubs) {
  if (platform && stub.implementedOn?.includes(platform)) continue;
  spec.skip(stub.title, {
    id: stub.id,
    covers: stub.covers,
    reason: `${stub.mode}: ${stub.reason}`,
  });
}
