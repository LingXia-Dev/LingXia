import android.app.UiAutomation;
import android.os.*;
import android.graphics.Rect;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuTabTransitionProbe {
 static Rect tab(String label)throws Exception {
  AccessibilityNodeInfo node=GpuChromeProbe.tabText(GpuApiProbe.a.getRootInActiveWindow(),label);
  GpuApiProbe.check(node!=null,"tab "+label);
  while(!node.isClickable()){AccessibilityNodeInfo parent=node.getParent();node.recycle();node=parent;GpuApiProbe.check(node!=null,"clickable tab "+label);}
  Rect bounds=new Rect();node.getBoundsInScreen(bounds);node.recycle();return bounds;
 }
 static void tap(Rect bounds)throws Exception {
  long time=SystemClock.uptimeMillis();
  MotionEvent down=MotionEvent.obtain(time,time,MotionEvent.ACTION_DOWN,bounds.centerX(),bounds.centerY(),0);
  MotionEvent up=MotionEvent.obtain(time,time+20,MotionEvent.ACTION_UP,bounds.centerX(),bounds.centerY(),0);
  down.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);up.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);
  try {GpuApiProbe.check(GpuApiProbe.a.injectInputEvent(down,true)&&GpuApiProbe.a.injectInputEvent(up,true),"native tab tap");}finally{down.recycle();up.recycle();}
 }
 public static void main(String[] args)throws Exception {
  HandlerThread thread=new HandlerThread("tab-transition");thread.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuApiProbe.a=automation;GpuRadioProbe.automation=automation;
  try {
   UiAutomation.class.getMethod("connect").invoke(automation);
   android.accessibilityservice.AccessibilityServiceInfo info=automation.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(info);
   GpuChromeProbe.awaitHomeReady();
   GpuApiProbe.click(args.length>0&&args[0].equals("dark")?"home-appearance-dark":"home-appearance-light");
   Rect home=tab("Home"),todo=tab("ToDo"),api=tab("API"),media=tab("Media"),device=tab("Device");
   for(int round=0;round<4;round++) {
    for(Rect target:new Rect[]{todo,api,home}) {tap(target);Thread.sleep(550);}
   }
   for(int round=0;round<8;round++) {
    for(Rect target:new Rect[]{api,todo,home}) {tap(target);Thread.sleep(75);}
   }
   Thread.sleep(1000);GpuApiProbe.node("home-name").recycle();
   tap(api);Thread.sleep(1000);GpuApiProbe.node("api-bridge-section").recycle();
   tap(todo);Thread.sleep(1000);GpuApiProbe.node("todo-input").recycle();
   tap(home);Thread.sleep(1000);GpuApiProbe.node("home-name").recycle();
   for(int round=0;round<2;round++) {
    tap(media);Thread.sleep(800);GpuApiProbe.node("media-page-title").recycle();
    tap(device);Thread.sleep(800);GpuApiProbe.node("device-mode-title").recycle();
    tap(home);Thread.sleep(800);GpuApiProbe.node("home-name").recycle();
   }
   GpuTodoProbe.touch("home-name");GpuRadioProbe.key(4);GpuChromeProbe.tab("API");GpuApiProbe.node("api-bridge-section").recycle();
   GpuChromeProbe.tab("Home");GpuApiProbe.node("home-name").recycle();
   System.out.println("PASS 47 tab switches across all five tabs, rapid-tap final target and switch after IME dismissal; inspect recording and app error log");
  }finally {UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
