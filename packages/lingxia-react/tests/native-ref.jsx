import React, { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { LxNativeRoot } from '../dist/native/LxNativeRoot.js';
import { LxNativeButton } from '../dist/native/LxNativeButton.js';

export async function run() {
  const result = { errors: [], handles: 0, press: 0 };
  const container = document.createElement('div'); document.body.append(container);
  const renderer = createRoot(container);
  renderer.render(<StrictMode><LxNativeRoot style={{ width: 320, height: 180 }} ref={handle => {
    if (!handle) return;
    try {
      if (handle.getLayout().width !== 320) throw Error('Root handle layout is incorrect');
      if (typeof handle.retry !== 'function') throw Error('Root handle lacks retry');
      result.handles++;
    } catch (error) { result.errors.push(String(error)); }
  }}><LxNativeButton id="strict-button" label="Play" onPress={() => result.press++} /></LxNativeRoot></StrictMode>);
  await new Promise(resolve => setTimeout(resolve, 200));
  container.querySelector('lx-native-button').dispatchEvent(new CustomEvent('press', { detail: { source: 'pointer' } }));
  renderer.unmount(); container.remove();
  return result;
}
