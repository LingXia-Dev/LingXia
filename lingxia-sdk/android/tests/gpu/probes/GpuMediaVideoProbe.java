import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuMediaVideoProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static int darkStatusPixels(UiAutomation a){
  android.graphics.Bitmap capture=a.takeScreenshot();
  android.graphics.Bitmap bitmap=capture.copy(android.graphics.Bitmap.Config.ARGB_8888,false);capture.recycle();int count=0;
  try{
   for(int y=10;y<70;y++)for(int x=20;x<bitmap.getWidth()-20;x++){
    int color=bitmap.getPixel(x,y);
    if(android.graphics.Color.red(color)<130&&android.graphics.Color.green(color)<130&&android.graphics.Color.blue(color)<130)count++;
   }
  }finally{bitmap.recycle();}
  return count;
 }
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("media-video-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);
   android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();
   info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1100);
   click("open-api");click("api-media-VideoTools");click("media-pickVideoForTools");
   GpuUiProbe.clickNative("Album");Thread.sleep(900);
   AccessibilityNodeInfo video=GpuMediaImageProbe.firstPhoto(a.getRootInActiveWindow());
   GpuUiProbe.check(video!=null,"newest fixture video");GpuUiProbe.check(video.performAction(AccessibilityNodeInfo.ACTION_CLICK),"select video");video.recycle();
   GpuUiProbe.clickNative("Done");
   GpuUiProbe.waitText("media-videoInfoResult-width","640");GpuUiProbe.waitText("media-videoInfoResult-height","1280");
   int originalStatusPixels=darkStatusPixels(a);
   GpuUiProbe.check(originalStatusPixels>300,"light-theme baseline status icons");
   GpuUiProbe.check(Long.parseLong(GpuApiProbe.text("media-videoInfoResult-durationMs"))>1000,"video duration");
   GpuUiProbe.set("media-thumbnailMaxWidth","160");GpuUiProbe.set("media-thumbnailMaxHeight","320");
   click("media-createVideoThumbnail");
   GpuUiProbe.waitText("media-thumbnailResult-width","160");GpuUiProbe.waitText("media-thumbnailResult-height","320");
   click("media-previewVideoThumbnail");Thread.sleep(700);GpuRadioProbe.screenshot("gpu-independent-video-thumbnail");GpuRadioProbe.key(4);Thread.sleep(500);
   GpuUiProbe.check(darkStatusPixels(a)>originalStatusPixels/2,"status icon contrast restored after thumbnail preview");
   GpuStreamProbe.clickNow("media-compressSelectedVideo");
   GpuStreamProbe.clickNow("media-cancelVideoCompress");
   Thread.sleep(400);
   AccessibilityNodeInfo retry=GpuApiProbe.node("media-compressSelectedVideo");
   GpuUiProbe.check(retry.isEnabled(),"compression controls restored after cancel");retry.recycle();
   click("media-compressSelectedVideo");
   GpuUiProbe.contains("media-videoCompressResult-uri","lx://");
   GpuUiProbe.check(Long.parseLong(GpuApiProbe.text("media-videoCompressResult-size"))>0,"nonempty compressed video");
   GpuUiProbe.check(Long.parseLong(GpuApiProbe.text("media-videoCompressResult-durationMs"))>1000,"compressed duration");
   GpuRadioProbe.screenshot("gpu-independent-video-compressed");
   click("media-previewCompressedVideo");Thread.sleep(1400);GpuRadioProbe.screenshot("gpu-independent-video-preview");GpuRadioProbe.key(4);Thread.sleep(500);
   GpuUiProbe.check(darkStatusPixels(a)>originalStatusPixels/2,"status icon contrast restored after video preview");
   GpuUiProbe.check(Long.parseLong(GpuApiProbe.text("media-videoCompressResult-size"))>0,"retained video result");
   GpuChromeProbe.back();GpuChromeProbe.back();
   System.out.println("PASS native video selection, metadata, 160x320 thumbnail/preview, cancel/restart, nonempty video compression, preview return and restored system icon contrast");
  }catch(Throwable error){error.printStackTrace();throw error;}
  finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
