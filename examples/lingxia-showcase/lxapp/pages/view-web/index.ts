import { labData } from '../../shared/lib/lab-data';
import { definePage } from '@lingxia/logic';

export default definePage({
  data: { ...labData() },
  setup(page) {
    return {
    };
  },
});
