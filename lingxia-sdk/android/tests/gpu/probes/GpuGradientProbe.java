import android.app.UiAutomation;
import android.os.*;
import android.graphics.*;

public class GpuGradientProbe {
 static void pixel(String id,int x,int y,int expected)throws Exception{GpuBorderProbe.pixel(id,x,y,expected);}
 public static void main(String[] args)throws Exception{
  HandlerThread t=new HandlerThread("gradient-probe");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo service=a.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(service);Thread.sleep(500);GpuBorderProbe.scale=GpuPaddingProbe.density();
   pixel("gradient-linear",120,32,0xff800080);pixel("gradient-linear",0,0,0xffffffff);pixel("gradient-linear",120,1,0xff000000);
   pixel("gradient-corner",238,1,0xff800080);pixel("gradient-corner",1,78,0xff800080);
   pixel("gradient-alpha",120,32,0xff8080ff);
   pixel("gradient-hard",120,32,0xffff0000);pixel("gradient-hard",200,32,0xff0000ff);
   Rect r=GpuBorderProbe.box("gradient-oklab");Bitmap shot=a.takeScreenshot(),b=shot.copy(Bitmap.Config.ARGB_8888,false);shot.recycle();int middle=b.getPixel(r.centerX(),r.centerY());b.recycle();
   GpuApiProbe.check(Color.green(middle)>30,"Oklab midpoint differs from sRGB");
   GpuRadioProbe.screenshot("gpu-gradient-fixture");
   GpuApiProbe.click("gradient-toggle");pixel("gradient-linear",120,32,0xffffffff);
   GpuApiProbe.click("gradient-toggle");pixel("gradient-linear",120,32,0xff800080);
   System.out.println("PASS gradient direction, non-square magic corners, Oklab, premultiplied alpha, hard stops, rounded border and reset/reuse");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
