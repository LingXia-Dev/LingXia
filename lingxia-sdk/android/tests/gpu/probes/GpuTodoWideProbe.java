import android.app.UiAutomation;
import android.os.*;
import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
public class GpuTodoWideProbe {
 static Rect bounds(String id)throws Exception{AccessibilityNodeInfo n=GpuApiProbe.node(id);Rect r=new Rect();n.getBoundsInScreen(r);n.recycle();return r;}
 public static void main(String[] args)throws Exception{
  String title="GPU wide task: review the layout and ensure this longer task wraps inside its row without covering the delete action";
  String density=GpuSearchProbe.shell("/system/bin/wm","density"),restore="reset";java.util.regex.Matcher m=java.util.regex.Pattern.compile("Override density: (\\d+)").matcher(density);if(m.find())restore=m.group(1);
  String auto=GpuGridProbe.settings("get","system","accelerometer_rotation"),rotation=GpuGridProbe.settings("get","system","user_rotation");
  HandlerThread t=new HandlerThread("todo-wide");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuApiProbe.a=a;GpuRadioProbe.automation=a;boolean connected=false;
  try{
   GpuSearchProbe.shell("/system/bin/wm","density","220");GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1800);
   UiAutomation.class.getMethod("connect").invoke(a);connected=true;android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(800);GpuChromeProbe.tab("ToDo");GpuApiProbe.node("todo-input").recycle();
   if(GpuTodoParityProbe.exists("todo-filter-all"))GpuApiProbe.click("todo-filter-all");
   Rect input=bounds("todo-input");GpuSearchProbe.shell("/system/bin/input","tap",""+input.centerX(),""+input.centerY());GpuTodoProbe.set(title);GpuRadioProbe.key(66);GpuRadioProbe.key(4);
   String id=GpuTodoProbe.findTask(a.getRootInActiveWindow(),title);GpuApiProbe.check(id!=null,"long task exists");float scale=GpuPaddingProbe.density();Rect count=bounds("todo-count"),filter=bounds("todo-filter-all"),task=bounds(id),delete=bounds(id.replace("todo-toggle-","todo-delete-"));
   android.graphics.Bitmap shot=a.takeScreenshot();try{GpuApiProbe.check(shot.getWidth()/scale>768,"wide viewport exercised");}finally{shot.recycle();}GpuApiProbe.check(count.right<filter.left&&Math.abs(count.centerY()-filter.centerY())<8*scale,"desktop count and filters share row");GpuApiProbe.check(task.right<=delete.left&&task.height()>50*scale,"long task wraps without overlapping delete");GpuApiProbe.check(input.width()/scale<550,"panel width cap");GpuRadioProbe.screenshot("gpu-todo-wide-light");
   GpuApiProbe.click(id);GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-dark");GpuChromeProbe.tab("ToDo");GpuApiProbe.node(id).recycle();
   Rect filters=bounds("todo-filter-all"),completed=bounds("todo-filter-completed"),clear=bounds("todo-clear-completed"),panel=bounds("todo-input");
   filters.union(completed);
   GpuApiProbe.check(completed.width()>=90*scale,"Completed label retains horizontal padding");
   GpuApiProbe.check(filters.right<=panel.right-18*scale,"filter labels remain within panel");
   GpuApiProbe.check(clear.top>=filters.bottom||clear.left>=filters.right,"clear action wraps instead of squeezing filters");
   GpuRadioProbe.screenshot("gpu-todo-wide-dark");GpuApiProbe.click(id.replace("todo-toggle-","todo-delete-"));
   System.out.println("PASS wide viewport footer alignment, capped panel, long task wrapping and completed dark state");
  }catch(Throwable failure){
   System.out.println("FAIL wide probe: "+failure);try{GpuRadioProbe.screenshot("gpu-todo-wide-failure");}catch(Throwable ignored){}throw failure;
  }finally{
   try{if(connected){try{GpuChromeProbe.tab("ToDo");String id=GpuTodoProbe.findTask(a.getRootInActiveWindow(),title);if(id!=null)GpuApiProbe.click(id.replace("todo-toggle-","todo-delete-"));GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");}finally{UiAutomation.class.getMethod("disconnect").invoke(a);}}}finally{
   GpuSearchProbe.shell("/system/bin/wm","density",restore);GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);t.quitSafely();}
  }
 }
}
