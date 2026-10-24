import android.app.UiAutomation;
import android.os.Bundle;
import android.os.HandlerThread;
import android.os.Looper;
import android.os.SystemClock;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityEvent;
import java.util.ArrayList;

public final class GpuHandlesScrollProbe {
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
    static boolean tapLabel(String text)throws Exception {AccessibilityNodeInfo node=label(text);if(node==null)return false;boolean clicked=node.performAction(AccessibilityNodeInfo.ACTION_CLICK);node.recycle();return clicked;}
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
    static void drag(android.graphics.Rect handle,float x,float y,long hold)throws Exception{
        long now=SystemClock.uptimeMillis();float startX=handle.centerX(),startY=handle.centerY();
        for(int i=0;i<=13;i++){
            int action=i==0?android.view.MotionEvent.ACTION_DOWN:i==13?android.view.MotionEvent.ACTION_UP:android.view.MotionEvent.ACTION_MOVE;
            android.view.MotionEvent event=android.view.MotionEvent.obtain(now,SystemClock.uptimeMillis(),action,startX+(x-startX)*Math.min(i,12)/12,startY+(y-startY)*Math.min(i,12)/12,0);
            event.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);require(automation.injectInputEvent(event,true),"drag injection");event.recycle();Thread.sleep(i==12 ? hold : 16);
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
            AccessibilityNodeInfo toggle=label("Short hello");if(toggle!=null){toggle.recycle();require(tapLabel("Short hello"),"single-line toggle");Thread.sleep(500);}
            String value="0123456789";value=value+value+value+value+value+value+value+value;
            set(value);choose(0,value.length());java.util.List<android.graphics.Rect> handles=handles("horizontal-tail");
            require(handles.size()==1,"offscreen anchor should be hidden");android.graphics.Rect box=bounds();
            drag(handles.get(0),box.left+20,handles.get(0).centerY(),2500);expect(value,0,0);
            handles("horizontal-start");System.out.println("PASS edge drag scrolls single-line selection to its start");
            require(tapLabel("Long note"),"multiline toggle");Thread.sleep(500);
            value="line-00\nline-01\nline-02\nline-03\nline-04\nline-05\nline-06\nline-07\nline-08\nline-09";
            set(value);choose(0,value.length());handles=handles("vertical-tail");require(handles.size()==1,"offscreen multiline anchor should be hidden");box=bounds();
            drag(handles.get(0),box.left+20,box.top+10,2500);expect(value,0,0);
            handles("vertical-start");key(KeyEvent.KEYCODE_Q,0);expect("q"+value,1,1);
            System.out.println("PASS edge drag scrolls multiline selection to its start and preserves insertion");
        }finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
    }
}
