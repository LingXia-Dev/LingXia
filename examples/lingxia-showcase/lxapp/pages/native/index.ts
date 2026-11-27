import { definePage } from '@lingxia/logic';
import { isNativeError, native } from '@lingxia/native';

// Each action calls a `#[lingxia::native]` route in native/src/notes.rs.
export default definePage({
  data: {
    notes: [] as { id: number; title: string }[],
    exported: '',
    status: 'native.state.ready',
    statusArgs: {} as Record<string, string>,
  },
  actions: page => ({
    async add({ title }: { title: string }) {
      try {
        const note = await native.notes.add({ title });
        page.update(d => {
          d.notes.push(note);
          d.status = 'native.state.added';
          d.statusArgs = { id: String(note.id) };
        });
      } catch (error) {
        if (!isNativeError(error, 'EMPTY_TITLE')) throw error;
        page.update(d => { d.status = 'native.state.emptyTitle'; d.statusArgs = {}; });
      }
    },
    async findMissing() {
      try {
        await native.notes.get(page.data.notes.length + 1);
      } catch (error) {
        if (!isNativeError(error, 'NOT_FOUND')) throw error;
        page.update(d => { d.status = 'native.state.notFound'; d.statusArgs = { id: String(error.data.id) }; });
      }
    },
    // The route's progress events stream to the View; its return value is the exported text.
    async *export() {
      const task = native.notes.export();
      for await (const step of task) yield step;
      const exported = await task.result;
      page.update(d => { d.exported = exported; d.status = 'native.state.exported'; d.statusArgs = {}; });
    },
  }),
});
