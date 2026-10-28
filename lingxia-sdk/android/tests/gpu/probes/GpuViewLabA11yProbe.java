import android.app.UiAutomation;
import android.os.Bundle;
import android.os.HandlerThread;
import android.os.Looper;
import android.os.SystemClock;
import android.view.View;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * View lab through an accessibility service: roles, collections and the polite live region; an editor
 * set, selected, copied, cut and pasted; static text selected and copied. Navigate first
 * (`lxdev lxapp nav to view-lab`). Copies are checked by pasting into an editor, not by reading the clipboard.
 */
public final class GpuViewLabA11yProbe {
 static UiAutomation a;
 static AccessibilityNodeInfo find(String id){return GpuRadioProbe.find(a.getRootInActiveWindow(),id);}
 static AccessibilityNodeInfo scrollable(AccessibilityNodeInfo n){return GpuTalkBackWalkProbe.scrollable(n);}
 /** Scroll the page until `id` is in the tree, then onto the screen. */
 static AccessibilityNodeInfo reveal(String id)throws Exception{
  for(int pass=0;pass<2;pass++)for(int i=0;i<40;i++){
   AccessibilityNodeInfo n=find(id);
   if(n!=null){n.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());Thread.sleep(400);return find(id);}
   AccessibilityNodeInfo page=scrollable(a.getRootInActiveWindow());
   if(page==null||!page.performAction(pass==0?AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD:AccessibilityNodeInfo.ACTION_SCROLL_FORWARD))break;
   Thread.sleep(400);
  }
  throw new AssertionError("no node "+id);
 }
 static void group(String label)throws Exception{
  GpuApiProbe.check(reveal("lab-tab-"+label).performAction(AccessibilityNodeInfo.ACTION_CLICK),"open group "+label);Thread.sleep(1500);
 }
 static String text(String id)throws Exception{return String.valueOf(reveal(id).getText());}
 static void selection(String id,int start,int end)throws Exception{
  Bundle range=new Bundle();
  range.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT,start);range.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT,end);
  GpuApiProbe.check(reveal(id).performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION,range),"select "+id);Thread.sleep(800);
 }
 static void setText(String id,String value)throws Exception{
  Bundle text=new Bundle();text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,value);
  GpuApiProbe.check(reveal(id).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,text),"set text "+id);Thread.sleep(1000);
 }
 static void act(String id,int action,String label)throws Exception{GpuApiProbe.check(reveal(id).performAction(action),label);Thread.sleep(1000);}
 static void equal(String actual,String expected,String label){GpuApiProbe.check(expected.equals(actual),label+": expected "+expected+" got "+actual);}

 static void roles()throws Exception{
  group("input");
  AccessibilityNodeInfo heading=reveal("lab-a11y-heading");GpuApiProbe.check(heading.isHeading(),"heading");
  AccessibilityNodeInfo list=reveal("lab-a11y-list");
  equal(String.valueOf(list.getClassName()),"android.widget.ListView","list class");
  GpuApiProbe.check(list.getCollectionInfo()!=null&&list.getCollectionInfo().getRowCount()==3,"three rows");
  GpuApiProbe.check(reveal("lab-a11y-status").getLiveRegion()==View.ACCESSIBILITY_LIVE_REGION_POLITE,"polite live region");
  List<AccessibilityEvent> events=new CopyOnWriteArrayList<>();
  a.setOnAccessibilityEventListener(event->events.add(AccessibilityEvent.obtain(event)));
  String before=text("lab-a11y-status");
  act("lab-a11y-save",AccessibilityNodeInfo.ACTION_CLICK,"save");
  String after=text("lab-a11y-status");
  GpuApiProbe.check(after.startsWith("Saved ")&&!after.equals(before),"status changed: "+after);
  AccessibilityEvent live=null;long end=SystemClock.uptimeMillis()+5000;
  while(live==null&&SystemClock.uptimeMillis()<end){
   for(AccessibilityEvent e:events)if(e.getEventType()==AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED
    &&(e.getContentChangeTypes()&AccessibilityEvent.CONTENT_CHANGE_TYPE_TEXT)!=0&&e.getText().toString().contains(after))live=e;
   Thread.sleep(50);
  }
  a.setOnAccessibilityEventListener(null);
  GpuApiProbe.check(live!=null,"polite live-region change event");
  System.out.println("PASS roles, collection and live region ("+after+")");
 }
 static void editor()throws Exception{
  group("input");
  String id="lab-input-name";
  GpuApiProbe.check(reveal(id).isEditable(),"editable");
  setText(id,"hello world");equal(text(id),"hello world","set text");
  selection(id,6,11);
  AccessibilityNodeInfo e=reveal(id);
  GpuApiProbe.check(e.getTextSelectionStart()==6&&e.getTextSelectionEnd()==11,"selection 6..11");
  List<AccessibilityNodeInfo.AccessibilityAction> offered=e.getActionList();
  for(AccessibilityNodeInfo.AccessibilityAction action:new AccessibilityNodeInfo.AccessibilityAction[]{
   AccessibilityNodeInfo.AccessibilityAction.ACTION_COPY,AccessibilityNodeInfo.AccessibilityAction.ACTION_CUT,AccessibilityNodeInfo.AccessibilityAction.ACTION_PASTE})
   GpuApiProbe.check(offered.contains(action),"offers "+action);
  act(id,AccessibilityNodeInfo.ACTION_CUT,"cut");equal(text(id),"hello ","after cut");
  selection(id,0,0);
  act(id,AccessibilityNodeInfo.ACTION_PASTE,"paste");equal(text(id),"worldhello ","after paste");
  selection(id,0,5);act(id,AccessibilityNodeInfo.ACTION_COPY,"copy");
  selection(id,11,11);act(id,AccessibilityNodeInfo.ACTION_PASTE,"paste copy");equal(text(id),"worldhello world","after copy + paste");
  System.out.println("PASS editor set text, selection, cut, paste, copy");
 }
 static void staticCopy()throws Exception{
  group("text");
  selection("lab-select-all",0,4);
  AccessibilityNodeInfo t=reveal("lab-select-all");
  GpuApiProbe.check(t.getTextSelectionStart()==0&&t.getTextSelectionEnd()==4,"static selection 0..4");
  act("lab-select-all",AccessibilityNodeInfo.ACTION_COPY,"copy static");
  GpuApiProbe.check(reveal("lab-select-all").getTextSelectionStart()==-1,"the selection goes after copy");
  group("input");
  setText("lab-input-name","");
  act("lab-input-name",AccessibilityNodeInfo.ACTION_PASTE,"paste static copy");equal(text("lab-input-name"),"8F3A","pasted static copy");
  System.out.println("PASS static text selection and copy");
 }
 public static void main(String[] args)throws Exception{
  String cases=System.getenv("GPU_CASE");if(cases==null)cases="roles,editor,copy";
  HandlerThread thread=new HandlerThread("view-lab-a11y");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  try{
   UiAutomation.class.getMethod("connect",int.class).invoke(a,UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES);
   android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();
   info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(800);
   for(String c:cases.split(",")){
    if(c.equals("roles"))roles();else if(c.equals("editor"))editor();else if(c.equals("copy"))staticCopy();
    else throw new AssertionError("unknown case "+c);
   }
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
