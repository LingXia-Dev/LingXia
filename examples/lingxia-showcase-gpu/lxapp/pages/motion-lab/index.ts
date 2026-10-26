// `interrupt` reverses the fade 100 ms in, closer than adb can tap twice.
Page({
  data: { dipped: false },
  interrupt() {
    this.setData({ dipped: true });
    setTimeout(() => this.setData({ dipped: false }), 100);
  },
});
