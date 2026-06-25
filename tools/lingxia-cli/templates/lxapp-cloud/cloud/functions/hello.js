import { greet } from "./shared/greeting.js";

// A LingXia cloud function. `lingxia dev` serves it in-process (no server, no
// login), using the same `lx.fn` contract as the real LingXiao runtime — so the
// API you call is identical whether mocked or real.
// Call it from Logic: await lx.cloud.invoke("hello", { name: "world" }).
export const hello = lx.fn("hello", (input) => ({ message: greet(input?.name) }));
