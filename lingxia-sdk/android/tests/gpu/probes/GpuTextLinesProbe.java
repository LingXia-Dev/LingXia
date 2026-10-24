import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
public final class GpuTextLinesProbe {
 static Rect bounds(String id)throws Exception {AccessibilityNodeInfo n=GpuRadioProbe.node(id);Rect r=new Rect();n.getBoundsInScreen(r);n.recycle();return r;}
 static String text(String id)throws Exception {AccessibilityNodeInfo n=GpuRadioProbe.node(id);String s=String.valueOf(n.getText());n.recycle();return s;}
 static void limited()throws Exception {GpuRadioProbe.show("moment-story");Rect r=bounds("moment-story");float scale=android.content.res.Resources.getSystem().getDisplayMetrics().density;GpuRadioProbe.require(Math.abs(r.height()-48*scale)<=2,"two 24dp text lines: "+r);}
 public static void main(String[] args)throws Exception {
  String auto=GpuGridProbe.settings("get","system","accelerometer_rotation"),rotation=GpuGridProbe.settings("get","system","user_rotation");
  HandlerThread thread=new HandlerThread("text-lines-probe");thread.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);GpuRadioProbe.automation=automation;
  try {
   UiAutomation.class.getMethod("connect").invoke(automation);android.accessibilityservice.AccessibilityServiceInfo service=automation.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(service);
   GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","0");Thread.sleep(700);GpuRadioProbe.click("tab-explore");limited();
   String complete=text("moment-story");GpuRadioProbe.require(complete.startsWith("A small hello")&&complete.endsWith("a fresh start.")&&complete.length()>200,"complete accessible text");
   AccessibilityNodeInfo original=GpuRadioProbe.node("moment-story");GpuRadioProbe.screenshot("gpu-text-lines-collapsed");
   GpuRadioProbe.click("story-toggle");GpuRadioProbe.show("moment-story");Rect expanded=bounds("moment-story");GpuRadioProbe.require(expanded.height()>180,"expanded text height");GpuRadioProbe.require(complete.equals(text("moment-story")),"source preserved on expansion");GpuRadioProbe.screenshot("gpu-text-lines-expanded");
   GpuRadioProbe.click("story-toggle");limited();GpuRadioProbe.require(original.refresh(),"identity after collapse");
   GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1400);limited();GpuRadioProbe.require(original.refresh(),"identity after width change");GpuRadioProbe.require(complete.equals(text("moment-story")),"complete text across rotation");GpuRadioProbe.screenshot("gpu-text-lines-landscape");
   GpuRadioProbe.click("theme");limited();GpuRadioProbe.screenshot("gpu-text-lines-dark");GpuRadioProbe.click("theme");
   GpuGridProbe.settings("put","system","user_rotation","0");Thread.sleep(1400);GpuRadioProbe.click("tab-hello");GpuRadioProbe.click("tab-explore");limited();original.recycle();
   System.out.println("PASS text line cap, expand/collapse, complete accessibility text, stable identity, rotation and themes");
  } finally {GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
