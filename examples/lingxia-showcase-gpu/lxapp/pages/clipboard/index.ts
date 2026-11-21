import { errorMessage } from "../../shared/lib/errors";
import { definePage } from '@lingxia/types/page';

const SAMPLE_PNG =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

function mediaPath(entry: unknown): string {
  if (!entry) return "";
  if (typeof entry === "string") return entry;
  const media = entry as { uri?: string; path?: string; filePath?: string };
  return media.uri || media.path || media.filePath || "";
}

export default definePage({
  data: {
    statusText: "clipboard.state.ready",
    statusArgs: {} as Record<string, string>,
    typesText: "clipboard.state.notPeeked",
    readTextValue: "",
    imagePath: "",
  },
  setup(page) {
    const helpers = {
      async fail(error: unknown, fallback: string) {
        const message = errorMessage(error, fallback);
        page.update(d => { d.statusText = message; });
        lx.ui.toast({ title: message, icon: "none" });
      },
      async peekTypes() {
        const types = await lx.clipboard.types();
        page.update(d => { d.typesText = types.length > 0 ? types.join(", ") : "clipboard.state.empty"; });
        return types;
      },
      // After a read that got nothing — dismissed, empty, or denied — show what
      // the clipboard actually holds. `types` never prompts, so this is what tells
      // "the user said no" apart from "there was nothing to paste".
      async showTypesAfterRead() {
        try {
          await helpers.peekTypes();
        } catch (error) {
          console.warn("clipboard types failed", error);
        }
      },
    };
    const actions = {
      async goBack() { await lx.nav.back(); },
      async writeText(params?: { text?: string }) {
        const text = params?.text ?? "";
        try {
          await lx.clipboard.writeText(text);
          await helpers.peekTypes();
          page.update(d => { d.statusText = text === "" ? "clipboard.state.wroteEmptyString" : "clipboard.state.wroteText"; });
        } catch (error) {
          await helpers.fail(error, "writeText failed");
        }
      },
      async readText() {
        try {
          const result = await lx.clipboard.readText();
          if (result.status === 'canceled') {
            page.update(d => {
              d.statusText = "clipboard.state.readCanceled";
              d.readTextValue = "";
            });
            await helpers.showTypesAfterRead();
            return;
          }
          if (result.status === 'empty') {
            page.update(d => {
              d.statusText = "clipboard.state.noTextOnClipboard";
              d.readTextValue = "";
            });
            await helpers.showTypesAfterRead();
            return;
          }
          page.update(d => {
            d.statusText = result.text === "" ? "clipboard.state.foundEmptyString" : "clipboard.state.readText";
            d.readTextValue = result.text;
          });
          await helpers.peekTypes();
        } catch (error) {
          await helpers.fail(error, "readText failed");
          await helpers.showTypesAfterRead();
        }
      },
      async writeTypedText(params?: { text?: string }) {
        const text = params?.text ?? "";
        try {
          await lx.clipboard.write({ type: "text", text });
          await helpers.peekTypes();
          page.update(d => { d.statusText = "clipboard.state.wroteTypedTextItem"; });
        } catch (error) {
          await helpers.fail(error, "write failed");
        }
      },
      async writeSampleImage() {
        try {
          const root = `${lx.fs.userCache}/clipboard-demo`;
          const fixture = `${root}/sample.png`;
          await lx.fs.mkdir(root, { recursive: true });
          await lx.fs.write(fixture, SAMPLE_PNG, { encoding: "base64", overwrite: true });
          await lx.clipboard.write({ type: "image", filePath: fixture });
          await helpers.peekTypes();
          page.update(d => { d.statusText = "clipboard.state.wroteSamplePng"; });
        } catch (error) {
          await helpers.fail(error, "write image failed");
        }
      },
      async chooseAndWriteImage() {
        try {
          const picked = await lx.pick.media({
            count: 1,
            mediaType: ["image"],
            sourceType: ["album", "camera"],
          });
          if (picked.status === 'canceled') {
            page.update(d => { d.statusText = "clipboard.state.imagePickerCanceled"; });
            return;
          }
          const filePath = mediaPath(picked.files[0]);
          await lx.clipboard.write({ type: "image", filePath });
          await helpers.peekTypes();
          page.update(d => { d.statusText = "clipboard.state.wroteChosenImage"; });
        } catch (error) {
          await helpers.fail(error, "write image failed");
        }
      },
      async readAll() {
        try {
          const result = await lx.clipboard.read();
          if (result.status === 'canceled') {
            page.update(d => { d.statusText = "clipboard.state.readCanceled"; });
            await helpers.showTypesAfterRead();
            return;
          }
          if (result.status === 'empty') {
            page.update(d => {
              d.statusText = "clipboard.state.clipboardEmpty";
              d.readTextValue = "";
              d.imagePath = "";
            });
            await helpers.showTypesAfterRead();
            return;
          }
          const textItem = result.items.find((item) => item.type === "text");
          const imageItem = result.items.find((item) => item.type === "image");
          page.update(d => {
            d.statusText = "clipboard.state.readTypes";
            d.statusArgs = { types: result.items.map((item) => item.type).join(", ") };
            d.readTextValue = textItem?.type === "text" ? textItem.text : "";
            d.imagePath = imageItem?.type === "image" ? imageItem.filePath : "";
          });
          await helpers.peekTypes();
        } catch (error) {
          await helpers.fail(error, "read failed");
          await helpers.showTypesAfterRead();
        }
      },
      async peekTypes() {
        try {
          await helpers.peekTypes();
          page.update(d => { d.statusText = "clipboard.state.peekedTypes"; });
        } catch (error) {
          await helpers.fail(error, "types failed");
        }
      },
      async clearClipboard() {
        try {
          await lx.clipboard.clear();
          await helpers.peekTypes();
          page.update(d => {
            d.statusText = "clipboard.state.cleared";
            d.readTextValue = "";
            d.imagePath = "";
          });
        } catch (error) {
          await helpers.fail(error, "clear failed");
        }
      },
    };
    return {
      actions,
    };
  },
});
