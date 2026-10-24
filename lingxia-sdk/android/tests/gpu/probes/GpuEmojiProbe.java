import android.app.UiAutomation;
import android.os.Bundle;
import android.os.HandlerThread;
import android.os.Looper;
import android.os.SystemClock;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityEvent;
import java.util.ArrayList;

public final class GpuEmojiProbe {
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
    static void key(int code){long now=SystemClock.uptimeMillis();for(int action:new int[]{KeyEvent.ACTION_DOWN,KeyEvent.ACTION_UP})require(automation.injectInputEvent(new KeyEvent(now,now,action,code,0,0,android.view.KeyCharacterMap.VIRTUAL_KEYBOARD,0,KeyEvent.FLAG_SOFT_KEYBOARD,android.view.InputDevice.SOURCE_KEYBOARD),true),"key injection");}
    public static void main(String[] args)throws Exception{
        HandlerThread thread=new HandlerThread("accessibility");thread.start();
        Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
        automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
        try{
            UiAutomation.class.getMethod("connect").invoke(automation);
            android.accessibilityservice.AccessibilityServiceInfo service=automation.getServiceInfo();
            service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
            automation.setServiceInfo(service);
            automation.setOnAccessibilityEventListener(event->{
                if("com.lingxia.example.gpu.dev".contentEquals(event.getPackageName()==null?"":event.getPackageName()))synchronized(events){events.add(event.getEventType());}
            });
            Thread.sleep(700);
            for(String emoji:new String[]{"\ud83d\udc68\u200d\ud83d\udc69\u200d\ud83d\udc67\u200d\ud83d\udc66","\ud83d\udc4d\ud83c\udffd","\ud83c\udde8\ud83c\uddf3","1\ufe0f\u20e3","\ud83d\ude00"}) {
                AccessibilityNodeInfo node=input();require(node!=null,"input missing");
                String value="L"+emoji;
                require(text(node,value),"set text");node.recycle();
                node=await(value,value.length(),value.length());node.recycle();
                long now=SystemClock.uptimeMillis();
                automation.injectInputEvent(new KeyEvent(now,now,KeyEvent.ACTION_DOWN,KeyEvent.KEYCODE_DEL,0,0,android.view.KeyCharacterMap.VIRTUAL_KEYBOARD,0,KeyEvent.FLAG_SOFT_KEYBOARD,android.view.InputDevice.SOURCE_KEYBOARD),true);
                automation.injectInputEvent(new KeyEvent(now,now,KeyEvent.ACTION_UP,KeyEvent.KEYCODE_DEL,0,0,android.view.KeyCharacterMap.VIRTUAL_KEYBOARD,0,KeyEvent.FLAG_SOFT_KEYBOARD,android.view.InputDevice.SOURCE_KEYBOARD),true);
                node=await("L",1,1);node.recycle();
                System.out.println("PASS backward delete: "+emoji);
            }
            for(String emoji:new String[]{"\ud83d\udc68\u200d\ud83d\udc69\u200d\ud83d\udc67\u200d\ud83d\udc66","\ud83d\udc4d\ud83c\udffd","\ud83c\udde8\ud83c\uddf3","1\ufe0f\u20e3","\ud83d\ude00"}) {
                String value="L"+emoji+"R";
                AccessibilityNodeInfo node=input();require(text(node,value),"forward setup");node.recycle();
                node=await(value,value.length(),value.length());require(select(node,1,1),"forward caret");node.recycle();
                node=await(value,1,1);node.recycle();key(KeyEvent.KEYCODE_FORWARD_DEL);
                node=await("LR",1,1);node.recycle();System.out.println("PASS forward delete: "+emoji);
            }
            AccessibilityNodeInfo node=input();String value="L\ud83d\udc68\u200d\ud83d\udc69\u200d\ud83d\udc67\u200d\ud83d\udc66";
            require(text(node,value),"soft delete setup");node.recycle();node=await(value,value.length(),value.length());node.recycle();
            long now=SystemClock.uptimeMillis();
            for(int action:new int[]{android.view.MotionEvent.ACTION_DOWN,android.view.MotionEvent.ACTION_UP}){
                android.view.MotionEvent event=android.view.MotionEvent.obtain(now,SystemClock.uptimeMillis(),action,994,1787,0);event.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);require(automation.injectInputEvent(event,true),"soft delete tap");event.recycle();
            }
            node=await("L",1,1);node.recycle();System.out.println("PASS physical Gboard soft delete: family emoji");
        }finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
    }
}
