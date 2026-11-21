import { definePage, lx, type DownloadTask, type DownloadResult } from '@lingxia/logic';
import { eventDetail, type NativeEvent } from "../../shared/lib/native-events";
import { errorMessage } from "../../shared/lib/errors";

/** The slice of the page the module-level download helpers write back to. */
// The page instance a download reports to; the download outlives a page.
interface PdfPage {
  readonly data: { readonly pdfProgressText?: unknown };
  update(recipe: (draft: Record<string, unknown>) => undefined): void;
}

type InputEventDetail = { value: string };
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

function formatProgressText(
  progress: number | undefined,
  downloadedBytes: number | undefined,
  totalBytes: number | undefined,
) {
  const downloaded = formatBytes(downloadedBytes || 0);
  if (typeof progress === "number" && Number.isFinite(progress) && totalBytes && totalBytes > 0) {
    const percent = `${Math.round(progress * 100)}%`;
    return `${percent} · ${downloaded} / ${formatBytes(totalBytes)}`;
  }
  if (totalBytes && totalBytes > 0) {
    return `${downloaded} / ${formatBytes(totalBytes)}`;
  }
  return `Streaming · ${downloaded} downloaded`;
}

function isCancelError(error: unknown) {
  const message = errorMessage(error, String(error ?? ""));
  return /cancel|abort/i.test(message);
}

function downloadResultPath(result: DownloadResult | undefined) {
  return result?.uri || "";
}

function supportsDownloadProgress(task: unknown): task is DownloadTask {
  const candidate = task as Partial<DownloadTask> | null;
  return !!(
    candidate &&
    typeof candidate === "object" &&
    candidate.progress &&
    typeof candidate.progress[Symbol.asyncIterator] === "function"
  );
}

function supportsTransferControl(task: unknown) {
  const candidate = task as Partial<DownloadTask> | null;
  return !!(
    candidate &&
    typeof candidate === "object" &&
    typeof candidate.pause === "function" &&
    typeof candidate.resume === "function"
  );
}

let pdfDownloadTask: DownloadTask | null = null;
let pdfDownloadObserver: Promise<void> | null = null;
let pdfDownloadUrl = "";
let pdfDownloadPage: PdfPage | null = null;
let pdfOpenRunId = 0;

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

function updatePdfPage(data: Record<string, unknown>) {
  if (pdfDownloadPage) {
    pdfDownloadPage.update(d => { Object.assign(d, data); });
  }
}

async function observePdfTask(task: DownloadTask) {
  try {
    for await (const event of task.progress) {
      if (event.kind === "progress") {
        const hasPreciseProgress =
          typeof event.progress === "number" &&
          Number.isFinite(event.progress) &&
          !!event.totalBytes &&
          event.totalBytes > 0;
        updatePdfPage({
          pdfDownloadState: "running",
          pdfTransferButtonText: "Pause Download",
          pdfProgressKnown: hasPreciseProgress,
          pdfDownloadProgress: hasPreciseProgress
            ? Number(((event.progress ?? 0) * 100).toFixed(1))
            : 0,
          pdfProgressText: formatProgressText(
            event.progress,
            event.downloadedBytes,
            event.totalBytes,
          ),
        });
        continue;
      }

      if (event.kind === "paused") {
        updatePdfPage({
          pdfDownloadState: "paused",
          pdfTransferButtonText: "Continue Download",
        });
        continue;
      }

      if (event.kind === "resumed") {
        updatePdfPage({
          pdfDownloadState: "running",
          pdfTransferButtonText: "Pause Download",
          pdfProgressText: pdfDownloadPage?.data.pdfProgressText || "Resuming transfer...",
        });
        continue;
      }

      if (event.kind === "canceled") {
        updatePdfPage({
          pdfDownloadState: "idle",
          pdfTransferButtonText: "Pause Download",
          pdfProgressText: "Download canceled",
        });
        continue;
      }

      if (event.kind === "completed") {
        const resultPath = downloadResultPath(event.result);
        updatePdfPage({
          pdfDownloadState: "opening",
          pdfTransferButtonText: "Pause Download",
          pdfProgressKnown: true,
          pdfDownloadProgress: 95,
          pdfProgressText: resultPath
            ? `Downloaded to ${resultPath}, opening...`
            : "Download complete, opening...",
        });
      }
    }
  } catch (error) {
    updatePdfPage({
      pdfDownloadState: "idle",
      pdfTransferButtonText: "Pause Download",
      pdfProgressText: errorMessage(error, "Download failed"),
    });
  } finally {
    pdfDownloadObserver = null;
  }
}

export default definePage({
  data: {
    activeDemo: "openFile",
    pdfUrl: "https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf",
    officeUrl: "https://example-files.online-convert.com/document/docx/example.docx",
    officeFileType: "docx",
    showMenu: true,
    chooseFileDefaultPath: "",
    chooseFileStatusText: "Choose a file",
    chooseFileSelectedPath: "",
    chooseFileSelectedType: "",
    isPdfDownloading: false,
    pdfDownloadState: "idle",
    pdfProgressKnown: false,
    pdfDownloadProgress: 0,
    pdfProgressText: "",
    pdfSupportsTransferControl: false,
    pdfTransferButtonText: "Pause Download",
    isOfficeFetching: false,
    officeStatusText: "",
  },
  setup(page) {
    const actions = {
      async goBack() { await lx.nav.back(); },
      selectSection({ section }: { section: string }) {
        if (section === "openFile" || section === "chooseFile") {
          page.update(d => { d.activeDemo = section; });
        }
      },
      onPdfUrlInput(event: NativeEvent<InputEventDetail>) {
        page.update(d => { d.pdfUrl = eventDetail(event).value || ""; });
      },
      onOfficeUrlInput(event: NativeEvent<InputEventDetail>) {
        const value = eventDetail(event).value || "";
        const detectedType = detectFileType(value);
        page.update(d => {
          d.officeUrl = value;
          d.officeFileType = detectedType || page.data.officeFileType;
        });
      },
      onOfficeFileTypeInput(event: NativeEvent<InputEventDetail>) {
        page.update(d => { d.officeFileType = eventDetail(event).value || ""; });
      },
      toggleShowMenu() {
        page.update(d => { d.showMenu = !page.data.showMenu; });
      },
      async openPdf() {
        const url = page.data.pdfUrl?.trim();
        if (!url) {
          lx.ui.toast({ title: "Please enter PDF URL", icon: "none" });
          return;
        }
        if (page.data.isPdfDownloading) return;
        let task = pdfDownloadTask;
        const shouldResumeExisting = Boolean(task && pdfDownloadUrl === url);
        page.update(d => {
          d.isPdfDownloading = true;
          d.pdfDownloadState = "running";
          d.pdfProgressKnown = false;
          d.pdfDownloadProgress = 0;
          d.pdfProgressText = shouldResumeExisting ? "Resuming transfer..." : "Starting transfer...";
          d.pdfSupportsTransferControl = false;
          d.pdfTransferButtonText = "Pause Download";
        });
        try {
          if (!shouldResumeExisting) {
            if (task && typeof task.cancel === "function") {
              task.cancel().catch(() => {});
            }
            task = lx.network.download(url);
            pdfDownloadTask = task;
            pdfDownloadUrl = url;
          }
          const runId = pdfOpenRunId + 1;
          pdfOpenRunId = runId;
          const canObserveProgress = supportsDownloadProgress(task);
          const canControlTransfer = supportsTransferControl(task);
          page.update(d => {
            d.pdfSupportsTransferControl = canControlTransfer;
            d.pdfTransferButtonText = canControlTransfer
              ? "Pause Download"
              : "Pause/Continue Unavailable";
            d.pdfProgressText = canObserveProgress
              ? "Starting transfer..."
              : "Downloading with promise-style result only...";
          });
          if (!task) {
            throw new Error("network.download did not return a task");
          }
          if (canObserveProgress && !pdfDownloadObserver) {
            pdfDownloadObserver = observePdfTask(task);
          }
          if (canControlTransfer && shouldResumeExisting) {
            await task.resume();
          }
          const result = await task.result;
          if (runId !== pdfOpenRunId || pdfDownloadPage !== page) {
            return;
          }
          const resultPath = downloadResultPath(result);
          if (!resultPath) {
            throw new Error("network.download did not return a file path");
          }
          page.update(d => {
            d.pdfDownloadState = "opening";
            d.pdfProgressKnown = true;
            d.pdfDownloadProgress = 95;
            d.pdfProgressText = `Downloaded to ${resultPath}, opening...`;
          });
          await lx.openFile({
            filePath: resultPath,
            fileType: "pdf",
            mode: "auto",
            showMenu: page.data.showMenu,
          });
          if (runId !== pdfOpenRunId || pdfDownloadPage !== page) {
            return;
          }
          page.update(d => {
            d.pdfDownloadState = "completed";
            d.pdfProgressKnown = true;
            d.pdfDownloadProgress = 100;
            d.pdfProgressText = `Opened ${resultPath}`;
          });
        } catch (error) {
          if (pdfDownloadPage !== page) {
            return;
          }
          page.update(d => {
            d.pdfDownloadState = "idle";
            d.pdfSupportsTransferControl = false;
            d.pdfTransferButtonText = "Pause Download";
            d.pdfProgressText = isCancelError(error)
              ? "Download canceled"
              : errorMessage(error, "Open PDF failed");
          });
          lx.ui.toast({ title: errorMessage(error, "Open PDF failed"), icon: "none" });
        } finally {
          if (pdfDownloadPage === page) {
            if (pdfDownloadTask === task && page.data.pdfDownloadState !== "paused") {
              pdfDownloadTask = null;
              pdfDownloadUrl = "";
            }
            page.update(d => {
              d.isPdfDownloading = false;
              d.pdfSupportsTransferControl = Boolean(pdfDownloadTask);
            });
          }
        }
      },
      async togglePdfTransfer() {
        const task = pdfDownloadTask;
        if (!task || !supportsTransferControl(task)) {
          lx.ui.toast({ title: "Current runtime does not support pause/resume yet", icon: "none" });
          return;
        }

        try {
          if (page.data.pdfDownloadState === "paused") {
            // A recreated page must own a completion waiter before resuming.
            if (!page.data.isPdfDownloading) {
              await actions.openPdf();
              return;
            }
            page.update(d => {
              d.pdfTransferButtonText = "Resuming...";
              d.pdfProgressText = page.data.pdfProgressText || "Resuming transfer...";
            });
            await task.resume();
            return;
          }

          if (page.data.pdfDownloadState === "running") {
            page.update(d => { d.pdfTransferButtonText = "Pausing..."; });
            await task.pause();
          }
        } catch (error) {
          lx.ui.toast({ title: errorMessage(error, "Transfer action failed"), icon: "none" });
        }
      },
      async chooseFileFromUserCache() {
        try {
          const result = await lx.pick.file({
            defaultPath: page.data.chooseFileDefaultPath,
          });
          if (result.status === 'canceled') {
            page.update(d => {
              d.chooseFileStatusText = "File selection canceled";
              d.chooseFileSelectedPath = "";
              d.chooseFileSelectedType = "";
            });
            return;
          }

          const selectedPath = result.files[0].path;
          const fileType = detectFileTypeFromPath(result.files[0].name);
          page.update(d => {
            d.chooseFileStatusText = "File selected";
            d.chooseFileSelectedPath = selectedPath;
            d.chooseFileSelectedType = fileType || "unknown";
          });
        } catch (error) {
          lx.ui.toast({ title: errorMessage(error, "pick.file failed"), icon: "none" });
          page.update(d => { d.chooseFileStatusText = errorMessage(error, "pick.file failed"); });
        }
      },
      async openChosenFile() {
        const filePath = page.data.chooseFileSelectedPath;
        if (!filePath) {
          lx.ui.toast({ title: "Choose a file first", icon: "none" });
          return;
        }

        try {
          await lx.openFile({
            filePath,
            fileType: page.data.chooseFileSelectedType || undefined,
            mode: "auto",
            showMenu: page.data.showMenu,
          });
        } catch (error) {
          lx.ui.toast({ title: errorMessage(error, "openFile failed"), icon: "none" });
        }
      },
      async openOffice() {
        const url = page.data.officeUrl?.trim();
        const fileType = page.data.officeFileType?.trim();
        if (!url) {
          lx.ui.toast({ title: "Please enter document URL", icon: "none" });
          return;
        }
        if (!fileType) {
          lx.ui.toast({ title: "Please enter file type", icon: "none" });
          return;
        }
        if (page.data.isOfficeFetching) return;
        page.update(d => {
          d.isOfficeFetching = true;
          d.officeStatusText = "Fetching document...";
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
          await lx.fs.write(filePath, buffer, {
            overwrite: true,
          });
          page.update(d => { d.officeStatusText = `Fetched ${formatBytes(buffer.byteLength)}, opening...`; });
          await lx.openFile({
            filePath,
            fileType,
            mode: "auto",
            showMenu: page.data.showMenu,
          });
          page.update(d => { d.officeStatusText = `Opened ${filePath}`; });
        } catch (error) {
          page.update(d => { d.officeStatusText = errorMessage(error, "Fetch failed"); });
          lx.ui.toast({ title: errorMessage(error, "Fetch failed"), icon: "none" });
        } finally {
          page.update(d => { d.isOfficeFetching = false; });
        }
      },
    };
    return {
      async onLoad(options = {}) {
        pdfDownloadPage = page;
        const requestedSection = options?.section === "chooseFile" ? "chooseFile" : "openFile";
        const hasPausedPdf = Boolean(pdfDownloadTask);
        page.update(d => {
          d.activeDemo = requestedSection;
          d.pdfUrl = hasPausedPdf ? pdfDownloadUrl : page.data.pdfUrl;
          d.chooseFileDefaultPath = lx.fs.userData || "";
          d.chooseFileStatusText = "Choose a file";
          d.isPdfDownloading = false;
          d.pdfDownloadState = hasPausedPdf ? "paused" : "idle";
          d.pdfProgressKnown = false;
          d.pdfDownloadProgress = 0;
          d.pdfProgressText = hasPausedPdf ? "Download paused, tap to continue" : "";
          d.pdfSupportsTransferControl = Boolean(hasPausedPdf);
          d.pdfTransferButtonText = hasPausedPdf ? "Continue Download" : "Pause Download";
          d.isOfficeFetching = false;
          d.officeStatusText = "";
        });
      },
      onUnload() {
        pdfOpenRunId += 1;
        pdfDownloadPage = null;
        const task = pdfDownloadTask;
        if (task && supportsTransferControl(task)) {
          task.pause().catch(() => {});
        }
      },
      actions,
    };
  },
});
