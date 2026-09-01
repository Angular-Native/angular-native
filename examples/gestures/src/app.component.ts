import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'
import type {
  NativePanEvent,
  NativePinchEvent,
  NativeRotateEvent
} from '@angular-native/primitives'

/**
 * Gestos y transformaciones.
 *
 * La tarjeta se mueve con un dedo, se escala y se gira con dos. Ninguna de las
 * tres cosas toca el layout: `translateX`, `scale` y `rotate` se aplican sobre
 * la vista ya colocada, así que seguir al dedo no recalcula nada.
 *
 * Los reconocedores son los del sistema —`UIPanGestureRecognizer` y compañía en
 * iOS, `GestureDetector` en Android—, así que los umbrales de cuándo un
 * arrastre empieza a contar son los de cada plataforma.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <View
      [style.width]="'100%'"
      [style.height]="'100%'"
      [backgroundColor]="'#0b1020'">
      <SafeArea
        [style.flex]="1"
        [style.padding]="20"
        [style.gap]="14">
        <Text [fontSize]="26" [fontWeight]="700" [color]="'#f8fafc'">
          gestos
        </Text>

        <Text [color]="'#94a3b8'" [fontSize]="15">
          {{ status() }}
        </Text>

        <View
          [style.height]="320"
          [backgroundColor]="'#111a33'"
          [borderRadius]="16"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'">
          <View
            [style.width]="140"
            [style.height]="140"
            [backgroundColor]="'#6366f1'"
            [borderRadius]="20"
            [style.alignItems]="'center'"
            [style.justifyContent]="'center'"
            [translateX]="x()"
            [translateY]="y()"
            [scale]="scale()"
            [rotate]="angle()"
            (pan)="onPan($event)"
            (pinch)="onPinch($event)"
            (rotation)="onRotate($event)"
            (doublePress)="reset()"
            (longPress)="onLongPress()">
            <Text [color]="'#ffffff'" [fontWeight]="600">
              {{ label() }}
            </Text>
          </View>
        </View>

        <View
          [style.height]="72"
          [backgroundColor]="'#1e293b'"
          [borderRadius]="12"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          (swipeLeft)="onSwipe('izquierda')"
          (swipeRight)="onSwipe('derecha')"
          (swipeUp)="onSwipe('arriba')"
          (swipeDown)="onSwipe('abajo')">
          <!--
            Este va con \`[style.fontSize]\` a propósito. Lo normal y lo
            recomendado es la entrada tipada \`[fontSize]\`, que el compilador
            comprueba; esto está aquí para que el otro camino —el que Angular
            deja escribir siempre— no se quede en nada sin avisar.
          -->
          <Text [color]="'#cbd5f5'" [style.fontSize]="18">desliza aquí: {{ swipe() }}</Text>
        </View>
      </SafeArea>
    </View>
  `
})
export class AppComponent {
  // Lo acumulado de gestos anteriores, y lo que va del gesto actual. Se
  // guardan aparte porque cada gesto manda su desplazamiento desde donde
  // empezó: sumarlo directamente contaría dos veces lo mismo en cada evento.
  private committedX = 0
  private committedY = 0
  private committedScale = 1
  private committedAngle = 0

  readonly x = signal(0)
  readonly y = signal(0)
  readonly scale = signal(1)
  readonly angle = signal(0)

  readonly label = signal('arrástrame')
  readonly status = signal('un dedo mueve; dos escalan y giran')
  readonly swipe = signal('—')


  onPan(event: NativePanEvent): void {
    this.x.set(this.committedX + event.translationX)
    this.y.set(this.committedY + event.translationY)
    if (event.state === 'end') {
      this.committedX = this.x()
      this.committedY = this.y()
      this.status.set(`soltado a ${Math.round(event.velocityX)} pt/s`)
    }
    if (event.state === 'cancel') {
      // El sistema se llevó el gesto: se vuelve a donde estaba, no se deja a
      // medias donde el dedo dejó de contar.
      this.x.set(this.committedX)
      this.y.set(this.committedY)
    }
  }

  onPinch(event: NativePinchEvent): void {
    this.scale.set(this.committedScale * event.scale)
    if (event.state === 'end') {
      this.committedScale = this.scale()
    }
  }

  onRotate(event: NativeRotateEvent): void {
    this.angle.set(this.committedAngle + event.rotation)
    if (event.state === 'end') {
      this.committedAngle = this.angle()
    }
  }

  onLongPress(): void {
    this.label.set('mantenido')
    this.status.set('pulsación larga')
  }

  onSwipe(direction: string): void {
    this.swipe.set(direction)
  }

  /** Dos toques devuelven la tarjeta a su sitio. */
  reset(): void {
    this.committedX = 0
    this.committedY = 0
    this.committedScale = 1
    this.committedAngle = 0
    this.x.set(0)
    this.y.set(0)
    this.scale.set(1)
    this.angle.set(0)
    this.label.set('arrástrame')
    this.status.set('a cero')
  }
}
