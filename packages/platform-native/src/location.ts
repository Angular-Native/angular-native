import { Injectable } from '@angular/core'
import {
  APP_BASE_HREF,
  LocationStrategy,
  PathLocationStrategy,
  PlatformLocation,
  type LocationChangeListener
} from '@angular/common'
import type { Provider } from '@angular/core'

/**
 * `PlatformLocation` sobre una pila en memoria.
 *
 * El router de Angular está escrito contra la History API del navegador. Aquí
 * no hay barra de direcciones ni historial del sistema, así que se le da una
 * pila propia: las rutas siguen siendo URLs, pero solo existen dentro de la
 * app.
 *
 * Es también el punto por donde entrará el gesto de volver atrás de iOS y el
 * botón físico de Android, que tienen que acabar llamando a `back()`.
 */
@Injectable()
export class NativePlatformLocation extends PlatformLocation {
  private stack: Array<{ url: string; state: unknown }> = [{ url: '/', state: null }]
  private index = 0
  private readonly listeners: LocationChangeListener[] = []

  override getBaseHrefFromDOM(): string {
    return '/'
  }

  override getState(): unknown {
    return this.stack[this.index].state
  }

  override onPopState(fn: LocationChangeListener): VoidFunction {
    this.listeners.push(fn)
    return () => {
      const at = this.listeners.indexOf(fn)
      if (at !== -1) this.listeners.splice(at, 1)
    }
  }

  /** No hay fragmentos que escuchar: sin barra de direcciones no hay `#`. */
  override onHashChange(): VoidFunction {
    return () => {}
  }

  override get href(): string {
    return this.stack[this.index].url
  }

  override get protocol(): string {
    return 'app:'
  }

  override get hostname(): string {
    return 'localhost'
  }

  override get port(): string {
    return ''
  }

  override get pathname(): string {
    return this.split().pathname
  }

  override set pathname(value: string) {
    this.replaceState(null, '', value)
  }

  override get search(): string {
    return this.split().search
  }

  override get hash(): string {
    return this.split().hash
  }

  override replaceState(state: unknown, _title: string, url: string): void {
    this.stack[this.index] = { url, state }
  }

  override pushState(state: unknown, _title: string, url: string): void {
    // Navegar desde el medio de la pila descarta lo que hubiera delante, igual
    // que en un navegador.
    this.stack.length = this.index + 1
    this.stack.push({ url, state })
    this.index = this.stack.length - 1
  }

  override forward(): void {
    this.historyGo(1)
  }

  override back(): void {
    this.historyGo(-1)
  }

  override historyGo(relativePosition = 0): void {
    const next = this.index + relativePosition
    if (next < 0 || next >= this.stack.length) return
    this.index = next
    // El router escucha aquí para deshacer la navegación.
    for (const listener of this.listeners) {
      listener({ type: 'popstate', state: this.stack[next].state })
    }
  }

  /** `true` si hay algo a lo que volver: lo usa el gesto de atrás. */
  get canGoBack(): boolean {
    return this.index > 0
  }

  private split(): { pathname: string; search: string; hash: string } {
    const url = this.stack[this.index].url
    const hashAt = url.indexOf('#')
    const hash = hashAt === -1 ? '' : url.slice(hashAt)
    const withoutHash = hashAt === -1 ? url : url.slice(0, hashAt)
    const searchAt = withoutHash.indexOf('?')
    return {
      pathname: searchAt === -1 ? withoutHash : withoutHash.slice(0, searchAt),
      search: searchAt === -1 ? '' : withoutHash.slice(searchAt),
      hash
    }
  }
}

/**
 * Lo que hay que añadir a los providers para que el router de Angular
 * funcione aquí. Va junto a `provideRouter(routes)`.
 */
export const NATIVE_LOCATION_PROVIDERS: Provider[] = [
  { provide: PlatformLocation, useClass: NativePlatformLocation },
  { provide: LocationStrategy, useClass: PathLocationStrategy },
  { provide: APP_BASE_HREF, useValue: '/' }
]
