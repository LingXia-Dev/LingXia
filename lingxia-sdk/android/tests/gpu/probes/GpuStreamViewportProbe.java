import android.app.UiAutomation;
import android.os.*;
import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuStreamViewportProbe {
 static void scroll(boolean forward)throws Exception{
  AccessibilityNodeInfo node=GpuApiProbe.node("page-scroll");
  node.performAction(forward?AccessibilityNodeInfo.ACTION_SCROLL_FORWARD:AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD);node.recycle();Thread.sleep(350);
 }
 static void reachHints(String screenshot)throws Exception{
  for(int n=0;n<4;n++)scroll(true);
  Rect hint=GpuThemeProbe.bounds("stream-hint-How does the bridge protocol work?"),viewport=GpuThemeProbe.bounds("page-scroll");
  GpuUiProbe.check(viewport.contains(hint),"last hint remains reachable inside reduced viewport");
  float scale=GpuThemeProbe.bounds("stream-send").height()/36f;
  GpuUiProbe.check(hint.height()>=40*scale-2,"last hint is fully visible, not just its clipped accessibility bounds");
  GpuUiProbe.check(viewport.bottom<=GpuThemeProbe.bounds("stream-input").top,"scroll content stays above composer");
  GpuRadioProbe.screenshot(screenshot);
  for(int n=0;n<4;n++)scroll(false);
  GpuUiProbe.check(GpuThemeProbe.bounds("page-scroll").contains(GpuThemeProbe.bounds("stream-empty")),"title remains reachable at scroll start");
 }
 public static void main(String[]args)throws Exception{
  HandlerThread thread=new HandlerThread("stream-viewport");thread.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  String auto=GpuGridProbe.settings("get","system","accelerometer_rotation"),rotation=GpuGridProbe.settings("get","system","user_rotation");
  try{
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);
   GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","0");Thread.sleep(900);
   for(boolean dark:new boolean[]{false,true}){
    GpuChromeProbe.tab("Home");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");GpuChromeProbe.tab("API");GpuApiProbe.click("api-stream");GpuStreamLayoutProbe.centeredEmpty();
    int full=GpuThemeProbe.bounds("page-scroll").height();GpuApiProbe.click("stream-input");Thread.sleep(600);
    GpuUiProbe.check(GpuThemeProbe.bounds("page-scroll").height()<full,"keyboard reduces message viewport");reachHints("gpu-stream-keyboard-last-hint-"+dark);GpuRadioProbe.screenshot("gpu-stream-keyboard-empty-"+dark);GpuRadioProbe.key(4);
    GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1100);
    android.graphics.Bitmap shot=a.takeScreenshot();GpuUiProbe.check(shot.getWidth()>shot.getHeight(),"physical landscape");shot.recycle();
    reachHints("gpu-stream-landscape-last-hint-"+dark);GpuRadioProbe.screenshot("gpu-stream-landscape-empty-"+dark);
    GpuGridProbe.settings("put","system","user_rotation","0");Thread.sleep(1100);GpuStreamLayoutProbe.centeredEmpty();GpuChromeProbe.back();
   }
   GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");System.out.println("PASS empty Stream keyboard/landscape scroll reachability and portrait recentering in both themes");
  }finally{GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
