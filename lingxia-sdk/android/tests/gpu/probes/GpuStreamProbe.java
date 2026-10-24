import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;

public class GpuStreamProbe {
 static void check(boolean v,String s){if(!v)throw new AssertionError(s);}
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void clickNow(String id)throws Exception{
  AccessibilityNodeInfo n=GpuApiProbe.node(id);
  n.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());n.recycle();
  Thread.sleep(100);n=GpuApiProbe.node(id);
  try{check(n.isEnabled(),"enabled "+id);check(n.performAction(AccessibilityNodeInfo.ACTION_CLICK),"click "+id);}
  finally{n.recycle();}
 }
 static String text(String id)throws Exception{return GpuApiProbe.text(id);}
 static void waitText(String id,String expected)throws Exception{
  String value="";for(int j=0;j<50;j++){value=text(id);if(value.equals(expected))return;Thread.sleep(300);}
  throw new AssertionError(id+" expected "+expected+" got "+value);
 }
 static void set(String value)throws Exception{
  AccessibilityNodeInfo n=GpuApiProbe.node("stream-input");Bundle b=new Bundle();b.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,value);
  check(n.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,b),"text input");n.recycle();Thread.sleep(250);
 }
 static void firstToken()throws Exception{
  for(int j=0;j<25;j++){String value=text("stream-live");if(!value.isEmpty()&&!value.startsWith("Waiting"))return;Thread.sleep(150);}
  throw new AssertionError("no live token");
 }
 public static void main(String[] args)throws Exception{
  HandlerThread t=new HandlerThread("stream-probe");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(800);
   click("open-api");click("api-stream");set("Explain the bridge");clickNow("stream-send");firstToken();GpuRadioProbe.screenshot("gpu-independent-stream-live");
   waitText("stream-status","Completed");waitText("stream-count","2 messages");GpuRadioProbe.screenshot("gpu-independent-stream-complete");
   // Some random replies last under a second; cancel as soon as the request is live.
   click("stream-clear");waitText("stream-count","0 messages");set("Cancel this response");clickNow("stream-send");waitText("stream-status","Streaming");clickNow("stream-stop");waitText("stream-status","Canceled");waitText("stream-count","2 messages");
   click("stream-clear");set("Leave while streaming");clickNow("stream-send");waitText("stream-status","Streaming");GpuChromeProbe.back();click("api-stream");waitText("stream-status","Ready");
   set("Fresh stream");click("stream-send");waitText("stream-status","Completed");waitText("stream-count","2 messages");click("stream-clear");GpuChromeProbe.back();GpuChromeProbe.back();
   System.out.println("PASS live stream events, terminal result/state, explicit cancellation, clear, navigation teardown and fresh stream");
  }catch(Throwable error){error.printStackTrace();throw error;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
