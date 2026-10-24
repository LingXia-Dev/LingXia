import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONObject;
public final class GpuStreamAutoHeightProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void set(String id,String value)throws Exception{click(id);AccessibilityNodeInfo node=GpuApiProbe.node(id);try{GpuUiProbe.check(node.isFocused(),"focused "+id);}finally{node.recycle();}GpuUiProbe.set(id,value);GpuUiProbe.waitText(id,value);}
 static void absent(String id)throws Exception{AccessibilityNodeInfo n=GpuRadioProbe.find(GpuApiProbe.a.getRootInActiveWindow(),id);boolean missing=n==null;if(n!=null)n.recycle();GpuUiProbe.check(missing,"absent "+id);}
 static JSONObject result(String id)throws Exception{return new JSONObject(GpuApiProbe.text(id));}
 public static void main(String[] args)throws Exception{
 HandlerThread thread=new HandlerThread("dialog-layout");thread.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
 for(boolean dark:new boolean[]{false,true}){
 GpuChromeProbe.tab("Home");click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");click("api-stream");
 set("stream-input","Hello");int shortHeight=GpuThemeProbe.bounds("stream-input").height();
 set("stream-input","A longer sentence with enough words to wrap across several lines in this narrow message composer while the keyboard is visible.");int wrapped=GpuThemeProbe.bounds("stream-input").height();GpuUiProbe.check(wrapped>shortHeight,"soft-wrap grows input");
 set("stream-input","More words to wrap. ".repeat(50));int capped=GpuThemeProbe.bounds("stream-input").height();System.out.println("Composer pixels short="+shortHeight+" wrapped="+wrapped+" cap="+capped);GpuUiProbe.check(capped>=wrapped&&Math.abs(capped-GpuThemeProbe.bounds("stream-send").height()*120.0/36.0)<=2,"editor cap matches reference 120px excluding wrapper padding");GpuRadioProbe.screenshot("gpu-stream-autoheight-"+(dark?"dark":"light"));
 set("stream-input","Hello");GpuUiProbe.check(Math.abs(GpuThemeProbe.bounds("stream-input").height()-shortHeight)<=2,"deletion shrinks input");
 set("stream-input","one\ntwo\nthree");GpuUiProbe.check(GpuThemeProbe.bounds("stream-input").height()>shortHeight,"explicit newlines grow");
 set("stream-input","");GpuRadioProbe.key(4);GpuStreamLayoutProbe.empty();GpuChromeProbe.back();
 }
 GpuChromeProbe.tab("Home");click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS composer soft-wrap/newline growth, cap, shrink and empty in light/dark");
 }catch(Throwable e){e.printStackTrace();throw e;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
