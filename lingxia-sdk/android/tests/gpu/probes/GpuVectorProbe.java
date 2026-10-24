import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;

public class GpuVectorProbe {
 public static void main(String[] args)throws Exception{
  HandlerThread t=new HandlerThread("vector-probe");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo service=a.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(service);Thread.sleep(500);GpuBorderProbe.scale=GpuPaddingProbe.density();
   GpuBorderProbe.pixel("vector-fill",40,40,0xffe7000b);GpuBorderProbe.pixel("vector-fill",5,40,0xffffffff);
   GpuBorderProbe.pixel("vector-stroke",10,40,0xffe7000b);GpuBorderProbe.pixel("vector-stroke",40,40,0xffffffff);
   GpuBorderProbe.pixel("vector-multicolor",20,40,0xffff0000);GpuBorderProbe.pixel("vector-multicolor",60,40,0xff0000ff);
   GpuBorderProbe.pixel("vector-fit",20,40,0xffffffff);GpuBorderProbe.pixel("vector-fit",80,40,0xff00ff00);GpuBorderProbe.pixel("vector-fit",140,40,0xffffffff);
   AccessibilityNodeInfo button=GpuApiProbe.node("vector-fill");GpuApiProbe.check(button.getChildCount()==0,"decorative vectors stay inside one accessible button");button.recycle();
   GpuApiProbe.click("vector-toggle");GpuBorderProbe.pixel("vector-fill",40,40,0xff155dfc);GpuBorderProbe.pixel("vector-stroke",10,40,0xff155dfc);
   GpuBorderProbe.pixel("vector-scroll",40,40,0xffff0000);AccessibilityNodeInfo scroll=GpuApiProbe.node("vector-scroll");scroll.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);scroll.recycle();Thread.sleep(500);GpuBorderProbe.pixel("vector-scroll",40,40,0xff0000ff);
   GpuRadioProbe.screenshot("gpu-vector-fixture");System.out.println("PASS vector fill, stroke holes, inherited color updates, layered paths, centered viewBox, accessibility and retained scroll pixels");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
