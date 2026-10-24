import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONObject;
public final class GpuStreamKeyboardProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void set(String id,String value)throws Exception{click(id);AccessibilityNodeInfo node=GpuApiProbe.node(id);try{GpuUiProbe.check(node.isFocused(),"focused "+id);}finally{node.recycle();}GpuUiProbe.set(id,value);GpuUiProbe.waitText(id,value);}
 static void absent(String id)throws Exception{AccessibilityNodeInfo n=GpuRadioProbe.find(GpuApiProbe.a.getRootInActiveWindow(),id);boolean missing=n==null;if(n!=null)n.recycle();GpuUiProbe.check(missing,"absent "+id);}
 static void dismissKeyboard()throws Exception{
  for(android.view.accessibility.AccessibilityWindowInfo window:GpuApiProbe.a.getWindows()){
   if(window.getType()==android.view.accessibility.AccessibilityWindowInfo.TYPE_INPUT_METHOD){GpuRadioProbe.key(4);return;}
  }
 }
 public static void main(String[] args)throws Exception{
 HandlerThread thread=new HandlerThread("dialog-layout");thread.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;GpuHandlesProbe.automation=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS|android.accessibilityservice.AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS;a.setServiceInfo(info);Thread.sleep(1000);

 for(boolean dark:new boolean[]{false,true}){
 GpuChromeProbe.tab("Home");click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");click("api-stream");
 set("stream-input","First line");
 long time=android.os.SystemClock.uptimeMillis();
 for(int action:new int[]{android.view.KeyEvent.ACTION_DOWN,android.view.KeyEvent.ACTION_UP}) {
  GpuUiProbe.check(a.injectInputEvent(new android.view.KeyEvent(time,time,action,android.view.KeyEvent.KEYCODE_ENTER,0,android.view.KeyEvent.META_SHIFT_ON),true),"inject Shift+Enter");
 }
 GpuUiProbe.waitText("stream-input","First line\n");absent("stream-live");
 java.lang.Process typing=Runtime.getRuntime().exec(new String[]{"/system/bin/input","text","Second%s"});
 GpuUiProbe.check(typing.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&typing.exitValue()==0,"type second line");
 GpuUiProbe.waitText("stream-input","First line\nSecond ");
 GpuRadioProbe.key(android.view.KeyEvent.KEYCODE_ENTER);
 GpuStreamProbe.firstToken();GpuUiProbe.waitText("stream-input","");
 GpuUiProbe.waitNative("First line\nSecond").recycle();GpuRadioProbe.screenshot("gpu-stream-keyboard-"+(dark?"dark":"light"));
 GpuStreamLayoutProbe.settled();dismissKeyboard();click("stream-clear");GpuStreamLayoutProbe.empty();
 set("stream-input","Soft keyboard message");Thread.sleep(800);
 boolean tapped=false;for(int attempt=0;attempt<30&&!tapped;attempt++){AccessibilityNodeInfo send=GpuHandlesProbe.label("Send");if(send!=null){GpuRadioProbe.screenshot("gpu-soft-send-before");tapped=send.performAction(AccessibilityNodeInfo.ACTION_CLICK);send.recycle();}if(!tapped)Thread.sleep(100);}
 GpuUiProbe.check(tapped,"Gboard exposes Send action");
 GpuStreamProbe.firstToken();GpuUiProbe.waitText("stream-input","");GpuUiProbe.waitNative("Soft keyboard message").recycle();
 GpuStreamLayoutProbe.settled();dismissKeyboard();click("stream-clear");GpuStreamLayoutProbe.empty();GpuChromeProbe.back();
 }

 GpuChromeProbe.tab("Home");click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS hardware Enter and Gboard Send submit, Shift+Enter inserts newline, both themes");
 }catch(Throwable e){e.printStackTrace();throw e;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
