import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONObject;
public final class GpuSurfaceParentLayoutProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void set(String id,String value)throws Exception{click(id);AccessibilityNodeInfo node=GpuApiProbe.node(id);try{GpuUiProbe.check(node.isFocused(),"focused "+id);}finally{node.recycle();}GpuUiProbe.set(id,value);GpuUiProbe.waitText(id,value);}
 static void absent(String id)throws Exception{AccessibilityNodeInfo n=GpuRadioProbe.find(GpuApiProbe.a.getRootInActiveWindow(),id);boolean missing=n==null;if(n!=null)n.recycle();GpuUiProbe.check(missing,"absent "+id);}
 static JSONObject result(String id)throws Exception{return new JSONObject(GpuApiProbe.text(id));}
 public static void main(String[] args)throws Exception{
 HandlerThread thread=new HandlerThread("dialog-layout");thread.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;GpuSurfaceProbe.automation=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
 for(boolean dark:new boolean[]{false,true}){
 GpuChromeProbe.tab("Home");click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");click("api-surface");
 click("surface-edge-left");click("surface-edge-bottom");click("surface-kind-lxapp");click("surface-kind-window");click("surface-chrome-system");click("surface-chrome-full");click("open-surface");GpuUiProbe.waitText("surface-status","not supported");
 click("surface-kind-float");click("surface-position-bottom");set("surface-width","101%");GpuRadioProbe.key(4);click("open-surface");GpuUiProbe.check(GpuApiProbe.text("size-error").contains("Width"),"reject excessive percent");set("surface-width","90%");set("surface-height","620");GpuRadioProbe.key(4);absent("size-error");GpuRadioProbe.screenshot("gpu-surface-parent-"+(dark?"dark":"light"));click("open-surface");GpuSurfaceProbe.expect("surface-show-count","1");GpuSurfaceProbe.click("surface-hide");GpuApiProbe.node("surface-parent-show").recycle();AccessibilityNodeInfo open=GpuApiProbe.node("open-surface");GpuUiProbe.check(!open.isEnabled(),"open disabled while active");open.recycle();click("surface-parent-show");GpuSurfaceProbe.expect("surface-show-count","2");GpuSurfaceProbe.click("surface-hide");click("surface-parent-close");absent("surface-parent-show");click("open-surface");GpuSurfaceProbe.expect("surface-show-count","1");GpuSurfaceProbe.click("surface-close");absent("surface-parent-show");GpuChromeProbe.back();
 }
 GpuChromeProbe.tab("Home");click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS Surface choices, size validation, window unsupported, float hide/show/close/reopen in light/dark");
 }catch(Throwable e){e.printStackTrace();throw e;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
