import { DestroyRef, inject, signal, type WritableSignal } from '@angular/core'

declare const __an_hot: {
  read(key: string): unknown
  register(key: string, read: () => unknown): () => void
}

/**
 * A signal that survives a hot reload.
 *
 * Reloading throws the engine away and stands up another one: the components are
 * new and their signals go back to their initial value. Whatever is declared
 * here is saved before the engine is thrown away and recovered in the new one,
 * so editing a file neither wipes what you had typed nor sends you back to the
 * top of the list.
 *
 * In a production build it costs nothing: there is never a reload, so the saved
 * value is never read.
 *
 * This is not React Native's *fast refresh*. That one keeps the components
 * themselves, and doing so means loading the modules separately and replacing
 * the metadata of the ones that changed.
 *
 * ```ts
 * readonly draft = hotState('note.draft', '')
 * ```
 *
 * The key has to be stable across reloads and unique in the app: it is the only
 * thing tying the signal from before to the one from after.
 */
export function hotState<T>(key: string, initial: T): WritableSignal<T> {
  const saved = typeof __an_hot === 'undefined' ? undefined : __an_hot.read(key)
  const value = signal<T>(saved === undefined ? initial : (saved as T))
  if (typeof __an_hot !== 'undefined') {
    const unregister = __an_hot.register(key, () => value())
    // If the component dies before the reload, its state is of no interest.
    inject(DestroyRef, { optional: true })?.onDestroy(unregister)
  }
  return value
}

/**
 * The same thing, but outside an injection context: it does not unregister
 * itself, so it is for state that lives as long as the app does.
 */
export function globalHotState<T>(key: string, initial: T): WritableSignal<T> {
  const saved = typeof __an_hot === 'undefined' ? undefined : __an_hot.read(key)
  const value = signal<T>(saved === undefined ? initial : (saved as T))
  if (typeof __an_hot !== 'undefined') {
    __an_hot.register(key, () => value())
  }
  return value
}
