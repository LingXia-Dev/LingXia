import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.KeyEvent;

public final class GpuLeaveGuardProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void expect(String id,String text)throws Exception{GpuUiProbe.waitText(id,text);}
 public static void main(String[] args)throws Exception {
  HandlerThread thread=new HandlerThread("leave-guard-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=automation;GpuApiProbe.a=automation;
  try {
   UiAutomation.class.getMethod("connect").invoke(automation);
   android.accessibilityservice.AccessibilityServiceInfo info=automation.getServiceInfo();
   info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(info);
   Thread.sleep(900);
   for(boolean dark:new boolean[]{false,true}) {
   GpuChromeProbe.tab("Home");click(dark?"home-appearance-dark":"home-appearance-light");expect("home-appearance-resolved",dark?"Dark":"Light");
   GpuChromeProbe.tab("API");click("api-leaveguard");
   expect("leave-status","Saved");
   GpuShareLayoutProbe.themePixel(dark);GpuRadioProbe.screenshot(dark?"gpu-leave-layout-dark":"gpu-leave-layout-light");
   GpuUiProbe.check(!GpuApiProbe.node("leave-save").isEnabled(),"clean save disabled");
   GpuUiProbe.set("leave-draft","Keep my edits");expect("leave-status","Unsaved changes");
   GpuRadioProbe.key(KeyEvent.KEYCODE_BACK);Thread.sleep(500);
   expect("leave-requests","leave requests: 0");
   GpuRadioProbe.key(KeyEvent.KEYCODE_BACK);Thread.sleep(500);
   expect("leave-question","Discard unsaved changes?");expect("leave-requests","leave requests: 1");
   GpuRadioProbe.key(KeyEvent.KEYCODE_BACK);Thread.sleep(300);expect("leave-requests","leave requests: 1");
   click("leave-keep");expect("leave-draft","Keep my edits");
   GpuChromeProbe.back();expect("leave-requests","leave requests: 2");
   GpuRadioProbe.screenshot(dark?"gpu-leave-confirm-dark":"gpu-leave-confirm-light");
   click("leave-discard");click("api-leaveguard");
   expect("leave-draft","");expect("leave-requests","leave requests: 0");
   GpuUiProbe.set("leave-draft","Saved edits");GpuRadioProbe.key(KeyEvent.KEYCODE_BACK);
   click("leave-save");expect("leave-status","Saved");
   GpuChromeProbe.back();GpuApiProbe.node("api-leaveguard").recycle();
   }
   GpuChromeProbe.tab("Home");click("home-appearance-light");expect("home-appearance-resolved","Light");
   System.out.println("PASS light/dark layout, native Back, repeated Back, keep, discard, save and fresh page");
  } catch(Throwable error) {error.printStackTrace();throw error;} finally {
   UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();
  }
 }
}
