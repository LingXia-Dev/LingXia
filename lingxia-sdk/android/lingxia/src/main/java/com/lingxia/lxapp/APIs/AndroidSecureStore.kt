package com.lingxia.lxapp.APIs

import android.content.Context
import android.content.SharedPreferences
import android.os.Build
import android.security.KeyPairGeneratorSpec
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.util.Log
import com.lingxia.app.LxApp
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.security.GeneralSecurityException
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.MessageDigest
import java.security.SecureRandom
import java.util.Calendar
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec
import javax.security.auth.x500.X500Principal

internal object AndroidSecureStore {
    private const val TAG = "LingXia.Device"
    private const val SECURE_STORE_PREFS = "lingxia_secure_store"
    private const val SECURE_STORE_ENTRY_PREFIX = "entry."
    private const val SECURE_STORE_WRAPPED_KEY_PREF = "__wrapped_master_key_v1"
    private const val SECURE_STORE_BLOB_VERSION = 1
    private const val SECURE_STORE_GCM_IV_BYTES = 12
    private const val SECURE_STORE_TAG_BITS = 128
    private const val ANDROID_KEYSTORE = "AndroidKeyStore"
    private const val AES_TRANSFORMATION = "AES/GCM/NoPadding"
    private const val RSA_TRANSFORMATION = "RSA/ECB/PKCS1Padding"

    private val lock = Any()
    private val secureRandom = SecureRandom()

    // In-process master key cache. Reads/writes are serialized via `lock`,
    // so the cache itself doesn't need extra synchronization beyond visibility.
    // Caching avoids re-unwrapping on every encrypt/decrypt — which on API < 23
    // also suppresses the keystore daemon's CACERT_<alias> warning spam (the
    // wrapped key is RSA self-signed and has no CA cert slot to read).
    @Volatile private var cachedMasterKey: SecretKey? = null

    fun readValueBase64(storageKey: String): String? {
        val normalizedKey = normalizeKey(storageKey)
        val context = requireContext()
        synchronized(lock) {
            return read(context, normalizedKey)?.let {
                Base64.encodeToString(it, Base64.NO_WRAP)
            }
        }
    }

    fun writeValueBase64(storageKey: String, valueBase64: String) {
        val normalizedKey = normalizeKey(storageKey)
        val context = requireContext()
        val decoded = try {
            Base64.decode(valueBase64, Base64.DEFAULT)
        } catch (e: IllegalArgumentException) {
            throw IllegalArgumentException("Invalid base64 payload for secure store", e)
        }

        synchronized(lock) {
            write(context, normalizedKey, decoded)
        }
    }

    fun deleteValue(storageKey: String) {
        val normalizedKey = normalizeKey(storageKey)
        val context = requireContext()
        synchronized(lock) {
            delete(context, normalizedKey)
        }
    }

    private fun requireContext(): Context {
        return LxApp.applicationContext()
            ?: LxApp.getCurrentActivity()
            ?: throw IllegalStateException("Secure store unavailable: no Android context")
    }

    private fun normalizeKey(storageKey: String): String {
        require(storageKey.isNotEmpty()) { "Secure store key must not be empty" }
        return storageKey
    }

    private fun read(context: Context, storageKey: String): ByteArray? {
        val prefs = prefs(context)
        val entryKey = entryPrefKey(storageKey)
        val encoded = prefs.getString(entryKey, null) ?: return null
        val blob = EncryptedBlob.decode(encoded) ?: return pruneUnreadableEntry(
            prefs,
            entryKey,
            "Secure store blob malformed"
        )

        return try {
            decrypt(context, prefs, storageKey, blob)
        } catch (e: GeneralSecurityException) {
            pruneUnreadableEntry(prefs, entryKey, "Secure store read failed", e)
        } catch (e: IllegalArgumentException) {
            pruneUnreadableEntry(prefs, entryKey, "Secure store read failed", e)
        }
    }

    private fun write(context: Context, storageKey: String, value: ByteArray) {
        val prefs = prefs(context)
        val blob = encrypt(context, prefs, storageKey, value)
        if (!prefs.edit().putString(entryPrefKey(storageKey), blob.encode()).commit()) {
            throw IllegalStateException("Failed to persist secure store entry")
        }
    }

    private fun delete(context: Context, storageKey: String) {
        val prefs = prefs(context)
        val entryKey = entryPrefKey(storageKey)
        if (prefs.contains(entryKey) && !prefs.edit().remove(entryKey).commit()) {
            throw IllegalStateException("Failed to delete secure store entry")
        }
    }

    private fun pruneUnreadableEntry(
        prefs: SharedPreferences,
        entryKey: String,
        message: String,
        error: Exception? = null
    ): ByteArray? {
        if (error == null) {
            Log.w(TAG, "$message for key hash $entryKey")
        } else {
            Log.w(TAG, "$message for key hash $entryKey", error)
        }
        prefs.edit().remove(entryKey).apply()
        return null
    }

    private fun encrypt(
        context: Context,
        prefs: SharedPreferences,
        storageKey: String,
        value: ByteArray
    ): EncryptedBlob {
        val key = getOrCreateMasterKey(context, prefs)
        val cipher = Cipher.getInstance(AES_TRANSFORMATION)
        val iv: ByteArray
        if (key is SecretKeySpec) {
            // Software key (legacy path): generate our own IV to avoid
            // vendor-specific Cipher IV quirks on some Android ROMs.
            iv = ByteArray(SECURE_STORE_GCM_IV_BYTES)
            secureRandom.nextBytes(iv)
            cipher.init(Cipher.ENCRYPT_MODE, key, GCMParameterSpec(SECURE_STORE_TAG_BITS, iv))
        } else {
            // Hardware-backed key: AndroidKeyStore generates the IV.
            // Read it immediately after init — some implementations
            // clear or corrupt it after doFinal().
            cipher.init(Cipher.ENCRYPT_MODE, key)
            val cipherIv = cipher.iv
            require(cipherIv != null && cipherIv.size == SECURE_STORE_GCM_IV_BYTES) {
                "Unexpected IV length for secure store: ${cipherIv?.size}"
            }
            iv = cipherIv
        }
        cipher.updateAAD(storageKey.toByteArray(StandardCharsets.UTF_8))
        val ciphertext = cipher.doFinal(value)
        return EncryptedBlob(SECURE_STORE_BLOB_VERSION, iv, ciphertext)
    }

    private fun decrypt(
        context: Context,
        prefs: SharedPreferences,
        storageKey: String,
        blob: EncryptedBlob
    ): ByteArray {
        require(blob.version == SECURE_STORE_BLOB_VERSION) {
            "Unsupported secure store blob version: ${blob.version}"
        }
        val cipher = Cipher.getInstance(AES_TRANSFORMATION)
        cipher.init(
            Cipher.DECRYPT_MODE,
            getOrCreateMasterKey(context, prefs),
            GCMParameterSpec(SECURE_STORE_TAG_BITS, blob.iv)
        )
        cipher.updateAAD(storageKey.toByteArray(StandardCharsets.UTF_8))
        return cipher.doFinal(blob.ciphertext)
    }

    private fun getOrCreateMasterKey(context: Context, prefs: SharedPreferences): SecretKey {
        cachedMasterKey?.let { return it }
        val key = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            getOrCreateModernMasterKey(context, prefs)
        } else {
            getOrCreateLegacyMasterKey(context, prefs)
        }
        cachedMasterKey = key
        return key
    }

    private fun getOrCreateModernMasterKey(
        context: Context,
        prefs: SharedPreferences
    ): SecretKey {
        val alias = masterKeyAlias(context)
        val keyStore = loadKeyStore()
        if (!keyStore.containsAlias(alias)) {
            clearPrefsIfRestoredWithoutKeys(prefs, "AndroidKeyStore alias missing")
            generateModernAesKey(alias)
        }

        val entry = keyStore.getEntry(alias, null) as? KeyStore.SecretKeyEntry
            ?: throw IllegalStateException("AndroidKeyStore secret key missing for alias $alias")
        return entry.secretKey
    }

    private fun generateModernAesKey(alias: String) {
        val keyGenerator =
            KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)

        fun buildSpec(keySize: Int): KeyGenParameterSpec {
            return KeyGenParameterSpec.Builder(
                alias,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true)
                .setKeySize(keySize)
                .build()
        }

        try {
            keyGenerator.init(buildSpec(256))
        } catch (e: Exception) {
            Log.w(TAG, "Falling back to 128-bit AndroidKeyStore AES key", e)
            keyGenerator.init(buildSpec(128))
        }
        keyGenerator.generateKey()
    }

    private fun getOrCreateLegacyMasterKey(
        context: Context,
        prefs: SharedPreferences
    ): SecretKey {
        val alias = legacyKeyAlias(context)
        val keyStore = loadKeyStore()
        if (!keyStore.containsAlias(alias)) {
            clearPrefsIfRestoredWithoutKeys(prefs, "Legacy AndroidKeyStore alias missing")
            generateLegacyKeyPair(context, alias)
        }

        val wrapped = prefs.getString(SECURE_STORE_WRAPPED_KEY_PREF, null)
        if (wrapped != null) {
            try {
                return unwrapLegacyMasterKey(keyStore, alias, wrapped)
            } catch (e: Exception) {
                Log.w(TAG, "Legacy secure store master key unwrap failed, resetting store", e)
                clearAllSecureStoreState(context, prefs)
            }
        }

        val rawKey = ByteArray(32)
        secureRandom.nextBytes(rawKey)
        val wrappedKey = wrapLegacyMasterKey(loadKeyStore(), alias, rawKey)
        if (!prefs.edit().putString(
                SECURE_STORE_WRAPPED_KEY_PREF,
                Base64.encodeToString(wrappedKey, Base64.NO_WRAP)
            ).commit()
        ) {
            throw IllegalStateException("Failed to persist wrapped legacy secure store key")
        }
        return SecretKeySpec(rawKey, "AES")
    }

    private fun wrapLegacyMasterKey(keyStore: KeyStore, alias: String, rawKey: ByteArray): ByteArray {
        val publicKey = keyStore.getCertificate(alias)?.publicKey
            ?: throw IllegalStateException("Legacy AndroidKeyStore certificate missing for alias $alias")
        val cipher = Cipher.getInstance(RSA_TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, publicKey)
        return cipher.doFinal(rawKey)
    }

    private fun unwrapLegacyMasterKey(
        keyStore: KeyStore,
        alias: String,
        wrappedKeyBase64: String
    ): SecretKey {
        val entry = keyStore.getEntry(alias, null) as? KeyStore.PrivateKeyEntry
            ?: throw IllegalStateException("Legacy AndroidKeyStore private key missing for alias $alias")
        val wrapped = Base64.decode(wrappedKeyBase64, Base64.DEFAULT)
        val cipher = Cipher.getInstance(RSA_TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, entry.privateKey)
        val rawKey = cipher.doFinal(wrapped)
        return SecretKeySpec(rawKey, "AES")
    }

    private fun generateLegacyKeyPair(context: Context, alias: String) {
        val start = Calendar.getInstance()
        val end = Calendar.getInstance().apply { add(Calendar.YEAR, 30) }
        val spec = KeyPairGeneratorSpec.Builder(context)
            .setAlias(alias)
            .setSubject(X500Principal("CN=$alias"))
            .setSerialNumber(BigInteger.ONE)
            .setStartDate(start.time)
            .setEndDate(end.time)
            .build()
        val generator = KeyPairGenerator.getInstance("RSA", ANDROID_KEYSTORE)
        generator.initialize(spec)
        generator.generateKeyPair()
    }

    private fun clearPrefsIfRestoredWithoutKeys(
        prefs: SharedPreferences,
        reason: String
    ) {
        if (prefs.all.isNotEmpty()) {
            Log.w(TAG, "Resetting Android secure store after restore mismatch: $reason")
            prefs.edit().clear().commit()
            cachedMasterKey = null
        }
    }

    private fun clearAllSecureStoreState(context: Context, prefs: SharedPreferences) {
        val keyStore = loadKeyStore()
        val aliases = listOf(masterKeyAlias(context), legacyKeyAlias(context))
        aliases.forEach { alias ->
            if (keyStore.containsAlias(alias)) {
                try {
                    keyStore.deleteEntry(alias)
                } catch (e: Exception) {
                    Log.w(TAG, "Failed to delete AndroidKeyStore alias $alias during reset", e)
                }
            }
        }
        prefs.edit().clear().commit()
        cachedMasterKey = null

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            generateModernAesKey(masterKeyAlias(context))
        } else {
            generateLegacyKeyPair(context, legacyKeyAlias(context))
        }
    }

    private fun prefs(context: Context): SharedPreferences {
        return context.getSharedPreferences(SECURE_STORE_PREFS, Context.MODE_PRIVATE)
    }

    private fun loadKeyStore(): KeyStore {
        return KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
    }

    private fun entryPrefKey(storageKey: String): String {
        val digest = MessageDigest.getInstance("SHA-256")
            .digest(storageKey.toByteArray(StandardCharsets.UTF_8))
        return buildString(SECURE_STORE_ENTRY_PREFIX.length + digest.size * 2) {
            append(SECURE_STORE_ENTRY_PREFIX)
            digest.forEach { byte ->
                append(((byte.toInt() ushr 4) and 0xF).toString(16))
                append((byte.toInt() and 0xF).toString(16))
            }
        }
    }

    private fun masterKeyAlias(context: Context): String {
        return "${context.packageName}.lingxia.secure_store.master.v1"
    }

    private fun legacyKeyAlias(context: Context): String {
        return "${context.packageName}.lingxia.secure_store.legacy.v1"
    }

    private data class EncryptedBlob(
        val version: Int,
        val iv: ByteArray,
        val ciphertext: ByteArray
    ) {
        fun encode(): String {
            return listOf(
                version.toString(),
                Base64.encodeToString(iv, Base64.NO_WRAP),
                Base64.encodeToString(ciphertext, Base64.NO_WRAP),
            ).joinToString(":")
        }

        companion object {
            fun decode(value: String): EncryptedBlob? {
                val parts = value.split(':')
                if (parts.size != 3) {
                    return null
                }

                val version = parts[0].toIntOrNull() ?: return null
                val iv = try {
                    Base64.decode(parts[1], Base64.DEFAULT)
                } catch (_: IllegalArgumentException) {
                    return null
                }
                val ciphertext = try {
                    Base64.decode(parts[2], Base64.DEFAULT)
                } catch (_: IllegalArgumentException) {
                    return null
                }
                if (iv.size != SECURE_STORE_GCM_IV_BYTES) {
                    return null
                }
                return EncryptedBlob(version, iv, ciphertext)
            }
        }
    }
}
