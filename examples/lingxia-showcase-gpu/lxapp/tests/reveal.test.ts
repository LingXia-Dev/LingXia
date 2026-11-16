import { spec } from '@lingxia/test';
import { openDemo } from './open-demo';

// `reveal(id)`: page code scrolls a child of a <scroll> into view.
spec('reveal scrolls a row into its scroll and back', { covers: ['view:scroll'], start: openDemo('jump') }, async (t) => {
  const view = t.app.view;
  const scroll = view.testId('lab-reveal-scroll');
  const inside = async (id: string) => {
    const [row, port] = [await view.testId(id).boundingBox(), await scroll.boundingBox()];
    return row.y >= port.y - 0.5 && row.y + row.height <= port.y + port.height + 0.5;
  };
  await t.waitFor(() => inside('lab-reveal-row-0'), { until: shown => shown });
  await t.waitFor(() => inside('lab-reveal-row-25'), { until: shown => !shown });
  await view.testId('lab-reveal-far').click();
  await t.waitFor(() => inside('lab-reveal-row-25'), { until: shown => shown });
  await view.testId('lab-reveal-first').click();
  await t.waitFor(() => inside('lab-reveal-row-0'), { until: shown => shown });
});
