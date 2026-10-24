import android.app.UiAutomation;
import android.os.*;
import android.graphics.*;

public class GpuFocusStyleProbe {
 static Bitmap pixels() {
  Bitmap shot=GpuApiProbe.a.takeScreenshot(),copy=shot.copy(Bitmap.Config.ARGB_8888,false);shot.recycle();return copy;
 }
 static int ring()throws Exception {
  Rect r=GpuThemeProbe.bounds("home-name");float scale=GpuPaddingProbe.density();Bitmap b=pixels();int count=0;
  try {for(int x=r.left+Math.round(18*scale);x<r.right-Math.round(18*scale);x++) {
   int c=b.getPixel(x,r.top-Math.max(1,Math.round(scale)));
   if(Color.blue(c)>Color.red(c)+20&&Color.blue(c)>Color.green(c)+5)count++;
  }}finally{b.recycle();}return count;
 }
 static int todoColor()throws Exception {
  Rect r=GpuThemeProbe.bounds("todo-input");Bitmap b=pixels();
  try{return Color.red(b.getPixel(r.right-Math.round(12*GpuPaddingProbe.density()),r.centerY()));}finally{b.recycle();}
 }
 public static void main(String[] args)throws Exception {
  HandlerThread t=new HandlerThread("focus-style");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try {
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);
   GpuApiProbe.node("home-name").recycle();GpuApiProbe.click("home-appearance-light");GpuRadioProbe.show("home-name");Thread.sleep(500);
   int base=ring();GpuTodoProbe.touch("home-name");Thread.sleep(600);int focused=ring();GpuRadioProbe.screenshot("gpu-focus-home");
   GpuRadioProbe.key(61);Thread.sleep(500);int blurred=ring();
   System.out.println("Home ring pixels base="+base+" focused="+focused+" blurred="+blurred);
   GpuApiProbe.check(focused>base+100&&Math.abs(blurred-base)<20,"Home ring appears on focus and disappears on blur");
   GpuRadioProbe.key(4);
   for(boolean dark:new boolean[]{false,true}) {
    GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");GpuChromeProbe.tab("ToDo");GpuRadioProbe.show("todo-input");Thread.sleep(500);
    if(GpuTodoParityProbe.exists("todo-filter-all")){Rect all=GpuThemeProbe.bounds("todo-filter-all"),completed=GpuThemeProbe.bounds("todo-filter-completed"),input=GpuThemeProbe.bounds("todo-input");GpuApiProbe.check(Math.abs(all.centerY()-completed.centerY())<=2&&completed.right<input.right,"ToDo filters fit one row within panel");}
    int before=todoColor();GpuTodoProbe.touch("todo-input");Thread.sleep(500);int after=todoColor();GpuRadioProbe.screenshot(dark?"gpu-focus-todo-dark":"gpu-focus-todo-light");
    GpuRadioProbe.key(61);Thread.sleep(500);int restored=todoColor();
    System.out.println("ToDo dark="+dark+" base="+before+" focused="+after+" blurred="+restored);
    GpuApiProbe.check((dark?after>before+3:after<before-1)&&Math.abs(restored-before)<=1,"ToDo focus background and blur restoration");
    GpuRadioProbe.key(4);GpuChromeProbe.tab("Home");
   }
   GpuApiProbe.click("home-appearance-light");System.out.println("PASS physical editor focus ring and light/dark backgrounds, blur restoration; no task edits");
  }finally {UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
