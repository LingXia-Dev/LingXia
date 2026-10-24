import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONObject;
public final class GpuStreamWrapProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void set(String id,String value)throws Exception{click(id);AccessibilityNodeInfo node=GpuApiProbe.node(id);try{GpuUiProbe.check(node.isFocused(),"focused "+id);}finally{node.recycle();}GpuUiProbe.set(id,value);GpuUiProbe.waitText(id,value);}
 static void absent(String id)throws Exception{AccessibilityNodeInfo n=GpuRadioProbe.find(GpuApiProbe.a.getRootInActiveWindow(),id);boolean missing=n==null;if(n!=null)n.recycle();GpuUiProbe.check(missing,"absent "+id);}
 static JSONObject result(String id)throws Exception{return new JSONObject(GpuApiProbe.text(id));}
 public static void main(String[] args)throws Exception{
 HandlerThread thread=new HandlerThread("dialog-layout");thread.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
 String value="https://example.com/"+"abcdefgh12345678".repeat(12);
 for(boolean dark:new boolean[]{false,true}){
 GpuChromeProbe.tab("Home");click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");click("api-stream");
 GpuStreamProbe.set(value);GpuRadioProbe.key(4);GpuStreamProbe.clickNow("stream-send");
 AccessibilityNodeInfo n=GpuUiProbe.waitNative(value);android.graphics.Rect r=new android.graphics.Rect();n.getBoundsInScreen(r);n.recycle();android.graphics.Bitmap shot=a.takeScreenshot();int width=shot.getWidth();shot.recycle();GpuUiProbe.check(r.left>=0&&r.right<width&&r.height()>100,"long URL wraps within viewport: "+r);GpuRadioProbe.screenshot("gpu-stream-wrap-"+(dark?"dark":"light"));
 GpuStreamProbe.clickNow("stream-stop");GpuStreamLayoutProbe.settled();click("stream-clear");GpuStreamLayoutProbe.empty();GpuChromeProbe.back();
 }
 GpuChromeProbe.tab("Home");click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS long unbroken chat URL wraps in light/dark, cancel/clear");
 }catch(Throwable e){e.printStackTrace();throw e;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
