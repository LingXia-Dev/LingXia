import android.app.UiAutomation;
import android.os.Bundle;
import android.os.HandlerThread;
import android.os.Looper;
import android.os.SystemClock;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityEvent;
import java.util.ArrayList;

public final class GpuAccessibilityIdentityCheckProbe {
    static UiAutomation automation;
    static final ArrayList<Integer> events=new ArrayList<>();
    static AccessibilityNodeInfo find(AccessibilityNodeInfo node) {
        if(node==null)return null;
        if("com.lingxia.example.gpu.dev:id/greeting-input".equals(node.getViewIdResourceName()))return node;
        for(int i=0;i<node.getChildCount();i++){AccessibilityNodeInfo found=find(node.getChild(i));if(found!=null){node.recycle();return found;}}
        node.recycle();return null;
    }
    static AccessibilityNodeInfo label(AccessibilityNodeInfo node,String text) {
        if(node==null)return null;
        if(text.contentEquals(node.getText()==null?"":node.getText()))return node;
        for(int i=0;i<node.getChildCount();i++){AccessibilityNodeInfo result=label(node.getChild(i),text);if(result!=null){node.recycle();return result;}}
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
            AccessibilityNodeInfo node=input();require(node!=null,"input missing");
            require(node.getText()==null||node.getText().length()==0,"placeholder must not be editable value");
            require(node.performAction(AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS),"accessibility focus rejected");
            AccessibilityNodeInfo original=AccessibilityNodeInfo.obtain(node);
            require(text(node,"A😀中文"),"unfocused set text rejected");node.recycle();
            node=await("A😀中文",5,5);
            AccessibilityNodeInfo accessible=automation.findFocus(AccessibilityNodeInfo.FOCUS_ACCESSIBILITY);
            require(accessible!=null && accessible.equals(original),"accessibility focus identity lost after edit frame"); accessible.recycle();
            require(original.refresh() && "A\uD83D\uDE00\u4e2d\u6587".contentEquals(original.getText()),"old node does not resolve current text");
            original.recycle();
            AccessibilityNodeInfo focused=automation.findFocus(AccessibilityNodeInfo.FOCUS_INPUT);
            require(focused!=null&&focused.isFocused()&&"com.lingxia.example.gpu.dev:id/greeting-input".equals(focused.getViewIdResourceName()),"virtual editor focus query");focused.recycle();
            require(!select(node,2,3),"split surrogate accepted");
            require(select(node,1,3),"emoji selection rejected");node.recycle();
            node=await("A😀中文",1,3);node.recycle();
            android.graphics.Bitmap shot=automation.takeScreenshot();
            try(java.io.FileOutputStream out=new java.io.FileOutputStream("/data/local/tmp/gpu-a11y-emoji.png")){shot.compress(android.graphics.Bitmap.CompressFormat.PNG,100,out);}shot.recycle();
            long now=SystemClock.uptimeMillis();
            Process key=Runtime.getRuntime().exec(new String[]{"/system/bin/input","keyevent",String.valueOf(KeyEvent.KEYCODE_X)});
            require(key.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&key.exitValue()==0,"keyboard injection");
            node=await("Ax中文",2,2);
            require(node.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,null),"clear rejected");node.recycle();
            node=await("",0,0);require(text(node,"Accessible hello"),"final set text");node.recycle();
            node=await("Accessible hello",16,16);node.recycle();
            AccessibilityNodeInfo stale=input();
            AccessibilityNodeInfo toggle=label(automation.getRootInActiveWindow(),"Long note");
            require(toggle!=null && toggle.performAction(AccessibilityNodeInfo.ACTION_CLICK),"multiline toggle");toggle.recycle();
            Thread.sleep(1200);
            AccessibilityNodeInfo replacement=input();require(replacement!=null && replacement.isMultiLine(),"textarea missing");
            require(!replacement.equals(stale),"replacement reused virtual node identity");
            require(!text(stale,"stale write"),"removed input accepted stale edit");
            require(!stale.refresh(),"removed virtual node remains queryable");
            replacement.recycle();stale.recycle();
            System.out.println("PASS: accessible focus survives edits; replaced nodes reject stale references");
            System.out.println("Observed text and selection events: "+events);
            synchronized(events){require(events.contains(AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED),"missing text event");require(events.contains(AccessibilityEvent.TYPE_VIEW_TEXT_SELECTION_CHANGED),"missing selection event");}
            System.out.println("PASS: unfocused setText, UTF-16 emoji selection, surrogate rejection, keyboard replacement, clear and accessibility events");
        }finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
    }
}
