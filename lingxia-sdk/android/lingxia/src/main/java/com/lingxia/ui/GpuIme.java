package com.lingxia.ui;

import android.content.Context;
import android.text.InputType;
import android.view.View;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.view.inputmethod.InputMethodManager;
import com.lingxia.ui.generated.GpuFrame;
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
    private GpuFrame.Ime ime() { GpuFrame.Ime ime=config == null ? null : new GpuFrame.Input(config).ime(); return ime == null ? new GpuFrame.Ime(new JSONObject()) : ime; }
    /** The last presented frame, to find the editor a NEXT/PREVIOUS action moved to. */
    private JSONObject frame;
    private android.app.Dialog picker;
    private boolean pendingFocus;
    /** The edit token this side last blurred: a frame drawn before the core saw the blur still shows it focused. */
    private long blurred=-1;
    private long epoch, touchSerial;
    private boolean caretActive = true;

    GpuIme(View view, Host host) {
        this.view=view; this.host=host;
        manager=(InputMethodManager)view.getContext().getSystemService(Context.INPUT_METHOD_SERVICE);
    }
    static boolean editable(JSONObject node) {
        GpuFrame.Input input=node == null ? null : new GpuFrame.Node(node).input();
        return input != null && input.enabled();
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
        try { message=new JSONObject().put("op","hit_test").put("identity",identity(node))
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
            || !connection.identity.equals(identity(node))) return false;
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
    /** `GpuPlatform.editText` codes: the core resends a not-ready edit and fails its caller on a failed one. */
    static final int EDIT_APPLIED=0, EDIT_NOT_READY=1, EDIT_FAILED=2;
    int accessibilityEdit(JSONObject node,long generation,String text,int start,int end) {
        if (!editable(node)) return EDIT_FAILED;
        if (!caretActive || pendingFocus) return EDIT_NOT_READY;
        boolean current=connection != null && connection.identity.equals(identity(node));
        String value=current ? menuText() : new GpuFrame.Node(node).input().value();
        if (value == null) value="";
        if (text == null && start == -1 && end == -1) start=end=current ? connection.focus() : value.length();
        if (text != null ? !GpuInputConnection.validText(text)
            : !GpuInputConnection.boundary(value,start) || !GpuInputConnection.boundary(value,end)) return EDIT_FAILED;
        if (current) {
            // The mirror refuses only while its edit queue is full.
            boolean applied=text != null ? replaceAccessibilityText(connection,text) : connection.selectFromTouch(start,end);
            return applied ? EDIT_APPLIED : EDIT_NOT_READY;
        }
        final int anchor=start,focus=end;
        focus(node,generation,null,editor -> {
            if (text != null) editor.replaceAll(text); else editor.selectFromTouch(anchor,focus);
        });
        return EDIT_APPLIED;
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
        String identity=identity(node);
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
                if (!pendingFocus && connection == null && reply != null) {
                    blurred=reply.optLong("token");
                    host.request(request("blur",identity,blurred),ignored -> host.render(false));
                }
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
        GpuFrame.Node[] nodes=new GpuFrame.FrameSnapshot(frame).nodes();
        GpuFrame.Node focused=null;
        for (int i=0; nodes != null && i<nodes.length; i++) {
            GpuFrame.Input input=nodes[i].input();
            Long token=input == null ? null : input.editToken();
            if (token != null && token > 0 && editable(nodes[i].json)) { focused=nodes[i]; break; }
        }
        if (focused == null) { if (connection != null) drop(true); return; }
        GpuFrame.Input input=focused.input();
        long token=input.editToken();
        GpuFrame.Edit edit=input.edit();
        JSONObject editState=edit == null ? null : edit.json;
        if (connection != null && connection.token == token && connection.identity.equals(focused.identity())) {
            if (ticket.connection == connection) connection.acceptFrame(ticket.revision,editState);
            String previous=ime().json.toString();
            config=input.json;
            configure(connection);
            if (!previous.equals(ime().json.toString())) manager.restartInput(view);
            host.editingChanged();
        } else if (connection != null || (input.raiseKeyboard() && token != blurred)) {
            activate(focused.json,token,editState);
        }
    }
    private void configure(GpuInputConnection editor) {
        GpuFrame.Ime ime=ime();
        editor.multiline=ime.multiline();
        editor.returnInsertsNewline=ime.returnInsertsNewline();
        editor.secure=ime.secure();
        Long maxLength=ime.maxLength();
        editor.maxLength=maxLength == null ? -1 : maxLength.intValue();
    }
    private String pickerKind() {
        GpuFrame.Picker picker=ime().picker();
        return picker == null || picker.kind() == null ? "" : picker.kind();
    }
    private JSONObject node(String identity) {
        GpuFrame.Node[] nodes=frame == null ? null : new GpuFrame.FrameSnapshot(frame).nodes();
        for (int i=0; nodes != null && i<nodes.length; i++)
            if (identity.equals(nodes[i].identity())) return nodes[i].json;
        return null;
    }
    private void activate(JSONObject node,long token,JSONObject state) {
        activate(node,token,state,null);
    }
    private void activate(JSONObject node,long token,JSONObject state,Consumer<GpuInputConnection> initialize) {
        if (state == null || token <= 0) return;
        drop(false);
        GpuFrame.Input input=new GpuFrame.Node(node).input();
        config=input == null ? null : input.json;
        final long created=epoch;
        connection=new GpuInputConnection(view,identity(node),token,state,new GpuInputConnection.Sink() {
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
                    if (submit && ime().hideOnSubmit()) manager.hideSoftInputFromWindow(view.getWindowToken(),0);
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
        drop(true);
        if (blur && previous != null) {
            blurred=previous.token;
            host.request(request("blur",previous.identity,previous.token),ignored -> host.render(false));
        }
    }
    void reset() { clear(false); blurred=-1; }
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
    static int inputType(GpuFrame.Ime ime) {
        if (ime == null) ime=new GpuFrame.Ime(new JSONObject());
        String keyboard=ime.keyboard() == null ? "" : ime.keyboard(); boolean secure=ime.secure();
        int type;
        switch (keyboard) {
            case "number": type=secure ? InputType.TYPE_CLASS_NUMBER|InputType.TYPE_NUMBER_VARIATION_PASSWORD
                : InputType.TYPE_CLASS_NUMBER|InputType.TYPE_NUMBER_FLAG_DECIMAL|InputType.TYPE_NUMBER_FLAG_SIGNED; break;
            case "phone": type=InputType.TYPE_CLASS_PHONE; break;
            case "email": type=InputType.TYPE_CLASS_TEXT|(secure ? InputType.TYPE_TEXT_VARIATION_PASSWORD : InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS); break;
            default: type=InputType.TYPE_CLASS_TEXT|(secure ? InputType.TYPE_TEXT_VARIATION_PASSWORD : 0); break;
        }
        if ((type & InputType.TYPE_MASK_CLASS) != InputType.TYPE_CLASS_TEXT) return type;
        switch (ime.capitalize() == null ? "" : ime.capitalize()) {
            case "sentences": type|=InputType.TYPE_TEXT_FLAG_CAP_SENTENCES; break;
            case "words": type|=InputType.TYPE_TEXT_FLAG_CAP_WORDS; break;
            case "characters": type|=InputType.TYPE_TEXT_FLAG_CAP_CHARACTERS; break;
            default: break;
        }
        if (!suggestions(ime)) type|=InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS;
        if (ime.multiline()) type|=InputType.TYPE_TEXT_FLAG_MULTI_LINE;
        return type;
    }
    InputConnection create(EditorInfo info) {
        // A picker editor takes its value from a dialog, never a keyboard.
        if (connection == null || !pickerKind().isEmpty()) return null;
        GpuFrame.Ime ime=ime();
        info.inputType=inputType(ime);
        info.imeOptions=EditorInfo.IME_FLAG_NO_EXTRACT_UI | action(ime.returnKey() == null ? "" : ime.returnKey());
        if (!suggestions(ime) && android.os.Build.VERSION.SDK_INT >= 26) info.imeOptions|=EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING;
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
        // The core seeds the dialog and bounds it, as [year, month, day] or [hour, minute, 0].
        GpuFrame.Picker spec=ime().picker();
        int[] seed=parts(spec == null ? null : spec.seed()), low=parts(spec == null ? null : spec.minParts()),
            high=parts(spec == null ? null : spec.maxParts());
        java.util.Calendar now=java.util.Calendar.getInstance();
        boolean[] chosen={false};
        android.app.Dialog dialog;
        if (GpuInput.KEYBOARD_DATE.equals(kind)) {
            android.app.DatePickerDialog picked=new android.app.DatePickerDialog(context,(widget,year,month,day) -> {
                chosen[0]=true; pick(editor,new int[]{year,month+1,day});
            }, seed != null ? seed[0] : now.get(java.util.Calendar.YEAR), seed != null ? seed[1]-1 : now.get(java.util.Calendar.MONTH),
                seed != null ? seed[2] : now.get(java.util.Calendar.DAY_OF_MONTH));
            if (low != null) picked.getDatePicker().setMinDate(millis(low));
            if (high != null) picked.getDatePicker().setMaxDate(millis(high));
            dialog=picked;
        } else {
            dialog=new android.app.TimePickerDialog(context,(widget,hour,minute) -> {
                chosen[0]=true; pick(editor,new int[]{hour,minute,0});
            }, seed != null ? seed[0] : now.get(java.util.Calendar.HOUR_OF_DAY), seed != null ? seed[1] : now.get(java.util.Calendar.MINUTE),
                android.text.format.DateFormat.is24HourFormat(context));
        }
        // A dismissal without a choice tells the core, which ends editing.
        dialog.setOnDismissListener(ignored -> {
            if (picker != dialog) return;
            picker=null;
            if (!chosen[0] && connection == editor) pick(editor,null);
        });
        picker=dialog;
        dialog.show();
    }
    /** Send a choice (or a dismissal, null) to the core; submit the value it answers. */
    private void pick(GpuInputConnection editor,int[] parts) {
        if (connection != editor) return;
        JSONObject message=request("pick",editor.identity,editor.token);
        try { message.put("parts",parts == null ? JSONObject.NULL : new JSONArray(parts)); }
        catch (org.json.JSONException impossible) { throw new IllegalArgumentException(impossible); }
        host.request(message,reply -> {
            if (connection != editor || reply == null) return;
            if (!reply.isNull("value")) editor.submitValue(reply.optString("value"));
            else host.render(reply.optBoolean("needs_view_render"));
        });
    }
    private static int[] parts(long[] value) {
        return value == null || value.length != 3 ? null : new int[]{(int)value[0],(int)value[1],(int)value[2]};
    }
    /** An editor offers suggestions unless the core turns them off; a missing config keeps them. */
    private static boolean suggestions(GpuFrame.Ime ime) { return ime.json.length() == 0 || ime.suggestions(); }
    private static String identity(JSONObject node) { return String.valueOf(new GpuFrame.Node(node).identity()); }
    private static long millis(int[] date) {
        java.util.Calendar day=java.util.Calendar.getInstance();
        day.clear(); day.set(date[0],date[1]-1,date[2]);
        return day.getTimeInMillis();
    }
}
