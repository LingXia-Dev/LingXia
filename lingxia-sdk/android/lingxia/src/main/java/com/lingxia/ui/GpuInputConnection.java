package com.lingxia.ui;

import android.text.TextUtils;
import android.view.KeyEvent;
import android.view.View;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.ExtractedText;
import android.view.inputmethod.ExtractedTextRequest;
import android.view.inputmethod.SurroundingText;
import org.json.JSONArray;
import org.json.JSONObject;
import java.nio.charset.StandardCharsets;
import java.util.TreeMap;

/** UI-thread IME mirror. The native editor remains authoritative after worker acknowledgement. */
final class GpuInputConnection extends BaseInputConnection {
    interface Sink {
        void send(long revision, JSONArray edits, boolean submit);
        void changed();
    }
    private static final int MAX_BYTES = 65536, MAX_PENDING_BYTES = 512 * 1024;
    private final Sink sink;
    private final View targetView;
    private static final android.text.method.BaseKeyListener DELETE_KEYS=new android.text.method.BaseKeyListener() {
        @Override public int getInputType() { return android.text.InputType.TYPE_CLASS_TEXT; }
    };
    private final android.content.ClipboardManager clipboard;
    final String identity;
    final long token;
    private String text;
    private int anchor, focus, composeStart = -1, composeEnd = -1;
    private int depth, queuedBytes, pendingBytes;
    private boolean closed, submitting, historyPending, canUndo, canRedo;
    private JSONArray queued = new JSONArray();
    private final TreeMap<Long, Integer> pending = new TreeMap<>();
    private long revision;
    int extractedToken = -1;
    boolean multiline, secure, submitOnEnter;

    GpuInputConnection(View view, String identity, long token, JSONObject state, Sink sink) {
        super(view, true);
        targetView=view;
        this.identity = identity; this.token = token; this.sink = sink;
        clipboard=(android.content.ClipboardManager)view.getContext().getSystemService(android.content.Context.CLIPBOARD_SERVICE);
        install(state);
    }
    private void install(JSONObject state) {
        text = state.optString("text");
        JSONArray selection = state.optJSONArray("selection"), composition = state.optJSONArray("composition");
        anchor = selection == null ? 0 : selection.optInt(0);
        focus = selection == null ? 0 : selection.optInt(1);
        composeStart = composition == null ? -1 : composition.optInt(0);
        composeEnd = composition == null ? -1 : composition.optInt(1);
        canUndo=state.optBoolean("can_undo"); canRedo=state.optBoolean("can_redo");
        if (!validText(text) || !boundary(text, anchor) || !boundary(text, focus)
                || (composeStart != -1 && (!boundary(text, composeStart) || !boundary(text, composeEnd) || composeStart >= composeEnd))) {
            throw new IllegalArgumentException("Invalid native input snapshot");
        }
    }
    long revision() { return revision; }
    boolean settled() { return !closed && depth == 0 && queued.length() == 0 && pending.isEmpty() && !submitting; }
    boolean isClosed() { return closed; }
    int anchor() { return anchor; }
    int focus() { return focus; }
    int composeStart() { return composeStart; }
    int composeEnd() { return composeEnd; }
    boolean canUndo() { return settled() && canUndo; }
    boolean canRedo() { return settled() && canRedo; }

    boolean acknowledge(long acknowledged, JSONObject state) {
        if (closed || !pending.containsKey(acknowledged)) return false;
        for (int bytes : pending.headMap(acknowledged, true).values()) pendingBytes -= bytes;
        pending.headMap(acknowledged, true).clear();
        if (acknowledged == revision && queued.length() == 0) {
            install(state);
            boolean resync=historyPending; historyPending=false;
            sink.changed();
            return resync;
        }
        return false;
    }
    void acceptFrame(long capturedRevision, JSONObject state) {
        if (!closed && capturedRevision == revision && pending.isEmpty() && queued.length() == 0) {
            install(state);
            sink.changed();
        }
    }
    @Override public void closeConnection() {
        closed = true; queued = new JSONArray(); pending.clear();
        pendingBytes = queuedBytes = depth = 0; historyPending=false;
    }
    // Unsupported BaseInputConnection operations must not mutate a second, hidden buffer.
    @Override public android.text.Editable getEditable() { return null; }
    @Override public boolean beginBatchEdit() {
        if (closed || historyPending || depth >= 64) return false;
        depth++; return true;
    }
    @Override public boolean endBatchEdit() {
        if (closed || depth == 0) return false;
        if (--depth == 0) flush();
        return depth > 0;
    }
    private static JSONObject command(String op, Object... pairs) {
        JSONObject result = new JSONObject();
        try {
            result.put("op", op);
            for (int i=0; i<pairs.length; i+=2) result.put((String)pairs[i], pairs[i+1]);
        } catch (org.json.JSONException impossible) { throw new IllegalArgumentException(impossible); }
        return result;
    }
    private boolean reserve(JSONObject... edits) {
        if (closed || historyPending || submitting || queued.length() + edits.length > 64 || pending.size() >= 64) return false;
        int bytes = 0;
        for (JSONObject edit : edits) bytes += edit.toString().getBytes(StandardCharsets.UTF_8).length + 64;
        if (pendingBytes + queuedBytes + bytes > MAX_PENDING_BYTES) return false;
        for (JSONObject edit : edits) queued.put(edit);
        queuedBytes += bytes; return true;
    }
    private void changed() {
        if (depth == 0) flush();
    }
    private void flush() {
        if (queued.length() == 0 && !submitting) return;
        JSONArray edits = queued; boolean submit = submitting; int bytes = queuedBytes;
        queued = new JSONArray(); queuedBytes = 0; submitting = false;
        pending.put(++revision, bytes); pendingBytes += bytes;
        sink.send(revision, edits, submit);
        sink.changed();
    }
    static boolean boundary(String value, int offset) {
        return offset >= 0 && offset <= value.length() && !(offset > 0 && offset < value.length()
            && Character.isHighSurrogate(value.charAt(offset-1)) && Character.isLowSurrogate(value.charAt(offset)));
    }
    static boolean validText(String value) {
        if (value.length() > MAX_BYTES) return false;
        for (int i=0; i<value.length(); i++) {
            char ch = value.charAt(i);
            if (Character.isHighSurrogate(ch)) {
                if (++i >= value.length() || !Character.isLowSurrogate(value.charAt(i))) return false;
            } else if (Character.isLowSurrogate(ch)) return false;
        }
        return value.getBytes(StandardCharsets.UTF_8).length <= MAX_BYTES;
    }
    private boolean replace(CharSequence value, int cursor, boolean composing) {
        return replace(value,cursor,composing,false);
    }
    private boolean replace(CharSequence value, int cursor, boolean composing, boolean key) {
        return replace(value,cursor,composing,key,null);
    }
    private boolean replace(CharSequence value, int cursor, boolean composing, boolean key, java.util.function.BooleanSupplier beforeChange) {
        if (closed || value == null) return false;
        String insert = value.toString();
        if (!validText(insert)) return false;
        int start = composeStart >= 0 && !key ? composeStart : Math.min(anchor, focus);
        int end = composeEnd >= 0 && !key ? composeEnd : Math.max(anchor, focus);
        String next = text.substring(0,start) + insert + text.substring(end);
        long position = cursor > 0 ? (long)start + insert.length() + cursor - 1 : (long)start + cursor;
        int selection = (int)Math.max(0, Math.min(next.length(), position));
        JSONObject replacement=command("replace", "text",insert,"cursor",cursor,"composing",composing);
        JSONObject[] edits=key && composeStart >= 0 ? new JSONObject[]{command("finish_composition"),replacement} : new JSONObject[]{replacement};
        int previousCount=queued.length(), previousBytes=queuedBytes;
        if (!validText(next) || !boundary(next, selection) || !reserve(edits)) return false;
        if (beforeChange != null && !beforeChange.getAsBoolean()) {
            while (queued.length() > previousCount) queued.remove(queued.length()-1);
            queuedBytes=previousBytes;
            return false;
        }
        text = next; anchor = focus = selection;
        composeStart = composing && !insert.isEmpty() ? start : -1;
        composeEnd = composeStart >= 0 ? start + insert.length() : -1;
        changed(); return true;
    }
    @Override public boolean commitText(CharSequence text, int cursor) { return replace(text,cursor,false); }
    boolean replaceAll(String value) {
        if (!validText(value) || !reserve(command("finish_composition"),
            command("selection","anchor",0,"focus",text.length()),
            command("replace","text",value,"cursor",1,"composing",false))) return false;
        text=value; anchor=focus=value.length(); composeStart=composeEnd=-1;
        changed(); return true;
    }
    @Override public boolean setComposingText(CharSequence text, int cursor) { return replace(text,cursor,true); }
    @Override public boolean setSelection(int start, int end) {
        if (!boundary(text,start) || !boundary(text,end) || !reserve(command("selection","anchor",start,"focus",end))) return false;
        anchor=start; focus=end; changed(); return true;
    }
    boolean selectFromTouch(int start, int end) {
        if (!boundary(text,start) || !boundary(text,end)) return false;
        JSONObject selection=command("selection","anchor",start,"focus",end);
        JSONObject[] edits=composeStart >= 0 ? new JSONObject[]{command("finish_composition"),selection} : new JSONObject[]{selection};
        if (!reserve(edits)) return false;
        composeStart=composeEnd=-1; anchor=start; focus=end; changed(); return true;
    }
    @Override public boolean setComposingRegion(int start, int end) {
        if (!boundary(text,start) || !boundary(text,end) || !reserve(command("composition","start",start,"end",end))) return false;
        composeStart = start == end ? -1 : Math.min(start,end);
        composeEnd = start == end ? -1 : Math.max(start,end);
        changed(); return true;
    }
    @Override public boolean finishComposingText() {
        if (!reserve(command("finish_composition"))) return false;
        composeStart=composeEnd=-1; changed(); return true;
    }
    private int remap(int offset, int start, int end) {
        return offset <= start ? offset : offset - (Math.min(offset,end)-start);
    }
    private void remove(int start, int end) {
        text = text.substring(0,start) + text.substring(end);
        anchor=remap(anchor,start,end); focus=remap(focus,start,end);
        if (composeStart >= 0) {
            composeStart=remap(composeStart,start,end); composeEnd=remap(composeEnd,start,end);
            if (composeStart == composeEnd) composeStart=composeEnd=-1;
        }
    }
    private boolean delete(int before, int after, boolean codePoints) {
        if (before < 0 || after < 0 || closed) return false;
        int start=Math.min(anchor,focus), end=Math.max(anchor,focus);
        int left = codePoints ? text.offsetByCodePoints(start,-Math.min(before,text.codePointCount(0,start))) : Math.max(0,start-before);
        int right = codePoints ? text.offsetByCodePoints(end,Math.min(after,text.codePointCount(end,text.length()))) : (int)Math.min(text.length(),(long)end+after);
        if (!boundary(text,left) || !boundary(text,right) || !reserve(command("delete_surrounding","before",before,"after",after,"code_points",codePoints))) return false;
        remove(end,right); remove(left,start); changed(); return true;
    }
    @Override public boolean deleteSurroundingText(int before,int after) { return delete(before,after,false); }
    @Override public boolean deleteSurroundingTextInCodePoints(int before,int after) { return delete(before,after,true); }
    private boolean deleteKey(KeyEvent event) {
        // Use the device's TextView deletion rules, without giving it the authoritative buffer.
        android.text.SpannableStringBuilder candidate=new android.text.SpannableStringBuilder(text);
        android.text.Selection.setSelection(candidate,anchor,focus);
        boolean deleted=event.getKeyCode() == KeyEvent.KEYCODE_FORWARD_DEL
            ? DELETE_KEYS.forwardDelete(targetView,candidate,event.getKeyCode(),event)
            : DELETE_KEYS.backspace(targetView,candidate,event.getKeyCode(),event);
        if (!deleted) return true;
        int start=android.text.Selection.getSelectionEnd(candidate), end=start+text.length()-candidate.length();
        if (!boundary(text,start) || !boundary(text,end) || end < start
            || !candidate.toString().equals(text.substring(0,start)+text.substring(end))) return false;
        JSONObject selection=command("selection","anchor",start,"focus",end);
        JSONObject replacement=command("replace","text","","cursor",1,"composing",false);
        if (!(composeStart >= 0 ? reserve(command("finish_composition"),selection,replacement) : reserve(selection,replacement))) return false;
        text=candidate.toString(); anchor=focus=start; composeStart=composeEnd=-1;
        changed(); return true;
    }
    @Override public boolean performEditorAction(int action) {
        if (closed || historyPending || submitting || queued.length() >= 64 || pending.size() >= 64) return false;
        submitting=true; composeStart=composeEnd=-1; changed(); return true;
    }
    private boolean copy(String value) {
        if (clipboard == null) return false;
        try {
            clipboard.setPrimaryClip(android.content.ClipData.newPlainText("",value));
            return true;
        } catch (SecurityException | IllegalStateException unavailable) { return false; }
    }
    private String pasteText() {
        if (clipboard == null) return null;
        try {
            android.content.ClipData clip=clipboard.getPrimaryClip();
            if (clip == null || clip.getItemCount() == 0 || clip.getItemCount() > 128) return null;
            StringBuilder value=new StringBuilder();
            for (int i=0;i<clip.getItemCount();i++) {
                // URI coercion can block or read external content on the UI thread.
                CharSequence item=clip.getItemAt(i).getText();
                if (item == null || (long)value.length()+item.length()+(i==0?0:1) > MAX_BYTES) return null;
                if (i != 0) value.append('\n');
                value.append(item);
            }
            String result=value.toString();
            return validText(result) ? result : null;
        } catch (SecurityException | IllegalStateException unavailable) { return null; }
    }
    @Override public boolean performContextMenuAction(int action) {
        if (closed || historyPending) return false;
        if (action == android.R.id.undo || action == android.R.id.redo) {
            boolean undo=action == android.R.id.undo;
            if (!(undo ? canUndo() : canRedo()) || !reserve(command(undo ? "undo" : "redo"))) return false;
            // The mirror has no history. Do not calculate edits against its old selection.
            historyPending=true; changed(); return true;
        }
        if (action == android.R.id.selectAll) {
            JSONObject selection=command("selection","anchor",0,"focus",text.length());
            JSONObject[] edits=composeStart >= 0 ? new JSONObject[]{command("finish_composition"),selection} : new JSONObject[]{selection};
            if (!reserve(edits)) return false;
            composeStart=composeEnd=-1; anchor=0; focus=text.length(); changed(); return true;
        }
        if (action == android.R.id.copy || action == android.R.id.cut) {
            if (secure || anchor == focus) return false;
            String selected=text.substring(Math.min(anchor,focus),Math.max(anchor,focus));
            return action == android.R.id.copy ? copy(selected) : replace("",1,false,true,() -> copy(selected));
        }
        if (action == android.R.id.paste || action == android.R.id.pasteAsPlainText) {
            String value=pasteText();
            return value != null && replace(value,1,false,true);
        }
        return false;
    }
    @Override public CharSequence getTextBeforeCursor(int length,int flags) {
        int end=Math.min(anchor,focus); return closed ? "" : text.substring(Math.max(0,end-Math.max(0,length)),end);
    }
    @Override public CharSequence getTextAfterCursor(int length,int flags) {
        int start=Math.max(anchor,focus); return closed ? "" : text.substring(start,(int)Math.min(text.length(),(long)start+Math.max(0,length)));
    }
    @Override public CharSequence getSelectedText(int flags) { return closed || anchor == focus ? null : text.substring(Math.min(anchor,focus),Math.max(anchor,focus)); }
    @Override public int getCursorCapsMode(int modes) { return closed ? 0 : TextUtils.getCapsMode(text,Math.min(anchor,focus),modes); }
    @Override public SurroundingText getSurroundingText(int before,int after,int flags) {
        if (closed || before < 0 || after < 0) return null;
        int start=Math.max(0,Math.min(anchor,focus)-before);
        int end=(int)Math.min(text.length(),(long)Math.max(anchor,focus)+after);
        return new SurroundingText(text.substring(start,end),anchor-start,focus-start,start);
    }
    @Override public ExtractedText getExtractedText(ExtractedTextRequest request,int flags) {
        if (closed) return null;
        if (request != null && (flags & GET_EXTRACTED_TEXT_MONITOR) != 0) extractedToken=request.token;
        ExtractedText result=new ExtractedText(); result.text=text; result.startOffset=0;
        result.partialStartOffset=result.partialEndOffset=-1; result.selectionStart=anchor; result.selectionEnd=focus;
        return result;
    }
    @Override public boolean sendKeyEvent(KeyEvent event) {
        if (closed || historyPending) return false;
        if (event.getAction() == KeyEvent.ACTION_UP) return true;
        if (event.getAction() != KeyEvent.ACTION_DOWN) return false;
        if (event.isCtrlPressed() && !event.isAltPressed() && !event.isMetaPressed()) {
            switch (event.getKeyCode()) {
                case KeyEvent.KEYCODE_A: return performContextMenuAction(android.R.id.selectAll);
                case KeyEvent.KEYCODE_C: return performContextMenuAction(android.R.id.copy);
                case KeyEvent.KEYCODE_X: return performContextMenuAction(android.R.id.cut);
                case KeyEvent.KEYCODE_V: return performContextMenuAction(android.R.id.paste);
                case KeyEvent.KEYCODE_Z: return performContextMenuAction(event.isShiftPressed() ? android.R.id.redo : android.R.id.undo);
                case KeyEvent.KEYCODE_Y: return performContextMenuAction(android.R.id.redo);
                default: return false;
            }
        }
        if (event.getKeyCode() == KeyEvent.KEYCODE_DEL || event.getKeyCode() == KeyEvent.KEYCODE_FORWARD_DEL) return deleteKey(event);
        if (event.getKeyCode() == KeyEvent.KEYCODE_ENTER) {
            if (multiline && (!submitOnEnter || event.isShiftPressed())) return replace("\n",1,false,true);
            // A hardware Enter confirming preedit must not also send the message.
            if (multiline && composeStart >= 0) return finishComposingText();
            return performEditorAction(0);
        }
        int code=event.getUnicodeChar();
        return code != 0 && (code & android.view.KeyCharacterMap.COMBINING_ACCENT) == 0 && replace(new String(Character.toChars(code)),1,false,true);
    }
}
