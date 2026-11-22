import { definePage } from '@lingxia/logic';
import { heldSnap, holdPage, holdPanel, type HeldChoice } from '../../shared/lib/held-snap';

export default definePage({
  data: { held: heldSnap() },
  setup(page) {
    return {
      actions: {
        holdPage: (choice: HeldChoice) => holdPage(page, choice),
        holdPanel: (choice: HeldChoice) => holdPanel(page, choice),
      },
    };
  },
});
