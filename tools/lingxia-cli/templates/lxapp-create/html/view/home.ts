const nameInput = document.getElementById('name') as HTMLInputElement;
const btn = document.getElementById('btn')!;
const greetingEl = document.getElementById('greeting')!;

btn.onclick = () => {
  const name = nameInput.value.trim();
  if (name) greet({ name });
};

nameInput.onkeydown = (e) => {
  if (e.key === 'Enter') btn.click();
};

window.LingXiaBridge?.subscribe((data: { greeting: string }) => {
  if (data.greeting) {
    greetingEl.textContent = data.greeting;
    greetingEl.style.display = 'block';
  }
});
