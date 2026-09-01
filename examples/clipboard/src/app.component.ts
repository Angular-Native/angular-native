import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core'
import { Clipboard } from '@angular-native/plugin-clipboard'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

const FRASES = [
  'hola desde angular-native',
  'un plugin es un paquete npm',
  'Swift y Java, compilados desde fuente'
]

/**
 * El portapapeles del sistema, desde un plugin.
 *
 * La app no sabe que hay un plugin de por medio: inyecta `Clipboard` y llama a
 * sus métodos, igual que inyectaría `Device`. Lo único que la distingue de una
 * app cualquiera es la línea del `package.json` que declara la dependencia; de
 * ahí sale todo lo demás, incluido que `an` compile el Swift y el Java.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <View
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.paddingTop]="'72'"
      [style.paddingHorizontal]="'20'"
      [style.gap]="'16'"
      [backgroundColor]="'#0b1020'">

      <Text [fontSize]="28" [fontWeight]="'bold'" [color]="'#f4f7ff'">portapapeles</Text>

      <Text [fontSize]="14" [color]="'#9fb0d4'">
        un plugin: paquete npm, Swift, Java y esta línea de TypeScript
      </Text>

      <View
        [style.paddingVertical]="'14'"
        [style.paddingHorizontal]="'14'"
        [backgroundColor]="'#1e2a4a'"
        [borderRadius]="10">
        <Text [fontSize]="17" [color]="'#f4f7ff'">{{ texto() }}</Text>
      </View>

      <View [style.flexDirection]="'row'" [style.gap]="'12'">
        <View
          [style.flexGrow]="'1'"
          [style.height]="'46'"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          [backgroundColor]="'#2f6fed'"
          [borderRadius]="10"
          (press)="copiar()">
          <Text [fontSize]="16" [fontWeight]="'600'" [color]="'#ffffff'">copiar</Text>
        </View>

        <View
          [style.flexGrow]="'1'"
          [style.height]="'46'"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="10"
          (press)="pegar()">
          <Text [fontSize]="16" [fontWeight]="'600'" [color]="'#9fb0d4'">pegar</Text>
        </View>

        <View
          [style.flexGrow]="'1'"
          [style.height]="'46'"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="10"
          (press)="siguiente()">
          <Text [fontSize]="16" [fontWeight]="'600'" [color]="'#9fb0d4'">otra frase</Text>
        </View>
      </View>

      <Text [fontSize]="15" [color]="'#6ee7b7'">en el portapapeles: {{ contenido() }}</Text>
      <Text [fontSize]="13" [color]="'#f2b8b5'">{{ estado() }}</Text>
    </View>
  `
})
export class AppComponent {
  private readonly clipboard = inject(Clipboard)

  private readonly frase = signal(0)
  readonly texto = computed(() => FRASES[this.frase() % FRASES.length])
  readonly contenido = signal('…')
  readonly estado = signal('')

  constructor() {
    // Al arrancar se pregunta si hay algo, no qué hay: en iOS 16 y posteriores
    // leer lo que copió otra app enseña un aviso del sistema, y salirle a
    // alguien nada más abrir la app sin que haya pedido nada está feo.
    // `hasText` no lo dispara. La llamada no bloquea: la respuesta llega en un
    // frame, el mismo o uno posterior.
    this.clipboard
      .hasText()
      .then((hay) => this.estado.set(hay ? 'hay algo copiado' : 'el portapapeles está vacío'))
      .catch((error: unknown) => this.fallo(error))
  }

  copiar(): void {
    this.clipboard
      .write(this.texto())
      .then(() => {
        this.estado.set('copiado')
        // Releer lo que acaba de escribir la propia app no dispara el aviso.
        return this.pegar()
      })
      .catch((error: unknown) => this.fallo(error))
  }

  pegar(): void {
    this.clipboard
      .read()
      .then((texto) => this.contenido.set(texto === '' ? '(vacío)' : texto))
      .catch((error: unknown) => this.fallo(error))
  }

  siguiente(): void {
    this.frase.update((actual) => actual + 1)
  }

  /**
   * Un plugin que falta no se disimula: se enseña el motivo. Sin plugin en el
   * `.app` la promesa se rechaza, y eso es exactamente lo que tiene que pasar.
   */
  private fallo(error: unknown): void {
    this.contenido.set('—')
    this.estado.set(`el portapapeles falló: ${error}`)
  }
}
