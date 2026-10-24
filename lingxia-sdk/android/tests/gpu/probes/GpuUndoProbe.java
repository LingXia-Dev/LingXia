import android.app.UiAutomation;
import android.os.Bundle;
import android.os.HandlerThread;
import android.os.Looper;
import android.os.SystemClock;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityEvent;
import java.util.ArrayList;

public final class GpuUndoProbe {
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
    static void composition(int ctrl)throws Exception {
        set("base");tap(644,1593,0);expect("baseh",5,5);
        AccessibilityNodeInfo preview=label("Your note: base");require(preview!=null,"preedit leaked to bound preview");preview.recycle();
        key(KeyEvent.KEYCODE_Z,ctrl);expect("base",4,4);
        preview=label("Your note: base");require(preview!=null,"undo changed committed preview");preview.recycle();
        System.out.println("PASS Gboard preedit remains private and undo restores committed text");
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
            int ctrl=KeyEvent.META_CTRL_ON|KeyEvent.META_CTRL_LEFT_ON;
            if(args.length>0 && "composition".equals(args[0])){composition(ctrl);return;}
            set("hello"); key(KeyEvent.KEYCODE_Z,ctrl); expect("",0,0);
            key(KeyEvent.KEYCODE_Z,ctrl|KeyEvent.META_SHIFT_ON); expect("hello",5,5);
            key(KeyEvent.KEYCODE_Z,ctrl); expect("",0,0);
            key(KeyEvent.KEYCODE_Y,ctrl); expect("hello",5,5);
            System.out.println("PASS Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y");
            AccessibilityNodeInfo node=input();require(select(node,1,4),"selection");node.recycle();expect("hello",1,4);
            key(KeyEvent.KEYCODE_X,0);expect("hxo",2,2);
            key(KeyEvent.KEYCODE_Z,ctrl);expect("hello",1,4);
            key(KeyEvent.KEYCODE_Y,ctrl);expect("hxo",2,2);
            System.out.println("PASS replacement undo restores exact selection");
            set("menu value");menu("Undo");expect("hxo",0,3);
            menu("Redo");expect("menu value",10,10);
            System.out.println("PASS native floating Undo and Redo menu");
            composition(ctrl);
            key(KeyEvent.KEYCODE_HOME,0);Thread.sleep(600);
            automation.executeShellCommand("am start -n com.lingxia.example.gpu.dev/com.lingxia.example.gpu.MainActivity").close();Thread.sleep(1000);
            node=input();require(node!=null && node.performAction(AccessibilityNodeInfo.ACTION_CLICK),"refocus after Home");node.recycle();
            expect("base",4,4);key(KeyEvent.KEYCODE_Z,ctrl);expect("menu value",0,10);
            System.out.println("PASS committed undo history survives Home and refocus");
            key(KeyEvent.KEYCODE_Q,0);expect("q",1,1);key(KeyEvent.KEYCODE_Y,ctrl);expect("q",1,1);
            System.out.println("PASS new typing after undo clears redo");
            android.graphics.Bitmap shot=automation.takeScreenshot();
            try(java.io.FileOutputStream out=new java.io.FileOutputStream("/data/local/tmp/gpu-undo.png")){shot.compress(android.graphics.Bitmap.CompressFormat.PNG,100,out);}shot.recycle();
        }finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
    }
}
