import { errorMessage } from "../../shared/lib/errors";

const SAMPLE_PNG =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

function mediaPath(entry: unknown): string {
  if (!entry) return "";
  if (typeof entry === "string") return entry;
  const media = entry as { tempFilePath?: string; path?: string; filePath?: string };
  return media.tempFilePath || media.path || media.filePath || "";
}

Page({
  data: {
    statusText: "Ready",
    typesText: "Not peeked",
    readTextValue: "",
    imagePath: "",
  },

  async _fail(error: unknown, fallback: string) {
    const message = errorMessage(error, fallback);
    this.setData({ statusText: message });
    lx.showToast({ title: message, icon: "none" });
  },

  async _peekTypes() {
    const result = await lx.clipboard.types();
    if (result.canceled) {
      this.setData({ typesText: "Canceled" });
      return result;
    }
    this.setData({
      typesText: result.types.length > 0 ? result.types.join(", ") : "empty",
    });
    return result;
  },

  // After a read that got nothing — dismissed, empty, or denied — show what
  // the clipboard actually holds. `types` never prompts, so this is what tells
  // "the user said no" apart from "there was nothing to paste".
  async _showTypesAfterRead() {
    try {
      await this._peekTypes();
    } catch (error) {
      console.warn("clipboard types failed", error);
    }
  },

  writeText: async function (params?: { text?: string }) {
    const text = params?.text ?? "";
    try {
      await lx.clipboard.writeText(text);
      await this._peekTypes();
      this.setData({
        statusText: text === "" ? "Wrote empty string" : "Wrote text",
      });
    } catch (error) {
      await this._fail(error, "writeText failed");
    }
  },

  readText: async function () {
    try {
      const result = await lx.clipboard.readText();
      if (result.canceled) {
        this.setData({ statusText: "Read canceled", readTextValue: "" });
        await this._showTypesAfterRead();
        return;
      }
      if (result.empty) {
        this.setData({ statusText: "No text on clipboard", readTextValue: "" });
        await this._showTypesAfterRead();
        return;
      }
      this.setData({
        statusText: result.text === "" ? "Found empty string" : "Read text",
        readTextValue: result.text,
      });
      await this._peekTypes();
    } catch (error) {
      await this._fail(error, "readText failed");
      await this._showTypesAfterRead();
    }
  },

  writeTypedText: async function (params?: { text?: string }) {
    const text = params?.text ?? "";
    try {
      await lx.clipboard.write({ type: "text", text });
      await this._peekTypes();
      this.setData({ statusText: "Wrote typed text item" });
    } catch (error) {
      await this._fail(error, "write failed");
    }
  },

  writeSampleImage: async function () {
    try {
      const root = `${lx.env.USER_CACHE_PATH}/clipboard-demo`;
      const fixture = `${root}/sample.png`;
      await lx.fs.mkdir(root, { recursive: true });
      await lx.fs.write(fixture, SAMPLE_PNG, { encoding: "base64" });
      await lx.clipboard.write({ type: "image", filePath: fixture });
      await this._peekTypes();
      this.setData({ statusText: "Wrote sample PNG" });
    } catch (error) {
      await this._fail(error, "write image failed");
    }
  },

  chooseAndWriteImage: async function () {
    try {
      const picked = await lx.chooseMedia({
        count: 1,
        mediaType: ["image"],
        sourceType: ["album", "camera"],
      });
      if (picked.canceled) {
        this.setData({ statusText: "Image picker canceled" });
        return;
      }
      const filePath = mediaPath(picked.entries[0]);
      await lx.clipboard.write({ type: "image", filePath });
      await this._peekTypes();
      this.setData({ statusText: "Wrote chosen image" });
    } catch (error) {
      await this._fail(error, "write image failed");
    }
  },

  readAll: async function () {
    try {
      const result = await lx.clipboard.read();
      if (result.canceled) {
        this.setData({ statusText: "Read canceled" });
        await this._showTypesAfterRead();
        return;
      }
      if (result.empty) {
        this.setData({
          statusText: "Clipboard empty",
          readTextValue: "",
          imagePath: "",
        });
        await this._showTypesAfterRead();
        return;
      }
      const textItem = result.items.find((item) => item.type === "text");
      const imageItem = result.items.find((item) => item.type === "image");
      this.setData({
        statusText: `Read ${result.items.map((item) => item.type).join(", ")}`,
        readTextValue: textItem?.type === "text" ? textItem.text : "",
        imagePath: imageItem?.type === "image" ? imageItem.filePath : "",
      });
      await this._peekTypes();
    } catch (error) {
      await this._fail(error, "read failed");
      await this._showTypesAfterRead();
    }
  },

  peekTypes: async function () {
    try {
      const result = await this._peekTypes();
      this.setData({
        statusText: result.canceled ? "Types canceled" : "Peeked types",
      });
    } catch (error) {
      await this._fail(error, "types failed");
    }
  },

  clearClipboard: async function () {
    try {
      await lx.clipboard.clear();
      await this._peekTypes();
      this.setData({
        statusText: "Cleared",
        readTextValue: "",
        imagePath: "",
      });
    } catch (error) {
      await this._fail(error, "clear failed");
    }
  },
});
