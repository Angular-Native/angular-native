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

    private static native long nativeNew(AnHost host, float width, float height);

    private static native int nativeEval(long handle, String name, String code);

    private static native int nativeReload(long handle, String name, String code);

    private static native void nativeSetViewport(long handle, float width, float height);

    private static native int nativeFrame(long handle, double nowMs);

    private static native void nativeDispatchEvent(
            long handle, int target, String name, float x, float y);

    private static native void nativeDispatchValueEvent(
            long handle, int target, String name, String value);

    private static native void nativeDispatchIndexEvent(
            long handle, int target, String name, int index);

    private static native void nativeFree(long handle);
}
