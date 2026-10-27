import android.app.UiAutomation;
import android.os.*;
import android.graphics.*;
import android.view.accessibility.AccessibilityNodeInfo;

public class GpuThemeProbe {
 // Resolved by the showcase theme pack (accent #0f9d58) through the default roles.
 static final int PAGE_LIGHT=0xfff2f3f7, PAGE_DARK=0xff0e1219, SURFACE_LIGHT=0xfffefeff, SURFACE_DARK=0xff171c27;
 static Rect bounds(String id)throws Exception {
  AccessibilityNodeInfo n=GpuApiProbe.node(id);Rect box=new Rect();n.getBoundsInScreen(box);n.recycle();return box;
 }
 static void pixel(Bitmap bitmap,int x,int y,int expected,String label) {
  int actual=bitmap.getPixel(x,y);
  GpuApiProbe.check(Math.abs(Color.red(actual)-Color.red(expected))<=3&&Math.abs(Color.green(actual)-Color.green(expected))<=3&&Math.abs(Color.blue(actual)-Color.blue(expected))<=3,label+" got "+Integer.toHexString(actual));
 }
 static void palette(boolean dark)throws Exception {
  GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-"+(dark?"dark":"light"));
  GpuChromeProbe.tab("ToDo");Thread.sleep(800);
  // ToDo paints the canvas around a surface panel whose input has no fill of its own.
  Rect input=bounds("todo-input");float scale=GpuPaddingProbe.density();
  Bitmap shot=GpuApiProbe.a.takeScreenshot(),bitmap=shot.copy(Bitmap.Config.ARGB_8888,false);shot.recycle();
  try {
   // Right edge: dev-session builds draw their badge on the left.
   pixel(bitmap,bitmap.getWidth()-Math.round(4*scale),input.centerY(),dark?PAGE_DARK:PAGE_LIGHT,"page token");
   pixel(bitmap,input.right-Math.round(12*scale),input.centerY(),dark?SURFACE_DARK:SURFACE_LIGHT,"surface token");
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
   System.out.println("PASS page and surface theme pixels, repeated appearance changes and rotation");
  }finally{GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
