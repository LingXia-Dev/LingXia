import android.app.UiAutomation;
import android.os.*;
import android.graphics.*;
import android.view.accessibility.AccessibilityNodeInfo;

public class GpuThemeProbe {
 static Rect bounds(String id)throws Exception {
  AccessibilityNodeInfo n=GpuApiProbe.node(id);Rect box=new Rect();n.getBoundsInScreen(box);n.recycle();return box;
 }
 static void pixel(Bitmap bitmap,int x,int y,int expected,String label) {
  int actual=bitmap.getPixel(x,y);
  GpuApiProbe.check(Math.abs(Color.red(actual)-Color.red(expected))<=3&&Math.abs(Color.green(actual)-Color.green(expected))<=3&&Math.abs(Color.blue(actual)-Color.blue(expected))<=3,label+" got "+Integer.toHexString(actual));
 }
 static void palette(boolean dark)throws Exception {
  String id="home-appearance-"+(dark?"dark":"light");GpuApiProbe.click(id);
  Rect selected=bounds(id),theme=bounds("theme");
  Bitmap shot=GpuApiProbe.a.takeScreenshot(),bitmap=shot.copy(Bitmap.Config.ARGB_8888,false);shot.recycle();
  try {
   pixel(bitmap,theme.right+6,theme.centerY(),dark?0xff17171a:0xfff2f2f7,"page token");
   pixel(bitmap,selected.left+10,selected.centerY(),dark?0xff0a84ff:0xff007aff,"primary token");
   pixel(bitmap,selected.left-6,selected.centerY(),dark?0xff1f1f22:0xffffffff,"surface token");
  }finally{bitmap.recycle();}
  GpuRadioProbe.screenshot(dark?"gpu-theme-dark":"gpu-theme-light");
 }
 public static void main(String[] args)throws Exception {
  HandlerThread thread=new HandlerThread("theme-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  String auto=GpuGridProbe.settings("get","system","accelerometer_rotation"),rotation=GpuGridProbe.settings("get","system","user_rotation");
  try {
   UiAutomation.class.getMethod("connect").invoke(a);
   android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(800);
   GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","0");Thread.sleep(800);
   palette(false);palette(true);
   GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1100);palette(true);
   GpuGridProbe.settings("put","system","user_rotation","0");Thread.sleep(900);palette(true);palette(false);
   System.out.println("PASS reference page/surface/primary theme pixels, repeated appearance changes and rotation");
  }finally{GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
