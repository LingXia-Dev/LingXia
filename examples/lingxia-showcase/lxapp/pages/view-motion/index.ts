import { definePage } from '@lingxia/logic';
export default definePage({
  data: { dipped: false },
  setup(thisPage) {
    const actions = {
      // Reverses the fade 100 ms in, sooner than two taps can land.
      interrupt() {
        thisPage.update(d => { d.dipped = true; });
        setTimeout(() => thisPage.update(d => { d.dipped = false; }), 100);
      },
    };
    return {
      actions,
    };
  },
});
