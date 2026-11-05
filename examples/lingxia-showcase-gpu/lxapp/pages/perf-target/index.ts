Page({
  data: { rows: 20 },
  onLoad(options: { rows?: string } = {}) {
    const rows = Number(options.rows);
    if (Number.isFinite(rows) && rows > 0) this.setData({ rows: Math.min(rows, 400) });
  },
});
