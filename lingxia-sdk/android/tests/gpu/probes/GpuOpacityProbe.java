import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;
public class GpuOpacityProbe {
 static AccessibilityNodeInfo scroll(AccessibilityNodeInfo n){
  if(n==null)return null;if(n.isScrollable())return n;
  for(int i=0;i<n.getChildCount();i++){AccessibilityNodeInfo found=scroll(n.getChild(i));if(found!=null){n.recycle();return found;}}n.recycle();return null;
 }
 static void pixels(boolean faded)throws Exception {
  for(int i=0;i<4;i++){AccessibilityNodeInfo s=scroll(GpuApiProbe.a.getRootInActiveWindow());if(s!=null){s.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);s.recycle();}Thread.sleep(200);}
  AccessibilityNodeInfo n=GpuApiProbe.node("opacity-class-toggle");android.graphics.Rect box=new android.graphics.Rect();n.getBoundsInScreen(box);n.recycle();
  float scale=box.height()/44f;
  box.top=box.bottom+Math.round(18*scale);
  android.graphics.Bitmap shot=GpuApiProbe.a.takeScreenshot();
  android.graphics.Bitmap bitmap=shot.copy(android.graphics.Bitmap.Config.ARGB_8888,false);shot.recycle();
  try {
   check(bitmap,box,scale,20,20,faded?128:0,faded?128:0,255);
   check(bitmap,box,scale,100,20,255,faded?128:0,faded?128:0);
   check(bitmap,box,scale,70,70,faded?64:0,faded?192:128,128);
   check(bitmap,box,scale,140,20,255,255,255);
  }finally{bitmap.recycle();}
 }
 static void check(android.graphics.Bitmap bitmap,android.graphics.Rect box,float scale,int x,int y,int r,int g,int b){
  int value=bitmap.getPixel(box.left+Math.round(x*scale),box.top+Math.round(y*scale));
  GpuUiProbe.check(Math.abs(android.graphics.Color.red(value)-r)<=3&&Math.abs(android.graphics.Color.green(value)-g)<=3&&Math.abs(android.graphics.Color.blue(value)-b)<=3,"opacity pixel "+x+","+y+" got "+Integer.toHexString(value));
 }
 public static void main(String[] args)throws Exception {
  HandlerThread thread=new HandlerThread("opacity-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  try {
   UiAutomation.class.getMethod("connect").invoke(a);
   android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(800);
   GpuApiProbe.click("home-appearance-light");GpuApiProbe.click("open-lab");pixels(true);GpuRadioProbe.screenshot("gpu-opacity-faded");
   GpuApiProbe.click("opacity-class-toggle");pixels(false);
   GpuApiProbe.click("opacity-class-toggle");pixels(true);
   String auto=GpuSearchProbe.shell("/system/bin/settings","get","system","accelerometer_rotation").trim();
   String rotation=GpuSearchProbe.shell("/system/bin/settings","get","system","user_rotation").trim();
   try {
    GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1000);
   }finally{GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);Thread.sleep(900);}
   pixels(true);
   GpuApiProbe.click("theme");pixels(true);GpuRadioProbe.screenshot("gpu-opacity-dark");
   GpuApiProbe.click("theme");GpuRadioProbe.key(4);
   System.out.println("PASS subtree opacity, overlap, sibling compositing, update, scrolling, rotation retention and light/dark GPU pixels");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
