import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuScanProbe {
 static void open()throws Exception{
  GpuApiProbe.click("media-startScan");Thread.sleep(600);
  AccessibilityNodeInfo permission=GpuUiProbe.nativeText(GpuApiProbe.a.getRootInActiveWindow(),"While using the app");
  if(permission!=null){permission.recycle();GpuUiProbe.clickNative("While using the app");Thread.sleep(700);}
 }
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("scan-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);
   android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
   GpuApiProbe.click("open-api");GpuApiProbe.click("api-media-ScanCode");
   GpuApiProbe.click("media-openScanSourcePicker");GpuUiProbe.clickNative("Camera & Photo");
   GpuApiProbe.click("media-openScanTypePicker");GpuUiProbe.clickNative("qrCode");
   open();GpuRadioProbe.key(4);Thread.sleep(600);
   GpuUiProbe.waitText("media-startScan","Scan code");GpuUiProbe.waitText("media-scan-type","No code scanned");
   open();GpuUiProbe.clickNative("Album");Thread.sleep(700);
   GpuRadioProbe.key(4);Thread.sleep(500);GpuUiProbe.clickNative("Album");Thread.sleep(700);
   AccessibilityNodeInfo image=GpuMediaImageProbe.firstPhoto(a.getRootInActiveWindow());GpuUiProbe.check(image!=null,"QR album fixture");
   GpuUiProbe.check(image.performAction(AccessibilityNodeInfo.ACTION_CLICK),"select QR image");image.recycle();GpuUiProbe.clickNative("Done");
   GpuUiProbe.waitText("media-scan-result","LingXia GPU scanner verification");GpuUiProbe.waitText("media-scan-type","QR_CODE");
   GpuRadioProbe.screenshot("gpu-independent-scan-result");
   GpuRadioProbe.key(4);Thread.sleep(500);GpuApiProbe.node("api-media-ScanCode").recycle();
   GpuRadioProbe.key(4);Thread.sleep(500);GpuApiProbe.node("open-api").recycle();
   System.out.println("PASS scanner cancel/reopen, album QR decoding and GPU result/type display");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
