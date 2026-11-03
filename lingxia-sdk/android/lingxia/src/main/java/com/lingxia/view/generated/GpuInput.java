// Generated from packages/lingxia-view/contracts/schema.json; do not edit.
package com.lingxia.view.generated;

/** Editor keyboards, IME actions, capitalization, autofill hints and snapshot keys. */
public final class GpuInput {
    private GpuInput() {}

    public static final String KEYBOARD_TEXT = "text";
    public static final String KEYBOARD_NUMBER = "number";
    public static final String KEYBOARD_EMAIL = "email";
    public static final String KEYBOARD_PHONE = "phone";
    public static final String KEYBOARD_SEARCH = "search";
    public static final String KEYBOARD_DATE = "date";
    public static final String KEYBOARD_TIME = "time";
    public static final String ENTER_ENTER = "enter";
    public static final String ENTER_DONE = "done";
    public static final String ENTER_GO = "go";
    public static final String ENTER_NEXT = "next";
    public static final String ENTER_PREVIOUS = "previous";
    public static final String ENTER_SEARCH = "search";
    public static final String ENTER_SEND = "send";
    public static final String CAPITALIZE_NONE = "none";
    public static final String CAPITALIZE_SENTENCES = "sentences";
    public static final String CAPITALIZE_WORDS = "words";
    public static final String CAPITALIZE_CHARACTERS = "characters";
    public static final String KEY_MAX_LENGTH = "max_length";
    public static final String KEY_AUTO_COMPLETE = "auto_complete";
    public static final String KEY_ENTER_KEY_HINT = "enter_key_hint";
    public static final String KEY_AUTO_CAPITALIZE = "auto_capitalize";
    public static final String KEY_SPELL_CHECK = "spell_check";
    public static final String KEY_MIN = "min";
    public static final String KEY_MAX = "max";
    public static final String KEY_STEP = "step";
    public static final String KEY_PICKER = "picker";

    /** The core's editor limits; an input mirror holds itself to them. */
    public static final int LIMIT_EDITOR_TEXT_BYTES = 65536;
    public static final int LIMIT_INPUT_REQUEST_BYTES = 524288;
    public static final int LIMIT_INPUT_BATCH_EDITS = 64;
    public static final int LIMIT_CLIPBOARD_ITEMS = 128;

    /** The Android autofill hint for an HTML autocomplete token, or null. */
    public static String autofillHint(String token) {
        switch (token) {
            case "username": return "username";
            case "current-password": return "password";
            case "new-password": return "newPassword";
            case "one-time-code": return "smsOTPCode";
            case "email": return "emailAddress";
            case "tel": return "phone";
            case "name": return "personName";
            case "given-name": return "personGivenName";
            case "family-name": return "personFamilyName";
            case "street-address": return "postalAddress";
            case "postal-code": return "postalCode";
            case "country": return "addressCountry";
            case "cc-number": return "creditCardNumber";
            case "cc-exp": return "creditCardExpirationDate";
            case "cc-csc": return "creditCardSecurityCode";
            case "bday": return "birthDateFull";
            default: return null;
        }
    }
}
