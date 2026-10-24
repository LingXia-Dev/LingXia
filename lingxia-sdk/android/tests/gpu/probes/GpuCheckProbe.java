import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.os.SystemClock;
import android.view.MotionEvent;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.graphics.Rect;

public final class GpuCheckProbe {
 static UiAutomation automation;
 static void require(boolean value,String message){if(!value)throw new AssertionError(message);}
 static AccessibilityNodeInfo find(AccessibilityNodeInfo n,String id){
  if(n==null)return null;
  if(("com.lingxia.example.gpu.dev:id/"+id).equals(n.getViewIdResourceName()))return n;
  for(int i=0;i<n.getChildCount();i++){AccessibilityNodeInfo found=find(n.getChild(i),id);if(found!=null){n.recycle();return found;}}
  n.recycle();return null;
 }
 static AccessibilityNodeInfo node(String id)throws Exception{
  for(int i=0;i<12;i++){
   AccessibilityNodeInfo n=find(automation.getRootInActiveWindow(),id);if(n!=null)return n;
   if(i>1){AccessibilityNodeInfo scroll=find(automation.getRootInActiveWindow(),"explore-scroll");if(scroll!=null){scroll.performAction(i<7?AccessibilityNodeInfo.ACTION_SCROLL_FORWARD:AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD);scroll.recycle();}}
   Thread.sleep(300);
  }
  throw new AssertionError("missing "+id);
 }
 static void click(String id)throws Exception{AccessibilityNodeInfo n=node(id);require(n.performAction(AccessibilityNodeInfo.ACTION_CLICK),"click "+id);n.recycle();Thread.sleep(500);}
 static void checked(String id,boolean expected,String role)throws Exception{
  AccessibilityNodeInfo n=node(id);require(n.isCheckable()&&n.isChecked()==expected,"checked "+id+" "+n);require(role.contentEquals(n.getClassName()),"role "+id);n.recycle();
 }
 static void tap(String id)throws Exception{
  AccessibilityNodeInfo n=node(id);Rect rect=new Rect();n.getBoundsInScreen(rect);n.recycle();
  require(rect.height()>=120,"44dp touch target");long t=SystemClock.uptimeMillis();
  MotionEvent down=MotionEvent.obtain(t,t,MotionEvent.ACTION_DOWN,rect.centerX(),rect.centerY(),0);
  MotionEvent up=MotionEvent.obtain(t,t+80,MotionEvent.ACTION_UP,rect.centerX(),rect.centerY(),0);
  down.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);up.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);
  try{require(automation.injectInputEvent(down,true),"touch down");require(automation.injectInputEvent(up,true),"touch up");}finally{down.recycle();up.recycle();}Thread.sleep(600);
 }
 static void key(int code)throws Exception{
  Process key=Runtime.getRuntime().exec(new String[]{"/system/bin/input","keyevent",String.valueOf(code)});
  require(key.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&key.exitValue()==0,"key injection");
 }
 static void screenshot(String name)throws Exception{
  android.graphics.Bitmap shot=automation.takeScreenshot();try(java.io.FileOutputStream out=new java.io.FileOutputStream("/data/local/tmp/"+name+".png")){shot.compress(android.graphics.Bitmap.CompressFormat.PNG,100,out);}shot.recycle();
 }
 static void keyboardCheck()throws Exception{
  AccessibilityNodeInfo scroll=node("explore-scroll");scroll.performAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD);scroll.recycle();Thread.sleep(600);
  System.out.println("PASS touch, checked state, disabled, local status; starting keyboard");
  for(int i=0;i<16;i++){
   System.out.println("tab "+i);key(KeyEvent.KEYCODE_TAB);Thread.sleep(100);System.out.println("query focus "+i);
   AccessibilityNodeInfo focus=automation.findFocus(AccessibilityNodeInfo.FOCUS_INPUT);
   System.out.println("focus="+(focus==null?"null":focus.getViewIdResourceName()));
   boolean target=focus!=null&&"com.lingxia.example.gpu.dev:id/keep-notes".equals(focus.getViewIdResourceName());if(focus!=null)focus.recycle();
   if(target){key(KeyEvent.KEYCODE_SPACE);Thread.sleep(500);checked("keep-notes",true,"android.widget.CheckBox");key(KeyEvent.KEYCODE_ENTER);Thread.sleep(500);checked("keep-notes",false,"android.widget.CheckBox");return;}
  }
  throw new AssertionError("keyboard focus did not reach checkbox");
 }
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("check-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  try{
   UiAutomation.class.getMethod("connect").invoke(automation);
   android.accessibilityservice.AccessibilityServiceInfo service=automation.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(service);
   Thread.sleep(700);click("tab-explore");
   checked("keep-notes",true,"android.widget.CheckBox");checked("reminders",false,"android.widget.Switch");
   screenshot("gpu-checks-initial");
   tap("keep-notes");checked("keep-notes",false,"android.widget.CheckBox");
   click("reminders");checked("reminders",true,"android.widget.Switch");
   AccessibilityNodeInfo disabled=node("reminders-disabled");require(!disabled.isEnabled()&&!disabled.isClickable()&&disabled.isChecked(),"disabled semantics");require(!disabled.performAction(AccessibilityNodeInfo.ACTION_CLICK),"disabled activation");disabled.recycle();
   AccessibilityNodeInfo status=node("preferences-status");require(status.getText().toString().contains("Notes off")&&status.getText().toString().contains("Daily moment on"),"local state label");status.recycle();
   keyboardCheck();
   System.out.println("PASS keyboard activation");
   AccessibilityNodeInfo stale=node("reminders");click("tab-hello");require(!stale.performAction(AccessibilityNodeInfo.ACTION_CLICK),"removed control activation");stale.recycle();click("tab-explore");
   checked("keep-notes",false,"android.widget.CheckBox");checked("reminders",true,"android.widget.Switch");
   android.graphics.Bitmap shot=automation.takeScreenshot();try(java.io.FileOutputStream out=new java.io.FileOutputStream("/data/local/tmp/gpu-checks.png")){shot.compress(android.graphics.Bitmap.CompressFormat.PNG,100,out);}shot.recycle();
   System.out.println("PASS: physical touch, accessibility activation, checked roles, disabled and stale actions, local state across tabs");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
