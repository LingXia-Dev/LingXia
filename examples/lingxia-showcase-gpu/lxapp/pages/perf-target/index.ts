import { definePage } from '@lingxia/types/page';
export default definePage({
  data: { rows: 20 },
  setup(page) {
    return {
      onLoad(options: { rows?: string } = {}) {
        const rows = Number(options.rows);
        if (Number.isFinite(rows) && rows > 0) page.update(d => { d.rows = Math.min(rows, 400); });
      },
    };
  },
});
