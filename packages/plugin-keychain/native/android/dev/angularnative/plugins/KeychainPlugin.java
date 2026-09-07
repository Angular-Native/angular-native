package dev.angularnative.plugins;

import android.app.Activity;
import android.content.Context;
import android.content.SharedPreferences;
import android.hardware.biometrics.BiometricPrompt;
import android.os.Build;
import android.os.CancellationSignal;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyInfo;
import android.security.keystore.KeyPermanentlyInvalidatedException;
import android.security.keystore.KeyProperties;
import android.util.Base64;

import dev.angularnative.AnPlugin;
import dev.angularnative.AnPluginCall;

import org.json.JSONException;
import org.json.JSONObject;

import java.nio.charset.StandardCharsets;
import java.security.KeyStore;

import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.SecretKeyFactory;
import javax.crypto.spec.GCMParameterSpec;

/**
 * The honest equivalent of the iOS keychain, on Android.
 *
 * <p><b>Android has no keychain.</b> There is no system API that stores a secret for you: what there
 * is is the <i>key store</i>, {@code AndroidKeyStore}, which holds <i>keys</i> —not data— and does
 * not let them out. So the equivalent is built out of two pieces:
 *
 * <ol>
 *   <li>a 256-bit AES key generated inside the store, which never leaves it and which on most
 *       devices lives in the TEE or in the StrongBox;
 *   <li>the secret encrypted with it —AES/GCM, with an authentication tag— in a preferences file
 *       private to the app.
 * </ol>
 *
 * <p>That is exactly what {@code androidx.security.EncryptedSharedPreferences} does, and here it is
 * written by hand for the same reason the biometrics use the platform's {@code BiometricPrompt}:
 * there is no Gradle in this repo, the Android dependencies are brought in one by one, and this is
 * eighty lines.
 *
 * <p><b>What it protects and what it does not</b> is at {@code https://angular-native.github.io/extending/plugins/}, and it is the important part.
 * The summary: uninstalling the app makes both pieces disappear; the backup may take the encrypted
 * file but never the key, so restored on another device it cannot be opened; and on a phone with no
 * secure hardware the key is kept by the system in software, which is less.
 * {@code backing()} says which of the two cases this one is.
 */
public final class KeychainPlugin implements AnPlugin {

    /** The preferences file. {@code MODE_PRIVATE}: only this app reads it. */
    private static final String PREFS = "an.keychain";

    private static final String KEYSTORE = "AndroidKeyStore";
    private static final String TRANSFORMATION = "AES/GCM/NoPadding";
    /** 128 tag bits: GCM's maximum, and what detects that somebody touched the ciphertext. */
    private static final int TAG_BITS = 128;

    /** One key for the ordinary case and another for what asks for biometrics. */
    private static final String ALIAS_PLAIN = "an.keychain.plain";
    private static final String ALIAS_BIO = "an.keychain.bio";

    /** {@code BiometricPrompt} arrived in Android 9; its {@code CryptoObject} cancellation too. */
    private static final int MIN_BIO_API = Build.VERSION_CODES.P;

    private Activity host;

    @Override
    public void attach(Activity host) {
        this.host = host;
    }

    @Override
    public void call(String method, JSONObject args, AnPluginCall respond) {
        if (host == null) {
            respond.reject("the keychain plugin has no Android context");
            return;
        }
        if ("backing".equals(method)) {
            respond.resolve(backing());
            return;
        }
        String key = args.optString("key", "");
        if (key.isEmpty()) {
            respond.reject("keychain." + method + " needs a non-empty key in 'key'");
            return;
        }

        switch (method) {
            case "set": {
                if (!args.has("value")) {
                    respond.reject("keychain.set needs the secret in 'value'");
                    return;
                }
                String value = args.optString("value");
                boolean biometrics = args.optBoolean("requireBiometrics", false);
                String reason = args.optString("reason", "");
                if (biometrics && reason.isEmpty()) {
                    respond.reject(
                            "keychain.set with requireBiometrics needs a 'reason': it is the title"
                                    + " of the system dialog");
                    return;
                }
                set(key, value, biometrics, reason, respond);
                return;
            }

            case "get":
                get(key, args.optString("reason", ""), respond);
                return;

            case "has":
                respond.resolve(prefs().contains(key));
                return;

            case "remove": {
                boolean existed = prefs().contains(key);
                prefs().edit().remove(key).apply();
                respond.resolve(existed);
                return;
            }

            default:
                respond.reject("the keychain plugin has no method " + method);
        }
    }

    // ── Storing ─────────────────────────────────────────────────────────────

    /**
     * Stores a secret.
     *
     * <p>With biometrics, <b>storing asks for the fingerprint too</b>. It is not an oversight: the
     * Android key store ties authentication to every use of the key, and encrypting is a use. iOS
     * does not work like this —over there only reading asks—, and that difference is written down in
     * the TypeScript contract and in the documentation, because papering over it would mean storing
     * the secret with a different key and then the protection would be a different protection.
     */
    private void set(
            String key, String value, boolean biometrics, String reason, AnPluginCall respond) {
        Cipher cipher;
        try {
            SecretKey secret = secretKey(biometrics ? ALIAS_BIO : ALIAS_PLAIN, biometrics);
            cipher = Cipher.getInstance(TRANSFORMATION);
            cipher.init(Cipher.ENCRYPT_MODE, secret);
        } catch (KeyPermanentlyInvalidatedException invalidated) {
            // The enrolled fingerprints changed. Whatever was stored with the old
            // key cannot be opened any more; anything new needs a new key.
            try {
                deleteKey(ALIAS_BIO);
                SecretKey secret = secretKey(ALIAS_BIO, true);
                cipher = Cipher.getInstance(TRANSFORMATION);
                cipher.init(Cipher.ENCRYPT_MODE, secret);
            } catch (Exception error) {
                respond.resolve(writeResult("unavailable", "the key could not be remade: " + error));
                return;
            }
        } catch (Exception error) {
            respond.resolve(writeResult("unavailable", describe(error)));
            return;
        }

        if (!biometrics) {
            try {
                store(key, cipher, value);
                respond.resolve(writeResult("saved", "AES/GCM with an AndroidKeyStore key"));
            } catch (Exception error) {
                respond.resolve(writeResult("unavailable", describe(error)));
            }
            return;
        }

        if (Build.VERSION.SDK_INT < MIN_BIO_API) {
            respond.resolve(
                    writeResult(
                            "unavailable",
                            "storing with biometrics needs Android 9 (API " + MIN_BIO_API
                                    + ") and the device is on API " + Build.VERSION.SDK_INT));
            return;
        }
        final Cipher ready = cipher;
        ask(
                reason,
                cipher,
                new Answer() {
                    @Override
                    public void authenticated(Cipher authorised) {
                        try {
                            store(key, authorised == null ? ready : authorised, value);
                            respond.resolve(
                                    writeResult(
                                            "saved",
                                            "AES/GCM with a single-use authenticated key"));
                        } catch (Exception error) {
                            respond.resolve(writeResult("unavailable", describe(error)));
                        }
                    }

                    @Override
                    public void denied(String detail) {
                        respond.resolve(writeResult("denied", detail));
                    }
                });
    }

    /** Encrypts and writes. The initialisation vector is picked by the store, not by this code. */
    private void store(String key, Cipher cipher, String value) throws Exception {
        byte[] encrypted = cipher.doFinal(value.getBytes(StandardCharsets.UTF_8));
        String line =
                Base64.encodeToString(cipher.getIV(), Base64.NO_WRAP)
                        + ":"
                        + Base64.encodeToString(encrypted, Base64.NO_WRAP);
        prefs().edit().putString(key, line).apply();
    }

    // ── Reading ─────────────────────────────────────────────────────────────

    private void get(String key, String reason, AnPluginCall respond) {
        String line = prefs().getString(key, null);
        if (line == null) {
            respond.resolve(readResult("notFound", null, "nothing is stored under that key"));
            return;
        }
        int cut = line.indexOf(':');
        if (cut < 0) {
            // A value that does not have the shape this plugin writes. It is said
            // so; returning `notFound` would make you believe nothing was ever
            // stored.
            respond.resolve(
                    readResult(
                            "unavailable",
                            null,
                            "what is stored does not have the shape <iv>:<ciphertext>"));
            return;
        }
        byte[] iv = Base64.decode(line.substring(0, cut), Base64.NO_WRAP);
        byte[] encrypted = Base64.decode(line.substring(cut + 1), Base64.NO_WRAP);

        // There is no way of knowing beforehand which of the two keys it was
        // encrypted with, so the plain one is tried first: if it does not
        // decrypt, it is one of the biometric ones. Trying is cheap and it saves
        // storing a marker that could stop matching reality.
        Cipher cipher = decryptor(ALIAS_PLAIN, iv);
        if (cipher != null) {
            try {
                byte[] plain = cipher.doFinal(encrypted);
                respond.resolve(
                        readResult("found", new String(plain, StandardCharsets.UTF_8), "AES/GCM"));
                return;
            } catch (Exception notThisKey) {
                // The GCM tag does not add up: this key did not encrypt it. Carry on.
            }
        }

        if (Build.VERSION.SDK_INT < MIN_BIO_API) {
            respond.resolve(
                    readResult(
                            "unavailable",
                            null,
                            "it is stored with biometrics and that needs Android 9 (API "
                                    + MIN_BIO_API + ")"));
            return;
        }
        Cipher bio;
        try {
            SecretKey secret = secretKey(ALIAS_BIO, true);
            bio = Cipher.getInstance(TRANSFORMATION);
            bio.init(Cipher.DECRYPT_MODE, secret, new GCMParameterSpec(TAG_BITS, iv));
        } catch (KeyPermanentlyInvalidatedException invalidated) {
            // The enrolled fingerprints or faces changed. The secret is still
            // there and it can never be opened again. It is not `denied` —that
            // one is fixed by trying again— and not `notFound` either.
            respond.resolve(
                    readResult(
                            "invalidated",
                            null,
                            "the key was invalidated when the enrolled biometrics changed"));
            return;
        } catch (Exception error) {
            respond.resolve(readResult("unavailable", null, describe(error)));
            return;
        }

        ask(
                reason.isEmpty() ? "Unlock" : reason,
                bio,
                new Answer() {
                    @Override
                    public void authenticated(Cipher authorised) {
                        try {
                            byte[] plain = (authorised == null ? bio : authorised).doFinal(encrypted);
                            respond.resolve(
                                    readResult(
                                            "found",
                                            new String(plain, StandardCharsets.UTF_8),
                                            "AES/GCM after authenticating"));
                        } catch (Exception error) {
                            respond.resolve(readResult("unavailable", null, describe(error)));
                        }
                    }

                    @Override
                    public void denied(String detail) {
                        respond.resolve(readResult("denied", null, detail));
                    }
                });
    }

    /** A decryptor with the plain key, or {@code null} if that key does not exist yet. */
    private Cipher decryptor(String alias, byte[] iv) {
        try {
            KeyStore store = KeyStore.getInstance(KEYSTORE);
            store.load(null);
            if (!store.containsAlias(alias)) {
                return null;
            }
            Cipher cipher = Cipher.getInstance(TRANSFORMATION);
            cipher.init(
                    Cipher.DECRYPT_MODE,
                    (SecretKey) store.getKey(alias, null),
                    new GCMParameterSpec(TAG_BITS, iv));
            return cipher;
        } catch (Exception error) {
            return null;
        }
    }

    // ── The system dialog ───────────────────────────────────────────────────

    /** What to do when the dialog ends. */
    private interface Answer {
        /** Authenticated. The {@code Cipher} that arrives is the one the system authorised. */
        void authenticated(Cipher authorised);

        void denied(String detail);
    }

    /**
     * Shows {@code BiometricPrompt} tied to this {@code Cipher}.
     *
     * <p>The {@code CryptoObject} is what keeps this from being theatre: without it, the app would
     * ask for the fingerprint and then decrypt all the same, and whoever could get past the dialog
     * would have the secret. With it, the key cannot be used until the system unlocks it, and the
     * one deciding is the hardware, not this code.
     */
    private void ask(String reason, Cipher cipher, Answer answer) {
        BiometricPrompt prompt =
                new BiometricPrompt.Builder(host)
                        .setTitle(reason)
                        .setNegativeButton(
                                "Cancel",
                                host.getMainExecutor(),
                                (dialog, which) -> {
                                    // The system also sends ERROR_NEGATIVE_BUTTON
                                    // through the error callback, and that is where
                                    // the answer is given: doing it twice would
                                    // leave the second answer hanging.
                                })
                        .build();
        prompt.authenticate(
                new BiometricPrompt.CryptoObject(cipher),
                new CancellationSignal(),
                host.getMainExecutor(),
                new BiometricPrompt.AuthenticationCallback() {
                    @Override
                    public void onAuthenticationSucceeded(
                            BiometricPrompt.AuthenticationResult result) {
                        BiometricPrompt.CryptoObject crypto = result.getCryptoObject();
                        answer.authenticated(crypto == null ? null : crypto.getCipher());
                    }

                    @Override
                    public void onAuthenticationError(int code, CharSequence message) {
                        answer.denied("BiometricPrompt error " + code + ": " + message);
                    }

                    @Override
                    public void onAuthenticationFailed() {
                        // An attempt that recognises nobody. No answer is given: the
                        // dialog stays on screen and the user can try again.
                    }
                });
    }

    // ── The key store ───────────────────────────────────────────────────────

    /**
     * The key from the store, creating it the first time round.
     *
     * <p>{@code setInvalidatedByBiometricEnrollment} is what makes the biometric variant worth
     * anything: without it, somebody who knew the device passcode could enrol their own fingerprint
     * and help themselves to the secret. With it, enrolling another fingerprint throws the key away.
     */
    private static SecretKey secretKey(String alias, boolean biometrics) throws Exception {
        KeyStore store = KeyStore.getInstance(KEYSTORE);
        store.load(null);
        SecretKey existing = (SecretKey) store.getKey(alias, null);
        if (existing != null) {
            return existing;
        }
        KeyGenParameterSpec.Builder builder =
                new KeyGenParameterSpec.Builder(
                                alias,
                                KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
                        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                        .setKeySize(256);
        if (biometrics) {
            builder.setUserAuthenticationRequired(true).setInvalidatedByBiometricEnrollment(true);
        }
        KeyGenerator generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE);
        generator.init(builder.build());
        return generator.generateKey();
    }

    private static void deleteKey(String alias) throws Exception {
        KeyStore store = KeyStore.getInstance(KEYSTORE);
        store.load(null);
        store.deleteEntry(alias);
    }

    /** Where this device's key really lives. */
    private JSONObject backing() {
        boolean hardware = false;
        String detail;
        try {
            SecretKey secret = secretKey(ALIAS_PLAIN, false);
            SecretKeyFactory factory =
                    SecretKeyFactory.getInstance(secret.getAlgorithm(), KEYSTORE);
            KeyInfo info = (KeyInfo) factory.getKeySpec(secret, KeyInfo.class);
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                int level = info.getSecurityLevel();
                hardware = level == KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT
                        || level == KeyProperties.SECURITY_LEVEL_STRONGBOX;
                detail = "KeyInfo.getSecurityLevel() = " + level;
            } else {
                // Deprecated since API 31, and it is the only thing there is below that.
                hardware = info.isInsideSecureHardware();
                detail = "KeyInfo.isInsideSecureHardware() = " + hardware;
            }
        } catch (Exception error) {
            // Not being able to find out is not papered over with a `false`, which
            // would read as "it is software" and it may not be.
            detail = "the store could not be asked: " + describe(error);
        }
        JSONObject json = new JSONObject();
        try {
            json.put("platform", "android");
            json.put("hardwareBacked", hardware);
            json.put("accessible", "for as long as the app is installed; the file is MODE_PRIVATE");
            json.put("detail", detail);
        } catch (JSONException error) {
            throw new IllegalStateException("the keychain answer could not be built", error);
        }
        return json;
    }

    // ── Helpers ─────────────────────────────────────────────────────────────

    private SharedPreferences prefs() {
        return host.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }

    private static JSONObject readResult(String outcome, String value, String detail) {
        JSONObject json = new JSONObject();
        try {
            json.put("outcome", outcome);
            // `JSONObject.put` with null deletes the key, and the contract says
            // `value` is always there. `JSONObject.NULL` is what travels as null.
            json.put("value", value == null ? JSONObject.NULL : value);
            json.put("detail", detail);
        } catch (JSONException error) {
            throw new IllegalStateException("the keychain answer could not be built", error);
        }
        return json;
    }

    private static JSONObject writeResult(String outcome, String detail) {
        JSONObject json = new JSONObject();
        try {
            json.put("outcome", outcome);
            json.put("detail", detail);
        } catch (JSONException error) {
            throw new IllegalStateException("the keychain answer could not be built", error);
        }
        return json;
    }

    /** The exception's name and its message. The whole trace does not fit in a `detail`. */
    private static String describe(Throwable error) {
        String message = error.getMessage();
        return error.getClass().getSimpleName() + (message == null ? "" : ": " + message);
    }
}
