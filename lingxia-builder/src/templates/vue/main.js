import { createApp, ref } from "vue";
import App from "./App.vue";

window.useLingXiaData = function () {
  const dataInstance = ref({});

  if (window.LingXiaBridge && window.LingXiaBridge.subscribe) {
    window.LingXiaBridge.subscribe((newData) => {
      if (newData && dataInstance.value) {
        Object.assign(dataInstance.value, newData);
      }
    });
  }

  return dataInstance;
};

// Page functions injection
/* {{PAGE_FUNCTIONS}} */

// Create and configure Vue app
const app = createApp(App);

// Register page functions to Vue global properties (before mount)
window.__PAGE_FUNCTIONS.forEach((funcName) => {
  app.config.globalProperties[funcName] = window[funcName];
});

app.mount("#app");
