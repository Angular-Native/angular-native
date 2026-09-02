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
 * El equivalente honrado del llavero de iOS, en Android.
 *
 * <p><b>Android no tiene llavero.</b> No hay ninguna API del sistema que guarde un secreto por ti:
 * lo que hay es el <i>almacén de claves</i>, {@code AndroidKeyStore}, que guarda <i>claves</i> —no
 * datos— y no las deja salir de ahí. Así que el equivalente se arma con dos piezas:
 *
 * <ol>
 *   <li>una clave AES de 256 bits generada dentro del almacén, que nunca sale de él y que en la
 *       mayoría de los aparatos vive en el TEE o en el StrongBox;
 *   <li>el secreto cifrado con ella —AES/GCM, con etiqueta de autenticación— en un fichero de
 *       preferencias privado de la app.
 * </ol>
 *
 * <p>Eso es exactamente lo que hace {@code androidx.security.EncryptedSharedPreferences}, y aquí
 * está escrito a mano por el mismo motivo que la biometría usa el {@code BiometricPrompt} de la
 * plataforma: en este repo no hay Gradle, las dependencias de Android se traen una a una, y esto
 * son ochenta líneas.
 *
 * <p><b>Lo que protege y lo que no</b> está en {@code docs/plugins.md}, y es la parte importante.
 * El resumen: al desinstalar la app desaparecen las dos piezas; la copia de seguridad puede
 * llevarse el fichero cifrado pero nunca la clave, así que restaurado en otro aparato no se abre;
 * y en un teléfono sin hardware seguro la clave la guarda el sistema en software, que es menos.
 * {@code backing()} dice cuál de los dos casos es este.
 */
public final class KeychainPlugin implements AnPlugin {

    /** El fichero de preferencias. {@code MODE_PRIVATE}: solo lo lee esta app. */
    private static final String PREFS = "an.keychain";

    private static final String KEYSTORE = "AndroidKeyStore";
    private static final String TRANSFORMATION = "AES/GCM/NoPadding";
    /** 128 bits de etiqueta: el máximo de GCM, y lo que detecta que alguien tocó el cifrado. */
    private static final int TAG_BITS = 128;

    /** Una clave para lo normal y otra para lo que pide biometría. */
    private static final String ALIAS_LLANO = "an.keychain.plain";
    private static final String ALIAS_BIO = "an.keychain.bio";

    /** {@code BiometricPrompt} llegó en Android 9; su cancelación por {@code CryptoObject}, igual. */
    private static final int MINIMO_BIO = Build.VERSION_CODES.P;

    private Activity host;

    @Override
    public void attach(Activity host) {
        this.host = host;
    }

    @Override
    public void call(String method, JSONObject args, AnPluginCall respond) {
        if (host == null) {
            respond.reject("el plugin keychain no tiene contexto de Android");
            return;
        }
        if ("backing".equals(method)) {
            respond.resolve(backing());
            return;
        }
        String key = args.optString("key", "");
        if (key.isEmpty()) {
            respond.reject("keychain." + method + " necesita una clave no vacía en 'key'");
            return;
        }

        switch (method) {
            case "set": {
                if (!args.has("value")) {
                    respond.reject("keychain.set necesita el secreto en 'value'");
                    return;
                }
                String value = args.optString("value");
                boolean biometrics = args.optBoolean("requireBiometrics", false);
                String reason = args.optString("reason", "");
                if (biometrics && reason.isEmpty()) {
                    respond.reject(
                            "keychain.set con requireBiometrics necesita un 'reason': es el título"
                                    + " del diálogo del sistema");
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
                boolean habia = prefs().contains(key);
                prefs().edit().remove(key).apply();
                respond.resolve(habia);
                return;
            }

            default:
                respond.reject("el plugin keychain no tiene ningún método " + method);
        }
    }

    // ── Guardar ─────────────────────────────────────────────────────────────

    /**
     * Guarda un secreto.
     *
     * <p>Con biometría, <b>guardar también pide la huella</b>. No es un descuido: el almacén de
     * claves de Android ata la autenticación a cada uso de la clave, y cifrar es un uso. iOS no
     * funciona así —allí solo pregunta la lectura—, y esa diferencia está dicha en el contrato de
     * TypeScript y en la documentación, porque disimularla obligaría a guardar el secreto con otra
     * clave y entonces la protección sería otra.
     */
    private void set(
            String key, String value, boolean biometrics, String reason, AnPluginCall respond) {
        Cipher cipher;
        try {
            SecretKey secret = clave(biometrics ? ALIAS_BIO : ALIAS_LLANO, biometrics);
            cipher = Cipher.getInstance(TRANSFORMATION);
            cipher.init(Cipher.ENCRYPT_MODE, secret);
        } catch (KeyPermanentlyInvalidatedException invalidada) {
            // Cambiaron las huellas registradas. Lo guardado con la clave vieja
            // ya no se abre; para lo nuevo hace falta una clave nueva.
            try {
                borrarClave(ALIAS_BIO);
                SecretKey secret = clave(ALIAS_BIO, true);
                cipher = Cipher.getInstance(TRANSFORMATION);
                cipher.init(Cipher.ENCRYPT_MODE, secret);
            } catch (Exception error) {
                respond.resolve(escritura("unavailable", "no se pudo rehacer la clave: " + error));
                return;
            }
        } catch (Exception error) {
            respond.resolve(escritura("unavailable", motivo(error)));
            return;
        }

        if (!biometrics) {
            try {
                guardar(key, cipher, value);
                respond.resolve(escritura("saved", "AES/GCM con clave del AndroidKeyStore"));
            } catch (Exception error) {
                respond.resolve(escritura("unavailable", motivo(error)));
            }
            return;
        }

        if (Build.VERSION.SDK_INT < MINIMO_BIO) {
            respond.resolve(
                    escritura(
                            "unavailable",
                            "guardar con biometría necesita Android 9 (API " + MINIMO_BIO
                                    + ") y el aparato tiene API " + Build.VERSION.SDK_INT));
            return;
        }
        final Cipher listo = cipher;
        preguntar(
                reason,
                cipher,
                new Respuesta() {
                    @Override
                    public void autenticado(Cipher autorizado) {
                        try {
                            guardar(key, autorizado == null ? listo : autorizado, value);
                            respond.resolve(
                                    escritura("saved", "AES/GCM con clave de un solo uso autenticado"));
                        } catch (Exception error) {
                            respond.resolve(escritura("unavailable", motivo(error)));
                        }
                    }

                    @Override
                    public void denegado(String detail) {
                        respond.resolve(escritura("denied", detail));
                    }
                });
    }

    /** Cifra y escribe. El vector de inicialización lo elige el almacén, no este código. */
    private void guardar(String key, Cipher cipher, String value) throws Exception {
        byte[] cifrado = cipher.doFinal(value.getBytes(StandardCharsets.UTF_8));
        String linea =
                Base64.encodeToString(cipher.getIV(), Base64.NO_WRAP)
                        + ":"
                        + Base64.encodeToString(cifrado, Base64.NO_WRAP);
        prefs().edit().putString(key, linea).apply();
    }

    // ── Leer ────────────────────────────────────────────────────────────────

    private void get(String key, String reason, AnPluginCall respond) {
        String linea = prefs().getString(key, null);
        if (linea == null) {
            respond.resolve(lectura("notFound", null, "no hay nada guardado con esa clave"));
            return;
        }
        int corte = linea.indexOf(':');
        if (corte < 0) {
            // Un valor que no tiene la forma que escribe este plugin. Se dice;
            // devolver `notFound` haría creer que nunca se guardó nada.
            respond.resolve(
                    lectura("unavailable", null, "lo guardado no tiene la forma <iv>:<cifrado>"));
            return;
        }
        byte[] iv = Base64.decode(linea.substring(0, corte), Base64.NO_WRAP);
        byte[] cifrado = Base64.decode(linea.substring(corte + 1), Base64.NO_WRAP);

        // No se sabe de antemano con cuál de las dos claves se cifró, así que
        // se prueba la llana primero: si no descifra, es de las de biometría.
        // Probar es barato y evita guardar un indicador que podría dejar de
        // coincidir con la realidad.
        Cipher cipher = descifrador(ALIAS_LLANO, iv);
        if (cipher != null) {
            try {
                byte[] claro = cipher.doFinal(cifrado);
                respond.resolve(
                        lectura("found", new String(claro, StandardCharsets.UTF_8), "AES/GCM"));
                return;
            } catch (Exception noEsSuya) {
                // La etiqueta de GCM no cuadra: no lo cifró esta clave. Se sigue.
            }
        }

        if (Build.VERSION.SDK_INT < MINIMO_BIO) {
            respond.resolve(
                    lectura(
                            "unavailable",
                            null,
                            "está guardado con biometría y eso necesita Android 9 (API "
                                    + MINIMO_BIO + ")"));
            return;
        }
        Cipher bio;
        try {
            SecretKey secret = clave(ALIAS_BIO, true);
            bio = Cipher.getInstance(TRANSFORMATION);
            bio.init(Cipher.DECRYPT_MODE, secret, new GCMParameterSpec(TAG_BITS, iv));
        } catch (KeyPermanentlyInvalidatedException invalidada) {
            // Cambiaron las huellas o las caras registradas. El secreto sigue
            // ahí y ya no se puede abrir nunca. Es distinto de `denied` —eso se
            // arregla volviendo a intentarlo— y distinto de `notFound`.
            respond.resolve(
                    lectura(
                            "invalidated",
                            null,
                            "la clave se invalidó al cambiar la biometría registrada"));
            return;
        } catch (Exception error) {
            respond.resolve(lectura("unavailable", null, motivo(error)));
            return;
        }

        preguntar(
                reason.isEmpty() ? "Desbloquear" : reason,
                bio,
                new Respuesta() {
                    @Override
                    public void autenticado(Cipher autorizado) {
                        try {
                            byte[] claro = (autorizado == null ? bio : autorizado).doFinal(cifrado);
                            respond.resolve(
                                    lectura(
                                            "found",
                                            new String(claro, StandardCharsets.UTF_8),
                                            "AES/GCM tras autenticar"));
                        } catch (Exception error) {
                            respond.resolve(lectura("unavailable", null, motivo(error)));
                        }
                    }

                    @Override
                    public void denegado(String detail) {
                        respond.resolve(lectura("denied", null, detail));
                    }
                });
    }

    /** Un descifrador con la clave llana, o {@code null} si esa clave no existe todavía. */
    private Cipher descifrador(String alias, byte[] iv) {
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

    // ── El diálogo del sistema ──────────────────────────────────────────────

    /** Qué hacer cuando el diálogo termina. */
    private interface Respuesta {
        /** Autenticó. El {@code Cipher} que llega es el que el sistema autorizó. */
        void autenticado(Cipher autorizado);

        void denegado(String detail);
    }

    /**
     * Enseña {@code BiometricPrompt} atado a este {@code Cipher}.
     *
     * <p>El {@code CryptoObject} es lo que hace que esto no sea teatro: sin él, la app preguntaría
     * por la huella y luego descifraría igual, y quien pudiera saltarse el diálogo tendría el
     * secreto. Con él, la clave no se puede usar hasta que el sistema la desbloquea, y el que
     * decide es el hardware, no este código.
     */
    private void preguntar(String reason, Cipher cipher, Respuesta respuesta) {
        BiometricPrompt prompt =
                new BiometricPrompt.Builder(host)
                        .setTitle(reason)
                        .setNegativeButton(
                                "Cancelar",
                                host.getMainExecutor(),
                                (dialog, which) -> {
                                    // El sistema manda además ERROR_NEGATIVE_BUTTON
                                    // por el callback de error, y es ahí donde se
                                    // contesta: hacerlo dos veces dejaría la
                                    // segunda respuesta en el aire.
                                })
                        .build();
        prompt.authenticate(
                new BiometricPrompt.CryptoObject(cipher),
                new CancellationSignal(),
                host.getMainExecutor(),
                new BiometricPrompt.AuthenticationCallback() {
                    @Override
                    public void onAuthenticationSucceeded(
                            BiometricPrompt.AuthenticationResult resultado) {
                        BiometricPrompt.CryptoObject objeto = resultado.getCryptoObject();
                        respuesta.autenticado(objeto == null ? null : objeto.getCipher());
                    }

                    @Override
                    public void onAuthenticationError(int code, CharSequence message) {
                        respuesta.denegado("BiometricPrompt error " + code + ": " + message);
                    }

                    @Override
                    public void onAuthenticationFailed() {
                        // Un intento que no reconoce a nadie. No se contesta: el
                        // diálogo sigue en pantalla y el usuario puede repetir.
                    }
                });
    }

    // ── El almacén de claves ────────────────────────────────────────────────

    /**
     * La clave del almacén, creándola la primera vez.
     *
     * <p>{@code setInvalidatedByBiometricEnrollment} es lo que hace que la variante con biometría
     * valga de algo: sin él, alguien que conociera el código del aparato podría registrar su propia
     * huella y quedarse con el secreto. Con él, registrar otra huella tira la clave.
     */
    private static SecretKey clave(String alias, boolean biometrics) throws Exception {
        KeyStore store = KeyStore.getInstance(KEYSTORE);
        store.load(null);
        SecretKey existente = (SecretKey) store.getKey(alias, null);
        if (existente != null) {
            return existente;
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

    private static void borrarClave(String alias) throws Exception {
        KeyStore store = KeyStore.getInstance(KEYSTORE);
        store.load(null);
        store.deleteEntry(alias);
    }

    /** Dónde vive de verdad la clave de este aparato. */
    private JSONObject backing() {
        boolean hardware = false;
        String detail;
        try {
            SecretKey secret = clave(ALIAS_LLANO, false);
            SecretKeyFactory factory =
                    SecretKeyFactory.getInstance(secret.getAlgorithm(), KEYSTORE);
            KeyInfo info = (KeyInfo) factory.getKeySpec(secret, KeyInfo.class);
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                int nivel = info.getSecurityLevel();
                hardware = nivel == KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT
                        || nivel == KeyProperties.SECURITY_LEVEL_STRONGBOX;
                detail = "KeyInfo.getSecurityLevel() = " + nivel;
            } else {
                // Obsoleto desde API 31, y es lo único que hay por debajo.
                hardware = info.isInsideSecureHardware();
                detail = "KeyInfo.isInsideSecureHardware() = " + hardware;
            }
        } catch (Exception error) {
            // Que no se pueda averiguar no se disimula con un `false`, que se
            // leería como «es software» y puede no serlo.
            detail = "no se pudo preguntar al almacén: " + motivo(error);
        }
        JSONObject json = new JSONObject();
        try {
            json.put("platform", "android");
            json.put("hardwareBacked", hardware);
            json.put("accessible", "mientras la app esté instalada; el fichero es MODE_PRIVATE");
            json.put("detail", detail);
        } catch (JSONException error) {
            throw new IllegalStateException("no se pudo armar la respuesta de keychain", error);
        }
        return json;
    }

    // ── Auxiliares ──────────────────────────────────────────────────────────

    private SharedPreferences prefs() {
        return host.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }

    private static JSONObject lectura(String outcome, String value, String detail) {
        JSONObject json = new JSONObject();
        try {
            json.put("outcome", outcome);
            // `JSONObject.put` con null borra la clave, y el contrato dice que
            // `value` va siempre. `JSONObject.NULL` es lo que viaja como null.
            json.put("value", value == null ? JSONObject.NULL : value);
            json.put("detail", detail);
        } catch (JSONException error) {
            throw new IllegalStateException("no se pudo armar la respuesta de keychain", error);
        }
        return json;
    }

    private static JSONObject escritura(String outcome, String detail) {
        JSONObject json = new JSONObject();
        try {
            json.put("outcome", outcome);
            json.put("detail", detail);
        } catch (JSONException error) {
            throw new IllegalStateException("no se pudo armar la respuesta de keychain", error);
        }
        return json;
    }

    /** El nombre de la excepción y su mensaje. La traza entera no cabe en un `detail`. */
    private static String motivo(Throwable error) {
        String mensaje = error.getMessage();
        return error.getClass().getSimpleName() + (mensaje == null ? "" : ": " + mensaje);
    }
}
