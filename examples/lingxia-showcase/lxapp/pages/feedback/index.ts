import { definePage, lx } from '@lingxia/logic';
export default definePage({
  data: {},
  setup(page) {
    const actions = {
      async submitFeedback(params: { category?: string; message?: string; email?: string }) {
        const category = typeof params?.category === "string" ? params.category.trim() : "Other";
        const message = typeof params?.message === "string" ? params.message.trim() : "";
        const email = typeof params?.email === "string" ? params.email.trim() : "";

        if (!message) {
          lx.ui.toast("Tell us what happened");
          return;
        }

        console.log("[Feedback] Submitted", { category, message, email });
        lx.ui.toast("Thanks for the feedback", { tone: "success" });
        await page.surface?.close();
      },
    };
    return {
      actions,
    };
  },
});
