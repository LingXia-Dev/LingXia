import { getPage, pageReady, subscribePage } from '@lingxia/html';

import type page from './index.ts';

const nameInput = document.getElementById('name') as HTMLInputElement | null;
const btn = document.getElementById('btn') as HTMLButtonElement | null;
const greetingEl = document.getElementById('greeting');

function render() {
  const { data } = getPage<typeof page>();
  if (!greetingEl) return;
  if (data.greeting) {
    greetingEl.textContent = data.greeting;
    greetingEl.style.display = 'block';
  } else {
    greetingEl.textContent = '';
    greetingEl.style.display = 'none';
  }
}

function submit() {
  const name = nameInput?.value.trim();
  if (name) getPage<typeof page>().actions.greet({ name });
}

btn?.addEventListener('click', submit);
nameInput?.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') submit();
});

// Plain HTML has no mount to gate: wait for the page's first state, then follow it.
void pageReady().then(() => {
  render();
  subscribePage(render);
});
