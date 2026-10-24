package com.lingxia.ui;

import android.os.Looper;
import java.time.Duration;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.Shadows;
import org.robolectric.annotation.Config;
import org.robolectric.annotation.LooperMode;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest=Config.NONE,sdk={23,31})
@LooperMode(LooperMode.Mode.PAUSED)
public class GpuCaretBlinkTest {
    private void advance(long ms) { Shadows.shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(ms)); }

    @Test public void unchangedFramesDoNotResetPhaseAndEditingRestartsVisible() {
        int[] frames={0}; GpuCaretBlink caret=new GpuCaretBlink(() -> frames[0]++);
        caret.update(true,"hello",5,5,-1,-1);
        advance(499); assertTrue(caret.visible()); assertEquals(0,frames[0]);
        caret.update(true,"hello",5,5,-1,-1);
        advance(1); assertFalse(caret.visible()); assertEquals(1,frames[0]);
        caret.update(true,"hello",5,5,-1,-1);
        advance(500); assertTrue(caret.visible()); assertEquals(2,frames[0]);
        advance(500); assertFalse(caret.visible());
        caret.update(true,"hello!",6,6,-1,-1);
        assertTrue(caret.visible()); assertEquals(4,frames[0]);
        advance(499); assertTrue(caret.visible());
        advance(1); assertFalse(caret.visible()); caret.stop();
    }
    @Test public void selectionAndInactiveEditorsHaveNoIdleTimer() {
        int[] frames={0}; GpuCaretBlink caret=new GpuCaretBlink(() -> frames[0]++);
        caret.update(true,"hello",0,5,-1,-1);
        advance(5000); assertEquals(0,frames[0]);
        caret.update(true,"hello",5,5,-1,-1); advance(500);
        assertFalse(caret.visible());
        caret.update(false,"hello",5,5,-1,-1);
        int stopped=frames[0]; advance(5000); assertEquals(stopped,frames[0]);
        caret.update(true,"hello",5,5,-1,-1); advance(500);
        assertFalse(caret.visible()); caret.stop();
        stopped=frames[0]; advance(5000); assertEquals(stopped,frames[0]);
    }
}
