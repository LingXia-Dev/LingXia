import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONObject;
public final class GpuTabbarLayoutProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void set(String id,String value)throws Exception{click(id);AccessibilityNodeInfo node=GpuApiProbe.node(id);try{GpuUiProbe.check(node.isFocused(),"focused "+id);}finally{node.recycle();}GpuUiProbe.set(id,value);GpuUiProbe.waitText(id,value);}
 static void absent(String id)throws Exception{AccessibilityNodeInfo n=GpuRadioProbe.find(GpuApiProbe.a.getRootInActiveWindow(),id);boolean missing=n==null;if(n!=null)n.recycle();GpuUiProbe.check(missing,"absent "+id);}
 static JSONObject result(String id)throws Exception{return new JSONObject(GpuApiProbe.text(id));}
 public static void main(String[] args)throws Exception{
 HandlerThread thread=new HandlerThread("dialog-layout");thread.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
 for(boolean dark:new boolean[]{false,true}){
 GpuChromeProbe.tab("Home");click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");click("api-ui-tabbar");
 click("tabbar-show");GpuUiProbe.waitNative("shown").recycle();GpuUiProbe.waitNative("API").recycle();
 set("tabbar-item-text","GPU API");GpuRadioProbe.key(4);click("tabbar-item-update");GpuUiProbe.waitNative("GPU API").recycle();
 set("tabbar-badge-input","42");GpuRadioProbe.key(4);click("tabbar-badge-set");GpuUiProbe.waitNative("42").recycle();GpuRadioProbe.screenshot("gpu-tabbar-badge-"+(dark?"dark":"light"));click("tabbar-badge-clear");GpuUiProbe.absentNative("42");
 click("tabbar-reddot-show");GpuRadioProbe.screenshot("gpu-tabbar-dot-"+(dark?"dark":"light"));click("tabbar-reddot-hide");
 click("tabbar-hide");GpuUiProbe.waitNative("hidden").recycle();GpuUiProbe.absentNative("GPU API");click("tabbar-show");GpuUiProbe.waitNative("shown").recycle();GpuUiProbe.waitNative("GPU API").recycle();
 set("tabbar-item-text","API");GpuRadioProbe.key(4);click("tabbar-item-update");GpuUiProbe.waitNative("API").recycle();GpuChromeProbe.back();
 }
 GpuChromeProbe.tab("Home");click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS tabbar label, badge set/clear, visibility and completion toasts in light/dark");
 }catch(Throwable e){e.printStackTrace();throw e;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
