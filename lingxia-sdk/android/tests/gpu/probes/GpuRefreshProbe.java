import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.os.SystemClock;
import android.graphics.Rect;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuRefreshProbe {
 static UiAutomation automation;
 static void check(boolean value,String message){if(!value)throw new AssertionError(message);}
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void expect(String id,String value)throws Exception{GpuUiProbe.waitText(id,value);}
 static Rect bounds(String id)throws Exception{
  AccessibilityNodeInfo n=GpuApiProbe.node(id);Rect rect=new Rect();n.getBoundsInScreen(rect);n.recycle();return rect;
 }
 static void top()throws Exception{
  for(int i=0;i<12;i++){
   AccessibilityNodeInfo n=GpuApiProbe.node("page-scroll");
   boolean moved=n.performAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD);n.recycle();Thread.sleep(150);
   if(!moved)break;
  }
  Thread.sleep(300);
 }
 static void event(long down,int action,float x,float y)throws Exception{
  MotionEvent e=MotionEvent.obtain(down,SystemClock.uptimeMillis(),action,x,y,0);e.setSource(InputDevice.SOURCE_TOUCHSCREEN);
  try{check(automation.injectInputEvent(e,true),"touch injection");}finally{e.recycle();}
 }
 static void drag(int dx,int dy,boolean cancel)throws Exception{
  Rect box=bounds("page-scroll");float x=box.centerX(),y=box.top+70;
  long down=SystemClock.uptimeMillis();event(down,MotionEvent.ACTION_DOWN,x,y);
  for(int i=1;i<=20;i++){Thread.sleep(20);event(down,MotionEvent.ACTION_MOVE,x+dx*i/20f,y+dy*i/20f);}
  event(down,cancel?MotionEvent.ACTION_CANCEL:MotionEvent.ACTION_UP,x+dx,y+dy);Thread.sleep(650);
 }
 static void systemInterrupt()throws Exception{
  top();String count=GpuApiProbe.text("pull-refresh-count");Rect box=bounds("page-scroll"),resting=bounds("pull-refresh-status");
  float x=box.centerX(),y=box.top+70;long down=SystemClock.uptimeMillis();
  event(down,MotionEvent.ACTION_DOWN,x,y);
  for(int i=1;i<=20;i++){Thread.sleep(20);event(down,MotionEvent.ACTION_MOVE,x,y+650*i/20f);}
  try(android.os.ParcelFileDescriptor command=automation.executeShellCommand("cmd statusbar expand-notifications");
      java.io.InputStream output=new android.os.ParcelFileDescriptor.AutoCloseInputStream(command)) {
   while(output.read()!=-1){}
  }
  Thread.sleep(600);
  AccessibilityNodeInfo root=automation.getRootInActiveWindow();
  try{check(root!=null&&"com.android.systemui".contentEquals(root.getPackageName()),"notification shade actually interrupts the held pull");}
  finally{if(root!=null)root.recycle();event(down,MotionEvent.ACTION_UP,x,y+650);GpuSearchProbe.shell("/system/bin/cmd","statusbar","collapse");}
  // Android's accessibility active window can remain the last touched shade.
  // Touch the inert page header to resume interaction before querying that window.
  Thread.sleep(600);long returnedTouch=SystemClock.uptimeMillis();
  event(returnedTouch,MotionEvent.ACTION_DOWN,x,box.top+30);
  event(returnedTouch,MotionEvent.ACTION_UP,x,box.top+30);
  boolean returned=false;
  for(int i=0;i<40;i++){
   AccessibilityNodeInfo current=automation.getRootInActiveWindow();
   try{
    if(i==0||i==39)System.out.println("after shade root="+(current==null?"null":current.getPackageName()+" / "+current.getClassName()+" children="+current.getChildCount()));
    returned=current!=null&&"com.lingxia.example.gpu.dev".contentEquals(current.getPackageName());
   }
   finally{if(current!=null)current.recycle();}
   if(returned)break;Thread.sleep(150);
  }
  check(returned,"app window becomes active after notification shade closes");
  expect("pull-refresh-status","Idle");expect("pull-refresh-count",count);Thread.sleep(500);
  check(bounds("pull-refresh-status").equals(resting),"system interruption restores refresh offset");
 }
 public static void main(String[] args)throws Exception{
  String auto=GpuGridProbe.settings("get","system","accelerometer_rotation"),rotation=GpuGridProbe.settings("get","system","user_rotation");
  HandlerThread thread=new HandlerThread("refresh-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=automation;GpuApiProbe.a=automation;
  try{
   UiAutomation.class.getMethod("connect").invoke(automation);
   android.accessibilityservice.AccessibilityServiceInfo info=automation.getServiceInfo();
   info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(info);Thread.sleep(900);
   click("open-api");click("api-refresh");expect("pull-refresh-status","Idle");expect("pull-refresh-count","0");Rect resting=bounds("page-back");
   click("pull-refresh-start");expect("pull-refresh-status","Refreshing...");expect("pull-refresh-count","1");
   check(!GpuApiProbe.text("pull-refresh-time").equals("Never"),"refresh timestamp");
   AccessibilityNodeInfo start=GpuApiProbe.node("pull-refresh-start");check(!start.isEnabled(),"start disabled during refresh");start.recycle();
   GpuRadioProbe.screenshot("gpu-independent-refresh-programmatic");
   click("pull-refresh-stop");expect("pull-refresh-status","Idle");Thread.sleep(600);check(bounds("page-back").equals(resting),"stop restores page position");click("pull-refresh-stop");expect("pull-refresh-count","1");
   top();drag(0,30,false);expect("pull-refresh-count","1");
   top();drag(250,10,false);expect("pull-refresh-count","1");
   top();drag(0,650,true);expect("pull-refresh-count","1");expect("pull-refresh-status","Idle");
   top();drag(0,650,false);expect("pull-refresh-status","Refreshing...");expect("pull-refresh-count","2");
   GpuRadioProbe.screenshot("gpu-independent-refresh-gesture");
   click("pull-refresh-stop");expect("pull-refresh-status","Idle");Thread.sleep(600);
   AccessibilityNodeInfo list=GpuApiProbe.node("page-scroll");check(list.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD),"scroll down before pull");list.recycle();Thread.sleep(400);
   drag(0,100,false);expect("pull-refresh-count","2");
   top();drag(0,650,false);expect("pull-refresh-count","3");
   GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1300);
   android.graphics.Bitmap shot=automation.takeScreenshot();check(shot.getWidth()>shot.getHeight(),"actual refresh landscape");shot.recycle();expect("pull-refresh-count","3");expect("pull-refresh-status","Refreshing...");
   Rect shifted=bounds("page-back"),title=new Rect();AccessibilityNodeInfo nativeTitle=GpuUiProbe.waitNative("Pull Down Refresh");nativeTitle.getBoundsInScreen(title);nativeTitle.recycle();
   check(shifted.top>title.bottom+shifted.height(),"rotated refresh indicator retains exposed space: "+shifted+" / "+title);
   GpuRadioProbe.screenshot("gpu-independent-refresh-landscape");
   GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);Thread.sleep(1000);
   GpuChromeProbe.back();click("api-refresh");expect("pull-refresh-count","0");check(bounds("page-back").equals(resting),"fresh page has no inherited refresh offset");expect("pull-refresh-status","Idle");
   click("pull-refresh-start");expect("pull-refresh-count","1");click("pull-refresh-stop");expect("pull-refresh-status","Idle");GpuChromeProbe.back();GpuChromeProbe.back();
   System.out.println("PASS GPU programmatic refresh, physical pull, short/horizontal/cancel gestures, scrolled content, rotation, offset restoration and page-instance teardown");
  }catch(Throwable error){error.printStackTrace();throw error;}
  finally{GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
