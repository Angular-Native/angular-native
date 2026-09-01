import { DestroyRef, inject, signal, type WritableSignal } from '@angular/core'

declare const __an_hot: {
  read(key: string): unknown
  register(key: string, read: () => unknown): () => void
}

/**
 * Una señal que sobrevive a una recarga en caliente.
 *
 * Recargar tira el motor y levanta otro: los componentes son nuevos y sus
 * señales vuelven a su valor inicial. Lo que se declare aquí se guarda antes
 * de tirar el motor y se recupera en el nuevo, así que editar un fichero no
 * borra lo que llevabas escrito ni te devuelve al principio de la lista.
 *
 * En una compilación de producción no cuesta nada: nunca hay recarga, así que
 * el valor guardado nunca se lee.
 *
 * No es el *fast refresh* de React Native. Ese conserva los propios
 * componentes, y para eso hace falta cargar los módulos por separado y
 * sustituir la metadata de los que cambiaron.
 *
 * ```ts
 * readonly borrador = hotState('nota.borrador', '')
 * ```
 *
 * La clave tiene que ser estable entre recargas y única en la app: es lo único
 * que relaciona la señal de antes con la de después.
 */
export function hotState<T>(key: string, initial: T): WritableSignal<T> {
  const saved = typeof __an_hot === 'undefined' ? undefined : __an_hot.read(key)
  const value = signal<T>(saved === undefined ? initial : (saved as T))
  if (typeof __an_hot !== 'undefined') {
    const unregister = __an_hot.register(key, () => value())
    // Si el componente muere antes de la recarga, su estado ya no interesa.
    inject(DestroyRef, { optional: true })?.onDestroy(unregister)
  }
  return value
}

/**
 * Igual, pero fuera de un contexto de inyección: no se da de baja sola, así
 * que es para estado que vive tanto como la app.
 */
export function globalHotState<T>(key: string, initial: T): WritableSignal<T> {
  const saved = typeof __an_hot === 'undefined' ? undefined : __an_hot.read(key)
  const value = signal<T>(saved === undefined ? initial : (saved as T))
  if (typeof __an_hot !== 'undefined') {
    __an_hot.register(key, () => value())
  }
  return value
}
