import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
public final class GpuVideoInputProbe {
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("video-input");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);
   android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(900);
   GpuChromeProbe.tab("API");
   for(int i=0;i<12;i++){
    GpuApiProbe.click("api-media-VideoTools");GpuUiProbe.waitText("media-page-title","Video Tools");
    GpuUiProbe.set("media-thumbnailMaxWidth","160");GpuUiProbe.waitText("media-thumbnailMaxWidth","160");
    GpuUiProbe.set("media-thumbnailMaxHeight","100");GpuUiProbe.waitText("media-thumbnailMaxHeight","100");
    GpuRadioProbe.key(4);GpuUiProbe.waitText("media-thumbnailMaxWidth","160");GpuUiProbe.waitText("media-thumbnailMaxHeight","100");
    System.out.println("PASS edit/dismiss cycle "+i);GpuChromeProbe.back();
   }
   GpuChromeProbe.tab("Home");
  }catch(Throwable e){e.printStackTrace();throw e;}
  finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
