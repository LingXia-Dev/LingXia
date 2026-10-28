package com.lingxia.ui;

import java.util.ArrayList;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.annotation.Config;
import static org.junit.Assert.assertEquals;

/** Navigation and chrome both wait on a page's next frame; neither may lose the other's wait. */
@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuPresentationWaitersTest {
    @Test public void everyWaiterIsAnsweredAndADisposedPageAnswersAtOnce() {
        GpuNative.load();
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", "test:waiters", null);
        ArrayList<String> answers = new ArrayList<>();
        view.afterNextPresentation(ok -> answers.add("navigation " + ok));
        view.afterNextPresentation(ok -> answers.add("chrome " + ok));
        view.dispose();
        assertEquals(java.util.Arrays.asList("navigation false", "chrome false"), answers);
        view.afterNextPresentation(ok -> answers.add("late " + ok));
        assertEquals("late false", answers.get(2));
    }
}
