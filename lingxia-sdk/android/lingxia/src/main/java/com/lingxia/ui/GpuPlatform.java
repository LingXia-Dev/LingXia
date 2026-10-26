package com.lingxia.ui;

/**
 * Android mechanics the shared Rust host drives. Every method runs on the main thread
 * except {@link #post}, which the View worker also calls.
 */
interface GpuPlatform {
    /** Run {@code GpuView.nativeRunTask(task)} on the main thread after the delay, exactly once. */
    void post(long task, long delayMillis);
    /** Call {@code nativeVsync} on the next display frame. */
    void requestVsync();
    /** {@code {width, height, scale, env}} for the next frame, or null while unmeasured. */
    String environment();
    /** Draw a CPU-rendered ARGB frame; false when the surface refused it. */
    boolean presentPixels(int[] argb, int width, int height);
    void gpuAttached(boolean attached);

    /** 0 focused, 1 focus cleared, 2 input focused, 3 hover enter, 4 hover exit, 5 value changed, 6 content changed. */
    void announce(int event, int id);
    void announceText(int id, boolean secure, String before, String after);
    void announceSelection(int id, boolean secure, String text, int start, int end);
    void announceClicked();
    /** A live region's text changed; assertive interrupts speech. */
    void announceLive(int id, String text, boolean assertive);

    /** The presented frame snapshot, before its effects run. */
    void frame(String snapshot, float scale);
    void imeFocus(int node, long commit, boolean touch, float x, float y, boolean word);
    void imeActivation();
    void imeReset();
    void inputReply(long callback, String reply);

    void press(int node);
    void clearPress();
    void focusRing(int node);
    void clearFocusRing();
    void claim(boolean exclusive);
    void armLongPress();
    void cancelLongPress();
    void fling(String region, boolean horizontal, float velocity);
    void stopFling();
    void clicked();
    void haptic();
    void editText(int node, String text, int start, int end);

    void loaded();
    /** A frame reached the screen and its effects ran. */
    void presented();
    void message(long connection, String message);
    boolean restart(long connection);
    /** Show recovery chrome; {@code message} -1 clears it, {@code action} -1 offers none. */
    void recovery(int message, int action, boolean retained, String error);
    void bootstrapOverdue(boolean overdue);
    /** Put text on the clipboard: an accessibility Copy of a static selection. */
    void copyText(String text);
}
