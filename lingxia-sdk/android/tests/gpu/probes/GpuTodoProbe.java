import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;
public class GpuTodoProbe {
 static void check(boolean ok,String s){if(!ok)throw new AssertionError(s);}
 static void click(String id)throws Exception{GpuRadioProbe.show(id);GpuRadioProbe.click(id);}
 static void touch(String id)throws Exception{GpuRadioProbe.show(id);AccessibilityNodeInfo n=GpuRadioProbe.node(id);android.graphics.Rect r=new android.graphics.Rect();n.getBoundsInScreen(r);n.recycle();java.lang.Process p=Runtime.getRuntime().exec(new String[]{"/system/bin/input","tap",String.valueOf(r.centerX()),String.valueOf(r.centerY())});check(p.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&p.exitValue()==0,"physical tap");Thread.sleep(500);}
 static String text(String id)throws Exception{AccessibilityNodeInfo n=GpuRadioProbe.node(id);String t=String.valueOf(n.getText());n.recycle();return t;}
 static void set(String s)throws Exception{AccessibilityNodeInfo n=GpuRadioProbe.node("todo-input");Bundle b=new Bundle();b.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,s);check(n.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,b),"set text");n.recycle();Thread.sleep(300);}
 static String findTask(AccessibilityNodeInfo n,String title){if(n==null)return null;String id=n.getViewIdResourceName();if(id!=null&&id.contains("todo-toggle-")&&title.contentEquals(n.getText()==null?"":n.getText())){n.recycle();return id.substring(id.indexOf("todo-toggle-"));}for(int i=0;i<n.getChildCount();i++){String f=findTask(n.getChild(i),title);if(f!=null){n.recycle();return f;}}n.recycle();return null;}
 public static void main(String[] args)throws Exception{
 HandlerThread thread=new HandlerThread("todo-probe");thread.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
 UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);GpuRadioProbe.automation=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(700);
 click("open-todo");click("todo-filter-all");
 if(args.length>0){
  if(args[0].equals("seed")){set("GPU cold-start task");click("todo-add");click("todo-filter-active");System.out.println("PASS persisted cold-start fixture");return;}
  String restored=findTask(a.getRootInActiveWindow(),"GPU cold-start task");check(restored!=null,"cold-process storage restoration");click(restored.replace("todo-toggle-","todo-delete-"));System.out.println("PASS cold-process storage restoration and cleanup");return;
 }

 for(String title:new String[]{"GPU todo alpha","GPU todo beta"}){String old=findTask(a.getRootInActiveWindow(),title);if(old!=null)click(old.replace("todo-toggle-","todo-delete-"));}
 set("GPU todo alpha");touch("todo-add");check(text("todo-input").isEmpty(),"draft cleared");
 set("GPU todo beta");click("todo-add");String alpha=findTask(a.getRootInActiveWindow(),"GPU todo alpha");String beta=findTask(a.getRootInActiveWindow(),"GPU todo beta");check(alpha!=null&&beta!=null,"two tasks");
 click("todo-toggle-all");check(text("todo-count").equals("0 active / 2 completed"),"toggle all complete");click("todo-toggle-all");check(text("todo-count").equals("2 active / 0 completed"),"toggle all active");click(alpha);click("todo-filter-active");check(findTask(a.getRootInActiveWindow(),"GPU todo alpha")==null,"active excludes completed");check(findTask(a.getRootInActiveWindow(),"GPU todo beta")!=null,"active contains beta");
 click("todo-filter-completed");check(findTask(a.getRootInActiveWindow(),"GPU todo alpha")!=null,"completed contains alpha");GpuRadioProbe.screenshot("gpu-todo-completed");
 click("todo-back");click("theme");click("open-todo");GpuRadioProbe.screenshot("gpu-todo-dark");check(findTask(a.getRootInActiveWindow(),"GPU todo alpha")!=null,"storage restores tasks and filter after page recreation");
 String auto=GpuGridProbe.settings("get","system","accelerometer_rotation"),rotation=GpuGridProbe.settings("get","system","user_rotation");
 try{GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1000);check(findTask(a.getRootInActiveWindow(),"GPU todo alpha")!=null,"landscape task");GpuRadioProbe.screenshot("gpu-todo-landscape");}finally{GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);Thread.sleep(900);}
 click("todo-back");click("theme");click("open-todo");click("todo-clear-completed");GpuRadioProbe.node("todo-empty").recycle();click("todo-filter-all");check(findTask(a.getRootInActiveWindow(),"GPU todo beta")!=null,"clear preserves active");GpuRadioProbe.screenshot("gpu-todo-active");
 click(beta.replace("todo-toggle-","todo-delete-"));GpuRadioProbe.node("todo-empty").recycle();check(text("todo-count").equals("0 active / 0 completed"),"empty count");
 click("todo-filter-completed");click("todo-back");click("open-todo");
 check(text("todo-empty").equals("A fresh start."),"empty completed filter survives recreation");
 click("todo-filter-active");click("todo-back");click("open-todo");
 check(text("todo-empty").equals("You are all caught up."),"empty active filter survives recreation");
 click("todo-filter-all");GpuRadioProbe.screenshot("gpu-todo-empty");
 System.out.println("PASS GPU navigation, add, controlled checkbox, filters, page recreation/storage, clear, delete and empty state");
 }finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
