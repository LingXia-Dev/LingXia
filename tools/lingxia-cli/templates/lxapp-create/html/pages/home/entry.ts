import { getActions, subscribe } from '@lingxia/html';

type PageData = { greeting?: string };
type PageActions = { greet(payload: { name: string }): void };

const nameInput = document.getElementById('name') as HTMLInputElement | null;
const btn = document.getElementById('btn') as HTMLButtonElement | null;
const greetingEl = document.getElementById('greeting');
const actions = getActions<PageActions>();

function submit() {
  const name = nameInput?.value.trim();
  if (name) actions.greet({ name });
}

btn?.addEventListener('click', submit);
nameInput?.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') submit();
});

subscribe((data: PageData) => {
  if (!greetingEl) return;
  if (data.greeting) {
    greetingEl.textContent = data.greeting;
    greetingEl.style.display = 'block';
  } else {
    greetingEl.textContent = '';
    greetingEl.style.display = 'none';
  }
});
