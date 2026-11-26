import { definePage, lx, type DownloadTask, type TransferProgress } from '@lingxia/logic';
import { failureText } from "../../shared/lib/errors";

function detectFileType(url: string | undefined) {
  if (!url) return "";
  const lowerUrl = url.toLowerCase();
  if (lowerUrl.endsWith(".pdf")) return "pdf";
  if (lowerUrl.endsWith(".doc")) return "doc";
  if (lowerUrl.endsWith(".docx")) return "docx";
  if (lowerUrl.endsWith(".xls")) return "xls";
  if (lowerUrl.endsWith(".xlsx")) return "xlsx";
  if (lowerUrl.endsWith(".ppt")) return "ppt";
  if (lowerUrl.endsWith(".pptx")) return "pptx";
  return "";
}

function detectFileTypeFromPath(filePath: string | undefined) {
  if (!filePath) return "";
  const trimmed = String(filePath).split("?")[0].split("#")[0];
  const fileName = trimmed.split("/").pop() || "";
  const dotIndex = fileName.lastIndexOf(".");
  const ext = dotIndex >= 0 ? fileName.slice(dotIndex + 1).toLowerCase() : "";
  return ext;
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  const digits = unitIndex === 0 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(digits)} ${units[unitIndex]}`;
}

function guessFileNameFromUrl(url: string, fallbackExt = "") {
  const clean = String(url || "").split("#")[0].split("?")[0];
  const fileName = clean.split("/").pop() || "";
  if (fileName && fileName.includes(".")) {
    return fileName;
  }
  const ext = fallbackExt ? `.${fallbackExt.replace(/^\./, "")}` : "";
  return `fetched-file-${Date.now()}${ext}`;
}

function sanitizeFileName(name: unknown) {
  const sanitized = String(name || "")
    .replace(/[<>:"/\\|?*\x00-\x1F]/g, "-")
    .replace(/\s+/g, " ")
    .trim();
  return sanitized || `file-${Date.now()}`;
}

function userDataFilePath(fileName: string) {
  const base = (lx.fs.userData || "lx://userdata").replace(/\/+$/, "");
  return `${base}/${sanitizeFileName(fileName)}`;
}

export default definePage({
  data: {
    activeDemo: "openFile",
    pdfUrl: "https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf",
    officeUrl: "https://example-files.online-convert.com/document/docx/example.docx",
    officeFileType: "docx",
    chooseFileDefaultPath: "",
    chooseFileStatusText: "file.state.choose",
    chooseFileSelectedPath: "",
    chooseFileSelectedType: "",
    isOfficeFetching: false,
    officeStatusText: "",
  },
  setup(page) {
    // The running download, for pause and resume; the stream below owns its outcome.
    let pdfTask: DownloadTask | null = null;
    const actions = {
      selectSection({ section }: { section: string }) {
        if (section === "openFile" || section === "chooseFile") {
          page.update(d => { d.activeDemo = section; });
        }
      },
      setPdfUrl({ url }: { url: string }) {
        page.update(d => { d.pdfUrl = url; });
      },
      setOfficeUrl({ url }: { url: string }) {
        const detectedType = detectFileType(url);
        page.update(d => {
          d.officeUrl = url;
          d.officeFileType = detectedType || page.data.officeFileType;
        });
      },
      setOfficeFileType({ type }: { type: string }) {
        page.update(d => { d.officeFileType = type; });
      },
      /** Download the PDF as one stream of progress, then open it; the stream returns the path. */
      async *downloadPdf(url: string): AsyncGenerator<TransferProgress, { path: string }> {
        const task = lx.network.download(url.trim());
        pdfTask = task;
        try {
          const { path } = yield* task;
          await lx.openFile(path, { mode: "auto" });
          return { path };
        } finally {
          if (pdfTask === task) pdfTask = null;
        }
      },
      async pausePdf() {
        await pdfTask?.pause();
      },
      async resumePdf() {
        await pdfTask?.resume();
      },
      async chooseFileFromUserCache() {
        try {
          const result = await lx.pick.file({
            defaultPath: page.data.chooseFileDefaultPath,
          });
          if (result.status === 'canceled') {
            page.update(d => {
              d.chooseFileStatusText = "file.state.canceled";
              d.chooseFileSelectedPath = "";
              d.chooseFileSelectedType = "";
            });
            return;
          }

          const selectedPath = result.file.path;
          const fileType = detectFileTypeFromPath(result.file.name);
          page.update(d => {
            d.chooseFileStatusText = "file.state.selected";
            d.chooseFileSelectedPath = selectedPath;
            d.chooseFileSelectedType = fileType || "unknown";
          });
        } catch (error) {
          lx.ui.toast(failureText(error, lx.app.t('file.pickFailed')));
          page.update(d => { d.chooseFileStatusText = failureText(error, lx.app.t('file.pickFailed')); });
        }
      },
      async openChosenFile() {
        const filePath = page.data.chooseFileSelectedPath;
        if (!filePath) {
          lx.ui.toast(lx.app.t('file.chooseFirst'));
          return;
        }

        try {
          await lx.openFile(filePath, { mode: "auto" });
        } catch (error) {
          lx.ui.toast(failureText(error, lx.app.t('common.requestFailed')));
        }
      },
      async openOffice() {
        const url = page.data.officeUrl?.trim();
        const fileType = page.data.officeFileType?.trim();
        if (!url) {
          lx.ui.toast(lx.app.t('file.enterDocUrl'));
          return;
        }
        if (!fileType) {
          lx.ui.toast(lx.app.t('file.enterType'));
          return;
        }
        if (page.data.isOfficeFetching) return;
        page.update(d => {
          d.isOfficeFetching = true;
          d.officeStatusText = lx.app.t('file.fetchingDocument');
        });
        try {
          const response = await fetch(url);
          if (!response.ok) {
            throw new Error(`Fetch failed with ${response.status}`);
          }
          const buffer = await response.arrayBuffer();
          const fileName = guessFileNameFromUrl(
            response.url || url,
            fileType || detectFileType(response.url || url),
          );
          const filePath = userDataFilePath(fileName);
          await lx.fs.write(filePath, { data: buffer, overwrite: true });
          page.update(d => { d.officeStatusText = lx.app.t('file.fetchedOpening', { size: formatBytes(buffer.byteLength) }); });
          await lx.openFile(filePath, { mode: "auto" });
          page.update(d => { d.officeStatusText = lx.app.t('file.opened', { path: filePath }); });
        } catch (error) {
          page.update(d => { d.officeStatusText = failureText(error, lx.app.t('file.fetchFailed')); });
          lx.ui.toast(failureText(error, lx.app.t('file.fetchFailed')));
        } finally {
          page.update(d => { d.isOfficeFetching = false; });
        }
      },
    };
    return {
      async onLoad(options = {}) {
        const requestedSection = options?.section === "chooseFile" ? "chooseFile" : "openFile";
        page.update(d => {
          d.activeDemo = requestedSection;
          d.chooseFileDefaultPath = lx.fs.userData || "";
          d.chooseFileStatusText = "file.state.choose";
          d.isOfficeFetching = false;
          d.officeStatusText = "";
        });
      },
      actions,
    };
  },
});
