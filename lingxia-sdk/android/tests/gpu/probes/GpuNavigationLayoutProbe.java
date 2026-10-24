import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONObject;
public final class GpuNavigationLayoutProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void set(String id,String value)throws Exception{click(id);AccessibilityNodeInfo node=GpuApiProbe.node(id);try{GpuUiProbe.check(node.isFocused(),"focused "+id);}finally{node.recycle();}GpuUiProbe.set(id,value);GpuUiProbe.waitText(id,value);}
 static void absent(String id)throws Exception{AccessibilityNodeInfo n=GpuRadioProbe.find(GpuApiProbe.a.getRootInActiveWindow(),id);boolean missing=n==null;if(n!=null)n.recycle();GpuUiProbe.check(missing,"absent "+id);}
 static JSONObject result(String id)throws Exception{return new JSONObject(GpuApiProbe.text(id));}
 public static void main(String[] args)throws Exception{
 HandlerThread thread=new HandlerThread("dialog-layout");thread.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
 for(boolean dark:new boolean[]{false,true}){
 GpuChromeProbe.tab("Home");click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");click("api-ui-navigation");
 String first=GpuApiProbe.text("lifecycle-instance-tag");String depth=GpuApiProbe.text("ui-page-stack-depth");
 click("lifecycle-bump-logic");click("lifecycle-bump-view");click("lifecycle-bump-module");GpuUiProbe.waitText("lifecycle-bump-logic","Logic +1: 1");GpuUiProbe.waitText("lifecycle-bump-view","View +1: 1");String module=GpuApiProbe.text("lifecycle-bump-module");GpuRadioProbe.screenshot("gpu-navigation-state-"+(dark?"dark":"light"));
 click("ui-navigate-to");String second=GpuApiProbe.text("lifecycle-instance-tag");GpuUiProbe.check(!first.equals(second),"new instance on push");GpuUiProbe.waitText("lifecycle-previous-tag",first);GpuUiProbe.waitText("lifecycle-bump-logic","Logic +1: 0");GpuUiProbe.waitText("lifecycle-bump-view","View +1: 0");GpuUiProbe.waitText("lifecycle-bump-module",module);GpuUiProbe.check(!depth.equals(GpuApiProbe.text("ui-page-stack-depth")),"stack grows");
 click("ui-navigate-back");GpuUiProbe.waitText("lifecycle-instance-tag",first);GpuUiProbe.waitText("lifecycle-bump-logic","Logic +1: 1");GpuUiProbe.waitText("lifecycle-bump-view","View +1: 1");
 click("ui-redirect-to");GpuUiProbe.check(!first.equals(GpuApiProbe.text("lifecycle-instance-tag")),"redirect identity");GpuUiProbe.waitText("ui-page-stack-depth",depth);
 click("lifecycle-open-popup");GpuRadioProbe.screenshot("gpu-navigation-popup-"+(dark?"dark":"light"));click("lifecycle-popup-close");absent("lifecycle-popup-close");click("lifecycle-open-popup");GpuRadioProbe.key(4);click("api-ui-navigation");absent("lifecycle-popup-close");GpuUiProbe.waitText("lifecycle-bump-view","View +1: 0");click("ui-switch-tab");GpuApiProbe.node("home-name").recycle();
 }
 click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS navigation stack, instance/local/Logic/module state, redirect depth, popup close/reentry, switchTab in light/dark");
 }catch(Throwable e){e.printStackTrace();throw e;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
