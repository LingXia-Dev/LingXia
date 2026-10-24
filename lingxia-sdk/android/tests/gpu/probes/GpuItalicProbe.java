import android.app.UiAutomation;
import android.os.*;
import android.graphics.*;
public final class GpuItalicProbe {
 static int difference(Bitmap image,Rect left,Rect right){
  GpuUiProbe.check(left.width()==right.width()&&left.height()==right.height(),"equal sample boxes");int changed=0;
  for(int y=0;y<left.height();y++)for(int x=0;x<left.width();x++){
   int a=Color.red(image.getPixel(left.left+x,left.top+y)),b=Color.red(image.getPixel(right.left+x,right.top+y));if(Math.abs(a-b)>48)changed++;
  }return changed;
 }
 public static void main(String[]args)throws Exception{
 HandlerThread t=new HandlerThread("italic-pixels");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1200);
 for(int pass=0;pass<4;pass++){
  Rect normal=GpuThemeProbe.bounds("italic-normal"),direct=GpuThemeProbe.bounds("italic-direct"),inherited=GpuThemeProbe.bounds("italic-inherited"),reset=GpuThemeProbe.bounds("italic-reset"),dynamic=GpuThemeProbe.bounds("italic-dynamic");
  Bitmap raw=a.takeScreenshot(),pixels=raw.copy(Bitmap.Config.ARGB_8888,false);raw.recycle();
  try{int changed=difference(pixels,normal,direct);GpuUiProbe.check(changed>100,"italic changes painted glyphs: "+changed);GpuUiProbe.check(difference(pixels,direct,inherited)<16,"inherited italic matches direct");GpuUiProbe.check(difference(pixels,normal,reset)<16,"normal resets inherited italic");GpuUiProbe.check(difference(pixels,pass%2==0?normal:direct,dynamic)<16,"dynamic style uses correct cached shaping");}finally{pixels.recycle();}
  GpuRadioProbe.screenshot("gpu-italic-"+pass);GpuApiProbe.click("italic-toggle");
 }
 GpuRadioProbe.key(4);System.out.println("PASS italic raster difference, inheritance/reset and repeated cache toggles");
 }catch(Throwable error){error.printStackTrace();throw error;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}System.exit(0);
 }
}
