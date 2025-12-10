import React from 'react';
import './styles.css';

type PageState = {
  greeting: string;
  greetCount: number;
};

type PageActions = {
  data: PageState;
  greet(payload: { name: string }): void;
};

declare function useLingXia(): PageActions;

export default function HomePage() {
  const { data, greet } = useLingXia();
  const [name, setName] = React.useState('');
  const [pending, setPending] = React.useState(false);

  React.useEffect(() => {
    if (pending) {
      const timer = setTimeout(() => setPending(false), 500);
      return () => clearTimeout(timer);
    }
  }, [data.greetCount, pending]);

  const submit = () => {
    const value = name.trim();
    if (!value) return;
    setPending(true);
    greet({ name: value });
  };

  return (
    <div className="home-screen">
      <div className="card">
        <p className="eyebrow">LingXia + React</p>
        <h1>Hello there 👋</h1>
        <p className="description">
          This starter demonstrates how logic data flows into a React view via the <code>useLingXia</code> hook.
        </p>
        <div className="form-row">
          <input
            value={name}
            placeholder="Enter a name"
            onChange={event => setName(event.target.value)}
            onKeyDown={event => {
              if (event.key === 'Enter') submit();
            }}
          />
          <button type="button" onClick={submit} disabled={pending}>
            {pending ? 'Sending...' : 'Greet'}
          </button>
        </div>
        <div className="greeting">{data.greeting}</div>
        <div className="meta">Invoked {data.greetCount} times</div>
      </div>
    </div>
  );
}
