import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;
public class GpuTodoParityProbe {
 static void check(boolean value,String reason){GpuTodoProbe.check(value,reason);}
 static void click(String id)throws Exception{GpuTodoProbe.click(id);}
 static boolean exists(String id){AccessibilityNodeInfo n=GpuRadioProbe.find(GpuRadioProbe.automation.getRootInActiveWindow(),id);if(n==null)return false;n.recycle();return true;}
 static String task(String title){return GpuTodoProbe.findTask(GpuRadioProbe.automation.getRootInActiveWindow(),title);}
 static void count(String expected)throws Exception{check(GpuTodoProbe.text("todo-count").equals(expected),"count "+expected);}
 static void filtersFit()throws Exception {
  float scale=GpuPaddingProbe.density();
  android.graphics.Rect viewport=GpuThemeProbe.bounds("todo-list");
  android.graphics.Rect previous=null;
  for(String name:new String[]{"all","active","completed"}) {
   android.graphics.Rect button=GpuThemeProbe.bounds("todo-filter-"+name);
   check(button.left>=viewport.left+28*scale&&button.right<=viewport.right-28*scale,"filter stays inside inset panel footer: "+name);
   check(button.width()/scale>=63.5f,"filter is not horizontally clipped: "+name);
   check(button.height()/scale>=43.5f,"filter retains 44dp touch height: "+name);
   if(previous!=null)check(Math.abs(button.top-previous.top)<2&&button.left>=previous.right+3*scale,"filters share a row with separate hit targets");
   previous=button;
  }
 }
 static void checkbox(String id,boolean checked)throws Exception{
  AccessibilityNodeInfo n=GpuRadioProbe.node(id);
  check(n.isCheckable()&&n.isChecked()==checked,"checkbox semantics "+checked);
  check(n.getChildCount()==0,"composed checkbox exposes one accessible control");
  check("android.widget.CheckBox".contentEquals(n.getClassName()),"checkbox accessibility role");n.recycle();
 }
 static void focusCheckbox(String id)throws Exception{boolean foundFocus=false;for(int tab=0;tab<20&&!foundFocus;tab++){GpuRadioProbe.key(61);AccessibilityNodeInfo focused=GpuRadioProbe.automation.findFocus(AccessibilityNodeInfo.FOCUS_INPUT);foundFocus=focused!=null&&("com.lingxia.example.gpu.dev:id/"+id).equals(focused.getViewIdResourceName());if(focused!=null)focused.recycle();}check(foundFocus,"Tab focuses checkbox");}
 static void labelTap(String id)throws Exception{
  GpuRadioProbe.show(id);AccessibilityNodeInfo n=GpuRadioProbe.node(id);android.graphics.Rect r=new android.graphics.Rect();n.getBoundsInScreen(r);n.recycle();
  java.lang.Process p=Runtime.getRuntime().exec(new String[]{"/system/bin/input","tap",String.valueOf(r.right-12),String.valueOf(r.centerY())});
  check(p.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&p.exitValue()==0,"label-area tap");Thread.sleep(500);
 }
 static void add(String title)throws Exception{GpuTodoProbe.touch("todo-input");GpuTodoProbe.set(title);GpuRadioProbe.key(66);check(task(title)!=null,"created "+title);GpuRadioProbe.key(4);}
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("todo-parity");thread.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),c);GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  String auto=null,rotation=null;boolean owned=false;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(800);
   GpuChromeProbe.awaitHomeReady();
   GpuChromeProbe.tab("ToDo");
   for(int ready=0;ready<40&&!exists("todo-empty")&&!exists("todo-count");ready++)Thread.sleep(250);
   check(exists("todo-input"),"ToDo content must be ready after switching tabs");
   check(exists("todo-empty")&&!exists("todo-count"),"empty baseline required before bulk-operation test; preserves existing user tasks");owned=true;
   check(!exists("todo-filter-all")&&!exists("todo-clear-completed"),"empty footer absent");
   check(!exists("todo-add")&&!exists("todo-back"),"reference Enter input and native navigation");
   add("GPU parity alpha");add("GPU parity beta");count("2 items left");filtersFit();
   click("todo-toggle-all");count("0 items left");check(exists("todo-clear-completed"),"clear only with completed tasks");
   click("todo-toggle-all");count("2 items left");check(!exists("todo-clear-completed"),"clear hidden for active tasks");
   String alpha=task("GPU parity alpha"),beta=task("GPU parity beta");checkbox(alpha,false);labelTap(alpha);checkbox(alpha,true);count("1 item left");
   focusCheckbox(alpha);GpuRadioProbe.key(62);checkbox(alpha,false);focusCheckbox(alpha);GpuRadioProbe.key(66);checkbox(alpha,true);count("1 item left");
   click("todo-filter-active");check(task("GPU parity alpha")==null&&task("GPU parity beta")!=null,"active filter");
   click("todo-filter-completed");check(task("GPU parity alpha")!=null&&task("GPU parity beta")==null,"completed filter");GpuRadioProbe.screenshot("gpu-todo-parity-completed");
   GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-dark");GpuChromeProbe.tab("ToDo");check(task("GPU parity alpha")!=null,"tasks and filter retained across tabs");filtersFit();GpuRadioProbe.screenshot("gpu-todo-parity-dark");
   auto=GpuGridProbe.settings("get","system","accelerometer_rotation");rotation=GpuGridProbe.settings("get","system","user_rotation");GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1100);check(task("GPU parity alpha")!=null,"landscape content");GpuRadioProbe.screenshot("gpu-todo-parity-landscape");
   GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);Thread.sleep(900);auto=null;
   click("todo-clear-completed");count("1 item left");check(task("GPU parity alpha")==null&&!exists("todo-empty"),"filtered-empty is not no-tasks state");check(!exists("todo-clear-completed"),"clear disappears after clearing");
   click("todo-filter-all");check(task("GPU parity beta")!=null,"clear preserves active");click(beta.replace("todo-toggle-","todo-delete-"));check(exists("todo-empty")&&!exists("todo-count")&&!exists("todo-filter-all"),"last deletion restores exact empty state");check(GpuTodoProbe.text("todo-empty").equals("No tasks yet"),"reference empty copy");
   System.out.println("PASS ToDo bulk toggle, individual toggle, filters, conditional footer, clear, empty state, dark theme and device rotation");
  }catch(Throwable error){error.printStackTrace();throw error;}finally{
   if(auto!=null){GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);Thread.sleep(700);}
   if(owned){try{GpuChromeProbe.tab("ToDo");if(exists("todo-filter-all"))click("todo-filter-all");for(String title:new String[]{"GPU parity alpha","GPU parity beta"}){String id=task(title);if(id!=null)click(id.replace("todo-toggle-","todo-delete-"));}GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");}catch(Exception ignored){}}
   UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();
  }
 }
}
