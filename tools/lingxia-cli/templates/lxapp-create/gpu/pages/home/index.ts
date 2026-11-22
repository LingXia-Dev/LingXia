import { definePage } from '@lingxia/logic';

const pause = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));

export default definePage({
  data: { greetCount: 0 },
  actions: page => ({
    // An `async *` action streams: the View receives each yielded word as it comes.
    async *greet({ name }: { name: string }) {
      const count = page.data.greetCount + 1;
      page.update(d => { d.greetCount = count; });
      for (const word of `Hello, ${name}! Greeting number ${count}.`.split(' ')) {
        await pause(120);
        yield word;
      }
    },
    reset() {
      page.update(d => { d.greetCount = 0; });
    },
  }),
});
