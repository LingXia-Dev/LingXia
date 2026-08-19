import { spec } from "@lingxia/test";
import stubs from "./backlog-stubs.mjs";

for (const stub of stubs) {
  spec.skip(stub.title, {
    id: stub.id,
    covers: stub.covers,
    reason: `${stub.mode}: ${stub.reason}`,
  });
}
