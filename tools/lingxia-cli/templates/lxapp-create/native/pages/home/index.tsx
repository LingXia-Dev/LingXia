import React from 'react';
import '../../app.css';
import { native } from '@lingxia/native';

export default function HomePage() {
  return (
    <main className="page">
      <section className="card">
        <h1>Hello, LingXia</h1>
        <p>Rust native APIs are available from the generated <code>native</code> module.</p>
      </section>
    </main>
  );
}
