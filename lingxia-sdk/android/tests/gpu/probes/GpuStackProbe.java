import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
public final class GpuStackProbe {
 static Rect bounds(String id)throws Exception {AccessibilityNodeInfo n=GpuRadioProbe.node(id);Rect r=new Rect();n.getBoundsInScreen(r);n.recycle();return r;}
 static void tap(int x,int y)throws Exception {Process p=Runtime.getRuntime().exec(new String[]{"/system/bin/input","tap",String.valueOf(x),String.valueOf(y)});GpuRadioProbe.require(p.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&p.exitValue()==0,"physical tap");Thread.sleep(600);}
 public static void main(String[] args)throws Exception {
  String auto=GpuGridProbe.settings("get","system","accelerometer_rotation"),rotation=GpuGridProbe.settings("get","system","user_rotation");
  HandlerThread thread=new HandlerThread("stack-probe");thread.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);GpuRadioProbe.automation=automation;
  try {
   UiAutomation.class.getMethod("connect").invoke(automation);android.accessibilityservice.AccessibilityServiceInfo service=automation.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(service);
   GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","0");Thread.sleep(700);
   GpuRadioProbe.click("tab-explore");GpuRadioProbe.show("stack-front");
   Rect back=bounds("stack-back"),front=bounds("stack-front");float scale=front.width()/200f;
   GpuRadioProbe.require(Math.abs(front.left-back.left-24*scale)<=2&&Math.abs(front.top-back.top-48*scale)<=2,"24dp horizontal and 48dp vertical offsets");
   GpuRadioProbe.require(Rect.intersects(back,front),"overlapping cards");
   tap(back.left+10,back.centerY());tap(front.centerX(),Math.min(front.bottom,back.bottom)-10);
   GpuRadioProbe.click("stack-front");GpuRadioProbe.screenshot("gpu-stack-portrait");
   AccessibilityNodeInfo original=GpuRadioProbe.node("stack-front");
   GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1400);GpuRadioProbe.show("stack-front");GpuRadioProbe.require(original.refresh(),"stack identity across rotation");GpuRadioProbe.screenshot("gpu-stack-landscape");
   GpuRadioProbe.click("theme");GpuRadioProbe.show("stack-front");GpuRadioProbe.screenshot("gpu-stack-dark");GpuRadioProbe.click("theme");
   GpuGridProbe.settings("put","system","user_rotation","0");Thread.sleep(1400);original.recycle();GpuRadioProbe.click("tab-hello");
   AccessibilityNodeInfo count=GpuRadioProbe.node("hello-count");GpuRadioProbe.require(String.valueOf(count.getText()).contains("5 times"),"back adds one, overlapping front and accessibility add two each");count.recycle();
   System.out.println("PASS stack offset geometry, frontmost physical tap, accessibility activation, rotation identity and theme");
  } finally {GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
