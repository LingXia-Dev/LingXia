// Helper function to detect file type from URL
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

function clampProgress(value) {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, Math.floor(value)));
}

function getFilenameFromUrl(url, fallbackExt) {
  let hash = 0;
  for (let i = 0; i < url.length; i++) {
    hash = (hash << 5) - hash + url.charCodeAt(i);
    hash |= 0;
  }
  const suffix = Math.abs(hash).toString(36);
  const ext = (fallbackExt || "bin").replace(/^\./, "");
  return `fetch_${suffix}.${ext}`;
}

function parseContentLength(response) {
  const lengthText = response?.headers?.get?.("content-length");
  const length = Number(lengthText);
  return Number.isFinite(length) && length > 0 ? length : 0;
}

function resolveOfficeFetchPath(url, fileType) {
  const fileName = getFilenameFromUrl(url, fileType);
  return `${lx.env.USER_CACHE_PATH}/${fileName}`;
}

function startPendingProgress(onProgress) {
  let value = 1;
  onProgress(value);
  const timer = setInterval(() => {
    if (value >= 90) return;
    value = Math.min(90, value + 2);
    onProgress(value);
  }, 180);
  return () => clearInterval(timer);
}

async function downloadWithFetchToFile(url, filePath, onProgress, signal) {
  let stopPendingProgress = startPendingProgress(onProgress);
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }
  if (!response.body) {
    throw new Error("Download response body is empty");
  }

  const file = await Rong.open(filePath, {
    write: true,
    create: true,
    truncate: true,
  });

  let writer;
  try {
    writer = file.writable.getWriter();
    const reader = response.body.getReader();
    if (signal) {
      signal.cancel = () => {
        signal.canceled = true;
        try {
          reader.cancel();
        } catch {}
      };
    }
    const totalBytes = parseContentLength(response);
    let downloadedBytes = 0;
    let fallbackProgress = 0;

    while (true) {
      if (signal?.canceled) {
        const abortError = new Error("Download canceled");
        abortError.name = "AbortError";
        throw abortError;
      }
      const { done, value } = await reader.read();
      if (done) {
        break;
      }
      if (!value || value.byteLength === 0) {
        continue;
      }
      if (stopPendingProgress) {
        stopPendingProgress();
        stopPendingProgress = null;
      }
      await writer.write(value);
      downloadedBytes += value.byteLength;
      if (totalBytes > 0) {
        onProgress(clampProgress((downloadedBytes / totalBytes) * 100));
      } else {
        fallbackProgress = Math.min(95, fallbackProgress + 3);
        onProgress(fallbackProgress);
      }
    }

    await writer.close();
    onProgress(100);
  } catch (error) {
    if (writer) {
      try {
        await writer.abort(error);
      } catch {}
    }
    throw error;
  } finally {
    if (stopPendingProgress) {
      stopPendingProgress();
      stopPendingProgress = null;
    }
    file.close();
  }
}

async function waitDownloadResult(task, handlers = {}) {
  let result = null;

  for await (const event of task) {
    if (event?.kind === "progress") {
      handlers.onProgress?.(clampProgress(event.progress * 100));
      continue;
    }
    if (event?.kind === "paused") {
      handlers.onPaused?.();
      continue;
    }
    if (event?.kind === "resumed") {
      handlers.onResumed?.();
      continue;
    }
    if (event?.kind === "canceled") {
      return null;
    }
    if (event?.kind === "success") {
      result = event.result;
      handlers.onProgress?.(100);
    }
  }

  if (!result?.tempFilePath) {
    throw new Error("Download finished without output file");
  }
  return result;
}

Page({
  data: {
    pdfUrl:
      "https://ontheline.trincoll.edu/images/bookdown/sample-local-pdf.pdf",
    officeUrl:
      "https://example-files.online-convert.com/document/docx/example.docx",
    officeFileType: "docx",
    showMenu: true,
    isPdfDownloading: false,
    isOfficeDownloading: false,
    pdfDownloadPaused: false,
    pdfDownloadProgress: 0,
    officeDownloadProgress: 0,
    officeCached: false,
  },

  onLoad: async function (options) {
    await this.refreshOfficeCachedState();
  },

  onShow: async function () {
    await this.refreshOfficeCachedState();
  },

  onPdfUrlInput: function (event) {
    const value = event?.detail?.value || "";
    this.setData({ pdfUrl: value });
  },

  onOfficeUrlInput: function (event) {
    const value = event?.detail?.value || "";
    const detectedType = detectFileType(value);
    this.setData({
      officeUrl: value,
      officeFileType: detectedType || this.data.officeFileType,
    });
    void this.refreshOfficeCachedState(value, detectedType || this.data.officeFileType);
  },

  onOfficeFileTypeInput: function (event) {
    const value = event?.detail?.value || "";
    this.setData({ officeFileType: value });
    void this.refreshOfficeCachedState(this.data.officeUrl, value);
  },

  toggleShowMenu: function () {
    this.setData({ showMenu: !this.data.showMenu });
  },

  openPdf: async function () {
    const url = this.data.pdfUrl?.trim();
    if (!url) {
      lx.showToast({
        title: "Please enter PDF URL",
        icon: "none",
      });
      return;
    }

    if (this.data.isPdfDownloading) {
      return;
    }

    this.setData({ isPdfDownloading: true, pdfDownloadProgress: 0, pdfDownloadPaused: false });

    try {
      const task = lx.downloadFile({ url });
      this._pdfDownloadTask = task;
      const downloadResult = await waitDownloadResult(task, {
        onProgress: (progress) => this.setData({ pdfDownloadProgress: progress }),
        onPaused: () => this.setData({ pdfDownloadPaused: true }),
        onResumed: () => this.setData({ pdfDownloadPaused: false }),
      });
      if (!downloadResult) {
        return;
      }

      // Open document
      await lx.openDocument({
        filePath: downloadResult.tempFilePath,
        fileType: "pdf",
        showMenu: this.data.showMenu,
      });
    } catch (error) {
      console.error("openPdf failed:", error);
      lx.showToast({
        title: error?.message || "Download failed",
        icon: "none",
      });
    } finally {
      this._pdfDownloadTask = null;
      this.setData({ isPdfDownloading: false, pdfDownloadPaused: false });
    }
  },

  pausePdfDownload: async function () {
    const task = this._pdfDownloadTask;
    if (!task || this.data.pdfDownloadPaused) {
      return;
    }
    try {
      await task.pause();
    } catch (error) {
      lx.showToast({
        title: error?.message || "Pause failed",
        icon: "none",
      });
    }
  },

  resumePdfDownload: async function () {
    const task = this._pdfDownloadTask;
    if (!task || !this.data.pdfDownloadPaused) {
      return;
    }
    try {
      await task.resume();
    } catch (error) {
      lx.showToast({
        title: error?.message || "Resume failed",
        icon: "none",
      });
    }
  },

  cancelPdfDownload: async function () {
    const task = this._pdfDownloadTask;
    if (!task) {
      return;
    }
    try {
      await task.cancel();
    } catch (error) {
      lx.showToast({
        title: error?.message || "Cancel failed",
        icon: "none",
      });
    }
  },

  openOffice: async function () {
    const url = this.data.officeUrl?.trim();
    const fileType = this.data.officeFileType?.trim();

    if (!url) {
      lx.showToast({
        title: "Please enter document URL",
        icon: "none",
      });
      return;
    }

    if (!fileType) {
      lx.showToast({
        title: "Please enter file type",
        icon: "none",
      });
      return;
    }

    if (this.data.isOfficeDownloading) {
      return;
    }

    this.setData({
      isOfficeDownloading: true,
      officeDownloadProgress: 0,
    });

    try {
      const filePath = resolveOfficeFetchPath(url, fileType);
      const stat = await Rong.stat(filePath).catch(() => null);
      if (stat) {
        this.setData({ officeCached: true, officeDownloadProgress: 100 });
        await lx.openDocument({
          filePath,
          fileType: fileType,
          showMenu: this.data.showMenu,
        });
        return;
      }

      const tmpPath = `${filePath}.tmp`;
      this._officeFetchCancelToken = { canceled: false, cancel: null };
      try {
        await downloadWithFetchToFile(
          url,
          tmpPath,
          (progress) => this.setData({ officeDownloadProgress: progress }),
          this._officeFetchCancelToken,
        );
        await Rong.rename(tmpPath, filePath);
        this.setData({ officeCached: true });

        // Open document
        await lx.openDocument({
          filePath,
          fileType: fileType,
          showMenu: this.data.showMenu,
        });
      } finally {
        this._officeFetchCancelToken = null;
      }
    } catch (error) {
      if (error?.name === "AbortError") {
        lx.showToast({
          title: "Download canceled",
          icon: "none",
        });
        return;
      }
      console.error("openOffice failed:", error);
      lx.showToast({
        title: error?.message || "Download failed",
        icon: "none",
      });
    } finally {
      this.setData({ isOfficeDownloading: false });
    }
  },

  cancelOfficeDownload: function () {
    const token = this._officeFetchCancelToken;
    if (!token) return;
    token.canceled = true;
    if (typeof token.cancel === "function") {
      token.cancel();
    }
  },

  refreshOfficeCachedState: async function (urlInput, fileTypeInput) {
    const url = (urlInput ?? this.data.officeUrl ?? "").trim();
    const fileType = (fileTypeInput ?? this.data.officeFileType ?? "").trim();
    if (!url || !fileType) {
      this.setData({ officeCached: false });
      return;
    }
    const filePath = resolveOfficeFetchPath(url, fileType);
    const stat = await Rong.stat(filePath).catch(() => null);
    this.setData({ officeCached: Boolean(stat) });
  },
});
