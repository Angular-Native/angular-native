package dev.angularnative;

/**
 * Cara Java del runtime en Rust. Es el equivalente exacto de la cabecera C que
 * consume el shell de iOS.
 */
public final class AnRuntime {

    static {
        System.loadLibrary("an_android");
    }

    private long handle;

    public AnRuntime(AnHost host, float widthDp, float heightDp) {
        handle = nativeNew(host, widthDp, heightDp);
    }

    public boolean isValid() {
        return handle != 0;
    }

    public int eval(String name, String code) {
        return nativeEval(handle, name, code);
    }

    public int reload(String name, String code) {
        return nativeReload(handle, name, code);
    }

    public void setViewport(float widthDp, float heightDp) {
        nativeSetViewport(handle, widthDp, heightDp);
    }

    /** Un frame completo. `nowMs` es la marca del Choreographer. */
    public int frame(double nowMs) {
        return nativeFrame(handle, nowMs);
    }

    /** Encola un evento nativo; se despacha al principio del frame siguiente. */
    public void dispatchEvent(int target, String name, float x, float y) {
        nativeDispatchEvent(handle, target, name, x, y);
    }

    /** Eventos que llevan texto: escribir, entrar y salir de un campo. */
    public void dispatchValueEvent(int target, String name, String value) {
        nativeDispatchValueEvent(handle, target, name, value);
    }

    /**
     * Gestos. Llevan más de dos cifras y un estado, así que no caben en
     * {@link #dispatchEvent}: los nombres de los campos viajan al lado de los
     * valores para que las dos plataformas manden exactamente lo mismo.
     */
    public void dispatchGesture(int target, String name, String state, String keys, float[] values) {
        nativeDispatchGesture(handle, target, name, state, keys, values);
    }

    /** Eventos que llevan un índice: la pestaña elegida, por ejemplo. */
    public void dispatchIndexEvent(int target, String name, int index) {
        nativeDispatchIndexEvent(handle, target, name, index);
    }

    public void close() {
        if (handle != 0) {
            nativeFree(handle);
            handle = 0;
        }
    }

    // ── Plugins ────────────────────────────────────────────────────────────
    //
    // No llevan `handle`: los plugins son del proceso, no de un runtime. Se
    // registran antes de crear el primero y sobreviven a un reinicio en
    // caliente, igual que el `.app` de iOS.

    /** Guarda el registro al que Rust le pasará cada llamada. */
    static void setPluginRegistry(AnPluginRegistry registry) {
        nativeSetPluginRegistry(registry);
    }

    /** Da de alta un plugin por el nombre con el que JS lo invoca. */
    static void registerPlugin(String name) {
        nativeRegisterPlugin(name);
    }

    /** Contesta a una llamada. `json` es el valor de vuelta ya serializado. */
    static void pluginResolve(long id, String json) {
        nativePluginResolve(id, json);
    }

    static void pluginReject(long id, String message) {
        nativePluginReject(id, message);
    }

    private static native long nativeNew(AnHost host, float width, float height);

    private static native int nativeEval(long handle, String name, String code);

    private static native int nativeReload(long handle, String name, String code);

    private static native void nativeSetViewport(long handle, float width, float height);

    private static native int nativeFrame(long handle, double nowMs);

    private static native void nativeDispatchEvent(
            long handle, int target, String name, float x, float y);

    private static native void nativeDispatchValueEvent(
            long handle, int target, String name, String value);

    private static native void nativeDispatchGesture(
            long handle, int target, String name, String state, String keys, float[] values);

    private static native void nativeDispatchIndexEvent(
            long handle, int target, String name, int index);

    private static native void nativeFree(long handle);

    private static native void nativeSetPluginRegistry(AnPluginRegistry registry);

    private static native void nativeRegisterPlugin(String name);

    private static native int nativePluginResolve(long id, String json);

    private static native int nativePluginReject(long id, String message);
}
