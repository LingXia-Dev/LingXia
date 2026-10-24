import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
public final class GpuShareFileProbe {
 public static void main(String[] args)throws Exception{
  HandlerThread t=new HandlerThread("share-file-probe");t.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),connection);GpuRadioProbe.automation=a;GpuApiProbe.a=a;GpuFileProbe.a=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
   GpuApiProbe.click("open-share");GpuApiProbe.click("share-choose-file");Thread.sleep(500);GpuUiProbe.clickNative("gpu-share-file-fixture.pdf");GpuUiProbe.waitText("share-status","File selected");
   GpuApiProbe.click("share-file");GpuShareReceiverProbe.receive();GpuRadioProbe.screenshot("gpu-share-file-return");GpuChromeProbe.back();
   System.out.println("PASS selected PDF delivered to local receiver and returned to GPU page");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
