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

let officeDownloadTask = null;
let officeDownloadSession = 0;

async function observeOfficeTask(page, task, sessionId) {
  for await (const event of task) {
    if (sessionId !== officeDownloadSession) {
      return;
    }

    if (event.kind === "progress") {
      const hasPreciseProgress =
        typeof event.progress === "number" &&
        Number.isFinite(event.progress) &&
        !!event.totalBytes &&
        event.totalBytes > 0;
      page.setData({
        officeDownloadState: "running",
        officeTransferButtonText: "Pause Download",
        officeProgressKnown: hasPreciseProgress,
        officeDownloadProgress: hasPreciseProgress
          ? Number((event.progress * 100).toFixed(1))
          : 0,
        officeProgressText: formatProgressText(
          event.progress,
          event.downloadedBytes,
          event.totalBytes,
        ),
      });
      continue;
    }

    if (event.kind === "paused") {
      page.setData({
        officeDownloadState: "paused",
        officeTransferButtonText: "Continue Download",
      });
      continue;
    }

    if (event.kind === "resumed") {
      page.setData({
        officeDownloadState: "running",
        officeTransferButtonText: "Pause Download",
        officeProgressText: page.data.officeProgressText || "Resuming transfer...",
      });
      continue;
    }

    if (event.kind === "canceled") {
      page.setData({
        officeDownloadState: "idle",
        officeTransferButtonText: "Pause Download",
        officeProgressText: "Download canceled",
      });
      continue;
    }

    if (event.kind === "success") {
      page.setData({
        officeDownloadState: "completed",
        officeTransferButtonText: "Pause Download",
        officeDownloadProgress: 100,
        officeProgressText: event.result?.filePath
          ? `Saved to ${event.result.filePath}`
          : "Download complete",
      });
    }
  }
}

Page({
  data: {
    activeDemo: "openFile",
    pdfUrl: "https://ontheline.trincoll.edu/images/bookdown/sample-local-pdf.pdf",
    officeUrl: "https://example-files.online-convert.com/document/docx/example.docx",
    officeFileType: "docx",
    showMenu: true,
    chooseFileDefaultPath: "",
    chooseFileStatusText: "Choose a file from usercache",
    chooseFileSelectedPath: "",
    chooseFileSelectedType: "",
    isPdfDownloading: false,
    isOfficeDownloading: false,
    officeDownloadState: "idle",
    officeProgressKnown: false,
    officeDownloadProgress: 0,
    officeProgressText: "Not started yet",
    officeSupportsTransferControl: false,
    officeTransferButtonText: "Pause Download",
  },

  onLoad: async function (options = {}) {
    const requestedSection = options?.section === "chooseFile" ? "chooseFile" : "openFile";
    this.setData({
      activeDemo: requestedSection,
      chooseFileDefaultPath: lx.env.USER_CACHE_PATH || "",
      chooseFileStatusText: "Choose a file from usercache",
    });
  },

  onUnload: function () {
    officeDownloadSession += 1;
    const task = officeDownloadTask;
    officeDownloadTask = null;
    if (task) {
      task.cancel().catch(() => {});
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
    this.setData({ isPdfDownloading: true });
    try {
      const response = await fetch(url);
      if (!response.ok) {
        throw new Error(`Fetch failed with ${response.status}`);
      }
      await lx.openURL({
        url: response.url || url,
        target: "self",
      });
    } catch (error) {
      lx.showToast({ title: error?.message || "Fetch failed", icon: "none" });
    } finally {
      this.setData({ isPdfDownloading: false });
    }
  },

  toggleOfficeTransfer: async function () {
    const task = officeDownloadTask;
    if (!task || !supportsTransferControl(task)) {
      lx.showToast({ title: "Current runtime does not support pause/resume yet", icon: "none" });
      return;
    }

    try {
      if (this.data.officeDownloadState === "paused") {
        this.setData({
          officeTransferButtonText: "Resuming...",
          officeProgressText: this.data.officeProgressText || "Resuming transfer...",
        });
        await task.resume();
        return;
      }

      if (this.data.officeDownloadState === "running") {
        this.setData({
          officeTransferButtonText: "Pausing...",
        });
        await task.pause();
      }
    } catch (error) {
      lx.showToast({ title: error?.message || "Transfer action failed", icon: "none" });
    }
  },

  toggleSection: function ({ section } = {}) {
    if (!section || !this.data.expandedSections || !(section in this.data.expandedSections)) {
      return;
    }
    this.setData({
      [`expandedSections.${section}`]: !this.data.expandedSections[section],
    });
  },

  chooseFileFromUserCache: async function () {
    try {
      const result = await lx.chooseFile({
        title: "Choose file from usercache",
        defaultPath: this.data.chooseFileDefaultPath,
      });
      if (result.canceled || !Array.isArray(result.paths) || result.paths.length === 0) {
        this.setData({
          chooseFileStatusText: "Choose file canceled",
          chooseFileSelectedPath: "",
          chooseFileSelectedType: "",
        });
        return;
      }

      const selectedPath = result.paths[0];
      const fileType = detectFileTypeFromPath(selectedPath);
      this.setData({
        chooseFileStatusText: "Selected from usercache",
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
    if (officeDownloadTask) return;

    const task = lx.downloadFile({ url });
    officeDownloadTask = task;
    const sessionId = officeDownloadSession + 1;
    officeDownloadSession = sessionId;
    const canObserveProgress = supportsDownloadProgress(task);
    const canControlTransfer = supportsTransferControl(task);
    this.setData({
      isOfficeDownloading: true,
      officeDownloadState: "running",
      officeProgressKnown: false,
      officeDownloadProgress: 0,
      officeProgressText: canObserveProgress
        ? "Starting transfer..."
        : "Downloading with promise-style result only...",
      officeSupportsTransferControl: canControlTransfer,
      officeTransferButtonText: canControlTransfer
        ? "Pause Download"
        : "Pause/Continue Unavailable",
    });

    const observer = canObserveProgress
      ? observeOfficeTask(this, task, sessionId)
      : Promise.resolve();

    try {
      const result = await task;
      await observer;
      if (sessionId !== officeDownloadSession) {
        return;
      }
      this.setData({
        officeDownloadState: "completed",
        officeProgressKnown: true,
        officeDownloadProgress: 100,
        officeProgressText: `Saved to ${result.filePath}`,
      });
      await lx.openFile({
        filePath: result.filePath,
        fileType,
        mode: "auto",
        showMenu: this.data.showMenu,
      });
    } catch (error) {
      await observer.catch(() => {});
      if (sessionId !== officeDownloadSession) {
        return;
      }
      this.setData({
        officeDownloadState: "idle",
        officeSupportsTransferControl: false,
        officeTransferButtonText: "Pause Download",
        officeProgressText: isCancelError(error)
          ? "Download canceled"
          : error?.message || "Download failed",
      });
      if (!isCancelError(error)) {
        lx.showToast({ title: error?.message || "Download failed", icon: "none" });
      }
    } finally {
      if (sessionId === officeDownloadSession) {
        officeDownloadTask = null;
        this.setData({
          isOfficeDownloading: false,
          officeSupportsTransferControl: false,
        });
      }
    }
  },
});
