import { Location } from '@angular/common'
import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  Injectable,
  signal,
  viewChild,
  type Provider
} from '@angular/core'
import {
  NavigationEnd,
  Router,
  RouteReuseStrategy,
  RouterOutlet,
  type ActivatedRouteSnapshot,
  type DetachedRouteHandle
} from '@angular/router'
import { StackView } from '@angular-native/primitives'

import { NativePlatformLocation } from './location'

/**
 * The direction of the last navigation.
 *
 * The router does not say whether you went forward or back: it says where you
 * went. What tells one from the other is the position in the history stack, and
 * that is where both the animation's direction and the decision to keep or throw
 * away the screen you are leaving come from.
 */
@Injectable({ providedIn: 'root' })
export class NavigationDirection {
  private readonly location = inject(NativePlatformLocation)
  private readonly last = signal(this.location.historyIndex)
  private readonly current = signal<'push' | 'pop' | 'none'>('none')

  readonly direction = computed(() => this.current())

  /**
   * Whether there is a screen underneath this one.
   *
   * The answer belongs to the location, which is what holds the stack; what
   * this adds is that it is a signal. At the bottom there is nowhere for
   * `back()` to go, and the press belongs to whatever the platform does with a
   * back nobody claimed.
   */
  readonly canGoBack = computed(() => {
    // Read for the dependency and not for the value: the index is what changes.
    this.last()
    return this.location.canGoBack
  })

  constructor() {
    inject(Router).events.subscribe((event) => {
      if (!(event instanceof NavigationEnd)) return
      const index = this.location.historyIndex
      const previous = this.last()
      this.last.set(index)
      this.current.set(index > previous ? 'push' : index < previous ? 'pop' : 'none')
    })
  }

}

/**
 * Keeps alive the screens sitting lower in the stack.
 *
 * Without this, going back builds the page again from scratch: the scroll
 * position is lost, so is what was typed into a form and any state the component
 * held. It is the mechanism Angular offers for exactly this, and the same one
 * Ionic uses.
 *
 * On the way back, what was stored for the screen being abandoned is thrown
 * away, since it will not come back down that road: storing without pruning
 * would be a leak.
 */
@Injectable()
export class NativeStackReuseStrategy implements RouteReuseStrategy {
  // It looks at the history directly and not at `NavigationDirection`: that one
  // injects the router, and the router injects this strategy. Reading the index
  // breaks the cycle, and the index is where the direction comes from anyway.
  private readonly location = inject(NativePlatformLocation)
  private readonly stored = new Map<string, DetachedRouteHandle>()
  private lastIndex = this.location.historyIndex

  shouldDetach(): boolean {
    return true
  }

  store(route: ActivatedRouteSnapshot, handle: DetachedRouteHandle | null): void {
    const index = this.location.historyIndex
    const popping = index < this.lastIndex
    this.lastIndex = index

    const key = keyOf(route)
    // On the way back, what was stored for the screen being abandoned is thrown
    // away: it will not come back down that road, and keeping it would be a
    // leak.
    if (!handle || popping) {
      this.stored.delete(key)
      return
    }
    this.stored.set(key, handle)
  }

  shouldAttach(route: ActivatedRouteSnapshot): boolean {
    return this.stored.has(keyOf(route))
  }

  retrieve(route: ActivatedRouteSnapshot): DetachedRouteHandle | null {
    return this.stored.get(keyOf(route)) ?? null
  }

  shouldReuseRoute(future: ActivatedRouteSnapshot, current: ActivatedRouteSnapshot): boolean {
    return future.routeConfig === current.routeConfig
  }
}

/** The full route, which is what identifies one particular screen. */
function keyOf(route: ActivatedRouteSnapshot): string {
  return route.pathFromRoot
    .map((segment) => segment.url.map((part) => part.toString()).join('/'))
    .filter(Boolean)
    .join('/')
}

/**
 * A native navigation stack.
 *
 * It goes where a `<router-outlet>` would go and does the same job, with three
 * differences: the new screen slides in and the previous one slides out behind
 * it, the previous one stays alive so that going back is instant, and iOS's edge
 * gesture and Android's back button navigate backwards.
 *
 * ```ts
 * bootstrapNativeApplication(AppComponent, {
 *   providers: [...NATIVE_LOCATION_PROVIDERS, ...NATIVE_STACK_PROVIDERS, provideRouter(routes)]
 * })
 * ```
 */
@Component({
  selector: 'an-native-stack',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [StackView, RouterOutlet],
  host: {
    '[style.flexGrow]': "'1'",
    '[style.minHeight]': "'0'"
  },
  template: `
    <an-stack-view [style.flexGrow]="'1'" [transition]="direction()">
      <router-outlet />
    </an-stack-view>
  `
})
export class NativeStack {
  private readonly location = inject(Location)
  private readonly navigation = inject(NavigationDirection)
  protected readonly direction = this.navigation.direction
  private readonly stack = viewChild(StackView)

  constructor() {
    // `(back)` is not in the template because it must not always be bound.
    //
    // Subscribing is how the app claims the press: the listener travels to the
    // host, and every host that has a system back consults it before doing
    // anything of its own. Android takes its `OnBackInvokedCallback` off the
    // dispatcher while nothing is listening, and only then does the system draw
    // the back-to-home preview; tvOS lets the menu button reach the platform,
    // which is what returns to the Apple TV home screen. Bound at the root of
    // the stack, the press is answered with a `back()` that has nowhere to go,
    // and the person gets neither the app's answer nor the system's.
    //
    // The condition is `canGoBack` and not something weaker on purpose: it is
    // exactly the condition under which `location.back()` does anything at all.
    effect((onCleanup) => {
      const stack = this.stack()
      if (!stack || !this.navigation.canGoBack()) return
      const subscription = stack.back.subscribe(() => this.location.back())
      onCleanup(() => subscription.unsubscribe())
    })
  }
}

/** What has to go into the providers for `NativeStack` to work. */
export const NATIVE_STACK_PROVIDERS: Provider[] = [
  { provide: RouteReuseStrategy, useClass: NativeStackReuseStrategy }
]
