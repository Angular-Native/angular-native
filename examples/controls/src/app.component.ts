import { ChangeDetectionStrategy, Component, computed, signal } from '@angular/core'
import { hotState } from '@angular-native/platform'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'

/**
 * Todos los controles del sistema en una pantalla.
 *
 * Ninguno está dibujado por el framework: son `UITabBar`, `UISwitch`,
 * `UISlider`, `UIActivityIndicatorView`, `UIProgressView` y `UIButton` de
 * verdad, con sus tamaños naturales, y sus equivalentes en Android.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [backgroundColor]="'#0b1020'">

      <an-safe-area [edges]="['top']">
      <an-scroll-view
        [style.flexGrow]="'1'"
        [style.overflow]="'scroll'"
        [scrollEnabled]="true"
        [showsScrollIndicator]="false"
        [ios]="{ pagingEnabled: false, keyboardDismissMode: 'onDrag' }">
      <an-view [style.flexGrow]="'1'" [style.padding]="'16'" [style.gap]="'18'">
        <an-text
          [fontSize]="24"
          [fontWeight]="'bold'"
          [letterSpacing]="2"
          [lineHeight]="34"
          [color]="'#f4f7ff'">
          {{ tabTitles[tab()] }}
        </an-text>

        <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'12'">
          <an-text [fontSize]="16" [color]="'#9fb0d4'" [style.flexGrow]="'1'">Notificaciones</an-text>
          <an-switch
            [on]="notify()"
            [color]="'#6ee7b7'"
            [thumbColor]="'#0b1020'"
            [android]="{ trackColor: '#334155' }"
            (onChange)="notify.set($event)" />
        </an-view>

        <an-view [style.gap]="'6'">
          <an-text [fontSize]="16" [color]="'#9fb0d4'">Volumen: {{ volumeLabel() }}</an-text>
          <an-slider
            [style.width]="'100%'"
            [value]="volume()"
            [minimumValue]="0"
            [maximumValue]="100"
            [color]="'#6ee7b7'"
            [minimumTrackColor]="'#6ee7b7'"
            [maximumTrackColor]="'#1e2a4a'"
            [thumbColor]="'#f4f7ff'"
            [ios]="{ continuous: true }"
            [android]="{ stepSize: 5 }"
            (valueChange)="volume.set($event)" />
        </an-view>

        <an-view [style.gap]="'6'">
          <an-text [fontSize]="16" [color]="'#9fb0d4'">Descarga</an-text>
          <an-progress-bar [style.width]="'100%'" [progress]="volume() / 100" [color]="'#6ee7b7'" />
        </an-view>

        <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'12'">
          <an-activity-indicator [animating]="notify()" [color]="'#f4f7ff'" />
          <an-text [fontSize]="14" [color]="'#6b7a99'">
            {{ notify() ? 'trabajando…' : 'en reposo' }}
          </an-text>
        </an-view>

        <!--
          El botón del subtítulo lleva alto propio: el tamaño natural de un
          control se pregunta una vez al arrancar, con uno de muestra, y ese no
          sabe que este va a llevar dos líneas.
        -->
        <an-view
          [style.flexDirection]="'row'"
          [style.alignItems]="'flex-start'"
          [style.gap]="'12'">
          <an-button
            [style.flexGrow]="'1'"
            [style.height]="'58'"
            [title]="'Modal'"
            [variant]="'filled'"
            [icon]="'star'"
            [fontSize]="17"
            [fontWeight]="'bold'"
            [color]="'#6ee7b7'"
            [ios]="{ subtitle: 'a pantalla completa' }"
            [android]="{ rippleColor: '#ffffff55', allCaps: false }"
            (press)="modal.set(true)"></an-button>
          <an-button
            [style.flexGrow]="'1'"
            [title]="'Diálogo'"
            [variant]="'outlined'"
            [icon]="'settings'"
            [iconPosition]="'trailing'"
            [enabled]="notify()"
            [color]="'#6ee7b7'"
            (press)="alert.set(true)"></an-button>
        </an-view>

        <!--
          Las cuatro variantes, con el mismo rótulo y el mismo color, para ver
          de un vistazo que las cuatro lo dibujan: la de relleno invierte el
          color del texto y las otras tres lo pintan del color pedido.
        -->
        <an-view [style.flexDirection]="'row'" [style.gap]="'8'">
          <an-button
            [style.flexGrow]="'1'"
            [title]="'Texto'"
            [variant]="'text'"
            [color]="'#6ee7b7'"></an-button>
          <an-button
            [style.flexGrow]="'1'"
            [title]="'Relleno'"
            [variant]="'filled'"
            [color]="'#6ee7b7'"></an-button>
          <an-button
            [style.flexGrow]="'1'"
            [title]="'Tonal'"
            [variant]="'tonal'"
            [color]="'#6ee7b7'"></an-button>
          <an-button
            [style.flexGrow]="'1'"
            [title]="'Borde'"
            [variant]="'outlined'"
            [color]="'#6ee7b7'"></an-button>
        </an-view>

        <an-text
          [fontSize]="13"
          [color]="'#6b7a99'"
          [textDecoration]="'underline'"
          [android]="{ selectable: true }">{{ answer() }}</an-text>
      </an-view>
      </an-scroll-view>
      </an-safe-area>

      <an-view [style.flexDirection]="'row'" [style.gap]="'18'" [style.alignItems]="'center'">
        <an-icon [name]="'home'" [color]="'#6ee7b7'" />
        <an-icon [name]="'search'" [size]="32" [color]="'#9fb0d4'" />
        <an-icon [name]="'settings'" [size]="40" [color]="'#f4f7ff'" />
        <an-icon [name]="'star'" [size]="28" [color]="'#fbbf24'" />
        <an-icon [name]="'share'" [size]="28" [color]="'#60a5fa'" />
      </an-view>

      <an-tab-bar
        [style.width]="'100%'"
        [items]="tabTitles"
        [icons]="tabIcons"
        [selectedIndex]="tab()"
        [color]="'#6ee7b7'"
        [unselectedColor]="'#6b7a99'"
        [ios]="{ translucent: true }"
        (select)="tab.set($event)" />

      <an-alert
        [visible]="alert()"
        [title]="'Confirmar'"
        [message]="'Esto lo presenta el sistema, no el framework.'"
        [buttons]="['Aceptar', 'Cancelar']"
        (select)="onAnswer($event)" />

      <an-modal
        [visible]="modal()"
        [presentation]="'fullScreen'"
        (dismiss)="modal.set(false)"
        [style.position]="'absolute'"
        [style.top]="'0'"
        [style.left]="'0'"
        [style.width]="'100%'"
        [style.height]="'100%'"
        [style.justifyContent]="'center'"
        [style.alignItems]="'center'"
        [backgroundColor]="'#000000cc'">
        <an-view
          [style.padding]="'24'"
          [style.gap]="'16'"
          [style.width]="'80%'"
          [backgroundColor]="'#141c33'"
          [borderRadius]="16">
          <an-text [fontSize]="18" [fontWeight]="'bold'" [color]="'#f4f7ff'">Presentado</an-text>
          <an-text [fontSize]="15" [color]="'#9fb0d4'">
            Un UIViewController en iOS y un Dialog en Android, no una vista
            puesta encima.
          </an-text>
          <an-button [title]="'Cerrar'" [color]="'#6ee7b7'" (press)="modal.set(false)"></an-button>
        </an-view>
      </an-modal>
    </an-view>
  `
})
export class AppComponent {
  readonly tabTitles = ['Ajustes', 'Actividad', 'Cuenta']

  // Estas sobreviven a una recarga en caliente: al guardar un fichero, la
  // pestaña y el volumen siguen donde estaban.
  readonly tab = hotState('controls.tab', 0)
  readonly volume = hotState('controls.volume', 35)

  readonly notify = signal(true)
  readonly modal = signal(false)
  readonly tabIcons = ['settings', 'list', 'profile']
  readonly alert = signal(false)
  readonly answer = signal('sin respuesta todavía')

  readonly volumeLabel = computed(() => `${Math.round(this.volume())}%`)

  onAnswer(index: number): void {
    this.alert.set(false)
    this.answer.set(index === 0 ? 'aceptaste' : 'cancelaste')
  }
}
