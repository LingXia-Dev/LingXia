import type { ShareOptions } from "@lingxia/types";
import { errorMessage } from "../../shared/lib/errors";
import { definePage } from '@lingxia/types/page';
function mediaPath(entry: unknown): string {
  if (!entry) return "";
  if (typeof entry === "string") return entry;
  const media = entry as { uri?: string; path?: string; filePath?: string };
  return media.uri || media.path || media.filePath || "";
}

export default definePage({
  data: {
    statusText: "Ready",
    selectedImagePath: "",
    selectedFilePath: "",
  },
  setup(page) {
    const helpers = {
      async runShare(options: ShareOptions) {
        try {
          page.update(d => { d.statusText = "Opening system share sheet..."; });
          const result = await lx.share(options);
          page.update(d => {
            d.statusText = result.outcome === "dismissed"
                ? "Share canceled"
                : result.outcome === "completed"
                  ? "Shared"
                  : "Share opened";
          });
        } catch (error) {
          const message = errorMessage(error, "share failed");
          page.update(d => { d.statusText = message; });
          lx.showToast({ title: message, icon: "none" });
        }
      },
    };
    const actions = {
      async goBack() { await lx.navigateBack(); },
      async shareText() {
        await helpers.runShare({
          title: "LingXia Showcase",
          text: "Plain text share; some receivers reject text-only content.",
        });
      },
      async shareCurrentPage() {
        await helpers.runShare({
          title: "LingXia Showcase",
          page: {
            query: {
              from: "share-demo",
            },
          },
        });
      },
      async chooseImage() {
        try {
          const result = await lx.chooseMedia({
            count: 1,
            mediaType: ["image"],
            sourceType: ["album", "camera"],
          });
          if (result.status === 'canceled') {
            page.update(d => { d.statusText = "No image selected"; });
            return;
          }
          page.update(d => {
            d.selectedImagePath = mediaPath(result.entries[0]);
            d.statusText = "Image selected";
          });
        } catch (error) {
          const message = errorMessage(error, "chooseMedia failed");
          page.update(d => { d.statusText = message; });
          lx.showToast({ title: message, icon: "none" });
        }
      },
      async shareSelectedImage() {
        const path = page.data.selectedImagePath;
        if (!path) {
          lx.showToast({ title: "Choose an image first", icon: "none" });
          return;
        }
        await helpers.runShare({
          title: "LingXia image",
          files: [path],
        });
      },
      async chooseFile() {
        try {
          const result = await lx.chooseFile({ multiple: false });
          page.update(d => {
            d.selectedFilePath = (result.status === 'canceled') ? "" : result.paths[0];
            d.statusText = (result.status === 'canceled') ? "File selection canceled" : "File selected";
          });
        } catch (error) {
          const message = errorMessage(error, "chooseFile failed");
          page.update(d => { d.statusText = message; });
          lx.showToast({ title: message, icon: "none" });
        }
      },
      async shareSelectedFile() {
        const path = page.data.selectedFilePath;
        if (!path) {
          lx.showToast({ title: "Choose a file first", icon: "none" });
          return;
        }
        await helpers.runShare({
          title: "LingXia file",
          files: [path],
        });
      },
    };
    return {
      actions,
    };
  },
});
