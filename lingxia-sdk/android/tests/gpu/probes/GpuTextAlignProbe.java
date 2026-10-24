import android.app.UiAutomation;
import android.os.*;
import android.graphics.*;
import android.view.accessibility.AccessibilityNodeInfo;

public class GpuTextAlignProbe {
 static float scale;
 static void check(boolean value,String message){GpuApiProbe.check(value,message);}
 static Rect box(String id)throws Exception{AccessibilityNodeInfo n=GpuApiProbe.node(id);Rect r=new Rect();n.getBoundsInScreen(r);n.recycle();return r;}
 static int[] ink(String id,int top,int bottom)throws Exception{
  Rect r=box(id);Bitmap shot=GpuApiProbe.a.takeScreenshot(),b=shot.copy(Bitmap.Config.ARGB_8888,false);shot.recycle();
  int left=r.right,right=r.left;
  for(int y=r.top+Math.round(top*scale);y<r.top+Math.round(bottom*scale);y++)for(int x=r.left+Math.round(7*scale);x<r.right-Math.round(7*scale);x++){
   int p=b.getPixel(x,y);if(Color.red(p)<80&&Color.green(p)<80&&Color.blue(p)<80){left=Math.min(left,x);right=Math.max(right,x);}
  }
  b.recycle();check(left<=right,"visible ink "+id+" row "+top);return new int[]{left,right};
 }
 static void equal(int[] a,int[] b,String message){check(Math.abs(a[0]-b[0])<=2&&Math.abs(a[1]-b[1])<=2,message);}
 static void tapInput(float x)throws Exception{Rect r=box("alignment-input");GpuSearchProbe.shell("/system/bin/input","tap",String.valueOf(r.left+Math.round(x*scale)),String.valueOf(r.centerY()));GpuPaddingProbe.waitKeyboard(true);}
 public static void main(String[] args)throws Exception{
  HandlerThread t=new HandlerThread("text-align-probe");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo service=a.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(service);Thread.sleep(500);scale=GpuPaddingProbe.density();
   int[][] ltr=new int[3][],rtl=new int[3][];int[] override=ink("alignment-override",11,36);
   String[] modes={"right","left","center","start","end","justify"};
   for(int i=0;i<modes.length;i++){
    check(GpuApiProbe.text("alignment-toggle").equals(modes[i]),"alignment mode "+modes[i]);
    int[] first=ink("alignment-lines",11,36),last=ink("alignment-lines",39,64),hebrew=ink("alignment-rtl",11,36);
    if(i<3){ltr[i]=first;rtl[i]=hebrew;}
    if(i==0||i==4)check(Math.abs(first[1]-last[1])<=2,"right aligned line ends");
    if(i==1||i==3||i==5)check(Math.abs(first[0]-last[0])<=2,"left aligned line starts");
    if(i==2)check(Math.abs(first[0]+first[1]-last[0]-last[1])<=4,"centered lines");
    if(i==3||i==5){equal(first,ltr[1],"LTR start");equal(hebrew,rtl[0],"RTL start");}
    if(i==4){equal(first,ltr[0],"LTR end");equal(hebrew,rtl[1],"RTL end");}
    equal(ink("alignment-override",11,36),override,"explicit left override");
    if(i==0)GpuRadioProbe.screenshot("gpu-text-align-right");
    if(i==2)GpuRadioProbe.screenshot("gpu-text-align-center");
    GpuApiProbe.click("alignment-toggle");
   }
   equal(ink("alignment-lines",11,36),ltr[0],"alignment restored after cache reuse");
   tapInput(229);GpuSearchProbe.shell("/system/bin/input","text","Z");GpuUiProbe.waitText("alignment-input","MMMMZ");GpuRadioProbe.key(4);GpuPaddingProbe.waitKeyboard(false);
   GpuApiProbe.click("alignment-toggle");GpuApiProbe.click("alignment-toggle");
   int[] inputInk=ink("alignment-input",14,42);Rect r=box("alignment-input");tapInput((inputInk[0]-r.left)/scale-1);
   GpuSearchProbe.shell("/system/bin/input","text","A");GpuUiProbe.waitText("alignment-input","AMMMMZ");GpuRadioProbe.key(4);GpuPaddingProbe.waitKeyboard(false);
   GpuRadioProbe.screenshot("gpu-text-align-input");
   System.out.println("PASS inherited multiline alignment, physical left/right vs RTL start/end, explicit override, cache reuse and right/center editor taps with real typing");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
