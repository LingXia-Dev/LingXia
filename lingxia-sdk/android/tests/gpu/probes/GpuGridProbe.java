import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;
public final class GpuGridProbe {
 static String settings(String... args)throws Exception{
  String[] command=new String[args.length+1];command[0]="/system/bin/settings";System.arraycopy(args,0,command,1,args.length);
  Process p=Runtime.getRuntime().exec(command);GpuRadioProbe.require(p.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&p.exitValue()==0,"settings command");
  String value=new java.io.BufferedReader(new java.io.InputStreamReader(p.getInputStream())).readLine();return value==null?"":value;
 }
 public static void main(String[] args)throws Exception{
  String auto=settings("get","system","accelerometer_rotation"),rotation=settings("get","system","user_rotation");
  HandlerThread thread=new HandlerThread("grid-probe");thread.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);GpuRadioProbe.automation=automation;
  try{
   UiAutomation.class.getMethod("connect").invoke(automation);android.accessibilityservice.AccessibilityServiceInfo service=automation.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(service);
   settings("put","system","accelerometer_rotation","0");settings("put","system","user_rotation","0");Thread.sleep(700);
   GpuRadioProbe.click("tab-explore");GpuRadioProbe.show("moment-1");
   AccessibilityNodeInfo original=GpuRadioProbe.node("moment-1");android.graphics.Rect portrait=new android.graphics.Rect();original.getBoundsInScreen(portrait);
   settings("put","system","user_rotation","1");Thread.sleep(1400);GpuRadioProbe.show("moment-1");
   GpuRadioProbe.require(original.refresh(),"stable keyed card across grid reflow");
   android.graphics.Rect first=new android.graphics.Rect(),second=new android.graphics.Rect();original.getBoundsInScreen(first);AccessibilityNodeInfo next=GpuRadioProbe.node("moment-2");next.getBoundsInScreen(second);next.recycle();
   GpuRadioProbe.require(Math.abs(first.width()-second.width())<=2 && second.left>first.right && Math.abs(first.top-second.top)<=2,"equal landscape columns and source order");
   GpuRadioProbe.require(original.performAction(AccessibilityNodeInfo.ACTION_CLICK),"grid button click");Thread.sleep(500);
   Process tap=Runtime.getRuntime().exec(new String[]{"/system/bin/input","tap",String.valueOf(second.centerX()),String.valueOf(second.centerY())});
   GpuRadioProbe.require(tap.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&tap.exitValue()==0,"grid physical tap");Thread.sleep(500);
   GpuRadioProbe.screenshot("gpu-grid-landscape");
   GpuRadioProbe.click("theme");GpuRadioProbe.show("moment-1");GpuRadioProbe.screenshot("gpu-grid-dark");GpuRadioProbe.click("theme");
   settings("put","system","user_rotation","0");Thread.sleep(1400);GpuRadioProbe.show("moment-1");GpuRadioProbe.require(original.refresh(),"stable card after portrait return");original.getBoundsInScreen(first);
   GpuRadioProbe.require(Math.abs(first.width()-portrait.width())<=2,"portrait card width restored");original.recycle();GpuRadioProbe.screenshot("gpu-grid-portrait");
   GpuRadioProbe.click("tab-hello");AccessibilityNodeInfo count=GpuRadioProbe.node("hello-count");GpuRadioProbe.require(String.valueOf(count.getText()).contains("2 times"),"grid button reaches Logic");count.recycle();
   System.out.println("PASS physical grid columns, reflow identity, portrait restoration and button Logic action");
  }finally{settings("put","system","user_rotation",rotation);settings("put","system","accelerometer_rotation",auto);UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
