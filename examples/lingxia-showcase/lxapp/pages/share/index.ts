import { definePage, lx, type ShareOptions } from '@lingxia/logic';
import { errorMessage } from "../../shared/lib/errors";
function mediaPath(entry: unknown): string {
  if (!entry) return "";
  if (typeof entry === "string") return entry;
  const media = entry as { uri?: string; path?: string; filePath?: string };
  return media.uri || media.path || media.filePath || "";
}

export default definePage({
  data: {
    statusText: "share.state.ready",
    selectedImagePath: "",
    selectedFilePath: "",
  },
  setup(page) {
    const helpers = {
      async runShare(options: ShareOptions) {
        try {
          page.update(d => { d.statusText = "share.state.opening"; });
          const result = await lx.share(options);
          page.update(d => {
            d.statusText = result.outcome === "dismissed"
                ? "share.state.canceled"
                : result.outcome === "completed"
                  ? "share.state.shared"
                  : "share.state.opened";
          });
        } catch (error) {
          const message = errorMessage(error, "share failed");
          page.update(d => { d.statusText = message; });
          lx.ui.toast(message);
        }
      },
    };
    const actions = {
      async shareText() {
        await helpers.runShare({
          title: lx.app.t('share.title'),
          text: lx.app.t('share.plainText'),
        });
      },
      async shareCurrentPage() {
        await helpers.runShare({
          title: lx.app.t('share.title'),
          page: {
            query: {
              from: "share-demo",
            },
          },
        });
      },
      async chooseImage() {
        try {
          const result = await lx.pick.media({ limit: 1, types: ["image"] });
          if (result.status === 'canceled') {
            page.update(d => { d.statusText = "share.state.noImage"; });
            return;
          }
          page.update(d => {
            d.selectedImagePath = mediaPath(result.files[0]);
            d.statusText = "share.state.image";
          });
        } catch (error) {
          const message = errorMessage(error, "pick.media failed");
          page.update(d => { d.statusText = message; });
          lx.ui.toast(message);
        }
      },
      async shareSelectedImage() {
        const path = page.data.selectedImagePath;
        if (!path) {
          lx.ui.toast(lx.app.t('share.chooseImageFirst'));
          return;
        }
        await helpers.runShare({
          title: lx.app.t('share.imageTitle'),
          files: [path],
        });
      },
      async chooseFile() {
        try {
          const result = await lx.pick.file();
          page.update(d => {
            d.selectedFilePath = (result.status === 'canceled') ? "" : result.file.path;
            d.statusText = (result.status === 'canceled') ? "share.state.fileCanceled" : "share.state.file";
          });
        } catch (error) {
          const message = errorMessage(error, "chooseFile failed");
          page.update(d => { d.statusText = message; });
          lx.ui.toast(message);
        }
      },
      async shareSelectedFile() {
        const path = page.data.selectedFilePath;
        if (!path) {
          lx.ui.toast(lx.app.t('share.chooseFileFirst'));
          return;
        }
        await helpers.runShare({
          title: lx.app.t('share.fileTitle'),
          files: [path],
        });
      },
    };
    return {
      actions,
    };
  },
});
