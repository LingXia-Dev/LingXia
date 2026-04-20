function detectFileType(url) {
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

function detectFileTypeFromPath(filePath) {
  if (!filePath) return "";
  const trimmed = String(filePath).split("?")[0].split("#")[0];
  const fileName = trimmed.split("/").pop() || "";
  const dotIndex = fileName.lastIndexOf(".");
  const ext = dotIndex >= 0 ? fileName.slice(dotIndex + 1).toLowerCase() : "";
  return ext;
}

function formatBytes(bytes) {
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

function formatProgressText(progress, downloadedBytes, totalBytes) {
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

function isCancelError(error) {
  const message = String(error?.message || error || "");
  return /cancel|abort/i.test(message);
}

function downloadResultPath(result) {
  return result?.filePath || result?.tempFilePath || "";
}

function supportsDownloadProgress(task) {
  return !!(
    task &&
    typeof task === "object" &&
    typeof task.next === "function" &&
    typeof task[Symbol.asyncIterator] === "function"
  );
}

function supportsTransferControl(task) {
  return !!(
    task &&
    typeof task === "object" &&
    typeof task.pause === "function" &&
    typeof task.resume === "function"
  );
}

let pdfDownloadTask = null;
let pdfDownloadObserver = null;
let pdfDownloadUrl = "";
let pdfDownloadPage = null;
let pdfOpenRunId = 0;

function guessFileNameFromUrl(url, fallbackExt = "") {
  const clean = String(url || "").split("#")[0].split("?")[0];
  const fileName = clean.split("/").pop() || "";
  if (fileName && fileName.includes(".")) {
    return fileName;
  }
  const ext = fallbackExt ? `.${fallbackExt.replace(/^\./, "")}` : "";
  return `fetched-file-${Date.now()}${ext}`;
}

function sanitizeFileName(name) {
  const sanitized = String(name || "")
    .replace(/[<>:"/\\|?*\x00-\x1F]/g, "-")
    .replace(/\s+/g, " ")
    .trim();
  return sanitized || `file-${Date.now()}`;
}

function userDataFilePath(fileName) {
  const base = (lx.env.USER_DATA_PATH || "lx://userdata").replace(/\/+$/, "");
  return `${base}/${sanitizeFileName(fileName)}`;
}

function updatePdfPage(data) {
  if (pdfDownloadPage) {
    pdfDownloadPage.setData(data);
  }
}

async function observePdfTask(task) {
  try {
    for await (const event of task) {
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
            ? Number((event.progress * 100).toFixed(1))
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
          pdfProgressText: pdfDownloadPage?.data?.pdfProgressText || "Resuming transfer...",
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
      pdfProgressText: error?.message || "Download failed",
    });
  } finally {
    pdfDownloadObserver = null;
  }
}

Page({
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

  onLoad: async function (options = {}) {
    pdfDownloadPage = this;
    const requestedSection = options?.section === "chooseFile" ? "chooseFile" : "openFile";
    const hasPausedPdf = pdfDownloadTask && this.data.pdfUrl?.trim() === pdfDownloadUrl;
    this.setData({
      activeDemo: requestedSection,
      chooseFileDefaultPath: lx.env.USER_DATA_PATH || "",
      chooseFileStatusText: "Choose a file",
      isPdfDownloading: false,
      pdfDownloadState: hasPausedPdf ? "paused" : "idle",
      pdfProgressKnown: false,
      pdfDownloadProgress: 0,
      pdfProgressText: hasPausedPdf ? "Download paused, tap to continue" : "",
      pdfSupportsTransferControl: Boolean(hasPausedPdf),
      pdfTransferButtonText: hasPausedPdf ? "Continue Download" : "Pause Download",
      isOfficeFetching: false,
      officeStatusText: "",
    });
  },

  onUnload: function () {
    pdfOpenRunId += 1;
    pdfDownloadPage = null;
    const task = pdfDownloadTask;
    if (task && supportsTransferControl(task)) {
      task.pause().catch(() => {});
    }
  },

  onPdfUrlInput: function (event) {
    this.setData({ pdfUrl: event?.detail?.value || "" });
  },

  onOfficeUrlInput: function (event) {
    const value = event?.detail?.value || "";
    const detectedType = detectFileType(value);
    this.setData({
      officeUrl: value,
      officeFileType: detectedType || this.data.officeFileType,
    });
  },

  onOfficeFileTypeInput: function (event) {
    this.setData({ officeFileType: event?.detail?.value || "" });
  },

  toggleShowMenu: function () {
    this.setData({ showMenu: !this.data.showMenu });
  },

  openPdf: async function () {
    const url = this.data.pdfUrl?.trim();
    if (!url) {
      lx.showToast({ title: "Please enter PDF URL", icon: "none" });
      return;
    }
    if (this.data.isPdfDownloading) return;
    let task = pdfDownloadTask;
    const shouldResumeExisting = Boolean(task && pdfDownloadUrl === url);
    this.setData({
      isPdfDownloading: true,
      pdfDownloadState: "running",
      pdfProgressKnown: false,
      pdfDownloadProgress: 0,
      pdfProgressText: shouldResumeExisting ? "Resuming transfer..." : "Starting transfer...",
      pdfSupportsTransferControl: false,
      pdfTransferButtonText: "Pause Download",
    });
    try {
      if (!shouldResumeExisting) {
        if (task && typeof task.cancel === "function") {
          task.cancel().catch(() => {});
        }
        task = lx.downloadFile({ url });
        pdfDownloadTask = task;
        pdfDownloadUrl = url;
      }
      const runId = pdfOpenRunId + 1;
      pdfOpenRunId = runId;
      const canObserveProgress = supportsDownloadProgress(task);
      const canControlTransfer = supportsTransferControl(task);
      this.setData({
        pdfSupportsTransferControl: canControlTransfer,
        pdfTransferButtonText: canControlTransfer
          ? "Pause Download"
          : "Pause/Continue Unavailable",
        pdfProgressText: canObserveProgress
          ? "Starting transfer..."
          : "Downloading with promise-style result only...",
      });
      if (canObserveProgress && !pdfDownloadObserver) {
        pdfDownloadObserver = observePdfTask(task);
      }
      if (canControlTransfer && shouldResumeExisting) {
        await task.resume();
      }
      const result = await task;
      if (runId !== pdfOpenRunId || pdfDownloadPage !== this) {
        return;
      }
      const resultPath = downloadResultPath(result);
      if (!resultPath) {
        throw new Error("downloadFile did not return a file path");
      }
      this.setData({
        pdfDownloadState: "opening",
        pdfProgressKnown: true,
        pdfDownloadProgress: 95,
        pdfProgressText: `Downloaded to ${resultPath}, opening...`,
      });
      await lx.openFile({
        filePath: resultPath,
        fileType: "pdf",
        mode: "auto",
        showMenu: this.data.showMenu,
      });
      if (runId !== pdfOpenRunId || pdfDownloadPage !== this) {
        return;
      }
      this.setData({
        pdfDownloadState: "completed",
        pdfProgressKnown: true,
        pdfDownloadProgress: 100,
        pdfProgressText: `Opened ${resultPath}`,
      });
    } catch (error) {
      if (pdfDownloadPage !== this) {
        return;
      }
      this.setData({
        pdfDownloadState: "idle",
        pdfSupportsTransferControl: false,
        pdfTransferButtonText: "Pause Download",
        pdfProgressText: isCancelError(error)
          ? "Download canceled"
          : error?.message || "Open PDF failed",
      });
      lx.showToast({ title: error?.message || "Open PDF failed", icon: "none" });
    } finally {
      if (pdfDownloadPage === this) {
        if (pdfDownloadTask === task && this.data.pdfDownloadState !== "paused") {
          pdfDownloadTask = null;
          pdfDownloadUrl = "";
        }
        this.setData({
          isPdfDownloading: false,
          pdfSupportsTransferControl: Boolean(pdfDownloadTask),
        });
      }
    }
  },

  togglePdfTransfer: async function () {
    const task = pdfDownloadTask;
    if (!task || !supportsTransferControl(task)) {
      lx.showToast({ title: "Current runtime does not support pause/resume yet", icon: "none" });
      return;
    }

    try {
      if (this.data.pdfDownloadState === "paused") {
        this.setData({
          pdfTransferButtonText: "Resuming...",
          pdfProgressText: this.data.pdfProgressText || "Resuming transfer...",
        });
        await task.resume();
        return;
      }

      if (this.data.pdfDownloadState === "running") {
        this.setData({
          pdfTransferButtonText: "Pausing...",
        });
        await task.pause();
      }
    } catch (error) {
      lx.showToast({ title: error?.message || "Transfer action failed", icon: "none" });
    }
  },

  chooseFileFromUserCache: async function () {
    try {
      const result = await lx.chooseFile({
        defaultPath: this.data.chooseFileDefaultPath,
      });
      if (result.canceled || !Array.isArray(result.paths) || result.paths.length === 0) {
        this.setData({
          chooseFileStatusText: "File selection canceled",
          chooseFileSelectedPath: "",
          chooseFileSelectedType: "",
        });
        return;
      }

      const selectedPath = result.paths[0];
      const fileType = detectFileTypeFromPath(selectedPath);
      this.setData({
        chooseFileStatusText: "File selected",
        chooseFileSelectedPath: selectedPath,
        chooseFileSelectedType: fileType || "unknown",
      });
    } catch (error) {
      lx.showToast({ title: error?.message || "chooseFile failed", icon: "none" });
      this.setData({
        chooseFileStatusText: error?.message || "chooseFile failed",
      });
    }
  },

  openChosenFile: async function () {
    const filePath = this.data.chooseFileSelectedPath;
    if (!filePath) {
      lx.showToast({ title: "Choose a file first", icon: "none" });
      return;
    }

    try {
      await lx.openFile({
        filePath,
        fileType: this.data.chooseFileSelectedType || undefined,
        mode: "auto",
        showMenu: this.data.showMenu,
      });
    } catch (error) {
      lx.showToast({ title: error?.message || "openFile failed", icon: "none" });
    }
  },

  openOffice: async function () {
    const url = this.data.officeUrl?.trim();
    const fileType = this.data.officeFileType?.trim();
    if (!url) {
      lx.showToast({ title: "Please enter document URL", icon: "none" });
      return;
    }
    if (!fileType) {
      lx.showToast({ title: "Please enter file type", icon: "none" });
      return;
    }
    if (this.data.isOfficeFetching) return;
    this.setData({
      isOfficeFetching: true,
      officeStatusText: "Fetching document...",
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
      await lx.getFileManager().writeFile({
        filePath,
        data: buffer,
        overwrite: true,
      });
      this.setData({
        officeStatusText: `Fetched ${formatBytes(buffer.byteLength)}, opening...`,
      });
      await lx.openFile({
        filePath,
        fileType,
        mode: "auto",
        showMenu: this.data.showMenu,
      });
      this.setData({
        officeStatusText: `Opened ${filePath}`,
      });
    } catch (error) {
      this.setData({
        officeStatusText: error?.message || "Fetch failed",
      });
      lx.showToast({ title: error?.message || "Fetch failed", icon: "none" });
    } finally {
      this.setData({ isOfficeFetching: false });
    }
  },
});
