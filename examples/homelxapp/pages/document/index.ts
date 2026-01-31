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

// Helper function to generate filename from URL
function getFilenameFromUrl(url, fileType) {
  // Create a simple hash from URL for consistent filename
  let hash = 0;
  for (let i = 0; i < url.length; i++) {
    const char = url.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash = hash & hash; // Convert to 32bit integer
  }
  const hashStr = Math.abs(hash).toString(36);
  return `doc_${hashStr}.${fileType}`;
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
  },

  onLoad: function (options) {
    console.log("Document page onLoad");
  },

  onShow: function () {
    console.log("Document page onShow");
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
  },

  onOfficeFileTypeInput: function (event) {
    const value = event?.detail?.value || "";
    this.setData({ officeFileType: value });
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

    this.setData({ isPdfDownloading: true });

    try {
      const filename = getFilenameFromUrl(url, "pdf");
      const targetPath = `${lx.env.USER_CACHE_PATH}/${filename}`;
      const tmpPath = `${targetPath}.tmp`;

      // Check if file already exists
      let fileExists = false;
      try {
        const stat = await Rong.stat(targetPath);
        if (stat) {
          console.log("File already exists, skipping download:", targetPath);
          fileExists = true;
        }
      } catch (e) {
        // File doesn't exist, need to download
        fileExists = false;
      }

      // Download if not exists
      if (!fileExists) {
        // Show downloading toast
        lx.showToast({
          title: "Downloading...",
          icon: "loading",
          duration: 0, // Don't auto-hide
        });

        // Download to temporary file
        const response = await fetch(url);

        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`);
        }

        const file = await Rong.open(tmpPath, {
          write: true,
          create: true,
          truncate: true,
        });

        await response.body.pipeTo(file.writable);
        file.close();

        // Rename tmp to final file
        await Rong.rename(tmpPath, targetPath);

        console.log("Download completed:", targetPath);

        // Hide downloading toast
        lx.hideToast();
      }

      // Open document
      await lx.openDocument({
        filePath: targetPath,
        fileType: "pdf",
        showMenu: this.data.showMenu,
      });
    } catch (error) {
      console.error("openPdf failed:", error);
      lx.hideToast();
      lx.showToast({
        title: error?.message || "Download failed",
        icon: "none",
      });
    } finally {
      this.setData({ isPdfDownloading: false });
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

    this.setData({ isOfficeDownloading: true });

    try {
      const filename = getFilenameFromUrl(url, fileType);
      const targetPath = `${lx.env.USER_CACHE_PATH}/${filename}`;
      const tmpPath = `${targetPath}.tmp`;

      // Check if file already exists
      let fileExists = false;
      try {
        const stat = await Rong.stat(targetPath);
        if (stat) {
          console.log("File already exists, skipping download:", targetPath);
          fileExists = true;
        }
      } catch (e) {
        // File doesn't exist, need to download
        fileExists = false;
      }

      // Download if not exists
      if (!fileExists) {
        // Show downloading toast
        lx.showToast({
          title: "Downloading...",
          icon: "loading",
          duration: 0, // Don't auto-hide
        });

        // Download to temporary file
        const response = await fetch(url);

        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`);
        }

        const file = await Rong.open(tmpPath, {
          write: true,
          create: true,
          truncate: true,
        });

        await response.body.pipeTo(file.writable);
        file.close();

        // Rename tmp to final file
        await Rong.rename(tmpPath, targetPath);

        console.log("Download completed:", targetPath);

        // Hide downloading toast
        lx.hideToast();
      }

      // Open document
      await lx.openDocument({
        filePath: targetPath,
        fileType: fileType,
        showMenu: this.data.showMenu,
      });
    } catch (error) {
      console.error("openOffice failed:", error);
      lx.hideToast();
      lx.showToast({
        title: error?.message || "Download failed",
        icon: "none",
      });
    } finally {
      this.setData({ isOfficeDownloading: false });
    }
  },
});
