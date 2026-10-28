import android.app.UiAutomation;
import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.FileOutputStream;
import java.io.FileWriter;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * Walks the page on screen with TalkBack running: each node takes accessibility focus and TalkBack's
 * speech caption is saved. Navigate first (`lxdev lxapp nav`); `GPU_TAG` names the output and
 * `GPU_GROUP` opens a view-lab group. Output: /data/local/tmp/walk-<tag>.txt and walk-<tag>-say-N.png.
 */
public final class GpuTalkBackWalkProbe {
 static UiAutomation a;
 static void visible(AccessibilityNodeInfo n,String pkg,List<AccessibilityNodeInfo> out){
  if(n==null)return;
  if(n.isVisibleToUser()&&pkg.contentEquals(n.getPackageName())&&(n.getText()!=null&&n.getText().length()>0||n.getContentDescription()!=null
   ||n.getViewIdResourceName()!=null||n.isClickable()||n.isScrollable()))out.add(n);
  for(int i=0;i<n.getChildCount();i++)visible(n.getChild(i),pkg,out);
 }
 static AccessibilityNodeInfo scrollable(AccessibilityNodeInfo n){
  if(n==null)return null;AccessibilityNodeInfo best=n.isScrollable()?n:null;
  for(int i=0;i<n.getChildCount();i++){AccessibilityNodeInfo c=scrollable(n.getChild(i));if(c!=null&&(best==null||area(c)>area(best)))best=c;}
  return best;
 }
 static long area(AccessibilityNodeInfo n){Rect r=new Rect();n.getBoundsInScreen(r);return (long)r.width()*r.height();}
 static String describe(AccessibilityNodeInfo n){
  Rect r=new Rect();n.getBoundsInScreen(r);StringBuilder s=new StringBuilder();
  s.append(n.getClassName()).append(" \"").append(n.getText()).append('"');
  if(n.getContentDescription()!=null)s.append(" desc=\"").append(n.getContentDescription()).append('"');
  if(n.getViewIdResourceName()!=null)s.append(" id=").append(n.getViewIdResourceName().replaceAll(".*:id/",""));
  if(n.isHeading())s.append(" heading");
  if(n.isClickable())s.append(" clickable");
  if(n.isEditable())s.append(" editable");
  return s.append(' ').append(r.toShortString()).toString();
 }
 static void caption(String name)throws Exception{
  Bitmap shot=a.takeScreenshot();
  Bitmap strip=Bitmap.createBitmap(shot,0,Math.round(shot.getHeight()*0.9f),shot.getWidth(),shot.getHeight()-Math.round(shot.getHeight()*0.9f));
  try(FileOutputStream out=new FileOutputStream("/data/local/tmp/"+name)){strip.compress(Bitmap.CompressFormat.PNG,100,out);}
  strip.recycle();shot.recycle();
 }
 public static void main(String[] args)throws Exception{
  String pkg=System.getenv("GPU_PACKAGE"),tag=System.getenv("GPU_TAG"),group=System.getenv("GPU_GROUP");
  if(tag==null)tag="page";
  HandlerThread thread=new HandlerThread("talkback-walk");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  int missing=0,spoken=0;
  try{
   // Keep TalkBack running: its captions are the evidence.
   UiAutomation.class.getMethod("connect",int.class).invoke(a,UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES);
   android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();
   info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(800);
   if(group!=null){
    AccessibilityNodeInfo button=GpuRadioProbe.find(a.getRootInActiveWindow(),"lab-tab-"+group.toLowerCase());
    GpuApiProbe.check(button!=null,"view-lab group "+group);
    GpuApiProbe.check(button.performAction(AccessibilityNodeInfo.ACTION_CLICK),"open group "+group);Thread.sleep(1500);
   }
   Set<String> seen=new HashSet<>();int n=0;
   try(FileWriter log=new FileWriter("/data/local/tmp/walk-"+tag+".txt")){
    for(int page=0;page<40;page++){
     List<AccessibilityNodeInfo> nodes=new ArrayList<>();visible(a.getRootInActiveWindow(),pkg,nodes);
     for(AccessibilityNodeInfo node:nodes){
      String line=describe(node),key=line.substring(0,line.lastIndexOf(' '));
      if(!seen.add(key))continue;
      if(node.performAction(AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS)){
       Thread.sleep(900);String name="walk-"+tag+"-say-"+n++ +".png";caption(name);line+=" say="+name;spoken++;
      }else missing++;
      log.write(line+"\n");
     }
     log.flush();
     AccessibilityNodeInfo scroll=scrollable(a.getRootInActiveWindow());
     if(scroll==null||!scroll.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD))break;
     Thread.sleep(800);
    }
   }
   System.out.println("WALK "+tag+" spoken="+spoken+" refused="+missing);
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
