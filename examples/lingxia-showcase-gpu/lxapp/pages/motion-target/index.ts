// A Page registers under the file that calls it, so this copy of motion-lab declares its own.
Page({
  data: { dipped: false },
  interrupt() {
    this.setData({ dipped: true });
    setTimeout(() => this.setData({ dipped: false }), 100);
  },
});
