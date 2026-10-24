package com.lingxia.ui;

import android.content.Context;
import android.text.InputType;
import android.view.View;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.view.inputmethod.InputMethodManager;
import org.json.JSONArray;
import org.json.JSONObject;
import java.util.function.Consumer;

/** UI-thread focus/IME lifecycle; Host queues all native work without blocking this thread. */
final class GpuIme {
    interface Host {
        void request(JSONObject request, Consumer<JSONObject> reply);
        void render(boolean rebuild);
        default void showTextMenu() { }
        default void hideTextMenu() { }
        default void editingChanged() { }
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
    private boolean multiline, pendingFocus, allowAutofocus=true;
    private long epoch, touchSerial;
    private final GpuCaretBlink caret;
    private boolean caretActive = true;

    GpuIme(View view, Host host) {
        this.view=view; this.host=host;
        caret=new GpuCaretBlink(() -> host.render(false));
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
    boolean caretVisible() { return caret.visible(); }
    void setCaretActive(boolean active) { caretActive=active; if (!active) { touchSerial++; host.hideTextMenu(); } updateCaret(); }
    boolean menuActive() { return caretActive && connection != null; }
    boolean menuSelected() { return connection != null && connection.anchor() != connection.focus(); }
    boolean menuSecure() { return connection != null && connection.secure; }
    boolean menuUndo() { return menuActive() && connection.canUndo(); }
    boolean menuRedo() { return menuActive() && connection.canRedo(); }
    String menuText() { return connection == null ? "" : connection.getExtractedText(null,0).text.toString(); }
    boolean menuAction(int action) { return menuActive() && connection.performContextMenuAction(action); }
    private void updateCaret() {
        if (connection == null) { caret.stop(); return; }
        caret.update(caretActive,connection.getExtractedText(null,0).text.toString(),connection.anchor(),connection.focus(),connection.composeStart(),connection.composeEnd());
    }
    boolean isActive() { return connection != null || pendingFocus; }
    boolean key(android.view.KeyEvent event) {
        if (connection == null) return false;
        return connection.sendKeyEvent(event);
    }
    void focus(JSONObject node,long generation) {
        focus(node,generation,null,false);
    }
    void touch(JSONObject node,long generation,float x,float y,boolean word) {
        host.hideTextMenu();
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
            focus(node,generation,new int[]{selection.optInt(0,-1),selection.optInt(1,-1)},word);
        });
    }
    private void focus(JSONObject node,long generation,int[] selection,boolean showMenu) {
        focus(node,generation,selection,showMenu,null);
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
        if (connection != null && !connection.settled()) return false;
        String value=current ? menuText() : node.optJSONObject("input").optString("value");
        if (text == null && start == -1 && end == -1) start=end=current ? connection.focus() : value.length();
        if (text != null ? !GpuInputConnection.validText(text)
            : !GpuInputConnection.boundary(value,start) || !GpuInputConnection.boundary(value,end)) return false;
        host.hideTextMenu();
        if (current) return text != null ? replaceAccessibilityText(connection,text) : connection.selectFromTouch(start,end);
        final int anchor=start,focus=end;
        focus(node,generation,null,false,editor -> {
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
    private void focus(JSONObject node,long generation,int[] selection,boolean showMenu,Consumer<GpuInputConnection> afterFocus) {
        touchSerial++;
        if (!editable(node)) return;
        String identity=node.optString("identity");
        if (connection != null && connection.identity.equals(identity)) {
            if (selection != null && connection.selectFromTouch(selection[0],selection[1]) && showMenu) host.showTextMenu();
            view.requestFocus(); manager.showSoftInput(view,InputMethodManager.SHOW_IMPLICIT); return;
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
                    if (selection != null && editor.selectFromTouch(selection[0],selection[1]) && showMenu) host.showTextMenu();
                    if (afterFocus != null) afterFocus.accept(editor);
                });
            }
            host.render(false);
        });
    }
    void presented(JSONObject frame,Ticket ticket) {
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
            String previous=configurationKey();
            config=input; multiline="textarea".equals(focused.optString("tag"));
            connection.multiline=multiline;
            connection.submitOnEnter=config.optBoolean("submit_on_enter");
            connection.secure=input.optBoolean("secure");
            if (!previous.equals(configurationKey())) manager.restartInput(view);
            host.editingChanged();
        } else if (connection != null || allowAutofocus) {
            activate(focused,input.optLong("edit_token"),input.optJSONObject("edit"));
        }
    }
    private String configurationKey() { return config.optString("keyboard")+":"+config.optBoolean("secure")+":"+multiline+":"+config.optBoolean("submit_on_enter"); }
    private void activate(JSONObject node,long token,JSONObject state) {
        activate(node,token,state,null);
    }
    private void activate(JSONObject node,long token,JSONObject state,Consumer<GpuInputConnection> initialize) {
        if (state == null || token <= 0) return;
        drop(false); allowAutofocus=true;
        config=node.optJSONObject("input"); multiline="textarea".equals(node.optString("tag"));
        final long created=epoch;
        connection=new GpuInputConnection(view,node.optString("identity"),token,state,new GpuInputConnection.Sink() {
            @Override public void send(long revision,JSONArray edits,boolean submit) {
                final GpuInputConnection current=connection;
                JSONObject message=request("edit",current.identity,current.token);
                try { message.put("edits",edits).put("submit",submit); }
                catch (org.json.JSONException impossible) { throw new IllegalArgumentException(impossible); }
                host.request(message,reply -> {
                    if (created != epoch || current != connection) return;
                    if (reply == null || reply.optLong("token") != current.token) {
                        drop(true); host.render(false); return;
                    }
                    if (current.acknowledge(revision,reply.optJSONObject("state"))) {
                        invalidateWrapper(); manager.restartInput(view);
                    }
                    if (submit && !multiline && !"search".equals(config.optString("keyboard"))) manager.hideSoftInputFromWindow(view.getWindowToken(),0);
                    host.render(reply.optBoolean("needs_view_render"));
                });
            }
            @Override public void changed() { selectionChanged(); }
        });
        connection.multiline=multiline;
        connection.submitOnEnter=config.optBoolean("submit_on_enter");
        connection.secure=config.optBoolean("secure");
        // Publish external text/selection before the keyboard can read its initial state.
        if (initialize != null) initialize.accept(connection);
        view.requestFocus(); manager.restartInput(view);
        manager.showSoftInput(view,InputMethodManager.SHOW_IMPLICIT);
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
        invalidateWrapper();
        if (connection != null) connection.closeConnection();
        connection=null;
        host.hideTextMenu();
        caret.stop();
        if (hide) { manager.hideSoftInputFromWindow(view.getWindowToken(),0); manager.restartInput(view); }
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
    InputConnection create(EditorInfo info) {
        if (connection == null) return null;
        String keyboard=config.optString("keyboard"); boolean secure=config.optBoolean("secure");
        int type=InputType.TYPE_CLASS_TEXT;
        if (keyboard.equals("number")) type=InputType.TYPE_CLASS_NUMBER|InputType.TYPE_NUMBER_FLAG_DECIMAL|InputType.TYPE_NUMBER_FLAG_SIGNED;
        else if (keyboard.equals("phone")) type=InputType.TYPE_CLASS_PHONE;
        else if (keyboard.equals("email")) type|=InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS;
        if (secure) type=keyboard.equals("number") ? InputType.TYPE_CLASS_NUMBER|InputType.TYPE_NUMBER_VARIATION_PASSWORD : InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_VARIATION_PASSWORD;
        if (multiline) type|=InputType.TYPE_TEXT_FLAG_MULTI_LINE;
        info.inputType=type;
        info.imeOptions=EditorInfo.IME_FLAG_NO_EXTRACT_UI | (multiline ? (config.optBoolean("submit_on_enter") ? EditorInfo.IME_ACTION_SEND : EditorInfo.IME_ACTION_NONE) : keyboard.equals("search") ? EditorInfo.IME_ACTION_SEARCH : EditorInfo.IME_ACTION_DONE);
        if (secure && android.os.Build.VERSION.SDK_INT >= 26) info.imeOptions|=EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING;
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
}
