import android.app.UiAutomation;
import android.os.*;
public class GpuHomeImageProbe {
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("home-image");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);
   android.accessibilityservice.AccessibilityServiceInfo service=a.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(service);
   GpuApiProbe.node("home-background").recycle();
   boolean loaded=false;
   for(int attempt=0;attempt<40&&!loaded;attempt++){
    android.graphics.Bitmap shot=a.takeScreenshot(),pixels=shot.copy(android.graphics.Bitmap.Config.ARGB_8888,false);shot.recycle();
    java.util.HashSet<Integer> colors=new java.util.HashSet<>();
    for(int y=pixels.getHeight()/5;y<pixels.getHeight()*3/4;y+=13){
     int pixel=pixels.getPixel(16,y),r=android.graphics.Color.red(pixel),g=android.graphics.Color.green(pixel),b=android.graphics.Color.blue(pixel);
     if(Math.max(r,Math.max(g,b))-Math.min(r,Math.min(g,b))>30)colors.add(pixel);
    }
    pixels.recycle();loaded=colors.size()>15;if(!loaded)Thread.sleep(500);
   }
   GpuApiProbe.check(loaded,"background image pixels after cold launch");
   GpuRadioProbe.screenshot("gpu-home-cached-background");
   System.out.println("PASS cold-launch Home background pixels");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
