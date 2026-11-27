import { definePage } from '@lingxia/logic';
import { native } from '@lingxia/native';

const pause = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));

export default definePage({
  data: { greetCount: 0 },
  actions: page => ({
    // `native.hello.greet` is the Rust route in native/src/lib.rs; the View receives each yielded word.
    async *greet({ name }: { name: string }) {
      const count = page.data.greetCount + 1;
      page.update(d => { d.greetCount = count; });
      const greeting = await native.hello.greet({ name, count });
      for (const word of greeting.split(' ')) {
        await pause(120);
        yield word;
      }
    },
    reset() {
      page.update(d => { d.greetCount = 0; });
    },
  }),
});
