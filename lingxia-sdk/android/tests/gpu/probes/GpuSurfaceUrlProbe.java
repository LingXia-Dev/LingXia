import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;

public final class GpuSurfaceUrlProbe {
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("surface-url-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=automation;GpuApiProbe.a=automation;
  try{
   UiAutomation.class.getMethod("connect").invoke(automation);
   android.accessibilityservice.AccessibilityServiceInfo info=automation.getServiceInfo();
   info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
   info.flags&=~android.accessibilityservice.AccessibilityServiceInfo.FLAG_INCLUDE_NOT_IMPORTANT_VIEWS;
   automation.setServiceInfo(info);Thread.sleep(900);
   GpuApiProbe.click("open-api");GpuApiProbe.click("api-surface");
   boolean invalid=args.length>0&&args[0].equals("invalid");
   if(invalid)GpuUiProbe.set("surface-url","http://example.com");
   else if(args.length>0&&!args[0].equals("error"))GpuUiProbe.set("surface-url",args[0]);
   GpuApiProbe.click("surface-open-aside");
   if(invalid){
    GpuUiProbe.contains("surface-status","Failed (invalid_arg):");
    GpuUiProbe.waitText("surface-active","Closed");
    GpuChromeProbe.back();GpuChromeProbe.back();
    System.out.println("PASS unsupported URL scheme is rejected and shown by GPU controller");
    return;
   }
   boolean failure=args.length>0&&args[0].equals("error");
   GpuUiProbe.waitNative(failure?"Couldn't load this page":args.length>1?args[1]:"Example Domain").recycle();
   if(failure)GpuUiProbe.waitNative("Retry").recycle();
   GpuRadioProbe.screenshot("gpu-independent-surface-url");
   GpuRadioProbe.key(4);
   GpuApiProbe.node("surface-open-aside").recycle();
   GpuUiProbe.contains("surface-status","Opened");
   GpuChromeProbe.back();GpuChromeProbe.back();
   System.out.println(failure?"PASS URL error document, Retry affordance and Back to GPU controller":"PASS GPU surface controls open URL aside, HTTP page loads, Back restores GPU controller");
  }catch(Throwable error){error.printStackTrace();throw error;}
  finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
