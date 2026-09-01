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
    <View
      [style.width]="'100%'"
      [style.height]="'100%'"
      [backgroundColor]="'#0b1020'">

      <SafeArea [edges]="['top']">
      <View [style.flexGrow]="'1'" [style.padding]="'16'" [style.gap]="'18'">
        <Text
          [fontSize]="24"
          [fontWeight]="'bold'"
          [letterSpacing]="2"
          [lineHeight]="34"
          [color]="'#f4f7ff'">
          {{ tabTitles[tab()] }}
        </Text>

        <View [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'12'">
          <Text [fontSize]="16" [color]="'#9fb0d4'" [style.flexGrow]="'1'">Notificaciones</Text>
          <Switch
            [on]="notify()"
            [color]="'#6ee7b7'"
            [thumbColor]="'#0b1020'"
            [android]="{ trackColor: '#334155' }"
            (onChange)="notify.set($event)" />
        </View>

        <View [style.gap]="'6'">
          <Text [fontSize]="16" [color]="'#9fb0d4'">Volumen: {{ volumeLabel() }}</Text>
          <Slider
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
        </View>

        <View [style.gap]="'6'">
          <Text [fontSize]="16" [color]="'#9fb0d4'">Descarga</Text>
          <ProgressBar [style.width]="'100%'" [progress]="volume() / 100" [color]="'#6ee7b7'" />
        </View>

        <View [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'12'">
          <ActivityIndicator [animating]="notify()" [color]="'#f4f7ff'" />
          <Text [fontSize]="14" [color]="'#6b7a99'">
            {{ notify() ? 'trabajando…' : 'en reposo' }}
          </Text>
        </View>

        <View [style.flexDirection]="'row'" [style.gap]="'12'">
          <Button
            [style.flexGrow]="'1'"
            [title]="'Modal'"
            [variant]="'filled'"
            [icon]="'star'"
            [fontSize]="17"
            [fontWeight]="'bold'"
            [color]="'#6ee7b7'"
            [ios]="{ subtitle: 'a pantalla completa' }"
            [android]="{ rippleColor: '#ffffff55', allCaps: false }"
            (press)="modal.set(true)"></Button>
          <Button
            [style.flexGrow]="'1'"
            [title]="'Diálogo'"
            [variant]="'outlined'"
            [icon]="'settings'"
            [iconPosition]="'trailing'"
            [enabled]="notify()"
            [color]="'#6ee7b7'"
            (press)="alert.set(true)"></Button>
        </View>

        <Text [fontSize]="13" [color]="'#6b7a99'">{{ answer() }}</Text>
      </View>
      </SafeArea>

      <View [style.flexDirection]="'row'" [style.gap]="'18'" [style.alignItems]="'center'">
        <Icon [name]="'home'" [color]="'#6ee7b7'" />
        <Icon [name]="'search'" [size]="32" [color]="'#9fb0d4'" />
        <Icon [name]="'settings'" [size]="40" [color]="'#f4f7ff'" />
        <Icon [name]="'star'" [size]="28" [color]="'#fbbf24'" />
        <Icon [name]="'share'" [size]="28" [color]="'#60a5fa'" />
      </View>

      <TabBar
        [style.width]="'100%'"
        [items]="tabTitles"
        [icons]="tabIcons"
        [selectedIndex]="tab()"
        [color]="'#6ee7b7'"
        (select)="tab.set($event)" />

      <Alert
        [visible]="alert()"
        [title]="'Confirmar'"
        [message]="'Esto lo presenta el sistema, no el framework.'"
        [buttons]="['Aceptar', 'Cancelar']"
        (select)="onAnswer($event)" />

      <Modal
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
        <View
          [style.padding]="'24'"
          [style.gap]="'16'"
          [style.width]="'80%'"
          [backgroundColor]="'#141c33'"
          [borderRadius]="16">
          <Text [fontSize]="18" [fontWeight]="'bold'" [color]="'#f4f7ff'">Presentado</Text>
          <Text [fontSize]="15" [color]="'#9fb0d4'">
            Un UIViewController en iOS y un Dialog en Android, no una vista
            puesta encima.
          </Text>
          <Button [title]="'Cerrar'" [color]="'#6ee7b7'" (press)="modal.set(false)"></Button>
        </View>
      </Modal>
    </View>
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
