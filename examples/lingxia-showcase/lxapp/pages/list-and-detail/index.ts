import { definePage } from '@lingxia/logic';

export default definePage({
  data: {
    selectedId: null as string | null,
    items: [
      { id: 'ord-101', title: 'Order #101', status: 'Paid', detail: 'Express delivery to Shenzhen · 3 items' },
      { id: 'ord-102', title: 'Order #102', status: 'Processing', detail: 'Warehouse pickup · 1 item' },
      { id: 'ord-103', title: 'Order #103', status: 'Shipped', detail: 'Standard shipping · 5 items' },
    ],
  },
  setup(page) {
    return {
      onLoad(options) {
        // A deep link `?id=7` opens straight onto that detail; ListDetail handles the back path.
        if (typeof options?.id === 'string' && options.id.length > 0) {
          page.update(d => { d.selectedId = options.id as string; });
        }
      },
      actions: {
        selectItem(payload?: { id: string | null }) {
          page.update(d => { d.selectedId = payload?.id ?? null; });
        },
      },
    };
  },
});
