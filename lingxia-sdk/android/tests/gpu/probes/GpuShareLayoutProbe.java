import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;
public final class GpuShareLayoutProbe {
 static android.graphics.Rect bounds(String id)throws Exception{AccessibilityNodeInfo n=GpuApiProbe.node(id);android.graphics.Rect r=new android.graphics.Rect();n.getBoundsInScreen(r);n.recycle();return r;}
 static void pair(String a,String b)throws Exception{GpuApiProbe.node(a).recycle();android.graphics.Rect first=bounds(a),second=bounds(b);GpuUiProbe.check(Math.abs(first.top-second.top)<3&&first.right<second.left,"side-by-side action pair");}
 static void themePixel(boolean dark)throws Exception{boolean matched=false;for(int i=0;i<25&&!matched;i++){android.graphics.Bitmap raw=GpuRadioProbe.automation.takeScreenshot(),shot=raw.copy(android.graphics.Bitmap.Config.ARGB_8888,false);raw.recycle();int pixel=shot.getPixel(5,shot.getHeight()/3);shot.recycle();int red=android.graphics.Color.red(pixel);matched=dark?red<80:red>180;if(!matched)Thread.sleep(200);}GpuUiProbe.check(matched,"rendered theme");}
 public static void main(String[] args)throws Exception{
  HandlerThread t=new HandlerThread("share-layout");t.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),connection);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
   for(boolean dark:new boolean[]{false,true}){
    GpuChromeProbe.tab("Home");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");GpuApiProbe.click("api-share");GpuApiProbe.node("share-text").recycle();themePixel(dark);GpuRadioProbe.screenshot(dark?"gpu-share-layout-dark":"gpu-share-layout-light");
    if(!dark){GpuApiProbe.click("share-text");AccessibilityNodeInfo root=a.getRootInActiveWindow();GpuUiProbe.check(root!=null&&!"com.lingxia.example.gpu.dev".contentEquals(root.getPackageName()),"native share sheet opens");root.recycle();GpuRadioProbe.key(4);GpuApiProbe.node("share-text").recycle();}
    pair("share-choose-image","share-image");pair("share-choose-file","share-file");GpuRadioProbe.screenshot(dark?"gpu-share-files-dark":"gpu-share-files-light");GpuApiProbe.click("share-choose-file");Thread.sleep(500);GpuRadioProbe.key(4);GpuUiProbe.waitText("share-status","File selection canceled");GpuChromeProbe.back();
   }
   GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS Share light/dark cards, action pairs, native text share dismissal and file cancellation");
  }catch(Throwable error){error.printStackTrace();throw error;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
