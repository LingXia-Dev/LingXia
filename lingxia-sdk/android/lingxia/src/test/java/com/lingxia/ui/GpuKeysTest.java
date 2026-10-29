package com.lingxia.ui;

import android.view.KeyEvent;
import org.junit.Test;
import static org.junit.Assert.assertEquals;

/** Hardware keys reach the core as its key codes; Back stays navigation. */
public class GpuKeysTest {
    @Test public void escapeDismissesAndBackIsNotAViewKey() {
        assertEquals(0, GpuView.key(KeyEvent.KEYCODE_TAB, 0));
        assertEquals(7, GpuView.key(KeyEvent.KEYCODE_ENTER, '\n'));
        assertEquals(8, GpuView.key(KeyEvent.KEYCODE_ESCAPE, 0));
        assertEquals(9, GpuView.key(KeyEvent.KEYCODE_PAGE_UP, 0));
        assertEquals(10, GpuView.key(KeyEvent.KEYCODE_PAGE_DOWN, 0));
        assertEquals(-1, GpuView.key(KeyEvent.KEYCODE_BACK, 0));
        // A printable character reaches the View's pickers.
        assertEquals(GpuView.CHAR_BASE + 'a', GpuView.key(KeyEvent.KEYCODE_A, 'a'));
    }
}
