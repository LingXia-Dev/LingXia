import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuMediaImageProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static AccessibilityNodeInfo firstPhoto(AccessibilityNodeInfo n){
  if(n==null)return null;
  if(n.isClickable()&&"android.widget.FrameLayout".contentEquals(n.getClassName())){
   for(int i=0;i<n.getChildCount();i++){
    AccessibilityNodeInfo child=n.getChild(i);
    if(child!=null){boolean image="android.widget.ImageView".contentEquals(child.getClassName());child.recycle();if(image)return n;}
   }
  }
  for(int i=0;i<n.getChildCount();i++){AccessibilityNodeInfo found=firstPhoto(n.getChild(i));if(found!=null){n.recycle();return found;}}
  n.recycle();return null;
 }
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("media-image-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);
   android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();
   info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1100);
   if(args.length==0||!args[0].equals("resume")){
    click("open-api");click("api-media-ImageInfo");click("media-pickImageForInfo");
    GpuUiProbe.clickNative("Album");
    Thread.sleep(600);
    AccessibilityNodeInfo allow=GpuUiProbe.nativeText(a.getRootInActiveWindow(),"Allow");
    if(allow!=null){allow.recycle();GpuUiProbe.clickNative("Allow");}
   }
   Thread.sleep(1200);
   GpuRadioProbe.screenshot("gpu-independent-media-picker");
   if(args.length>0&&args[0].equals("inspect"))return;
   // The freshly scanned fixture is first in the native picker's date ordering.
   AccessibilityNodeInfo photo=firstPhoto(a.getRootInActiveWindow());
   GpuUiProbe.check(photo!=null,"fixture photo");GpuUiProbe.check(photo.performAction(AccessibilityNodeInfo.ACTION_CLICK),"select photo");photo.recycle();
   GpuUiProbe.clickNative("Done");
   GpuUiProbe.waitText("media-imageInfoResult-width","1024");
   GpuUiProbe.waitText("media-imageInfoResult-height","1024");
   GpuUiProbe.set("media-compressedWidth","256");GpuUiProbe.set("media-compressedHeight","256");
   click("media-compressSelectedImage");
   GpuUiProbe.waitText("media-compressResult-width","256");GpuUiProbe.waitText("media-compressResult-height","256");
   GpuUiProbe.check(Integer.parseInt(GpuApiProbe.text("media-compressResult-size"))>0,"nonempty compression output");
   GpuRadioProbe.screenshot("gpu-independent-media-compressed");
   click("media-previewCompressedImage");Thread.sleep(900);
   GpuRadioProbe.screenshot("gpu-independent-media-preview");
   GpuRadioProbe.key(4);Thread.sleep(700);
   GpuUiProbe.waitText("media-compressResult-width","256");
   GpuChromeProbe.back();GpuChromeProbe.back();
   System.out.println("PASS native album selection, 1024x1024 image info, 256x256 nonempty compression, preview and return state");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
