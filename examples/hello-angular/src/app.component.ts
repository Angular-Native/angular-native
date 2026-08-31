import { ChangeDetectionStrategy, Component, computed, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, type NativePressEvent } from '@angular-native/primitives'

/**
 * Un componente Angular normal. Lo único distinto es que los elementos son
 * primitivas nativas: la plantilla, las señales, el `@for` y los bindings son
 * exactamente los de siempre.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <View [style.paddingTop]="'64'" [style.paddingHorizontal]="'16'" [style.gap]="'16'"
          [backgroundColor]="'#0b1020'" [style.width]="'100%'" [style.height]="'100%'">

      <Text [fontSize]="28" [fontWeight]="'bold'" [color]="'#f4f7ff'">angular-native</Text>

      <View [style.flexDirection]="'row'" [style.gap]="'12'">
        @for (card of cards; track card.color) {
          <View [style.flexGrow]="card.grow" [style.height]="'88'"
                [backgroundColor]="card.color" [borderRadius]="12"
                (press)="onPress($event)"></View>
        }
      </View>

      <Text [fontSize]="16" [color]="'#f4f7ff'">{{ tapLabel() }}</Text>

      <Text [fontSize]="16" [color]="'#9fb0d4'">
        Esto es una plantilla de Angular con señales, corriendo en QuickJS.
        Cada elemento es una UIView de verdad.
      </Text>

      <Text [fontSize]="16" [color]="'#6ee7b7'">{{ label() }}</Text>

      @if (seconds() >= 3) {
        <Text [fontSize]="14" [color]="'#f59e0b'">El &#64;if entró a los 3 segundos.</Text>
      }
    </View>
  `
})
export class AppComponent {
  readonly cards = [
    { color: '#1e2a4a', grow: 1 },
    { color: '#2b1e4a', grow: 2 }
  ]

  readonly seconds = signal(0)
  readonly label = computed(() => `segundos en marcha: ${this.seconds()}`)

  readonly taps = signal(0)
  readonly lastPoint = signal<NativePressEvent | null>(null)
  readonly tapLabel = computed(() => {
    const point = this.lastPoint()
    if (!point) return 'toca una tarjeta'
    return `toques: ${this.taps()} (último en ${Math.round(point.x)}, ${Math.round(point.y)})`
  })

  onPress(event: NativePressEvent): void {
    this.taps.update((value) => value + 1)
    this.lastPoint.set(event)
  }

  constructor() {
    setInterval(() => this.seconds.update((value) => value + 1), 1000)
  }
}
