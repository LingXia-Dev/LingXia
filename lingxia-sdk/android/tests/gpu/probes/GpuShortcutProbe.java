import android.view.KeyEvent;
import android.view.InputEvent;
import android.view.InputDevice;
import android.view.KeyCharacterMap;
import android.os.SystemClock;
public final class GpuShortcutProbe {
    public static void main(String[] args) throws Exception {
        Class<?> type=Class.forName("android.hardware.input.InputManager");
        Object manager=type.getMethod("getInstance").invoke(null);
        java.lang.reflect.Method inject=type.getMethod("injectInputEvent",InputEvent.class,int.class);
        int code=KeyEvent.keyCodeFromString("KEYCODE_"+args[0]);
        long now=SystemClock.uptimeMillis();
        for(int action:new int[]{KeyEvent.ACTION_DOWN,KeyEvent.ACTION_UP}) {
            KeyEvent event=new KeyEvent(now,now,action,code,0,KeyEvent.META_CTRL_ON|KeyEvent.META_CTRL_LEFT_ON,KeyCharacterMap.VIRTUAL_KEYBOARD,0,0,InputDevice.SOURCE_KEYBOARD);
            if (!Boolean.TRUE.equals(inject.invoke(manager,event,2))) throw new IllegalStateException("Injection rejected");
        }
    }
}
