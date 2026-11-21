import { definePage } from '@lingxia/types/page';
export default definePage({
  data: {},
  setup() {
    return {
      onLoad(options) {
        console.log('Navigator page onLoad options:', options);
      },
    };
  },
});
