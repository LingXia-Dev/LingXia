import React from 'react';
import { LxNavigator, useLxPage } from '@lingxia/react';
import '../../app.css';

type PageState = { greeting: string };
type PageActions = { greet(payload: { name: string }): void };

export default function HomePage() {
  const { data, actions } = useLxPage<PageState, PageActions>();
  const [name, setName] = React.useState('');
  const submit = () => name.trim() && actions.greet({ name: name.trim() });

  return (
    <main className="page">
      <section className="card">
        <h1>Hello, LingXia</h1>
        <div className="form">
          <input
            className="input"
            data-testid="home-name"
            value={name}
            placeholder="Enter your name"
            onChange={e => setName(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && submit()}
          />
          <button className="btn" data-testid="home-greet" onClick={submit} disabled={!name.trim()}>
            Say Hello
          </button>
        </div>
        {data.greeting && <p className="greeting" data-testid="home-greeting">{data.greeting}</p>}
        <LxNavigator url="https://www.lingxia.app" className="link">
          lingxia.app →
        </LxNavigator>
      </section>
    </main>
  );
}
