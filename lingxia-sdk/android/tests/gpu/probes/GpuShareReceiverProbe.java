import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;
public final class GpuShareReceiverProbe {
 static void receive()throws Exception{
  AccessibilityNodeInfo target=GpuUiProbe.waitNative("LingXia Test Receiver");
  target.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());target.recycle();Thread.sleep(700);
  GpuUiProbe.clickNative("LingXia Test Receiver");GpuUiProbe.waitNative("Received locally").recycle();
  GpuRadioProbe.key(4);Thread.sleep(700);
 }
 public static void main(String[] args)throws Exception{
  HandlerThread t=new HandlerThread("share-receiver-probe");t.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),connection);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
   GpuApiProbe.click("open-share");GpuApiProbe.click("share-text");receive();
   GpuApiProbe.click("share-page");receive();
   GpuApiProbe.click("share-choose-image");GpuUiProbe.clickNative("Album");Thread.sleep(700);
   AccessibilityNodeInfo image=GpuMediaImageProbe.firstPhoto(a.getRootInActiveWindow());GpuUiProbe.check(image!=null,"image fixture");GpuUiProbe.check(image.performAction(AccessibilityNodeInfo.ACTION_CLICK),"select image");image.recycle();GpuUiProbe.clickNative("Done");GpuUiProbe.waitText("share-status","Image selected");
   GpuApiProbe.click("share-image");receive();GpuRadioProbe.screenshot("gpu-share-receiver-return");GpuChromeProbe.back();
   System.out.println("PASS text/page/image delivered to local receiver and returned to GPU page");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
