import type { LxChannel, LxStream } from "@lingxia/bridge";
import type { PageActions } from "@lingxia/page-runtime";
import * as React from "react";
import type { DeepReadonly } from "@lingxia/page-runtime";
import {
  LxMediaSwiper,
  LxPicker,
  LxVideo,
  useLxChannel,
  useLxPage,
  useLxStream,
  type LxMediaSwiperRef,
} from "../dist/index.js";

type Chunk = { n: number };
declare const none: () => LxStream<Chunk, "done">;
declare const optional: (params?: { query: string }) => LxStream<Chunk, "done">;
declare const required: (params: { query: string }) => LxStream<Chunk, "done">;
declare const channel: (params: { room: string }) => Promise<LxChannel<{ text: string }, { text: string }>>;

export function Hooks() {
  // No payload: options may be left out.
  const a = useLxStream(none);
  const latest: Chunk | undefined = a.data;
  const result: "done" | undefined = a.result;
  // An optional payload can be passed.
  useLxStream(optional, { params: { query: "x" } });
  useLxStream(optional);
  // A required payload must be given, as options and as `params`.
  useLxStream(required, { params: () => ({ query: "x" }) });
  // @ts-expect-error the method needs its payload
  useLxStream(required);
  // @ts-expect-error `params` is required for it
  useLxStream(required, { manual: true });
  // @ts-expect-error the payload has its method's type
  useLxStream(required, { params: { query: 1 } });

  // A reducer needs its starting value, and `data` is the accumulator.
  const reduced = useLxStream(none, {
    initial: { n: 0, count: 0 },
    reduce: (acc, chunk) => ({ n: chunk.n, count: acc.count + 1 }),
  });
  const count: number | undefined = reduced.data?.count;
  // @ts-expect-error `reduce` without `initial`
  useLxStream(none, { reduce: (acc: number[], chunk: Chunk) => [...acc, chunk.n] });

  const session = useLxChannel(channel, { params: { room: "a" } });
  const sent: boolean = session.send({ text: "hi" });
  // @ts-expect-error the channel needs its payload
  useLxChannel(channel);
  void latest; void result; void count; void sent;
  return null;
}

// Untyped actions take one optional payload.
export function Untyped() {
  const { actions } = useLxPage();
  void actions.save();
  void actions.save({ id: "a" });
  return null;
}

// The View side of a page contract.
type Actions = PageActions<{
  save(value: { id: string }): Promise<void>;
  move(from: string, to: string): void;
  feed(): AsyncGenerator<Chunk, "end", unknown>;
}>;
declare const actions: Actions;
void actions.save({ id: "a" });
// @ts-expect-error a page action takes at most one JSON payload
actions.move("a", "b");
const feed: LxStream<Chunk, "end"> = actions.feed();
void feed;

// Readonly page data goes straight into components.
type Page = DeepReadonly<{
  slides: { id: string; type: "image"; src: string }[];
  columns: string[][];
  choice: string[];
  rates: number[];
}>;
declare const page: Page;
export function Components() {
  const swiper = React.useRef<LxMediaSwiperRef | null>(null);
  swiper.current?.goToIndex(0);
  return (
    <>
      <LxMediaSwiper
        ref={swiper}
        items={page.slides}
        onChange={({ index, previousIndex, source }) => void [index, previousIndex, source]}
        onError={({ code }) => void code}
        onClick={() => {}}
      />
      <LxPicker columns={page.columns} value={page.choice} onKeyDown={() => {}} />
      <LxVideo playbackRates={page.rates} onDoubleClick={() => {}} />
    </>
  );
}
