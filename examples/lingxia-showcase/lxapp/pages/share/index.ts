function mediaPath(entry) {
  if (!entry) return "";
  if (typeof entry === "string") return entry;
  return entry.tempFilePath || entry.path || entry.filePath || "";
}

Page({
  data: {
    statusText: "Ready",
    selectedImagePath: "",
    selectedFilePath: "",
  },

  async _runShare(options) {
    try {
      this.setData({ statusText: "Opening system share sheet..." });
      const result = await lx.share(options);
      this.setData({
        statusText: result.completed === false ? "Share canceled" : "Share opened",
      });
    } catch (error) {
      const message = error?.message || "share failed";
      this.setData({ statusText: message });
      lx.showToast({ title: message, icon: "none" });
    }
  },

  shareText: async function() {
    await this._runShare({
      title: "LingXia Showcase",
      text: "Plain text share; some receivers reject text-only content.",
    });
  },

  shareCurrentPage: async function() {
    await this._runShare({
      title: "LingXia Showcase",
      page: {
        query: {
          from: "share-demo",
        },
      },
    });
  },

  chooseImage: async function() {
    try {
      const result = await lx.chooseMedia({
        count: 1,
        mediaType: ["image"],
        sourceType: ["album", "camera"],
      });
      if (result.canceled) {
        this.setData({ statusText: "No image selected" });
        return;
      }
      this.setData({
        selectedImagePath: mediaPath(result.entries[0]),
        statusText: "Image selected",
      });
    } catch (error) {
      const message = error?.message || "chooseMedia failed";
      this.setData({ statusText: message });
      lx.showToast({ title: message, icon: "none" });
    }
  },

  shareSelectedImage: async function() {
    const path = this.data.selectedImagePath;
    if (!path) {
      lx.showToast({ title: "Choose an image first", icon: "none" });
      return;
    }
    await this._runShare({
      title: "LingXia image",
      files: [path],
    });
  },

  chooseFile: async function() {
    try {
      const result = await lx.chooseFile({ multiple: false });
      this.setData({
        selectedFilePath: result.canceled ? "" : result.paths[0],
        statusText: result.canceled ? "File selection canceled" : "File selected",
      });
    } catch (error) {
      const message = error?.message || "chooseFile failed";
      this.setData({ statusText: message });
      lx.showToast({ title: message, icon: "none" });
    }
  },

  shareSelectedFile: async function() {
    const path = this.data.selectedFilePath;
    if (!path) {
      lx.showToast({ title: "Choose a file first", icon: "none" });
      return;
    }
    await this._runShare({
      title: "LingXia file",
      files: [path],
    });
  },
});
