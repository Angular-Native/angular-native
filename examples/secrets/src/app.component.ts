import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core'
import {
  Biometrics,
  type BiometricAvailability,
  type BiometricOutcome
} from '@angular-native/plugin-biometrics'
import { Keychain, type KeychainReadOutcome } from '@angular-native/plugin-keychain'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/** La clave con la que se guarda. Una app de verdad tendría más de una. */
const CLAVE = 'token-de-sesion'

/** Lo que se guarda. Aquí es de mentira; en una app sería el testigo de sesión. */
const SECRETO = 'sk_live_9f3a-nadie-deberia-ver-esto'

/**
 * Cada final de una autenticación, dicho para quien está delante.
 *
 * Es la razón de que `authenticate` no devuelva un booleano: las once
 * situaciones piden once frases, y cuatro de ellas piden además que la app
 * haga algo distinto —mandar a Ajustes, ofrecer el código, esconder el botón—.
 * Con un `true`/`false` esta tabla no se podría escribir.
 */
const EXPLICACION: Record<BiometricOutcome, string> = {
  success: 'autenticado',
  failed: 'no te ha reconocido',
  userCancel: 'lo has cancelado tú',
  userFallback: 'has pedido el código; vuelve a intentarlo permitiéndolo',
  systemCancel: 'lo ha cerrado el sistema',
  timeout: 'se ha agotado el tiempo',
  noHardware: 'este aparato no tiene sensor biométrico',
  notEnrolled: 'no hay ninguna cara ni huella registrada: ve a Ajustes',
  passcodeNotSet: 'el aparato no tiene código, y sin código no hay biometría',
  lockedOut: 'demasiados intentos; desbloquea el aparato con el código',
  permanentlyLockedOut: 'bloqueado hasta que desbloquees el aparato con el código',
  unavailable: 'el sistema no presta la biometría ahora mismo'
}

const LECTURA: Record<KeychainReadOutcome, string> = {
  found: 'leído',
  notFound: 'no hay nada guardado todavía',
  denied: 'está guardado y no has demostrado ser tú',
  invalidated: 'cambió la biometría del aparato: ya no se puede abrir',
  unavailable: 'no hay dónde guardar esto en este aparato'
}

/**
 * Un secreto guardado en el llavero del sistema y protegido con biometría.
 *
 * Los dos plugins van juntos a propósito: guardar algo «seguro» que cualquiera
 * puede volver a leer no protege de nada, y pedir la cara sin atarla a lo que
 * cifra es teatro. Lo que hace el botón de abajo es lo segundo bien hecho: el
 * secreto se guarda con un control de acceso que exige biometría, y quien lo
 * desbloquea es el sistema, no este código.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.paddingTop]="'72'"
      [style.paddingHorizontal]="'20'"
      [style.gap]="'14'"
      [backgroundColor]="'#0b1020'">

      <an-text [fontSize]="28" [fontWeight]="'bold'" [color]="'#f4f7ff'">secretos</an-text>

      <an-text [fontSize]="14" [color]="'#9fb0d4'">{{ sensor() }}</an-text>

      <an-view
        [style.paddingVertical]="'14'"
        [style.paddingHorizontal]="'14'"
        [style.minHeight]="'56'"
        [style.justifyContent]="'center'"
        [backgroundColor]="'#1e2a4a'"
        [borderRadius]="10">
        <an-text [fontSize]="16" [color]="'#f4f7ff'">{{ contenido() }}</an-text>
      </an-view>

      <an-text [fontSize]="13" [color]="'#9fb0d4'">{{ almacen() }}</an-text>

      <an-view [style.flexDirection]="'row'" [style.gap]="'10'">
        <an-view
          [style.flexGrow]="'1'"
          [style.height]="'46'"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          [backgroundColor]="'#2f6fed'"
          [borderRadius]="10"
          (press)="guardar()">
          <an-text [fontSize]="15" [fontWeight]="'600'" [color]="'#ffffff'">guardar</an-text>
        </an-view>

        <an-view
          [style.flexGrow]="'1'"
          [style.height]="'46'"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="10"
          (press)="leer()">
          <an-text [fontSize]="15" [fontWeight]="'600'" [color]="'#9fb0d4'">leer</an-text>
        </an-view>

        <an-view
          [style.flexGrow]="'1'"
          [style.height]="'46'"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="10"
          (press)="borrar()">
          <an-text [fontSize]="15" [fontWeight]="'600'" [color]="'#9fb0d4'">borrar</an-text>
        </an-view>
      </an-view>

      <an-view
        [style.height]="'46'"
        [style.alignItems]="'center'"
        [style.justifyContent]="'center'"
        [backgroundColor]="'#1e2a4a'"
        [borderRadius]="10"
        (press)="comprobar()">
        <an-text [fontSize]="15" [fontWeight]="'600'" [color]="'#9fb0d4'">
          solo autenticar, sin leer nada
        </an-text>
      </an-view>

      <an-text [fontSize]="15" [color]="'#6ee7b7'">{{ estado() }}</an-text>
      <an-text [fontSize]="12" [color]="'#7d8bb0'">{{ detalle() }}</an-text>
    </an-view>
  `
})
export class AppComponent {
  private readonly biometrics = inject(Biometrics)
  private readonly keychain = inject(Keychain)

  readonly sensor = signal('preguntando al sistema…')
  /** Si hay algo guardado. Va aparte de `estado` a propósito: es lo que sigue
   * siendo cierto después de que la última acción termine y su mensaje pase. */
  readonly almacen = signal('preguntando al llavero…')
  readonly contenido = signal('—')
  readonly estado = signal('')
  readonly detalle = signal('')

  constructor() {
    // Al arrancar se pregunta qué hay, sin sacar ningún diálogo: `availability`
    // no interrumpe a nadie. De aquí sale lo que pone el botón.
    this.biometrics
      .availability()
      .then((info) => this.sensor.set(this.describir(info)))
      .catch((error: unknown) => this.fallo(error))

    // Y si ya hay algo guardado, se dice —sin leerlo—. `has` tampoco pide la
    // cara: saber que el elemento existe no es abrirlo.
    this.keychain
      .has(CLAVE)
      .then((hay) => this.almacen.set(hay ? 'hay un secreto guardado' : 'no hay nada guardado'))
      .catch((error: unknown) => this.fallo(error))
  }

  /**
   * Guarda el secreto exigiendo biometría para volver a leerlo.
   *
   * En iOS esto no pregunta nada: el diálogo sale al leer. En Android sí
   * pregunta, porque allí la clave que cifra es de un solo uso autenticado.
   * La diferencia está en el contrato del plugin, no escondida aquí.
   */
  guardar(): void {
    this.keychain
      .set(CLAVE, SECRETO, {
        requireBiometrics: true,
        reason: 'Guardar el testigo de sesión protegido con tu cara'
      })
      .then((resultado) => {
        this.detalle.set(resultado.detail)
        if (resultado.outcome === 'saved') {
          this.contenido.set('—')
          this.almacen.set('hay un secreto guardado')
          this.estado.set('guardado; ahora hace falta tu cara para leerlo')
          return
        }
        this.estado.set(
          resultado.outcome === 'denied'
            ? 'no se guardó: no autenticaste'
            : 'no se puede guardar en este aparato'
        )
      })
      .catch((error: unknown) => this.fallo(error))
  }

  /** Lee el secreto. Aquí es donde iOS saca Face ID. */
  leer(): void {
    this.estado.set('pidiendo tu cara…')
    this.keychain
      .get(CLAVE, { reason: 'Enseñar el testigo de sesión guardado' })
      .then((lectura) => {
        this.detalle.set(lectura.detail)
        this.estado.set(LECTURA[lectura.outcome])
        // Un valor solo se enseña con `found`. Con `denied` el secreto sigue
        // guardado y sin abrir, que no es lo mismo que no tenerlo.
        this.contenido.set(lectura.outcome === 'found' ? (lectura.value ?? '') : '—')
      })
      .catch((error: unknown) => this.fallo(error))
  }

  borrar(): void {
    this.keychain
      .remove(CLAVE)
      .then((habia) => {
        this.contenido.set('—')
        this.almacen.set('no hay nada guardado')
        this.detalle.set('')
        this.estado.set(habia ? 'borrado' : 'no había nada que borrar')
      })
      .catch((error: unknown) => this.fallo(error))
  }

  /** Biometría a secas, sin llavero de por medio. */
  comprobar(): void {
    this.estado.set('pidiendo tu cara…')
    this.biometrics
      .authenticate({
        reason: 'Comprobar que eres tú',
        subtitle: 'sin leer ningún secreto',
        cancelTitle: 'Ahora no'
      })
      .then((resultado) => {
        this.estado.set(EXPLICACION[resultado.outcome])
        this.detalle.set(`${resultado.kind} · ${resultado.detail}`)
      })
      .catch((error: unknown) => this.fallo(error))
  }

  private describir(info: BiometricAvailability): string {
    const nombre: Record<string, string> = {
      faceId: 'Face ID',
      touchId: 'Touch ID',
      opticId: 'Optic ID',
      unknown: 'biometría',
      none: 'sin sensor'
    }
    const que = nombre[info.kind] ?? info.kind
    return info.status === 'available'
      ? `${que}, listo`
      : `${que}: ${EXPLICACION[info.status]}`
  }

  /**
   * Un plugin que falta no se disimula. Sin el plugin dentro del `.app` la
   * promesa se rechaza, y eso es exactamente lo que tiene que verse.
   */
  private fallo(error: unknown): void {
    this.contenido.set('—')
    this.estado.set(`falló: ${error}`)
  }
}
