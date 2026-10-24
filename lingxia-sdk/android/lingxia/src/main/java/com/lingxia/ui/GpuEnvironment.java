package com.lingxia.ui;

import java.math.BigInteger;
import org.json.JSONObject;
import org.json.JSONException;

/** UI-thread host state, retained independently of the module and its bridge connection. */
final class GpuEnvironment {
    private BigInteger languageRevision, surfaceRevision;
    private String language, sizeClass;
    private boolean aside;

    boolean update(JSONObject update) throws JSONException {
        String raw = update.getString("revision");
        if (!raw.matches("[0-9]{1,20}")) throw new IllegalArgumentException("Invalid revision");
        BigInteger revision = new BigInteger(raw);
        if (revision.bitLength() > 64) throw new IllegalArgumentException("Revision overflow");
        switch (update.getString("kind")) {
            case "language": {
                if (languageRevision != null && revision.compareTo(languageRevision) <= 0) return false;
                String next = update.getString("language");
                if (next.isEmpty() || next.getBytes(java.nio.charset.StandardCharsets.UTF_8).length > 128)
                    throw new IllegalArgumentException("Invalid display language");
                language = next;
                languageRevision = revision;
                return true;
            }
            case "surface": {
                if (surfaceRevision != null && revision.compareTo(surfaceRevision) <= 0) return false;
                JSONObject context = update.getJSONObject("context");
                String next = context.getString("sizeClass");
                if (!next.equals("compact") && !next.equals("regular"))
                    throw new IllegalArgumentException("Invalid size class");
                Object nextAside = context.get("aside");
                if (!(nextAside instanceof Boolean)) throw new IllegalArgumentException("Invalid aside");
                sizeClass = next;
                aside = (Boolean) nextAside;
                surfaceRevision = revision;
                return true;
            }
            default: throw new IllegalArgumentException("Unknown environment update");
        }
    }

    JSONObject apply(JSONObject measured) throws JSONException {
        if (language != null) measured.put("displayLanguage", language);
        if (sizeClass != null) measured.put("sizeClass", sizeClass).put("aside", aside);
        return measured;
    }
}
