import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuFeedbackProbe {
 static void click(String id)throws Exception{if(id.equals("api-feedback")){try(java.io.InputStream in=new java.net.URL("http://127.0.0.1:39590/__probe/feedback").openStream()){while(in.read()!=-1){}}Thread.sleep(1000);}else GpuApiProbe.click(id);}
 static void expect(String id,String value)throws Exception{GpuUiProbe.waitText(id,value);}
 static void enabled(boolean expected)throws Exception{
  AccessibilityNodeInfo n=GpuApiProbe.node("feedback-submit");
  try{GpuUiProbe.check(n.isEnabled()==expected,"submit enabled="+expected);}finally{n.recycle();}
 }
 static AccessibilityNodeInfo closeButton(AccessibilityNodeInfo node){
  if(node==null)return null;
  if("Close".contentEquals(node.getContentDescription()==null?"":node.getContentDescription()))return node;
  for(int i=0;i<node.getChildCount();i++){
   AccessibilityNodeInfo found=closeButton(node.getChild(i));
   if(found!=null){node.recycle();return found;}
  }
  node.recycle();return null;
 }
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("feedback-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=automation;GpuApiProbe.a=automation;
  try{
   UiAutomation.class.getMethod("connect").invoke(automation);
   android.accessibilityservice.AccessibilityServiceInfo info=automation.getServiceInfo();
   info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
   info.flags&=~android.accessibilityservice.AccessibilityServiceInfo.FLAG_INCLUDE_NOT_IMPORTANT_VIEWS;
   automation.setServiceInfo(info);Thread.sleep(900);
   for(boolean dark:new boolean[]{false,true}) {
   GpuChromeProbe.tab("Home");click(dark?"home-appearance-dark":"home-appearance-light");expect("home-appearance-resolved",dark?"Dark":"Light");
   GpuChromeProbe.tab("API");click("api-feedback");
   AccessibilityNodeInfo background=GpuRadioProbe.find(automation.getRootInActiveWindow(),"api-ui-navigation");
   GpuUiProbe.check(background==null,"modal hides background accessibility");
   GpuRadioProbe.screenshot(dark?"gpu-feedback-initial-dark":"gpu-feedback-initial-light");
   enabled(false);GpuUiProbe.set("feedback-message","   ");enabled(false);
   click("feedback-bug");
   AccessibilityNodeInfo selected=GpuApiProbe.node("feedback-bug");
   GpuUiProbe.check(selected.isEnabled(),"Bug category button enabled");selected.recycle();
   GpuUiProbe.set("feedback-message","GPU feedback");
   GpuRadioProbe.key(KeyEvent.KEYCODE_ENTER);
   expect("feedback-message","GPU feedback\n");
   GpuUiProbe.set("feedback-message","GPU feedback\nKeep multiline input");
   GpuUiProbe.set("feedback-email","gpu@example.com");
   GpuRadioProbe.key(KeyEvent.KEYCODE_BACK);enabled(true);
   GpuRadioProbe.screenshot(dark?"gpu-feedback-layout-dark":"gpu-feedback-layout-light");
   click("feedback-submit");
   GpuUiProbe.waitNative("Thanks for the feedback").recycle();
   GpuApiProbe.node("api-ui-navigation").recycle();
   click("api-feedback");expect("feedback-message","");expect("feedback-email","");enabled(false);
   GpuRadioProbe.key(KeyEvent.KEYCODE_BACK);Thread.sleep(500);GpuApiProbe.node("api-ui-navigation").recycle();
   click("api-feedback");
   AccessibilityNodeInfo close=closeButton(automation.getRootInActiveWindow());
   GpuUiProbe.check(close!=null,"native surface close button");
   GpuUiProbe.check(close.performAction(AccessibilityNodeInfo.ACTION_CLICK),"native close");close.recycle();
   Thread.sleep(500);GpuApiProbe.node("api-ui-navigation").recycle();
   Thread.sleep(700);int frames=GpuChannelProbe.frames();Thread.sleep(1600);
   GpuUiProbe.check(GpuChannelProbe.frames()==frames,"closed surface has no idle rendering");
   }
   GpuChromeProbe.tab("Home");click("home-appearance-light");expect("home-appearance-resolved","Light");
   System.out.println("PASS light/dark feedback GPU float, blank gating, category, multiline Enter, email, submit toast, programmatic/native close and fresh reentry");
  }catch(Throwable error){error.printStackTrace();throw error;}finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
  System.exit(0);
 }
}
