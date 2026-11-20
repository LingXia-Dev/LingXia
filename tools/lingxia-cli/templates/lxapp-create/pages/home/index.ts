import { definePage } from '@lingxia/types/page';
import type { HomePage } from './contract';

export default definePage({
  data: { greeting: '', greetCount: 0 } satisfies HomePage['data'],
  setup: page => ({
    actions: {
      async greet({ name }: { name: string }) {
        const count = page.data.greetCount + 1;
        page.update(d => {
          d.greetCount = count;
          d.greeting = `Hello, ${name}! (#${count})`;
        });
      },
    },
  }),
});
