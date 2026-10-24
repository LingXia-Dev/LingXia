import android.app.UiAutomation;
import android.os.*;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuSurfaceProbe {
 static UiAutomation automation;
 static AccessibilityNodeInfo node(String id)throws Exception{
  for(int i=0;i<24;i++){
   AccessibilityNodeInfo found=GpuRadioProbe.find(automation.getRootInActiveWindow(),id);
   if(found!=null)return found;
   AccessibilityNodeInfo scroll=GpuRadioProbe.find(automation.getRootInActiveWindow(),"surface-scroll");
   if(scroll==null)scroll=GpuRadioProbe.find(automation.getRootInActiveWindow(),"page-scroll");
   if(scroll==null)scroll=GpuRadioProbe.find(automation.getRootInActiveWindow(),"home-scroll");
   if(scroll!=null){scroll.performAction(i<14?AccessibilityNodeInfo.ACTION_SCROLL_FORWARD:AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD);scroll.recycle();}
   Thread.sleep(160);
  }
  throw new AssertionError("missing "+id);
 }
 static void click(String id)throws Exception{
  AccessibilityNodeInfo n=node(id);n.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());n.recycle();Thread.sleep(220);
  n=node(id);GpuUiProbe.check(n.performAction(AccessibilityNodeInfo.ACTION_CLICK),"click "+id);n.recycle();Thread.sleep(500);
 }
 static String text(String id)throws Exception{AccessibilityNodeInfo n=node(id);String value=String.valueOf(n.getText());n.recycle();return value;}
 static void expect(String id,String expected)throws Exception{
  String actual="";for(int i=0;i<25;i++){actual=text(id);if(expected.equals(actual))return;Thread.sleep(160);}
  throw new AssertionError(id+" expected "+expected+" got "+actual);
 }
 static void set(String id,String value)throws Exception{
  AccessibilityNodeInfo n=node(id);Bundle args=new Bundle();args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,value);
  GpuUiProbe.check(n.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,args),"set "+id);n.recycle();Thread.sleep(300);
 }
 public static void main(String[] args)throws Exception{
  String auto=GpuGridProbe.settings("get","system","accelerometer_rotation"),rotation=GpuGridProbe.settings("get","system","user_rotation");
  HandlerThread thread=new HandlerThread("surface-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=automation;GpuApiProbe.a=automation;
  try{
   UiAutomation.class.getMethod("connect").invoke(automation);
   android.accessibilityservice.AccessibilityServiceInfo info=automation.getServiceInfo();
   info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
   info.flags&=~android.accessibilityservice.AccessibilityServiceInfo.FLAG_INCLUDE_NOT_IMPORTANT_VIEWS;
   automation.setServiceInfo(info);Thread.sleep(900);
   for(boolean dark:new boolean[]{false,true}) {
   GpuChromeProbe.tab("Home");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");GpuApiProbe.click("api-surface");
   click("surface-open-window");expect("surface-status","not supported");expect("surface-active","Closed");
   click("surface-open-float");expect("surface-show-count","1");expect("surface-hide-count","0");GpuRadioProbe.screenshot(dark?"gpu-surface-layout-dark":"gpu-surface-layout-light");
   click("surface-increment");click("surface-increment");expect("surface-counter","2");
   set("surface-message","Retained draft");expect("surface-message","Retained draft");
   click("surface-hide");expect("surface-active","Hidden");
   Thread.sleep(500);int frames=GpuChannelProbe.frames();Thread.sleep(1400);
   GpuUiProbe.check(frames==GpuChannelProbe.frames(),"hidden GPU surface is idle");
   click("surface-parent-send");click("surface-parent-show");
   expect("surface-show-count","2");expect("surface-hide-count","1");
   expect("surface-counter","2");expect("surface-message","Retained draft");
   expect("surface-inbound-count","1");expect("surface-inbound","{\"message\":\"Hello from opener\"}");
   GpuRadioProbe.screenshot("gpu-independent-surface-restored");
   GpuGridProbe.settings("put","system","accelerometer_rotation","0");
   GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1200);
   android.graphics.Bitmap landscape=automation.takeScreenshot();
   GpuUiProbe.check(landscape.getWidth()>landscape.getHeight(),"actual surface landscape");
   AccessibilityNodeInfo scroll=node("surface-scroll");android.graphics.Rect bounds=new android.graphics.Rect();scroll.getBoundsInScreen(bounds);scroll.recycle();
   GpuUiProbe.check(bounds.width()>landscape.getWidth()/2,"surface width adapts to landscape: "+bounds);landscape.recycle();
   expect("surface-counter","2");expect("surface-message","Retained draft");
   GpuRadioProbe.screenshot("gpu-independent-surface-landscape");
   GpuGridProbe.settings("put","system","user_rotation",rotation);
   GpuGridProbe.settings("put","system","accelerometer_rotation",auto);Thread.sleep(1000);
   expect("surface-counter","2");expect("surface-message","Retained draft");
   click("surface-send-message");expect("surface-active","Closed");
   GpuUiProbe.check(text("surface-status").startsWith("Message: Retained draft"),"surface reply reaches opener");
   click("surface-open-float");expect("surface-counter","0");expect("surface-show-count","1");expect("surface-hide-count","0");
   click("surface-hide");click("surface-parent-close");expect("surface-active","Closed");
   click("surface-open-float");expect("surface-counter","0");click("surface-close");expect("surface-active","Closed");
   GpuChromeProbe.back();
   }
   GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");
   System.out.println("PASS unsupported desktop window, GPU float lifecycle, hide/show state, hidden delivery, reply, close/reset and idle rendering");
  }catch(Throwable error){error.printStackTrace();throw error;}
  finally{GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
