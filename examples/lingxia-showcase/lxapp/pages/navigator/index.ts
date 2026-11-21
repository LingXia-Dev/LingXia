import { definePage } from '@lingxia/logic';
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
