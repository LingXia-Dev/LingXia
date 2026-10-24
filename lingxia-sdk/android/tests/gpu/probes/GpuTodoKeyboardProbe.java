import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;
public class GpuTodoKeyboardProbe {
 public static void main(String[] args)throws Exception{
 HandlerThread t=new HandlerThread("todo-keyboard");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(800);
 GpuChromeProbe.tab("ToDo");if(GpuRadioProbe.find(a.getRootInActiveWindow(),"todo-filter-all")!=null)GpuTodoProbe.click("todo-filter-all");for(int cleanup=0;cleanup<10;cleanup++){String old=GpuTodoProbe.findTask(a.getRootInActiveWindow(),"GPUKeyboard");if(old==null)break;GpuRadioProbe.click(old.replace("todo-toggle-","todo-delete-"));}GpuTodoProbe.touch("todo-input");
 java.lang.Process p=Runtime.getRuntime().exec(new String[]{"/system/bin/input","text","GPUKeyboard"});GpuTodoProbe.check(p.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&p.exitValue()==0,"type");Thread.sleep(500);GpuTodoProbe.check(GpuTodoProbe.text("todo-input").equals("GPUKeyboard"),"keyboard edit");GpuRadioProbe.screenshot("gpu-todo-keyboard");GpuRadioProbe.key(66);Thread.sleep(700);
 String id=GpuTodoProbe.findTask(a.getRootInActiveWindow(),"GPUKeyboard");GpuTodoProbe.check(id!=null,"Enter submits task");GpuTodoProbe.check(GpuTodoProbe.text("todo-input").isEmpty(),"submit clears input");GpuTodoProbe.click(id.replace("todo-toggle-","todo-delete-"));GpuTodoProbe.check(GpuTodoProbe.findTask(a.getRootInActiveWindow(),"GPUKeyboard")==null,"test task removed");GpuRadioProbe.key(4);GpuRadioProbe.screenshot("gpu-todo-empty-parity");GpuChromeProbe.tab("Home");System.out.println("PASS physical text input, keyboard layout, Enter submit and cleanup");
 }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}