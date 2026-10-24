import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONObject;
public final class GpuNavbarLayoutProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void set(String id,String value)throws Exception{click(id);AccessibilityNodeInfo node=GpuApiProbe.node(id);try{GpuUiProbe.check(node.isFocused(),"focused "+id);}finally{node.recycle();}GpuUiProbe.set(id,value);GpuUiProbe.waitText(id,value);}
 static void absent(String id)throws Exception{AccessibilityNodeInfo n=GpuRadioProbe.find(GpuApiProbe.a.getRootInActiveWindow(),id);boolean missing=n==null;if(n!=null)n.recycle();GpuUiProbe.check(missing,"absent "+id);}
 static JSONObject result(String id)throws Exception{return new JSONObject(GpuApiProbe.text(id));}
 static void chrome(String title,int color)throws Exception{AccessibilityNodeInfo n=GpuUiProbe.waitNative(title);android.graphics.Rect r=new android.graphics.Rect();n.getBoundsInScreen(r);n.recycle();Thread.sleep(250);android.graphics.Bitmap shot=GpuApiProbe.a.takeScreenshot(),bitmap=shot.copy(android.graphics.Bitmap.Config.ARGB_8888,false);shot.recycle();try{GpuThemeProbe.pixel(bitmap,bitmap.getWidth()-12,r.centerY(),color,"native navbar "+title);}finally{bitmap.recycle();}}
 public static void main(String[] args)throws Exception{
 HandlerThread thread=new HandlerThread("dialog-layout");thread.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
 for(boolean dark:new boolean[]{false,true}){
 GpuChromeProbe.tab("Home");click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");click("api-ui-navbar");
 set("navbar-title-input","My navigation");GpuRadioProbe.key(4);click("navbar-set-title");GpuUiProbe.waitNative("My navigation").recycle();
 String[] names={"Dark","Blue","Light","Green"};int[] colors={0xff1f2937,0xff3b82f6,0xffffffff,0xff10b981};
 for(int i=0;i<names.length;i++){click("navbar-preset-"+names[i].toLowerCase());chrome(names[i]+" Theme",colors[i]);}
 set("navbar-bg-input","#234567");set("navbar-fg-input","#ffffff");GpuRadioProbe.key(4);click("navbar-set-colors");chrome("Green Theme",0xff234567);
 click("navbar-home-hide");click("navbar-home-auto");GpuRadioProbe.screenshot("gpu-navbar-custom-"+(dark?"dark":"light"));
 click("navbar-reset");chrome("User Interface",dark?0xff1c1c1e:0xffffffff);GpuRadioProbe.screenshot("gpu-navbar-reset-"+(dark?"dark":"light"));GpuChromeProbe.back();
 }
 GpuChromeProbe.tab("Home");click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS navbar title, four preset titles/colors, custom color and reset in light/dark");
 }catch(Throwable e){e.printStackTrace();throw e;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
