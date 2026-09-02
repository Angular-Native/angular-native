import { Location } from '@angular/common'
import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  Injectable,
  signal,
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
    <an-stack-view [style.flexGrow]="'1'" [transition]="direction()" (back)="onBack()">
      <router-outlet />
    </an-stack-view>
  `
})
export class NativeStack {
  private readonly location = inject(Location)
  protected readonly direction = inject(NavigationDirection).direction

  protected onBack(): void {
    this.location.back()
  }
}

/** What has to go into the providers for `NativeStack` to work. */
export const NATIVE_STACK_PROVIDERS: Provider[] = [
  { provide: RouteReuseStrategy, useClass: NativeStackReuseStrategy }
]
