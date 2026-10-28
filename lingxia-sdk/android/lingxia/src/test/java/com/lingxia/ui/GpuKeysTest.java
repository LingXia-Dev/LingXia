package com.lingxia.ui;

import android.view.KeyEvent;
import org.junit.Test;
import static org.junit.Assert.assertEquals;

/** Hardware keys reach the core as its key codes; Back stays navigation. */
public class GpuKeysTest {
    @Test public void escapeDismissesAndBackIsNotAViewKey() {
        assertEquals(0, GpuView.key(KeyEvent.KEYCODE_TAB));
        assertEquals(7, GpuView.key(KeyEvent.KEYCODE_ENTER));
        assertEquals(8, GpuView.key(KeyEvent.KEYCODE_ESCAPE));
        assertEquals(-1, GpuView.key(KeyEvent.KEYCODE_BACK));
        assertEquals(-1, GpuView.key(KeyEvent.KEYCODE_A));
    }
}
