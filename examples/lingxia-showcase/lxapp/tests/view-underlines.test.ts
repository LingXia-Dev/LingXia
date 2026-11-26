import { expect, test } from '@lingxia/test';
import { openDemo } from './open-demo';

// What the lines look like is for the eye: the shots go with the run, light and dark.
test('a dotted, a dashed and a struck line draw in their own colours', { covers: ['view:text-decoration'], start: openDemo('underlines') }, async (t) => {
  const view = t.app.view;
  const text = view.testId('lab-underlines-text');
  // A press on the link brings the demo into view.
  await view.testId('lab-underlines-link').click();
  await expect(text).toContainText('API key');
  for (const theme of ['light', 'dark'] as const) {
    await view.environment({ appearance: theme, still: true });
    await view.settled();
    await t.attach(`underlines-${theme}.png`, await view.screenshot());
  }
  await view.environment(null);
});
