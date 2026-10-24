import android.app.UiAutomation;
import android.os.*;
import android.graphics.Rect;

public class GpuFlexBasisProbe {
 static void size(String id,float expected,boolean vertical)throws Exception {
  Rect bounds=GpuThemeProbe.bounds(id);float actual=(vertical?bounds.height():bounds.width())/GpuPaddingProbe.density();
  GpuApiProbe.check(Math.abs(actual-expected)<1,id+" got "+actual+" expected "+expected);
 }
 static void measure(boolean narrow)throws Exception {
  size("flex-auto-a",narrow?95:125,false);size("flex-auto-b",narrow?145:175,false);
  size("flex-zero-a",narrow?120:150,false);size("flex-zero-b",narrow?120:150,false);
  size("flex-half",narrow?120:150,false);size("flex-column-a",40,true);size("flex-column-b",60,true);
 }
 public static void main(String[] args)throws Exception {
  HandlerThread thread=new HandlerThread("flex-basis");thread.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),c);GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  try {
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);
   GpuApiProbe.node("flex-resize").recycle();measure(false);GpuRadioProbe.screenshot("gpu-flex-basis-wide");
   GpuApiProbe.click("flex-resize");measure(true);GpuRadioProbe.screenshot("gpu-flex-basis-narrow");
   GpuApiProbe.click("flex-resize");measure(false);GpuRadioProbe.key(4);
   System.out.println("PASS content basis, zero basis, percentage basis and column growth across resize");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
