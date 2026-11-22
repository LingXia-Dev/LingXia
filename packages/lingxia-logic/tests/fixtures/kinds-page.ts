import { definePage } from "@lingxia/logic";

export const trace: string[] = [];

export default definePage({
  data: { count: 0 },
  setup(page) {
    return {
      onLoad(params: { fail?: string }) {
        if (params.fail) throw new Error(params.fail);
        const tick = setInterval(() => trace.push("tick"), 5);
        page.signal.addEventListener("abort", () => clearInterval(tick));
      },
      onUnload() {
        trace.push(`unload aborted=${page.signal.aborted}`);
      },
      actions: {
        add(by: number) {
          page.update(d => { d.count += by; });
          return page.data.count;
        },
        async save(name: string, times: number) {
          return `${name}x${times}`;
        },
        async *count(to: number) {
          try {
            for (let i = 1; i <= to; i++) yield i;
          } finally {
            trace.push("count closed");
          }
        },
        poll(tag: string) {
          const id = setInterval(() => trace.push(tag), 5);
          page.signal.addEventListener("abort", () => clearInterval(id));
        },
        leak() {
          setInterval(() => trace.push("leak"), 5);
        },
        async later() {
          await Promise.resolve();
          setTimeout(() => trace.push("late"), 15);
        },
        video() {
          return page.video("v");
        },
      },
    };
  },
});
