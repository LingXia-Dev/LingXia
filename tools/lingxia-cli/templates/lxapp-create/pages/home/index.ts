import { definePage } from '@lingxia/logic';

export default definePage({
  data: { greeting: '', greetCount: 0 },
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
