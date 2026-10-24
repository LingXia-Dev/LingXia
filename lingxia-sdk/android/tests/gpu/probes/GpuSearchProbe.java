import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.os.Bundle;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuSearchProbe {
 static void require(boolean value,String message){GpuRadioProbe.require(value,message);}
 static String shell(String...command)throws Exception {
  Process p=Runtime.getRuntime().exec(command);java.io.ByteArrayOutputStream out=new java.io.ByteArrayOutputStream();byte[] buffer=new byte[4096];int read;while((read=p.getInputStream().read(buffer))!=-1)out.write(buffer,0,read);byte[] bytes=out.toByteArray();
  require(p.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&p.exitValue()==0,"shell command");return new String(bytes,"UTF-8");
 }
 static void value(String value)throws Exception {
  GpuRadioProbe.click("moment-search");
  AccessibilityNodeInfo node=GpuRadioProbe.node("moment-search");Bundle args=new Bundle();args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,value);
  require(node.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,args),"set query");node.recycle();Thread.sleep(800);
  require(shell("/system/bin/dumpsys","input_method").contains("mInputShown=true"),"editing query shows IME");
 }
 static String text(String id)throws Exception {AccessibilityNodeInfo n=GpuRadioProbe.node(id);String s=String.valueOf(n.getText());n.recycle();return s;}
 static void status(String expected)throws Exception {
  for(int i=0;i<20;i++){if(text("discover-status").contains(expected))return;Thread.sleep(200);}throw new AssertionError("status missing "+expected+": "+text("discover-status"));
 }
 public static void main(String[]args)throws Exception {
  HandlerThread thread=new HandlerThread("search-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=automation;
  try {
   UiAutomation.class.getMethod("connect").invoke(automation);
   android.accessibilityservice.AccessibilityServiceInfo service=automation.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(service);
   Thread.sleep(800);GpuRadioProbe.click("tab-discover");
   AccessibilityNodeInfo field=GpuRadioProbe.node("moment-search");
   require(field.isEditable()&&!field.isMultiLine()&&"android.widget.EditText".contentEquals(field.getClassName()),"search editable semantics");
   require(field.getText()==null||field.getText().length()==0,"placeholder is not value");field.recycle();
   value("042");require(!text("discover-status").contains("found"),"typing must not submit");
   String ime=shell("/system/bin/dumpsys","input_method");require(ime.contains("mInputShown=true"),"search IME shown");
   require(ime.matches("(?s).*imeOptions=0x[0-9a-fA-F]*3\\b.*"),"Search IME action: "+ime);
   GpuRadioProbe.screenshot("gpu-search-keyboard");
   android.graphics.Bitmap screen=automation.takeScreenshot();int width=screen.getWidth(),height=screen.getHeight();screen.recycle();
   shell("/system/bin/input","tap",String.valueOf(Math.round(width*.9194)),String.valueOf(Math.round(height*.8986)));
   status("1 moment found");require(shell("/system/bin/dumpsys","input_method").contains("mInputShown=true"),"search submission retains IME");
   GpuRadioProbe.key(KeyEvent.KEYCODE_BACK);require(text("discovery-title-42").equals("Moment 042"),"filtered row identity");GpuRadioProbe.screenshot("gpu-search-results");
   value("missing");GpuRadioProbe.key(KeyEvent.KEYCODE_ENTER);status("0 moments found");GpuRadioProbe.key(KeyEvent.KEYCODE_BACK);
   require(text("search-empty").startsWith("No moments"),"empty result message");
   AccessibilityNodeInfo first=GpuRadioProbe.node("discover-first");require(!first.isEnabled()&&!first.performAction(AccessibilityNodeInfo.ACTION_CLICK),"empty navigation disabled");first.recycle();
   GpuRadioProbe.click("clear-search");require(text("moment-search").isEmpty(),"clear query");require(!text("discover-status").contains("found"),"clear results");require(text("discovery-title-1").equals("Moment 001"),"clear restores list");
   value("999");GpuRadioProbe.key(KeyEvent.KEYCODE_ENTER);status("1 moment found");GpuRadioProbe.key(KeyEvent.KEYCODE_BACK);
   GpuRadioProbe.click("tab-hello");GpuRadioProbe.click("tab-discover");require(text("moment-search").equals("999"),"query retained across tabs");status("1 moment found");require(text("discovery-title-999").equals("Moment 999"),"result retained across tabs");
   GpuRadioProbe.click("theme");require(text("moment-search").equals("999"),"query across theme");GpuRadioProbe.screenshot("gpu-search-dark");GpuRadioProbe.click("theme");
   String auto=shell("/system/bin/settings","get","system","accelerometer_rotation").trim();String rotation=shell("/system/bin/settings","get","system","user_rotation").trim();
   try {
    shell("/system/bin/settings","put","system","accelerometer_rotation","0");shell("/system/bin/settings","put","system","user_rotation","1");Thread.sleep(1400);
    require(text("moment-search").equals("999"),"query after landscape");status("1 moment found");require(text("discovery-title-999").equals("Moment 999"),"landscape result visible");GpuRadioProbe.screenshot("gpu-search-landscape");
    shell("/system/bin/settings","put","system","user_rotation","0");Thread.sleep(1400);require(text("moment-search").equals("999"),"query after portrait");
   }finally{shell("/system/bin/settings","put","system","user_rotation",rotation);shell("/system/bin/settings","put","system","accelerometer_rotation",auto);}
   GpuRadioProbe.click("clear-search");System.out.println("PASS Search IME soft action, continued editing, filtered identity, empty state, disabled navigation, clear and tab retention");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
