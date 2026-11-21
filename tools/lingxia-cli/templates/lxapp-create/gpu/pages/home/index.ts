import { definePage } from '@lingxia/types/page';

export default definePage({
  data: { greeting: '', greetCount: 0 },
  actions: page => ({
    greet({ name }: { name: string }) {
      const count = page.data.greetCount + 1;
      page.update(d => {
        d.greetCount = count;
        d.greeting = `Hello, ${name}! (#${count})`;
      });
    },
  }),
});
