import { definePage } from '@lingxia/logic';
import { native } from '@lingxia/native';

export default definePage({
  data: { greeting: '', greetCount: 0 },
  setup: page => ({
    actions: {
      // `native.hello.greet` is the Rust route in native/src/lib.rs.
      async greet({ name }: { name: string }) {
        const count = page.data.greetCount + 1;
        const greeting = await native.hello.greet({ name, count });
        page.update(d => {
          d.greetCount = count;
          d.greeting = greeting;
        });
      },
    },
  }),
});
