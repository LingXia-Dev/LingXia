package com.lingxia.ui;

import android.content.Context;
import android.text.InputType;
import android.view.View;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.view.inputmethod.InputMethodManager;
import com.lingxia.ui.generated.GpuInput;
import org.json.JSONArray;
import org.json.JSONObject;
import java.util.function.Consumer;

/** UI-thread focus/IME lifecycle; Host queues all native work without blocking this thread. */
final class GpuIme {
    interface Host {
        void request(JSONObject request, Consumer<JSONObject> reply);
        void render(boolean rebuild);
        default void editingChanged() { }
        /** The focused editor's text state for the host's caret blink; null text when none is connected. */
        default void caret(boolean eligible, String text, int anchor, int focus, int compositionStart, int compositionEnd) { }
        /** The connected editor changed (null when none), for autofill. */
        default void editorChanged(String identity) { }
    }
    static final class Ticket {
        final long epoch, revision;
        final GpuInputConnection connection;
        Ticket(long epoch, GpuInputConnection connection) {
            this.epoch=epoch; this.connection=connection;
            revision=connection == null ? 0 : connection.revision();
        }
    }
    private final View view;
    private final Host host;
    private final InputMethodManager manager;
    private GpuInputConnection connection;
    private InputConnectionWrapper wrapper;
    private JSONObject config;
    /** The core's keyboard configuration for the connected editor (`input.ime`). */
    private JSONObject ime() { JSONObject ime=config == null ? null : config.optJSONObject("ime"); return ime == null ? new JSONObject() : ime; }
    /** The last presented frame, to find the editor a NEXT/PREVIOUS action moved to. */
    private JSONObject frame;
    private android.app.Dialog picker;
    private boolean pendingFocus, allowAutofocus=true;
    private long epoch, touchSerial;
    private boolean caretActive = true;

    GpuIme(View view, Host host) {
        this.view=view; this.host=host;
        manager=(InputMethodManager)view.getContext().getSystemService(Context.INPUT_METHOD_SERVICE);
    }
    static boolean editable(JSONObject node) {
        JSONObject input=node == null ? null : node.optJSONObject("input");
        return input != null && input.optBoolean("enabled",true);
    }
    private static JSONObject request(String op,String identity,long value) {
        try { return new JSONObject().put("op",op).put("identity",identity).put(op.equals("focus") ? "generation" : "token",value); }
        catch (org.json.JSONException impossible) { throw new IllegalArgumentException(impossible); }
    }
    Ticket capture() { return new Ticket(epoch,connection); }
    boolean hasFocus() { return connection != null; }
    void setCaretActive(boolean active) { caretActive=active; if (!active) { touchSerial++; } updateCaret(); }
    boolean menuActive() { return caretActive && connection != null; }
    boolean menuSelected() { return connection != null && connection.anchor() != connection.focus(); }
    boolean menuSecure() { return connection != null && connection.secure; }
    boolean menuUndo() { return menuActive() && connection.canUndo(); }
    boolean menuRedo() { return menuActive() && connection.canRedo(); }
    String menuText() { return connection == null ? "" : connection.getExtractedText(null,0).text.toString(); }
    boolean menuAction(int action) { return menuActive() && connection.performContextMenuAction(action); }
    private void updateCaret() {
        if (connection == null) { host.caret(caretActive,null,0,0,-1,-1); return; }
        host.caret(caretActive,connection.getExtractedText(null,0).text.toString(),connection.anchor(),connection.focus(),connection.composeStart(),connection.composeEnd());
    }
    boolean isActive() { return connection != null || pendingFocus; }
    boolean key(android.view.KeyEvent event) {
        if (connection == null) return false;
        return connection.sendKeyEvent(event);
    }
    void focus(JSONObject node,long generation) {
        focus(node,generation,null);
    }
    void touch(JSONObject node,long generation,float x,float y,boolean word) {
       
        if (!caretActive || !editable(node) || pendingFocus || Float.isNaN(x) || Float.isInfinite(x) || Float.isNaN(y) || Float.isInfinite(y)) return;
        if (connection != null && !connection.settled()) return;
        Ticket ticket=capture(); final long serial=++touchSerial;
        JSONObject message;
        try { message=new JSONObject().put("op","hit_test").put("identity",node.optString("identity"))
            .put("generation",generation).put("point",new JSONArray().put(x).put(y)).put("word",word); }
        catch (org.json.JSONException impossible) { throw new IllegalArgumentException(impossible); }
        host.request(message,reply -> {
            if (serial != touchSerial || ticket.epoch != epoch || ticket.connection != connection
                || (connection != null && (ticket.revision != connection.revision() || !connection.settled()))) return;
            JSONArray selection=reply == null ? null : reply.optJSONArray("selection");
            if (selection == null || selection.length() != 2) return;
            focus(node,generation,new int[]{selection.optInt(0,-1),selection.optInt(1,-1)});
        });
    }
    private void focus(JSONObject node,long generation,int[] selection) {
        focus(node,generation,selection,null);
    }
    enum DragResult { EDITED, UNCHANGED, STALE_FRAME, CANCELLED, SUPERSEDED }
    boolean dragSelection(JSONObject node,long generation,float x,float y,int edgeX,int edgeY,int fixed,boolean collapsed,
            java.util.function.BooleanSupplier valid,Consumer<DragResult> complete) {
        if (!menuActive() || pendingFocus || !connection.settled() || !editable(node)
            || !connection.identity.equals(node.optString("identity"))) return false;
        Ticket ticket=capture(); final long serial=++touchSerial;
        JSONObject message;
        try { message=new JSONObject().put("op","drag_hit_test").put("identity",connection.identity)
            .put("generation",generation).put("token",connection.token).put("point",new JSONArray().put(x).put(y))
            .put("edge",new JSONArray().put(edgeX).put(edgeY)); }
        catch (org.json.JSONException impossible) { throw new IllegalArgumentException(impossible); }
        host.request(message,reply -> {
            if (!caretActive || serial != touchSerial || ticket.epoch != epoch
                || ticket.connection != connection || ticket.revision != connection.revision() || !connection.settled()) {
                complete.accept(DragResult.CANCELLED); return;
            }
            if (!valid.getAsBoolean()) { complete.accept(DragResult.SUPERSEDED); return; }
            JSONArray selection=reply == null ? null : reply.optJSONArray("selection");
            if (selection == null) { complete.accept(DragResult.STALE_FRAME); return; }
            int end=selection.optInt(1,-1), start=collapsed ? end : fixed;
            if (start == connection.anchor() && end == connection.focus() && connection.composeStart() < 0) {
                complete.accept(DragResult.UNCHANGED); return;
            }
            complete.accept(connection.selectFromTouch(start,end) ? DragResult.EDITED : DragResult.CANCELLED);
        });
        return true;
    }
    boolean accessibilityEdit(JSONObject node,long generation,String text,int start,int end) {
        if (!caretActive || !editable(node) || pendingFocus) return false;
        boolean current=connection != null && connection.identity.equals(node.optString("identity"));
        String value=current ? menuText() : node.optJSONObject("input").optString("value");
        if (text == null && start == -1 && end == -1) start=end=current ? connection.focus() : value.length();
        if (text != null ? !GpuInputConnection.validText(text)
            : !GpuInputConnection.boundary(value,start) || !GpuInputConnection.boundary(value,end)) return false;
       
        if (current) return text != null ? replaceAccessibilityText(connection,text) : connection.selectFromTouch(start,end);
        final int anchor=start,focus=end;
        focus(node,generation,null,editor -> {
            if (text != null) editor.replaceAll(text); else editor.selectFromTouch(anchor,focus);
        });
        return true;
    }
    private boolean replaceAccessibilityText(GpuInputConnection editor,String text) {
        if (!editor.replaceAll(text)) return false;
        // The keyboard may still hold composition from before this external replacement.
        invalidateWrapper();
        manager.restartInput(view);
        return true;
    }
    private void focus(JSONObject node,long generation,int[] selection,Consumer<GpuInputConnection> afterFocus) {
        touchSerial++;
        if (!editable(node)) return;
        String identity=node.optString("identity");
        if (connection != null && connection.identity.equals(identity)) {
            if (selection != null) connection.selectFromTouch(selection[0],selection[1]);
            view.requestFocus();
            String kind=pickerKind();
            if (kind.isEmpty()) manager.showSoftInput(view,InputMethodManager.SHOW_IMPLICIT);
            else if (picker == null) showPicker(kind);
            return;
        }
        drop(false); pendingFocus=true;
        final long requested=epoch;
        host.request(request("focus",identity,generation),reply -> {
            if (requested != epoch) {
                if (!pendingFocus && connection == null && reply != null) host.request(request("blur",identity,reply.optLong("token")),ignored -> host.render(false));
                return;
            }
            pendingFocus=false;
            if (reply != null) {
                activate(node,reply.optLong("token"),reply.optJSONObject("state"),editor -> {
                    if (selection != null) editor.selectFromTouch(selection[0],selection[1]);
                    if (afterFocus != null) afterFocus.accept(editor);
                });
            }
            host.render(false);
        });
    }
    void presented(JSONObject frame,Ticket ticket) {
        this.frame=frame;
        if (ticket.epoch != epoch || pendingFocus) return;
        JSONArray nodes=frame.optJSONArray("nodes");
        JSONObject focused=null;
        for (int i=0; nodes != null && i<nodes.length(); i++) {
            JSONObject node=nodes.optJSONObject(i), input=node == null ? null : node.optJSONObject("input");
            if (input != null && input.optLong("edit_token") > 0 && editable(node)) { focused=node; break; }
        }
        if (focused == null) { if (connection != null) drop(true); return; }
        JSONObject input=focused.optJSONObject("input");
        if (connection != null && connection.token == input.optLong("edit_token") && connection.identity.equals(focused.optString("identity"))) {
            if (ticket.connection == connection) connection.acceptFrame(ticket.revision,input.optJSONObject("edit"));
            String previous=ime().toString();
            config=input;
            configure(connection);
            if (!previous.equals(ime().toString())) manager.restartInput(view);
            host.editingChanged();
        } else if (connection != null || allowAutofocus) {
            activate(focused,input.optLong("edit_token"),input.optJSONObject("edit"));
        }
    }
    private void configure(GpuInputConnection editor) {
        JSONObject ime=ime();
        editor.multiline=ime.optBoolean("multiline");
        editor.returnInsertsNewline=ime.optBoolean("return_inserts_newline");
        editor.secure=ime.optBoolean("secure");
        editor.maxLength=ime.isNull("max_length") ? -1 : ime.optInt("max_length",-1);
    }
    private String pickerKind() {
        JSONObject picker=ime().optJSONObject("picker");
        return picker == null ? "" : picker.optString("kind");
    }
    private JSONObject node(String identity) {
        JSONArray nodes=frame == null ? null : frame.optJSONArray("nodes");
        for (int i=0; nodes != null && i<nodes.length(); i++) {
            JSONObject node=nodes.optJSONObject(i);
            if (node != null && identity.equals(node.optString("identity"))) return node;
        }
        return null;
    }
    private void activate(JSONObject node,long token,JSONObject state) {
        activate(node,token,state,null);
    }
    private void activate(JSONObject node,long token,JSONObject state,Consumer<GpuInputConnection> initialize) {
        if (state == null || token <= 0) return;
        drop(false); allowAutofocus=true;
        config=node.optJSONObject("input");
        final long created=epoch;
        connection=new GpuInputConnection(view,node.optString("identity"),token,state,new GpuInputConnection.Sink() {
            @Override public void send(long revision,JSONArray edits,boolean submit) {
                final GpuInputConnection current=connection;
                JSONObject message=request("edit",current.identity,current.token);
                try { message.put("edits",edits).put("submit",submit); }
                catch (org.json.JSONException impossible) { throw new IllegalArgumentException(impossible); }
                host.request(message,reply -> {
                    if (created != epoch || current != connection) return;
                    String moved=reply == null ? null : reply.optString("moved_to",null);
                    if (moved != null) {
                        // NEXT/PREVIOUS: the reply's token and state belong to the editor focus moved to.
                        JSONObject next=node(moved);
                        if (next != null && editable(next)) activate(next,reply.optLong("token"),reply.optJSONObject("state"));
                        else { drop(true); }
                        host.render(reply.optBoolean("needs_view_render"));
                        return;
                    }
                    if (reply == null || reply.optLong("token") != current.token) {
                        drop(true); host.render(false); return;
                    }
                    if (current.acknowledge(revision,reply.optJSONObject("state"))) {
                        invalidateWrapper(); manager.restartInput(view);
                    }
                    if (submit && ime().optBoolean("hide_on_submit")) manager.hideSoftInputFromWindow(view.getWindowToken(),0);
                    host.render(reply.optBoolean("needs_view_render"));
                });
            }
            @Override public void changed() { selectionChanged(); }
        });
        configure(connection);
        // Publish external text/selection before the keyboard can read its initial state.
        if (initialize != null) initialize.accept(connection);
        view.requestFocus(); manager.restartInput(view);
        String kind=pickerKind();
        if (kind.isEmpty()) manager.showSoftInput(view,InputMethodManager.SHOW_IMPLICIT);
        else { manager.hideSoftInputFromWindow(view.getWindowToken(),0); showPicker(kind); }
        host.editorChanged(connection.identity);
        selectionChanged();
    }
    private void selectionChanged() {
        if (connection == null) return;
        updateCaret();
        manager.updateSelection(view,connection.anchor(),connection.focus(),connection.composeStart(),connection.composeEnd());
        if (connection.extractedToken >= 0) manager.updateExtractedText(view,connection.extractedToken,connection.getExtractedText(null,0));
        host.editingChanged();
    }
    private void drop(boolean hide) {
        epoch++; pendingFocus=false;
        if (picker != null) { android.app.Dialog open=picker; picker=null; open.setOnDismissListener(null); open.dismiss(); }
        invalidateWrapper();
        boolean editing = connection != null;
        if (editing) connection.closeConnection();
        connection=null;
        if (editing) host.editorChanged(null);
       
        host.caret(caretActive,null,0,0,-1,-1);
        // Without an editor there is no keyboard to hide; the IME round trip would still relayout the window.
        if (hide && editing) { manager.hideSoftInputFromWindow(view.getWindowToken(),0); manager.restartInput(view); }
    }
    void clear(boolean blur) {
        GpuInputConnection previous=connection;
        drop(true); allowAutofocus=false;
        if (blur && previous != null) host.request(request("blur",previous.identity,previous.token),ignored -> host.render(false));
    }
    void reset() { clear(false); allowAutofocus=true; }
    private BaseInputConnection inert() {
        return new BaseInputConnection(view,true) {
            @Override public android.text.Editable getEditable() { return null; }
            @Override public boolean commitText(CharSequence text,int cursor) { return false; }
            @Override public boolean setComposingText(CharSequence text,int cursor) { return false; }
            @Override public boolean sendKeyEvent(android.view.KeyEvent event) { return false; }
        };
    }
    private void invalidateWrapper() {
        if (wrapper != null) { wrapper.setTarget(inert()); wrapper=null; }
    }
    /** The IME action for the core's return key. */
    static int action(String returnKey) {
        switch (returnKey) {
            case "done": return EditorInfo.IME_ACTION_DONE;
            case "go": return EditorInfo.IME_ACTION_GO;
            case "next": return EditorInfo.IME_ACTION_NEXT;
            case "previous": return EditorInfo.IME_ACTION_PREVIOUS;
            case "search": return EditorInfo.IME_ACTION_SEARCH;
            case "send": return EditorInfo.IME_ACTION_SEND;
            default: return EditorInfo.IME_ACTION_NONE;
        }
    }
    /** EditorInfo input type for the core's keyboard configuration. */
    static int inputType(JSONObject ime) {
        String keyboard=ime.optString("keyboard"); boolean secure=ime.optBoolean("secure");
        int type;
        switch (keyboard) {
            case "number": type=secure ? InputType.TYPE_CLASS_NUMBER|InputType.TYPE_NUMBER_VARIATION_PASSWORD
                : InputType.TYPE_CLASS_NUMBER|InputType.TYPE_NUMBER_FLAG_DECIMAL|InputType.TYPE_NUMBER_FLAG_SIGNED; break;
            case "phone": type=InputType.TYPE_CLASS_PHONE; break;
            case "email": type=InputType.TYPE_CLASS_TEXT|(secure ? InputType.TYPE_TEXT_VARIATION_PASSWORD : InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS); break;
            default: type=InputType.TYPE_CLASS_TEXT|(secure ? InputType.TYPE_TEXT_VARIATION_PASSWORD : 0); break;
        }
        if ((type & InputType.TYPE_MASK_CLASS) != InputType.TYPE_CLASS_TEXT) return type;
        switch (ime.optString("capitalize")) {
            case "sentences": type|=InputType.TYPE_TEXT_FLAG_CAP_SENTENCES; break;
            case "words": type|=InputType.TYPE_TEXT_FLAG_CAP_WORDS; break;
            case "characters": type|=InputType.TYPE_TEXT_FLAG_CAP_CHARACTERS; break;
            default: break;
        }
        if (!ime.optBoolean("suggestions",true)) type|=InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS;
        if (ime.optBoolean("multiline")) type|=InputType.TYPE_TEXT_FLAG_MULTI_LINE;
        return type;
    }
    InputConnection create(EditorInfo info) {
        // A picker editor takes its value from a dialog, never a keyboard.
        if (connection == null || !pickerKind().isEmpty()) return null;
        JSONObject ime=ime();
        info.inputType=inputType(ime);
        info.imeOptions=EditorInfo.IME_FLAG_NO_EXTRACT_UI | action(ime.optString("return_key"));
        if (!ime.optBoolean("suggestions",true) && android.os.Build.VERSION.SDK_INT >= 26) info.imeOptions|=EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING;
        info.initialSelStart=connection.anchor(); info.initialSelEnd=connection.focus();
        // Android may close an old wrapper after asking for its replacement.
        invalidateWrapper();
        wrapper=new InputConnectionWrapper(connection,true) {
            @Override public void closeConnection() {
                setTarget(inert());
            }
        };
        return wrapper;
    }
    /** Date or time from a platform dialog, seeded from the value and bounded by min/max. */
    private void showPicker(String kind) {
        final GpuInputConnection editor=connection;
        // Pages are built on the application context; dialogs need the window's.
        Context context=view.isAttachedToWindow() ? view.getRootView().getContext() : view.getContext();
        String value=editor.getExtractedText(null,0).text.toString();
        JSONObject bounds=ime().optJSONObject("picker");
        String min=bounds == null || bounds.isNull("min") ? "" : bounds.optString("min");
        String max=bounds == null || bounds.isNull("max") ? "" : bounds.optString("max");
        android.app.Dialog dialog;
        if (GpuInput.KEYBOARD_DATE.equals(kind)) {
            java.util.Calendar seed=java.util.Calendar.getInstance();
            int[] parsed=date(value);
            if (parsed != null) seed.set(parsed[0],parsed[1]-1,parsed[2]);
            android.app.DatePickerDialog picked=new android.app.DatePickerDialog(context,(widget,year,month,day) ->
                confirm(editor,String.format(java.util.Locale.ROOT,"%04d-%02d-%02d",year,month+1,day)),
                seed.get(java.util.Calendar.YEAR),seed.get(java.util.Calendar.MONTH),seed.get(java.util.Calendar.DAY_OF_MONTH));
            Long low=millis(min), high=millis(max);
            if (low != null) picked.getDatePicker().setMinDate(low);
            if (high != null) picked.getDatePicker().setMaxDate(high);
            dialog=picked;
        } else {
            int[] parsed=time(value);
            java.util.Calendar now=java.util.Calendar.getInstance();
            dialog=new android.app.TimePickerDialog(context,(widget,hour,minute) ->
                confirm(editor,String.format(java.util.Locale.ROOT,"%02d:%02d",hour,minute)), parsed != null ? parsed[0] : now.get(java.util.Calendar.HOUR_OF_DAY), parsed != null ? parsed[1] : now.get(java.util.Calendar.MINUTE),
                android.text.format.DateFormat.is24HourFormat(context));
        }
        // Cancel, back or outside touch blur the editor; confirm submits first.
        dialog.setOnDismissListener(ignored -> { if (picker == dialog) { picker=null; if (connection == editor) clear(true); } });
        picker=dialog;
        dialog.show();
    }
    private void confirm(GpuInputConnection editor,String value) {
        if (connection != editor) return;
        editor.submitValue(value);
    }
    static int[] date(String value) {
        if (value == null || !value.matches("\\d{4}-\\d{2}-\\d{2}")) return null;
        return new int[]{Integer.parseInt(value.substring(0,4)),Integer.parseInt(value.substring(5,7)),Integer.parseInt(value.substring(8,10))};
    }
    static int[] time(String value) {
        if (value == null || !value.matches("\\d{2}:\\d{2}(:\\d{2})?")) return null;
        return new int[]{Integer.parseInt(value.substring(0,2)),Integer.parseInt(value.substring(3,5))};
    }
    private static Long millis(String value) {
        int[] parsed=date(value);
        if (parsed == null) return null;
        java.util.Calendar day=java.util.Calendar.getInstance();
        day.clear(); day.set(parsed[0],parsed[1]-1,parsed[2]);
        return day.getTimeInMillis();
    }
}
