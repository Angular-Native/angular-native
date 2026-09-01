import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'
import type { NativeTextEvent } from '@angular-native/primitives'

/**
 * Cabecera, texto de varias líneas, navegador embebido y hoja de acciones.
 *
 * La cabecera y el navegador son del sistema; la hoja de acciones la presenta
 * el sistema y no ocupa sitio en el layout. El `<WebView>` es una vista más:
 * ocupa lo que le dé el layout y convive con las demás.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <View [style.width]="'100%'" [style.height]="'100%'" [backgroundColor]="'#0b1020'">
      <SafeArea [edges]="['top', 'left', 'right']" [style.flex]="1">
        <NavigationBar
          [style.height]="44"
          [title]="'Notas'"
          [showsBack]="true"
          [backTitle]="'Atrás'"
          (back)="ultimo.set('atrás')" />

        <View [style.flex]="1" [style.padding]="'16'" [style.gap]="'12'">
          <Text [color]="'#94a3b8'" [fontSize]="14">último: {{ ultimo() }}</Text>

          <TextEditor
            [style.height]="110"
            [color]="'#e2e8f0'"
            [value]="nota()"
            (change)="onNota($event)"></TextEditor>

          <Button
            [title]="'Compartir…'"
            [variant]="'tonal'"
            [color]="'#6ee7b7'"
            [style.height]="44"
            (press)="hoja.set(true)"></Button>

          <WebView [style.flex]="1" [borderRadius]="12" [html]="pagina" />
        </View>
      </SafeArea>

      <Alert
        [visible]="hoja()"
        [sheet]="true"
        [title]="'Compartir la nota'"
        [buttons]="opciones"
        (select)="onOpcion($event)" />
    </View>
  `
})
export class AppComponent {
  readonly opciones = ['Copiar', 'Enviar por correo', 'Cancelar']

  readonly ultimo = signal('nada')
  readonly nota = signal('Un texto de varias líneas.\nLa segunda cabe entera.')
  readonly hoja = signal(false)

  /**
   * HTML suelto en vez de una dirección: así el ejemplo no depende de que haya
   * red, y se ve igual que si viniera de fuera.
   */
  readonly pagina = `
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <body style="margin:0;font:16px -apple-system,system-ui;background:#111a33;color:#cbd5f5">
      <div style="padding:16px">
        <h2 style="margin:0 0 8px">Dentro de un WebView</h2>
        <p style="margin:0;color:#94a3b8">
          Esto lo dibuja el navegador del sistema, no el framework.
        </p>
      </div>
    </body>`

  onNota(event: NativeTextEvent): void {
    this.nota.set(event.value)
    this.ultimo.set(`escribiste ${event.value.length} letras`)
  }

  onOpcion(index: number): void {
    this.hoja.set(false)
    this.ultimo.set(this.opciones[index] ?? 'nada')
  }
}
