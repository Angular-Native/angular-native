package dev.angularnative;

/**
 * The Java face of the Rust runtime. It is the exact equivalent of the C header
 * the iOS shell consumes.
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

    /** One whole frame. `nowMs` is the Choreographer's timestamp. */
    public int frame(double nowMs) {
        return nativeFrame(handle, nowMs);
    }

    /** Queues a native event; it is dispatched at the start of the next frame. */
    public void dispatchEvent(int target, String name, float x, float y) {
        nativeDispatchEvent(handle, target, name, x, y);
    }

    /** Events that carry text: typing, and entering and leaving a field. */
    public void dispatchValueEvent(int target, String name, String value) {
        nativeDispatchValueEvent(handle, target, name, value);
    }

    /**
     * Gestures. They carry more than two numbers plus a state, so they do not fit
     * in {@link #dispatchEvent}: the field names travel alongside the values so
     * that both platforms send exactly the same thing.
     */
    public void dispatchGesture(int target, String name, String state, String keys, float[] values) {
        nativeDispatchGesture(handle, target, name, state, keys, values);
    }

    /** Events that carry an index: the selected tab, for instance. */
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
    // They carry no `handle`: plugins belong to the process, not to a runtime.
    // They are registered before the first one is created and survive a hot
    // restart, just as in the iOS `.app`.

    /** Keeps the registry Rust will hand every call to. */
    static void setPluginRegistry(AnPluginRegistry registry) {
        nativeSetPluginRegistry(registry);
    }

    /** Registers a plugin under the name JS calls it by. */
    static void registerPlugin(String name) {
        nativeRegisterPlugin(name);
    }

    /** Answers one call. `json` is the return value, already serialised. */
    static void pluginResolve(long id, String json) {
        nativePluginResolve(id, json);
    }

    static void pluginReject(long id, String message) {
        nativePluginReject(id, message);
    }

    // ── Built-in modules ───────────────────────────────────────────────────
    //
    // The four the framework brings. They carry no `handle` for the same reason
    // the plugins do not: they belong to the process, are installed before the
    // first runtime is created and survive a hot restart. What they do not share
    // with the plugins is the mailbox, so that a built-in cannot be shadowed by
    // an npm package claiming its name.

    /** Keeps the object Rust will hand every built-in call to. */
    static void setBuiltinModules(AnBuiltinModules modules) {
        nativeSetBuiltinModules(modules);
    }

    /** Answers one call. `json` is the return value, already serialised. */
    static void builtinResolve(long id, String json) {
        nativeBuiltinResolve(id, json);
    }

    static void builtinReject(long id, String message) {
        nativeBuiltinReject(id, message);
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

    private static native void nativeSetBuiltinModules(AnBuiltinModules modules);

    private static native int nativeBuiltinResolve(long id, String json);

    private static native int nativeBuiltinReject(long id, String message);
}
