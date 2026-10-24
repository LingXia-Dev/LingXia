import android.app.UiAutomation;
import android.os.*;
import android.view.*;
import android.view.accessibility.*;
import android.accessibilityservice.AccessibilityServiceInfo;
import android.graphics.Rect;
import java.util.*;

public final class GpuExplorationProbe {
    static UiAutomation automation;
    static final List<String> events=new ArrayList<>();
    static void require(boolean value,String message){if(!value)throw new AssertionError(message);}
    static AccessibilityNodeInfo find(AccessibilityNodeInfo node,String id){
        if(node==null)return null;
        if(id.equals(node.getViewIdResourceName()))return node;
        for(int i=0;i<node.getChildCount();i++){AccessibilityNodeInfo found=find(node.getChild(i),id);if(found!=null){node.recycle();return found;}}
        node.recycle();return null;
    }
    static AccessibilityNodeInfo input(){return find(automation.getRootInActiveWindow(),"com.lingxia.example.gpu.dev:id/greeting-input");}
    static void motion(long down,int action,float x,float y){
        MotionEvent event=MotionEvent.obtain(down,SystemClock.uptimeMillis(),action,x,y,0);event.setSource(InputDevice.SOURCE_MOUSE);
        require(automation.injectInputEvent(event,true),"touch injection");event.recycle();
    }
    public static void main(String[] args)throws Exception{
        HandlerThread thread=new HandlerThread("exploration");thread.start();
        Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
        automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
        try{
            UiAutomation.class.getMethod("connect").invoke(automation);
            AccessibilityServiceInfo service=automation.getServiceInfo();
            service.flags|=AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS|AccessibilityServiceInfo.FLAG_REQUEST_TOUCH_EXPLORATION_MODE;
            automation.setServiceInfo(service);
            automation.setOnAccessibilityEventListener(event->{
                if(event.getEventType()!=AccessibilityEvent.TYPE_VIEW_HOVER_ENTER && event.getEventType()!=AccessibilityEvent.TYPE_VIEW_HOVER_EXIT)return;
                AccessibilityNodeInfo source=event.getSource();
                String value=event.getEventType()+":"+(source==null?"missing":source.getViewIdResourceName()+":"+source.getClassName());
                if(source!=null)source.recycle(); synchronized(events){events.add(value);} System.out.println(value);
            });
            Thread.sleep(1600);
            AccessibilityNodeInfo node=input();require(node!=null,"missing input");Rect r=new Rect();node.getBoundsInScreen(r);node.recycle();
            long down=SystemClock.uptimeMillis();motion(down,MotionEvent.ACTION_HOVER_ENTER,r.centerX(),r.centerY());Thread.sleep(700);
            motion(down,MotionEvent.ACTION_HOVER_MOVE,r.centerX()+10,r.centerY());Thread.sleep(350);
            motion(down,MotionEvent.ACTION_HOVER_EXIT,r.centerX()+10,r.centerY());Thread.sleep(1200);
            synchronized(events){
                String suffix=":com.lingxia.example.gpu.dev:id/greeting-input:android.widget.EditText";
                require(events.contains(AccessibilityEvent.TYPE_VIEW_HOVER_ENTER+suffix),"missing virtual input hover enter: "+events);
                require(events.contains(AccessibilityEvent.TYPE_VIEW_HOVER_EXIT+suffix),"missing virtual input hover exit: "+events);
            }
            node=input();require(node!=null&&!node.isFocused(),"exploration unexpectedly focused IME");node.recycle();
            System.out.println("PASS: device hover exploration resolves virtual input enter/exit without activating editing");
        }finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
    }
}
