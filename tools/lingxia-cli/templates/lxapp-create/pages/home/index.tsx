import React from 'react';
import { LxNavigator } from '@lingxia/react';
import { useLingXia } from '@lingxia/react';
import '../../app.css';

type PageState = { greeting: string; greetCount: number };
type PageActions = { greet(payload: { name: string }): void };

export default function HomePage() {
  const { data, greet } = useLingXia<PageState, PageActions>();
  const [name, setName] = React.useState('');
  const submit = () => name.trim() && greet({ name: name.trim() });

  return (
    <div style={S.page}>
      <div style={S.card}>
        <img src="/public/AppIcon.png" style={S.logo} />
        <h1 style={S.title}>Hello, LingXia</h1>
        <p style={S.subtitle}>Build once, run everywhere</p>

        <div style={S.form}>
          <input
            value={name}
            placeholder="Enter your name"
            onChange={e => setName(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && submit()}
            style={S.input}
          />
          <button onClick={submit} disabled={!name.trim()} style={S.btn}>
            Say Hello
          </button>
        </div>

        {data.greeting && <p style={S.greeting}>{data.greeting}</p>}

        <div style={S.footer}>
          <LxNavigator url="https://www.lingxia.app" style={S.link}>
            Documentation →
          </LxNavigator>
        </div>
      </div>
    </div>
  );
}

const S: Record<string, React.CSSProperties> = {
  page: {
    minHeight: '100vh',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    padding: 20,
  },
  card: {
    width: '100%',
    maxWidth: 360,
    padding: 32,
    background: '#fff',
    borderRadius: 20,
    boxShadow: '0 4px 24px rgba(0,0,0,0.08)',
    textAlign: 'center',
  },
  logo: {
    width: 72,
    height: 72,
    borderRadius: 16,
    boxShadow: '0 2px 12px rgba(0,0,0,0.1)',
  },
  title: {
    margin: '20px 0 6px',
    fontSize: 26,
    fontWeight: 700,
    color: '#1d1d1f',
  },
  subtitle: {
    margin: 0,
    fontSize: 15,
    color: '#86868b',
  },
  form: {
    display: 'flex',
    flexDirection: 'column',
    gap: 12,
    marginTop: 28,
  },
  input: {
    width: '100%',
    height: 48,
    padding: '0 16px',
    border: '1px solid #d2d2d7',
    borderRadius: 12,
    fontSize: 16,
    outline: 'none',
    background: '#fafafa',
    transition: 'border-color 0.2s',
  },
  btn: {
    width: '100%',
    height: 48,
    border: 'none',
    borderRadius: 12,
    background: '#007aff',
    color: '#fff',
    fontSize: 16,
    fontWeight: 600,
    cursor: 'pointer',
    transition: 'background 0.2s',
  },
  greeting: {
    marginTop: 24,
    padding: 16,
    background: '#f0fdf4',
    border: '1px solid #bbf7d0',
    borderRadius: 12,
    color: '#166534',
    fontSize: 15,
    whiteSpace: 'pre-line',
    textAlign: 'left',
  },
  footer: {
    marginTop: 28,
    paddingTop: 20,
    borderTop: '1px solid #f0f0f0',
  },
  link: {
    color: '#007aff',
    fontSize: 14,
    fontWeight: 500,
    textDecoration: 'none',
  },
};
