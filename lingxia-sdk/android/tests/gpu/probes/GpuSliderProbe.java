import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.os.Bundle;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityEvent;
import android.graphics.Rect;

public final class GpuSliderProbe {
 static final int SET_PROGRESS=16908349;
 static UiAutomation automation;
 static java.util.List<Integer> events=java.util.Collections.synchronizedList(new java.util.ArrayList<>());
 static void require(boolean value,String message){GpuRadioProbe.require(value,message);}
 static String shell(String...command)throws Exception {
  Process p=Runtime.getRuntime().exec(command);java.io.ByteArrayOutputStream out=new java.io.ByteArrayOutputStream();byte[] b=new byte[4096];int n;while((n=p.getInputStream().read(b))!=-1)out.write(b,0,n);
  require(p.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&p.exitValue()==0,"shell command");return out.toString("UTF-8");
 }
 static boolean progress(AccessibilityNodeInfo node,float value){Bundle args=new Bundle();args.putFloat(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE,value);return node.performAction(SET_PROGRESS,args);}
 static void progress(float value)throws Exception{AccessibilityNodeInfo n=GpuRadioProbe.node("pause-length");require(progress(n,value),"set progress");n.recycle();Thread.sleep(400);}
 static void value(float expected)throws Exception{
  for(int i=0;i<20;i++){
   AccessibilityNodeInfo n=GpuRadioProbe.node("pause-length");float actual=n.getRangeInfo().getCurrent();n.recycle();if(actual==expected){AccessibilityNodeInfo label=GpuRadioProbe.node("pause-status");String text=String.valueOf(label.getText());label.recycle();if(text.equals((int)expected+" min"))return;}Thread.sleep(150);
  }throw new AssertionError("range/status expected "+expected);
 }
 static Rect box()throws Exception{GpuRadioProbe.show("pause-length");AccessibilityNodeInfo n=GpuRadioProbe.node("pause-length");Rect r=new Rect();n.getBoundsInScreen(r);n.recycle();return r;}
 static void swipe(Rect r,boolean reverse)throws Exception{
  int left=r.left+r.height()/2,right=r.right-r.height()/2;
  shell("/system/bin/input","swipe",String.valueOf(reverse?right:left),String.valueOf(r.centerY()),String.valueOf(reverse?left:right),String.valueOf(r.centerY()),"600");Thread.sleep(700);
 }
 public static void main(String[]args)throws Exception{
  HandlerThread thread=new HandlerThread("slider-probe");thread.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);GpuRadioProbe.automation=automation;
  try{
   UiAutomation.class.getMethod("connect").invoke(automation);android.accessibilityservice.AccessibilityServiceInfo service=automation.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(service);
   automation.setOnAccessibilityEventListener(event->{if("com.lingxia.example.gpu.dev".contentEquals(event.getPackageName()==null?"":event.getPackageName()))events.add(event.getEventType());});
   Thread.sleep(800);GpuRadioProbe.click("tab-explore");box();
   AccessibilityNodeInfo n=GpuRadioProbe.node("pause-length");require("android.widget.SeekBar".contentEquals(n.getClassName())&&"Minutes for a pause".contentEquals(n.getContentDescription()),"range role and label");require(n.getRangeInfo().getMin()==0&&n.getRangeInfo().getMax()==10,"range endpoints");require(!progress(n,Float.NaN),"reject NaN");n.recycle();value(3);
   progress(9.6f);value(10);n=GpuRadioProbe.node("pause-length");require(n.performAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD),"range decrement");n.recycle();value(9);
   n=GpuRadioProbe.node("pause-length");require(n.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD),"range increment");n.recycle();value(10);
   swipe(box(),true);value(0);swipe(box(),false);value(10);
   Rect r=box();int left=r.left+r.height()/2,right=r.right-r.height()/2;shell("/system/bin/input","tap",String.valueOf(left+(right-left)*4/10),String.valueOf(r.centerY()));value(3);
   r=box();int oldTop=r.top;shell("/system/bin/input","swipe",String.valueOf(r.centerX()),String.valueOf(r.centerY()),String.valueOf(r.centerX()),String.valueOf(r.centerY()-200),"450");Thread.sleep(800);
   n=GpuRadioProbe.find(automation.getRootInActiveWindow(),"pause-length");if(n!=null){n.getBoundsInScreen(r);require(r.top<oldTop,"vertical motion scrolls page");n.recycle();}box();value(3);
   System.out.println("PASS range semantics, accessibility snap/endpoints, tap/continuous drag and vertical scroll arbitration");
   boolean focused=false;
   for(int i=0;i<24;i++){GpuRadioProbe.key(KeyEvent.KEYCODE_TAB);n=automation.findFocus(AccessibilityNodeInfo.FOCUS_INPUT);focused=n!=null&&"com.lingxia.example.gpu.dev:id/pause-length".equals(n.getViewIdResourceName());if(n!=null)n.recycle();if(focused)break;}
   require(focused,"Tab reaches slider");GpuRadioProbe.key(KeyEvent.KEYCODE_MOVE_END);value(10);GpuRadioProbe.key(KeyEvent.KEYCODE_DPAD_LEFT);value(9);GpuRadioProbe.key(KeyEvent.KEYCODE_DPAD_DOWN);value(6);GpuRadioProbe.key(KeyEvent.KEYCODE_DPAD_UP);value(9);GpuRadioProbe.key(KeyEvent.KEYCODE_MOVE_HOME);value(0);GpuRadioProbe.key(KeyEvent.KEYCODE_SPACE);value(0);GpuRadioProbe.key(KeyEvent.KEYCODE_DPAD_RIGHT);value(3);
   GpuRadioProbe.click("pause-toggle");n=GpuRadioProbe.node("pause-length");require(!n.isEnabled()&&!progress(n,9)&&!n.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD),"disabled range");n.recycle();value(3);GpuRadioProbe.click("pause-toggle");
   AccessibilityNodeInfo stale=GpuRadioProbe.node("pause-length");GpuRadioProbe.click("tab-hello");require(!progress(stale,10),"stale range rejected");stale.recycle();GpuRadioProbe.click("tab-explore");box();value(3);
   GpuRadioProbe.key(KeyEvent.KEYCODE_HOME);shell("/system/bin/am","start","-n","com.lingxia.example.gpu.dev/com.lingxia.example.gpu.MainActivity");Thread.sleep(800);box();value(3);progress(6);value(6);
   require(events.contains(AccessibilityEvent.TYPE_VIEW_SELECTED),"presented range events");GpuRadioProbe.screenshot("gpu-slider");GpuRadioProbe.click("theme");box();value(6);GpuRadioProbe.screenshot("gpu-slider-dark");GpuRadioProbe.click("theme");
   System.out.println("PASS keyboard steps/Home/End, disabled and stale rejection, tab/background retention and light/dark rendering");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
