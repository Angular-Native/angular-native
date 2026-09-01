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
 * Sentido de la última navegación.
 *
 * El router no dice si se avanzó o se retrocedió: dice a dónde se fue. Lo que
 * distingue una cosa de otra es la posición en la pila del historial, y de ahí
 * sale tanto el sentido de la animación como si hay que guardar la pantalla
 * que se deja o tirarla.
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
 * Mantiene vivas las pantallas que quedan por debajo en la pila.
 *
 * Sin esto, volver atrás recrea la página desde cero: se pierde el scroll, lo
 * escrito en un formulario y cualquier estado del componente. Es el mecanismo
 * que Angular ofrece para exactamente esto, y el mismo que usa Ionic.
 *
 * Al retroceder se tira lo guardado de la pantalla que se abandona, que ya no
 * volverá por ese camino: guardar sin podar sería una fuga.
 */
@Injectable()
export class NativeStackReuseStrategy implements RouteReuseStrategy {
  // Mira el historial directamente y no `NavigationDirection`: esa inyecta el
  // router, y el router inyecta esta estrategia. El ciclo lo rompe leer el
  // índice, que es de donde sale el sentido de todas formas.
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
    // Al retroceder se tira lo guardado de la pantalla que se abandona: ya no
    // volverá por ese camino, y guardarla sería una fuga.
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

/** La ruta completa, que es lo que identifica una pantalla concreta. */
function keyOf(route: ActivatedRouteSnapshot): string {
  return route.pathFromRoot
    .map((segment) => segment.url.map((part) => part.toString()).join('/'))
    .filter(Boolean)
    .join('/')
}

/**
 * Pila de navegación nativa.
 *
 * Va donde iría un `<router-outlet>` y hace lo mismo, con tres diferencias: la
 * pantalla nueva entra deslizándose y la anterior se desplaza detrás, la
 * anterior sigue viva para que volver sea instantáneo, y el gesto de borde de
 * iOS y el botón de atrás de Android navegan hacia atrás.
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

/** Lo que hay que añadir a los providers para que `NativeStack` funcione. */
export const NATIVE_STACK_PROVIDERS: Provider[] = [
  { provide: RouteReuseStrategy, useClass: NativeStackReuseStrategy }
]
