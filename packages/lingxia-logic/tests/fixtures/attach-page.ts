import { definePage, isLxError, lx } from "@lingxia/logic";

export default definePage({
  data: { attached: "", error: "" },
  setup(page) {
    return {
      actions: {
        async attach() {
          try {
            const picked = await lx.pick.file({ filters: [{ extensions: ["pdf"] }] });
            if (picked.status === "ok") page.update(d => { d.attached = picked.file.name; });
            return picked.status;
          } catch (error) {
            page.update(d => { d.error = isLxError(error) ? error.code : "unknown"; });
            return "failed";
          }
        },
      },
    };
  },
});
