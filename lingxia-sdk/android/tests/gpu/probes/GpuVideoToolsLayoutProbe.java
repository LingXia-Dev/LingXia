import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuVideoToolsLayoutProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void setVisible(String id,String value)throws Exception{
  GpuApiProbe.click(id);AccessibilityNodeInfo node=GpuApiProbe.node(id);
  try{GpuUiProbe.check(node.isFocused(),"focused "+id);}finally{node.recycle();}
  GpuUiProbe.set(id,value);GpuUiProbe.waitText(id,value);
 }
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
   for(boolean dark:new boolean[]{false,true}){
   GpuChromeProbe.tab("Home");click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");click("api-media-VideoTools");GpuUiProbe.waitText("media-page-title","Video Tools");Thread.sleep(250);GpuRadioProbe.screenshot("gpu-video-tools-empty-"+(dark?"dark":"light"));click("media-pickVideoForTools");
   GpuUiProbe.clickNative("Album");Thread.sleep(900);
   AccessibilityNodeInfo video=GpuMediaImageProbe.firstPhoto(a.getRootInActiveWindow());
   GpuUiProbe.check(video!=null,"newest fixture video");GpuUiProbe.check(video.performAction(AccessibilityNodeInfo.ACTION_CLICK),"select video");video.recycle();
   GpuUiProbe.clickNative("Done");
   GpuUiProbe.check(!GpuApiProbe.text("media-videoInfoResult-resolution").contains("--"),"source resolution");
   int originalStatusPixels=darkStatusPixels(a);
   if(!dark)GpuUiProbe.check(originalStatusPixels>300,"light-theme baseline status icons");
   GpuUiProbe.check(Double.parseDouble(GpuApiProbe.text("media-videoInfoResult-durationMs").replace(" s",""))>0,"video duration");
   System.out.println("Editing theme="+(dark?"dark":"light"));
   setVisible("media-thumbnailMaxWidth","160");setVisible("media-thumbnailMaxHeight","100");
   System.out.println("Before dismiss theme="+(dark?"dark":"light"));GpuRadioProbe.key(4);Thread.sleep(300);System.out.println("After dismiss height="+GpuApiProbe.text("media-thumbnailMaxHeight"));GpuUiProbe.waitText("media-thumbnailMaxWidth","160");GpuUiProbe.waitText("media-thumbnailMaxHeight","100");click("media-createVideoThumbnail");
   GpuUiProbe.check(Integer.parseInt(GpuApiProbe.text("media-thumbnailResult-width").replace(" px",""))<=160,"bounded thumbnail width");GpuUiProbe.check(Integer.parseInt(GpuApiProbe.text("media-thumbnailResult-height").replace(" px",""))<=100,"independent height bound");GpuRadioProbe.screenshot("gpu-video-tools-thumbnail-"+(dark?"dark":"light"));
   click("media-previewVideoThumbnail");Thread.sleep(700);GpuRadioProbe.screenshot("gpu-independent-video-thumbnail");GpuRadioProbe.key(4);Thread.sleep(500);
   if(!dark)GpuUiProbe.check(darkStatusPixels(a)>originalStatusPixels/2,"status icon contrast restored after thumbnail preview");
   GpuStreamProbe.clickNow("media-compressSelectedVideo");
   GpuStreamProbe.clickNow("media-cancelVideoCompress");
   Thread.sleep(400);
   AccessibilityNodeInfo retry=GpuApiProbe.node("media-compressSelectedVideo");
   GpuUiProbe.check(retry.isEnabled(),"compression controls restored after cancel");retry.recycle();
   click("media-compressSelectedVideo");
   GpuUiProbe.contains("media-videoCompressResult-uri","lx://");
   GpuUiProbe.check(!GpuApiProbe.text("media-videoCompressResult-size").equals("0 B"),"nonempty compressed video");
   GpuUiProbe.check(Double.parseDouble(GpuApiProbe.text("media-videoCompressResult-durationMs").replace(" s",""))>0,"compressed duration");
   GpuRadioProbe.screenshot("gpu-video-tools-compressed-"+(dark?"dark":"light"));
   click("media-previewCompressedVideo");Thread.sleep(1400);GpuRadioProbe.screenshot("gpu-independent-video-preview");GpuRadioProbe.key(4);Thread.sleep(500);
   if(!dark)GpuUiProbe.check(darkStatusPixels(a)>originalStatusPixels/2,"status icon contrast restored after video preview");
   GpuUiProbe.check(!GpuApiProbe.text("media-videoCompressResult-size").equals("0 B"),"retained video result");
   GpuChromeProbe.back();
   }
   GpuChromeProbe.tab("Home");click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");
   System.out.println("PASS video tools light/dark real selection/metadata, bounded thumbnail/preview, cancel/restart, compression and preview return");
  }catch(Throwable error){error.printStackTrace();throw error;}
  finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
