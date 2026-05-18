import { useLxPage } from "@lingxia/react";

type PageData = { count: number };
type PageActions = { increment: () => void };

export default function HomePage() {
  const { data, actions } = useLxPage<PageData, PageActions>();
  return (
    <main style={{ padding: 24 }}>
      <h1>Hello from the host app</h1>
      <p>Count: {data.count}</p>
      <button onClick={() => actions.increment()}>+1</button>
    </main>
  );
}
