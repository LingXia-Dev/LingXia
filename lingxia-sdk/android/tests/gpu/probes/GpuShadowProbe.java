import android.app.UiAutomation;
import android.os.*;
import android.graphics.*;

public class GpuShadowProbe {
 static void pixel(String id,int x,int y,int expected)throws Exception{GpuBorderProbe.pixel(id,x,y,expected);}
 static int sample(String id,int x,int y)throws Exception{
  Rect r=GpuBorderProbe.box(id);Bitmap shot=GpuApiProbe.a.takeScreenshot(),b=shot.copy(Bitmap.Config.ARGB_8888,false);shot.recycle();
  int color=b.getPixel(r.left+Math.round(x*GpuBorderProbe.scale),r.top+Math.round(y*GpuBorderProbe.scale));b.recycle();return color;
 }
 static void blur()throws Exception{
  int near=Color.red(sample("shadow-input",122,24)),far=Color.red(sample("shadow-input",130,24));
  GpuApiProbe.check(near>100&&near<190&&far>near&&far<255,"input external shadow falloff "+near+"/"+far);
 }
 public static void main(String[] args)throws Exception{
  HandlerThread t=new HandlerThread("shadow-probe");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo service=a.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(service);Thread.sleep(500);GpuBorderProbe.scale=GpuPaddingProbe.density();
   pixel("shadow-layers",60,24,0xffffffff);pixel("shadow-layers",126,24,0xff80007f);pixel("shadow-layers",136,24,0xff0000ff);blur();
   int blue=sample("shadow-tailwind",80,53);GpuApiProbe.check(Color.blue(blue)>Color.red(blue),"Tailwind colored shadow outside box");
   GpuApiProbe.click("shadow-toggle");pixel("shadow-layers",126,24,0xffffffff);GpuApiProbe.click("shadow-toggle");pixel("shadow-layers",126,24,0xff80007f);
   GpuBorderProbe.tap("shadow-input",108,24);GpuPaddingProbe.waitKeyboard(true);GpuSearchProbe.shell("/system/bin/input","text","Z");GpuUiProbe.waitText("shadow-input","MMMMZ");blur();
   GpuRadioProbe.key(4);GpuPaddingProbe.waitKeyboard(false);blur();GpuRadioProbe.screenshot("gpu-shadow-fixture");
   System.out.println("PASS outer shadow knockout, layer order, blur falloff outside editor clips, Tailwind color, reset/reuse and real typing");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
