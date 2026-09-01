import { ChangeDetectionStrategy, Component, computed, signal } from '@angular/core'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

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
  imports: [NATIVE_PRIMITIVES],
  template: `
    <View
      [style.width]="'100%'"
      [style.height]="'100%'"
      [backgroundColor]="'#0b1020'">

      <View [style.flexGrow]="'1'" [style.padding]="'16'" [style.paddingTop]="'64'" [style.gap]="'18'">
        <Text [fontSize]="24" [fontWeight]="'bold'" [color]="'#f4f7ff'">
          {{ tabTitles[tab()] }}
        </Text>

        <View [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'12'">
          <Text [fontSize]="16" [color]="'#9fb0d4'" [style.flexGrow]="'1'">Notificaciones</Text>
          <Switch [on]="notify()" [color]="'#6ee7b7'" (onChange)="notify.set($event)" />
        </View>

        <View [style.gap]="'6'">
          <Text [fontSize]="16" [color]="'#9fb0d4'">Volumen: {{ volumeLabel() }}</Text>
          <Slider
            [style.width]="'100%'"
            [value]="volume()"
            [minimumValue]="0"
            [maximumValue]="100"
            [color]="'#6ee7b7'"
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

        <Button
          [style.width]="'100%'"
          [title]="'Abrir la capa'"
          [color]="'#6ee7b7'"
          (press)="modal.set(true)"></Button>
      </View>

      <TabBar
        [style.width]="'100%'"
        [items]="tabTitles"
        [selectedIndex]="tab()"
        [color]="'#6ee7b7'"
        (select)="tab.set($event)" />

      <Modal
        [visible]="modal()"
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
          <Text [fontSize]="18" [fontWeight]="'bold'" [color]="'#f4f7ff'">Una capa</Text>
          <Text [fontSize]="15" [color]="'#9fb0d4'">
            Montada sobre la raíz, no presentada como controlador.
          </Text>
          <Button [title]="'Cerrar'" [color]="'#6ee7b7'" (press)="modal.set(false)"></Button>
        </View>
      </Modal>
    </View>
  `
})
export class AppComponent {
  readonly tabTitles = ['Ajustes', 'Actividad', 'Cuenta']

  readonly tab = signal(0)
  readonly notify = signal(true)
  readonly volume = signal(35)
  readonly modal = signal(false)

  readonly volumeLabel = computed(() => `${Math.round(this.volume())}%`)
}
