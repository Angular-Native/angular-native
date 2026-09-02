import { Injectable } from '@angular/core'

import { globalHotState } from './hot-state'
import {
  APP_BASE_HREF,
  LocationStrategy,
  PathLocationStrategy,
  PlatformLocation,
  type LocationChangeListener
} from '@angular/common'
import type { Provider } from '@angular/core'

/** History saved by an earlier reload, if there was one. */
interface SavedHistory {
  stack: Array<{ url: string; state: unknown }>
  index: number
}

let historySource: (() => SavedHistory) | null = null
const savedHistory = globalHotState<SavedHistory | null>('an.history', null)

function restoreHistory(): SavedHistory | null {
  return savedHistory()
}

function rememberHistory(read: () => SavedHistory): void {
  historySource = read
  savedHistory.set(read())
  // The signal is read on reload, so keeping it up to date is enough.
  const original = historySource
  savedHistory.update(() => original())
}

/**
 * `PlatformLocation` over an in-memory stack.
 *
 * Angular's router is written against the browser's History API. There is no
 * address bar and no system history here, so it gets a stack of its own: routes
 * are still URLs, but they exist only inside the app.
 *
 * It is also the point iOS's back gesture and Android's physical button will
 * come in through, since both have to end up calling `back()`.
 */
@Injectable()
export class NativePlatformLocation extends PlatformLocation {
  private stack: Array<{ url: string; state: unknown }> = [{ url: '/', state: null }]
  private index = 0
  private readonly listeners: LocationChangeListener[] = []

  constructor() {
    super()
    // The stack survives a hot reload: being thrown back to the start of the
    // app every time you save a file is the first thing that grates.
    const saved = restoreHistory()
    if (saved) {
      this.stack = saved.stack
      this.index = saved.index
    }
    rememberHistory(() => ({ stack: this.stack, index: this.index }))
  }

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

  /** There are no fragments to listen to: with no address bar there is no `#`. */
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
    this.remember()
  }

  private remember(): void {
    if (historySource) savedHistory.set(historySource())
  }

  override pushState(state: unknown, _title: string, url: string): void {
    // Navigating from the middle of the stack throws away whatever was ahead of
    // it, just as in a browser.
    this.stack.length = this.index + 1
    this.stack.push({ url, state })
    this.index = this.stack.length - 1
    this.remember()
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
    this.remember()
    // The router listens here in order to undo the navigation.
    for (const listener of this.listeners) {
      listener({ type: 'popstate', state: this.stack[next].state })
    }
  }

  /** `true` if there is something to go back to: the back gesture uses it. */
  get canGoBack(): boolean {
    return this.index > 0
  }

  /**
   * The current position in the stack. Comparing indices between navigations is
   * what tells going forward from going back, and that is where the animation's
   * direction comes from.
   */
  get historyIndex(): number {
    return this.index
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
 * What has to go into the providers for Angular's router to work here. It goes
 * alongside `provideRouter(routes)`.
 */
export const NATIVE_LOCATION_PROVIDERS: Provider[] = [
  // The concrete class is registered as well as the token: the navigation stack
  // needs the history index, which `PlatformLocation` does not expose.
  NativePlatformLocation,
  { provide: PlatformLocation, useExisting: NativePlatformLocation },
  { provide: LocationStrategy, useClass: PathLocationStrategy },
  { provide: APP_BASE_HREF, useValue: '/' }
]
