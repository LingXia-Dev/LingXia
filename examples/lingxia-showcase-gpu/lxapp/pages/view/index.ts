Page({
  data: {},
  async open({ page }: { page: string }) { await lx.navigateTo({ page }); },
});
