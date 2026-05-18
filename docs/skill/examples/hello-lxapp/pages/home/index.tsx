// View layer. Runs in the WebView.
// `useLxPage` returns `{ data, actions }` — data is reactive, actions
// are bridge functions wired to the public methods on Page({}).
//
// Type fields as REQUIRED unless your Logic actually populates them
// lazily — the runtime guarantees both `data` and `actions` are wired
// by first paint. All-optional types create `?.()` and `??` noise.
import { useLxPage } from "@lingxia/react";

type PageData = {
  count: number;
  message: string;
};

type PageActions = {
  increment: () => void;
  reset: () => void;
};

export default function HomePage() {
  const { data, actions } = useLxPage<PageData, PageActions>();

  return (
    <main style={{ padding: 24, fontFamily: "system-ui" }}>
      <h1>Hello LxApp</h1>
      <p>{data.message}</p>
      <p style={{ fontSize: 48 }}>{data.count}</p>
      <button onClick={() => actions.increment()}>+1</button>
      <button onClick={() => actions.reset()} style={{ marginLeft: 8 }}>
        Reset
      </button>
    </main>
  );
}
