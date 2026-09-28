import { getPage, pageReady, subscribePage } from '@lingxia/html';

import type { HomePage } from './contract';

const nameInput = document.getElementById('name') as HTMLInputElement | null;
const btn = document.getElementById('btn') as HTMLButtonElement | null;
const greetingEl = document.getElementById('greeting');

function render() {
  const { data } = getPage<HomePage['data'], HomePage['actions']>();
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
  if (name) getPage<HomePage['data'], HomePage['actions']>().actions.greet({ name });
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
