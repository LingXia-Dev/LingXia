import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuMediaProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void disabled(String id)throws Exception{
  AccessibilityNodeInfo n=GpuApiProbe.node(id);
  try{GpuUiProbe.check(!n.isEnabled(),"expected disabled "+id);}finally{n.recycle();}
 }
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("media-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);
   android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();
   info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1100);
   click("open-api");
   for(String mode:new String[]{"Pictures","Videos"}){
    click("api-media-"+mode);
    GpuUiProbe.waitText("media-selection-count","0 selected");
    click("media-openCountPicker");GpuUiProbe.clickNative("1");
    GpuUiProbe.waitText("media-openCountPicker","Maximum selection: 1");
    disabled("media-previewSelectedMedia");GpuChromeProbe.back();
   }
   click("api-media-ScanCode");
   click("media-openScanSourcePicker");GpuUiProbe.clickNative("Camera & Photo");
   GpuUiProbe.waitText("media-openScanSourcePicker","Source: Camera and album");
   click("media-openScanTypePicker");GpuUiProbe.clickNative("qrCode");
   GpuUiProbe.waitText("media-openScanTypePicker","Code types: qrCode");GpuChromeProbe.back();
   click("api-media-ImageInfo");disabled("media-compressSelectedImage");
   GpuUiProbe.waitText("media-compressQuality","80");
   GpuRadioProbe.screenshot("gpu-independent-media-image");GpuChromeProbe.back();
   click("api-media-VideoTools");disabled("media-createVideoThumbnail");disabled("media-compressSelectedVideo");
   GpuRadioProbe.screenshot("gpu-independent-media-video");GpuChromeProbe.back();
   click("api-media-SaveToAlbum");GpuApiProbe.node("media-captureImageForAlbum").recycle();GpuApiProbe.node("media-captureVideoForAlbum").recycle();
   GpuChromeProbe.back();GpuChromeProbe.back();
   System.out.println("PASS six GPU media modes, native option sheets, initial fields and empty-source action gates");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
