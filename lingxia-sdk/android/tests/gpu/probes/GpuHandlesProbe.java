import android.app.UiAutomation;
import android.os.Bundle;
import android.os.HandlerThread;
import android.os.Looper;
import android.os.SystemClock;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityEvent;
import java.util.ArrayList;

public final class GpuHandlesProbe {
    static UiAutomation automation;
    static final ArrayList<Integer> events=new ArrayList<>();
    static AccessibilityNodeInfo find(AccessibilityNodeInfo node) {
        if(node==null)return null;
        if("com.lingxia.example.gpu.dev:id/greeting-input".equals(node.getViewIdResourceName()))return node;
        for(int i=0;i<node.getChildCount();i++){AccessibilityNodeInfo found=find(node.getChild(i));if(found!=null){node.recycle();return found;}}
        node.recycle();return null;
    }
    static AccessibilityNodeInfo input(){return find(automation.getRootInActiveWindow());}
    static void require(boolean value,String message){if(!value)throw new AssertionError(message);}
    static AccessibilityNodeInfo await(String text,int start,int end)throws Exception{
        for(int i=0;i<100;i++){
            AccessibilityNodeInfo node=input();
            if(node!=null&&text.contentEquals(node.getText()==null?"":node.getText())&&node.getTextSelectionStart()==start&&node.getTextSelectionEnd()==end)return node;
            if(node!=null)node.recycle();Thread.sleep(100);
        }
        AccessibilityNodeInfo node=input();throw new AssertionError("Expected "+text+" ["+start+","+end+"]; actual "+node);
    }
    static boolean text(AccessibilityNodeInfo node,String text){Bundle args=new Bundle();args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,text);return node.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,args);}
    static boolean select(AccessibilityNodeInfo node,int start,int end){Bundle args=new Bundle();args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT,start);args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT,end);return node.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION,args);}
    static void key(int code,int meta){long now=SystemClock.uptimeMillis();for(int action:new int[]{KeyEvent.ACTION_DOWN,KeyEvent.ACTION_UP})require(automation.injectInputEvent(new KeyEvent(now,now,action,code,0,meta,android.view.KeyCharacterMap.VIRTUAL_KEYBOARD,0,0,android.view.InputDevice.SOURCE_KEYBOARD),true),"key injection");}
    static void expect(String text,int start,int end)throws Exception { AccessibilityNodeInfo node=await(text,start,end);node.recycle();Thread.sleep(150); }
    static void set(String value)throws Exception { AccessibilityNodeInfo node=input();require(node!=null && text(node,value),"set text");node.recycle();expect(value,value.length(),value.length()); }
    static void tap(float x,float y,long hold)throws Exception {
        long now=SystemClock.uptimeMillis();
        for(int action:new int[]{android.view.MotionEvent.ACTION_DOWN,android.view.MotionEvent.ACTION_UP}){
            if(action==android.view.MotionEvent.ACTION_UP && hold>0)Thread.sleep(hold);
            android.view.MotionEvent event=android.view.MotionEvent.obtain(now,SystemClock.uptimeMillis(),action,x,y,0);event.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);require(automation.injectInputEvent(event,true),"tap");event.recycle();
        }
    }
    static AccessibilityNodeInfo label(AccessibilityNodeInfo node,String label){
        if(node==null)return null;
        if(label.contentEquals(node.getText()==null?"":node.getText()) || label.contentEquals(node.getContentDescription()==null?"":node.getContentDescription()))return node;
        for(int i=0;i<node.getChildCount();i++){AccessibilityNodeInfo found=label(node.getChild(i),label);if(found!=null){node.recycle();return found;}}
        node.recycle();return null;
    }
    static AccessibilityNodeInfo label(String text){for(android.view.accessibility.AccessibilityWindowInfo window:automation.getWindows()){AccessibilityNodeInfo found=label(window.getRoot(),text);if(found!=null)return found;}return null;}
    static boolean tapLabel(String text)throws Exception {AccessibilityNodeInfo node=label(text);if(node==null)return false;android.graphics.Rect r=new android.graphics.Rect();node.getBoundsInScreen(r);node.recycle();tap(r.centerX(),r.centerY(),0);return true;}
    static void menu(String item)throws Exception {
        AccessibilityNodeInfo node=input();android.graphics.Rect r=new android.graphics.Rect();node.getBoundsInScreen(r);node.recycle();
        tap(r.left+70,r.centerY(),850);Thread.sleep(700);
        if(!tapLabel(item)){require(tapLabel("More options"),"toolbar overflow");Thread.sleep(400);require(tapLabel(item),"menu "+item);}
    }
    static android.graphics.Rect bounds(){AccessibilityNodeInfo n=input();require(n!=null,"missing input");android.graphics.Rect r=new android.graphics.Rect();n.getBoundsInScreen(r);n.recycle();return r;}
    static void choose(int a,int b)throws Exception{AccessibilityNodeInfo n=input();String value=n.getText().toString();require(select(n,a,b),"set selection");n.recycle();expect(value,a,b);}
    static java.util.List<android.graphics.Rect> handles(String name)throws Exception{
        Thread.sleep(250);android.graphics.Bitmap captured=automation.takeScreenshot();android.graphics.Bitmap shot=captured.copy(android.graphics.Bitmap.Config.ARGB_8888,false);captured.recycle();
        try(java.io.FileOutputStream out=new java.io.FileOutputStream("/data/local/tmp/gpu-handles-"+name+".png")){shot.compress(android.graphics.Bitmap.CompressFormat.PNG,100,out);}
        android.graphics.Rect bounds=bounds();int w=shot.getWidth(),h=shot.getHeight();int[] pixels=new int[w*h];shot.getPixels(pixels,0,w,0,0,w,h);shot.recycle();
        boolean[] blue=new boolean[w*h];
        for(int y=Math.max(0,bounds.top);y<Math.min(h,bounds.bottom+30);y++)for(int x=Math.max(0,bounds.left-30);x<Math.min(w,bounds.right+30);x++){
            int c=pixels[y*w+x],r=android.graphics.Color.red(c),g=android.graphics.Color.green(c),b=android.graphics.Color.blue(c);
            blue[y*w+x]=r>=50&&r<=100&&g>=70&&g<=130&&b>=180&&b<=240;
        }
        java.util.List<android.graphics.Rect> found=new java.util.ArrayList<>();java.util.ArrayDeque<Integer> queue=new java.util.ArrayDeque<>();
        for(int p=0;p<blue.length;p++)if(blue[p]){
            int l=p%w,t=p/w,r=l,b=t,count=0;blue[p]=false;queue.add(p);
            while(!queue.isEmpty()){int q=queue.removeFirst(),x=q%w,y=q/w;count++;l=Math.min(l,x);r=Math.max(r,x);t=Math.min(t,y);b=Math.max(b,y);
                for(int next:new int[]{q-1,q+1,q-w,q+w})if(next>=0&&next<blue.length&&blue[next]){blue[next]=false;queue.add(next);}}
            if(count>150&&r-l>=25&&r-l<=65&&b-t>=30&&b-t<=100)found.add(new android.graphics.Rect(l,t,r+1,b+1));
        }
        found.sort((a,b)->Math.abs(a.centerY()-b.centerY())>20 ? Integer.compare(a.centerY(),b.centerY()) : Integer.compare(a.centerX(),b.centerX()));
        System.out.println(name+" handle bounds: "+found);return found;
    }
    static void drag(android.graphics.Rect handle,float x,float y)throws Exception{
        long now=SystemClock.uptimeMillis();float startX=handle.centerX(),startY=handle.centerY();
        for(int i=0;i<=12;i++){
            int action=i==0?android.view.MotionEvent.ACTION_DOWN:i==12?android.view.MotionEvent.ACTION_UP:android.view.MotionEvent.ACTION_MOVE;
            android.view.MotionEvent event=android.view.MotionEvent.obtain(now,SystemClock.uptimeMillis(),action,startX+(x-startX)*i/12,startY+(y-startY)*i/12,0);
            event.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);require(automation.injectInputEvent(event,true),"drag injection");event.recycle();Thread.sleep(50);
        }
    }
    public static void main(String[] args)throws Exception{
        HandlerThread thread=new HandlerThread("accessibility");thread.start();
        Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
        automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
        try{
            UiAutomation.class.getMethod("connect").invoke(automation);
            android.accessibilityservice.AccessibilityServiceInfo service=automation.getServiceInfo();
            service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS|android.accessibilityservice.AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS;
            automation.setServiceInfo(service);
            automation.setOnAccessibilityEventListener(event->{
                if("com.lingxia.example.gpu.dev".contentEquals(event.getPackageName()==null?"":event.getPackageName()))synchronized(events){events.add(event.getEventType());}
            });
            Thread.sleep(700);
            set("hello world");choose(0,5);java.util.List<android.graphics.Rect> handles=handles("initial");require(handles.size()==2,"two selection handles");
            android.graphics.Rect box=bounds();drag(handles.get(1),box.right-20,handles.get(1).centerY());expect("hello world",0,11);
            System.out.println("PASS drag focus extends selection to text end");
            handles=handles("extended");drag(handles.get(0),box.right-20,handles.get(0).centerY());expect("hello world",11,11);
            handles=handles("collapsed");require(handles.size()==1,"one caret handle");drag(handles.get(0),box.left+25,handles.get(0).centerY());expect("hello world",0,0);
            key(KeyEvent.KEYCODE_X,0);expect("xhello world",1,1);
            System.out.println("PASS anchor crossing collapses selection and caret drag controls insertion");
            set("hello world");choose(11,6);handles=handles("reversed");box=bounds();
            drag(handles.get(1),box.left+25,handles.get(1).centerY());expect("hello world",6,0);
            key(KeyEvent.KEYCODE_Q,0);expect("qworld",1,1);
            System.out.println("PASS reverse selection preserves fixed endpoint while dragging anchor");
            require(tapLabel("Long note"),"textarea toggle");Thread.sleep(800);
            set("one\ntwo\nthree");choose(0,13);handles=handles("multiline");require(handles.size()==2,"multiline handles");
            drag(handles.get(1),handles.get(0).centerX(),handles.get(0).centerY());expect("one\ntwo\nthree",0,0);
            key(KeyEvent.KEYCODE_Z,0);expect("zone\ntwo\nthree",1,1);
            System.out.println("PASS multiline focus drag follows the displayed line geometry");
        }finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
    }
}
