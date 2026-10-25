import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;
// ToDo helpers shared by the parity, keyboard, focus and layout probes.
public class GpuTodoProbe {
 static void check(boolean ok,String s){if(!ok)throw new AssertionError(s);}
 static void click(String id)throws Exception{GpuRadioProbe.show(id);GpuRadioProbe.click(id);}
 static void touch(String id)throws Exception{GpuRadioProbe.show(id);AccessibilityNodeInfo n=GpuRadioProbe.node(id);android.graphics.Rect r=new android.graphics.Rect();n.getBoundsInScreen(r);n.recycle();java.lang.Process p=Runtime.getRuntime().exec(new String[]{"/system/bin/input","tap",String.valueOf(r.centerX()),String.valueOf(r.centerY())});check(p.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&p.exitValue()==0,"physical tap");Thread.sleep(500);}
 static String text(String id)throws Exception{AccessibilityNodeInfo n=GpuRadioProbe.node(id);String t=String.valueOf(n.getText());n.recycle();return t;}
 static void set(String s)throws Exception{AccessibilityNodeInfo n=GpuRadioProbe.node("todo-input");Bundle b=new Bundle();b.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,s);check(n.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,b),"set text");n.recycle();Thread.sleep(300);}
 static String findTask(AccessibilityNodeInfo n,String title){if(n==null)return null;String id=n.getViewIdResourceName();if(id!=null&&id.contains("todo-toggle-")&&title.contentEquals(n.getText()==null?"":n.getText())){n.recycle();return id.substring(id.indexOf("todo-toggle-"));}for(int i=0;i<n.getChildCount();i++){String f=findTask(n.getChild(i),title);if(f!=null){n.recycle();return f;}}n.recycle();return null;}
}
