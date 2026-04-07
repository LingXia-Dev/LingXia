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

Page({
  data: {
    pdfUrl: "https://ontheline.trincoll.edu/images/bookdown/sample-local-pdf.pdf",
    officeUrl: "https://example-files.online-convert.com/document/docx/example.docx",
    officeFileType: "docx",
    showMenu: true,
    isPdfDownloading: false,
    isOfficeDownloading: false,
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
      const result = await lx.downloadFile({ url });
      await lx.openFile({
        filePath: result.tempFilePath,
        fileType: "pdf",
        mode: "review",
        showMenu: this.data.showMenu,
      });
    } catch (error) {
      lx.showToast({ title: error?.message || "Download failed", icon: "none" });
    } finally {
      this.setData({ isPdfDownloading: false });
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
    if (this.data.isOfficeDownloading) return;
    this.setData({ isOfficeDownloading: true });
    try {
      const result = await lx.downloadFile({ url });
      await lx.openFile({
        filePath: result.tempFilePath,
        fileType,
        mode: "auto",
        showMenu: this.data.showMenu,
      });
    } catch (error) {
      lx.showToast({ title: error?.message || "Download failed", icon: "none" });
    } finally {
      this.setData({ isOfficeDownloading: false });
    }
  },
});
