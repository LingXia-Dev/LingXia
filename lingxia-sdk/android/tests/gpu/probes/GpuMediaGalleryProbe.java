import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;
public final class GpuMediaGalleryProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void pick(String id,String label)throws Exception{click(id);GpuUiProbe.clickNative(label);}
 static AccessibilityNodeInfo photoAt(AccessibilityNodeInfo n,int[] skip){
  if(n==null)return null;
  if(n.isClickable()&&"android.widget.FrameLayout".contentEquals(n.getClassName())){
   for(int i=0;i<n.getChildCount();i++){AccessibilityNodeInfo child=n.getChild(i);if(child!=null){boolean image="android.widget.ImageView".contentEquals(child.getClassName());child.recycle();if(image){if(skip[0]--==0)return n;break;}}}
  }
  for(int i=0;i<n.getChildCount();i++){AccessibilityNodeInfo found=photoAt(n.getChild(i),skip);if(found!=null){n.recycle();return found;}}n.recycle();return null;
 }
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("media-gallery");thread.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
   for(boolean dark:new boolean[]{false,true}){
    GpuChromeProbe.tab("Home");click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");
    for(String mode:new String[]{"Pictures","Videos"}){
     click("api-media-"+mode);GpuUiProbe.waitText("media-page-title","Media Manager");GpuMediaProbe.disabled("media-previewSelectedMedia");
     pick("media-openSourcePicker","Camera");GpuMediaProbe.disabled("media-openCountPicker");GpuUiProbe.waitText("media-selection-count","0/1");pick("media-openSourcePicker","Album");GpuFileLayoutProbe.enabled("media-openCountPicker",true);pick("media-openCountPicker","2");GpuUiProbe.waitText("media-selection-count","0/2");
     pick("media-openPreviewRotatePicker","90\u00b0");GpuUiProbe.waitText("media-openPreviewRotatePicker","Preview Rotate: 90\u00b0");pick("media-openPreviewObjectFitPicker","contain");GpuUiProbe.waitText("media-openPreviewObjectFitPicker","Preview Fit: contain");click("media-preview-index");GpuUiProbe.waitText("media-preview-index","Index Indicator: Hidden");
     GpuRadioProbe.screenshot("gpu-gallery-empty-"+mode+"-"+(dark?"dark":"light"));click("media-launchMediaDemo");Thread.sleep(800);
     GpuRadioProbe.screenshot("gpu-gallery-native-before");AccessibilityNodeInfo photo=GpuMediaImageProbe.firstPhoto(a.getRootInActiveWindow());GpuUiProbe.check(photo!=null,"album item");GpuUiProbe.check(photo.performAction(AccessibilityNodeInfo.ACTION_CLICK),"select media");photo.recycle();if(mode.equals("Pictures")){AccessibilityNodeInfo second=photoAt(a.getRootInActiveWindow(),new int[]{1});GpuUiProbe.check(second!=null,"second album image");GpuUiProbe.check(second.performAction(AccessibilityNodeInfo.ACTION_CLICK),"select second image");second.recycle();}Thread.sleep(300);GpuRadioProbe.screenshot("gpu-gallery-native-selected");GpuUiProbe.clickNative("Done");GpuUiProbe.waitText("media-selection-count",mode.equals("Pictures")?"2/2":"1/2");
     if(mode.equals("Pictures")){GpuApiProbe.node("media-image-1").recycle();AccessibilityNodeInfo add=GpuRadioProbe.find(a.getRootInActiveWindow(),"media-launchMediaDemo");GpuUiProbe.check(add==null,"selection limit hides add tile");AccessibilityNodeInfo image=GpuApiProbe.node("media-image-0");image.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());image.recycle();Thread.sleep(300);}
     GpuRadioProbe.screenshot("gpu-gallery-selected-"+mode+"-"+(dark?"dark":"light"));click("media-previewSelectedMedia");Thread.sleep(900);GpuRadioProbe.key(4);Thread.sleep(500);GpuUiProbe.contains("media-preview-source","lx://");GpuFileLayoutProbe.enabled("media-previewSelectedMedia",true);AccessibilityNodeInfo source=GpuApiProbe.node("media-preview-source"),root=a.getRootInActiveWindow();android.graphics.Rect sourceRect=new android.graphics.Rect(),screen=new android.graphics.Rect();source.getBoundsInScreen(sourceRect);root.getBoundsInScreen(screen);source.recycle();root.recycle();GpuUiProbe.check(sourceRect.right<=screen.right-32,"result path stays inside page");GpuUiProbe.check(sourceRect.height()>50,"long result path wraps");GpuRadioProbe.screenshot("gpu-gallery-result-"+mode+"-"+(dark?"dark":"light"));GpuChromeProbe.back();
    }
   }
   GpuChromeProbe.tab("Home");click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS light/dark image/video gallery settings, camera count gate, album selection, native preview and result");
  }catch(Throwable e){e.printStackTrace();throw e;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
