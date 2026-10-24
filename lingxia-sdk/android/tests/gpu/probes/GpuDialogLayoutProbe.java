import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONObject;
public final class GpuDialogLayoutProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void set(String id,String value)throws Exception{click(id);AccessibilityNodeInfo node=GpuApiProbe.node(id);try{GpuUiProbe.check(node.isFocused(),"focused "+id);}finally{node.recycle();}GpuUiProbe.set(id,value);GpuUiProbe.waitText(id,value);}
 static void absent(String id)throws Exception{AccessibilityNodeInfo n=GpuRadioProbe.find(GpuApiProbe.a.getRootInActiveWindow(),id);boolean missing=n==null;if(n!=null)n.recycle();GpuUiProbe.check(missing,"absent "+id);}
 static JSONObject result(String id)throws Exception{return new JSONObject(GpuApiProbe.text(id));}
 public static void main(String[] args)throws Exception{
 HandlerThread thread=new HandlerThread("dialog-layout");thread.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
 for(boolean dark:new boolean[]{false,true}){
  GpuChromeProbe.tab("Home");click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");
  click("api-ui-toast");set("toast-title","GPU Toast");set("toast-duration","10000");GpuRadioProbe.key(4);click("toast-icon");GpuUiProbe.clickNative("None");click("toast-position");GpuUiProbe.clickNative("Top");click("toast-mask");AccessibilityNodeInfo mask=GpuApiProbe.node("toast-mask");GpuUiProbe.check(mask.isChecked(),"mask selected");mask.recycle();click("toast-mask");GpuRadioProbe.screenshot("gpu-dialog-toast-"+(dark?"dark":"light"));click("toast-show");GpuUiProbe.waitNative("GPU Toast").recycle();click("toast-hide");GpuUiProbe.absentNative("GPU Toast");GpuChromeProbe.back();
  click("api-ui-actionsheet");absent("actionsheet-result");click("actionsheet-show");GpuUiProbe.clickNative("\u67e5\u770b\u65e5\u5fd7");GpuUiProbe.check(result("actionsheet-result").getString("id").equals("1"),"selected action id");GpuRadioProbe.screenshot("gpu-dialog-sheet-"+(dark?"dark":"light"));click("actionsheet-show");GpuRadioProbe.key(4);GpuUiProbe.check(result("actionsheet-result").getString("status").equals("canceled"),"cancel action sheet");GpuChromeProbe.back();
  click("api-ui-modal");absent("modal-result");set("modal-title","GPU Dialog");set("modal-content","A native dialog from the GPU page");GpuRadioProbe.key(4);GpuRadioProbe.screenshot("gpu-dialog-modal-"+(dark?"dark":"light"));click("modal-show");GpuUiProbe.waitNative("GPU Dialog").recycle();GpuUiProbe.clickNative("Cancel");GpuUiProbe.check(result("modal-result").getString("status").equals("canceled"),"cancel modal");click("modal-show");GpuUiProbe.clickNative("OK");GpuUiProbe.check(result("modal-result").getString("status").equals("ok"),"confirm modal");GpuRadioProbe.screenshot("gpu-dialog-result-"+(dark?"dark":"light"));click("modal-clear");absent("modal-result");click("modal-show-cancel");set("modal-confirm-text","Proceed");GpuRadioProbe.key(4);click("modal-show");GpuUiProbe.absentNative("Cancel");GpuUiProbe.clickNative("Proceed");GpuUiProbe.check(result("modal-result").getString("status").equals("ok"),"custom confirm without cancel");GpuChromeProbe.back();
 }
 GpuChromeProbe.tab("Home");click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS light/dark toast parameters/show/hide, action sheet select/cancel, modal confirm/cancel/custom confirm/clear");
 }catch(Throwable e){e.printStackTrace();throw e;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
