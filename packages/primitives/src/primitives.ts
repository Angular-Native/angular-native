import { DestroyRef, Directive, ElementRef, inject, Input, Renderer2 } from '@angular/core'
import { outputFromObservable } from '@angular/core/rxjs-interop'
import { map, Observable } from 'rxjs'

/**
 * Carga de un `(press)`. Las coordenadas van en puntos y son relativas a la
 * vista que recibió el toque.
 *
 */
export interface NativePressEvent {
  x: number
  y: number
}

/**
 * En qué punto del gesto llega el evento.
 *
 * `cancel` no es un fallo: el sistema se lleva el gesto cuando otro gana —al
 * arrastrar dentro de una lista que empieza a desplazarse, por ejemplo—. Quien
 * mueve algo con el dedo tiene que devolverlo a su sitio, no dejarlo a medias.
 */
export type NativeGestureState = 'begin' | 'move' | 'end' | 'cancel'

/**
 * Arrastre.
 *
 * `translation` va desde donde empezó el dedo, no desde el evento anterior:
 * así basta con sumarlo a la posición inicial, sin acumular nada ni arrastrar
 * el error de redondeo de cada paso.
 */
export interface NativePanEvent {
  x: number
  y: number
  translationX: number
  translationY: number
  /** Puntos por segundo. Sirve para seguir por inercia al soltar. */
  velocityX: number
  velocityY: number
  state: NativeGestureState
}

/** Pellizco. `scale` es relativa al principio del gesto, no absoluta. */
export interface NativePinchEvent {
  scale: number
  velocity: number
  state: NativeGestureState
}

/** Giro con dos dedos, en radianes desde que empezó el gesto. */
export interface NativeRotateEvent {
  rotation: number
  velocity: number
  state: NativeGestureState
}

/** Un evento que solo lleva la posición elegida. */
export interface NativeIndexEvent {
  index: number
}

/** Un evento que solo lleva un número. */
export interface NativeValueEvent {
  value: number
}

/** Un evento que solo lleva texto. */
export interface NativeTextEvent {
  value: string
}

/** Marco resuelto de una vista, relativo a su padre y en puntos. */
export interface NativeLayoutEvent {
  x: number
  y: number
  width: number
  height: number
}

/** Márgenes que el sistema se reserva: notch, barra de estado, home. */
export interface NativeSafeAreaInsets {
  top: number
  right: number
  bottom: number
  left: number
}

/** Desplazamiento actual de un `ScrollView`, en puntos. */
export interface NativeScrollEvent {
  x: number
  y: number
}

/**
 * Las claves que un control acepta en `[ios]` o en `[android]`.
 *
 * Se declara la lista aunque el tipo del objeto ya la diga, y no es
 * redundante: el tipo lo comprueba el compilador sobre lo que ve, y no ve un
 * objeto armado a trozos ni uno que viene de fuera. La lista es la que queda
 * en tiempo de ejecución, y es también la que lee `check-wrapper.sh` para
 * exigir que el host de esa plataforma —y solo ese— la mire.
 */
interface PlatformKeys {
  readonly primitive: string
  readonly platform: 'ios' | 'android'
  readonly keys: ReadonlySet<string>
}

function platformKeys(
  primitive: string,
  platform: 'ios' | 'android',
  keys: readonly string[]
): PlatformKeys {
  return { primitive, platform, keys: new Set(keys) }
}

/**
 * Avisa una vez por clave que nadie va a mirar.
 *
 * Mismo trato que `warnUnknownStyle()` en el renderer y por el mismo motivo:
 * una prop que viaja, no la reconoce nadie y no da error es un fallo que se ve
 * como "esto no hace nada" y se busca en el sitio equivocado. Una vez por
 * clave, porque el objeto se vuelve a evaluar en cada detección de cambios.
 */
const warnedPlatformProps = new Set<string>()

function warnUnknownPlatformProp(where: PlatformKeys, key: string): void {
  const seen = `${where.primitive}.${where.platform}.${key}`
  if (warnedPlatformProps.has(seen)) return
  warnedPlatformProps.add(seen)
  console.warn(
    `[angular-native] <${where.primitive}> no tiene "${key}" en [${where.platform}], ` +
      `así que no hará nada. Acepta: ${[...where.keys].sort().join(', ')}. ` +
      'Si existe en las dos plataformas es una entrada normal, no va aquí.'
  )
}

/**
 * Primitivas nativas como directivas.
 *
 * La alternativa era `CUSTOM_ELEMENTS_SCHEMA`, que además de exigir un guion en
 * el nombre apaga la comprobación de propiedades: `[bakcgroundColor]` con
 * errata pasaría el compilador y fallaría en silencio en el dispositivo.
 *
 * Con directivas, cada prop es un `@Input` declarado: el compilador de
 * plantillas la comprueba, el editor la autocompleta, y la directiva es el
 * sitio natural donde convertir el valor antes de mandarlo al core.
 */
@Directive()
export abstract class NativeVisual {
  protected readonly node = inject(ElementRef).nativeElement
  protected readonly renderer = inject(Renderer2)

  protected set(name: string, value: unknown): void {
    this.renderer.setProperty(this.node, name, value ?? null)
  }

  /** Lo que se mandó la última vez en cada objeto de plataforma. */
  private readonly platformSent = new Map<string, Set<string>>()

  /**
   * Descompone `[ios]` o `[android]` en props sueltas con su prefijo.
   *
   * El prefijo hace dos cosas: que el host de la otra plataforma pueda
   * descartar la prop sin saber qué es, y que el nombre siga siendo greppable
   * —`"ios:subtitle"` tiene que aparecer en el host de iOS y no en el de
   * Android, y eso lo comprueba un script—.
   */
  protected platform(where: PlatformKeys, value: Record<string, unknown> | null): void {
    const previous = this.platformSent.get(where.platform)
    const sent = new Set<string>()
    for (const [key, raw] of Object.entries(value ?? {})) {
      if (!where.keys.has(key)) {
        warnUnknownPlatformProp(where, key)
        continue
      }
      sent.add(key)
      this.set(`${where.platform}:${key}`, raw)
    }
    // Una clave que estaba puesta y ya no está tiene que volver a su valor de
    // fábrica: el control no se entera solo de que se la han quitado.
    if (previous) {
      for (const key of previous) {
        if (!sent.has(key)) this.set(`${where.platform}:${key}`, null)
      }
    }
    this.platformSent.set(where.platform, sent)
  }

  /**
   * Un gesto que solo existe si la plantilla lo pide.
   *
   * El observable es frío: el `UIGestureRecognizer` se engancha al
   * suscribirse y se suelta al destruir la vista. Angular suscribe una salida
   * únicamente cuando hay un `(press)` bindeado, así que una vista que nadie
   * escucha no paga nada. Declararlo como evento de elemento habría dado el
   * mismo coste, pero `$event` sería `Event` y habría que castear en cada
   * plantilla.
   */
  protected nativeEvent<T>(name: string): Observable<T> {
    return new Observable<T>((subscriber) => {
      const unlisten = this.renderer.listen(this.node, name, (payload) => {
        subscriber.next(payload as T)
      })
      return () => unlisten()
    })
  }

  readonly press = outputFromObservable(this.nativeEvent<NativePressEvent>('press'))
  readonly doublePress = outputFromObservable(this.nativeEvent<NativePressEvent>('doublePress'))

  /**
   * Mantener pulsado. Solo llega una vez, cuando el sistema decide que el
   * gesto cuenta: cada plataforma tiene su umbral de tiempo, y respetarlo es
   * lo que hace que la app se sienta de esa plataforma.
   */
  readonly longPress = outputFromObservable(this.nativeEvent<NativePressEvent>('longPress'))

  readonly pan = outputFromObservable(this.nativeEvent<NativePanEvent>('pan'))
  readonly pinch = outputFromObservable(this.nativeEvent<NativePinchEvent>('pinch'))
  /**
   * Girar con dos dedos.
   *
   * Se llama `rotation` y no `rotate` porque `[rotate]` ya es la
   * transformación, y una clase no puede tener dos miembros con el mismo
   * nombre. Queda además más claro cuál es cuál: `[rotate]` manda, `(rotation)`
   * cuenta.
   */
  readonly rotation = outputFromObservable(this.nativeEvent<NativeRotateEvent>('rotate'))

  // Deslizar. Cada dirección es su propia salida porque cada una engancha su
  // reconocedor: escuchar solo `swipeLeft` no cuesta los otros tres.
  readonly swipeLeft = outputFromObservable(this.nativeEvent<NativePressEvent>('swipeLeft'))
  readonly swipeRight = outputFromObservable(this.nativeEvent<NativePressEvent>('swipeRight'))
  readonly swipeUp = outputFromObservable(this.nativeEvent<NativePressEvent>('swipeUp'))
  readonly swipeDown = outputFromObservable(this.nativeEvent<NativePressEvent>('swipeDown'))

  /**
   * El marco que le asignó el layout, cada vez que cambia.
   *
   * No lo produce ninguna plataforma: lo emite el core al terminar el commit,
   * porque es él quien calcula el marco. Sale gratis en iOS y en Android.
   */
  readonly layout = outputFromObservable(this.nativeEvent<NativeLayoutEvent>('layout'))

  /**
   * Márgenes que el sistema se reserva, y cada vez que cambian: al rotar, al
   * aparecer el teclado, al entrar en pantalla dividida.
   */
  readonly safeArea = outputFromObservable(this.nativeEvent<NativeSafeAreaInsets>('safeArea'))

  @Input() set backgroundColor(value: string | null) {
    this.set('backgroundColor', value)
  }

  /**
   * Cuántos milisegundos tarda esta vista en llegar a sus valores nuevos.
   *
   * Con esto puesto, mover, escalar, cambiar la opacidad o recolocar la vista
   * deja de ser un salto: la anima la plataforma, en su hilo de dibujo, sin
   * volver a pasar por JavaScript en cada frame. Por eso una animación sigue
   * yendo suave aunque el hilo del motor esté ocupado.
   *
   * Lo que se anima es el cambio, no un valor concreto: se pone una vez y
   * vale para todos los que vengan después. Cero o `null` lo apaga.
   */
  @Input() set animate(value: number | null) {
    this.set('animate', value)
  }

  @Input() set animateDelay(value: number | null) {
    this.set('animateDelay', value)
  }

  /** Por defecto `ease-out`: sale rápido y frena al llegar. */
  @Input() set animateEasing(
    value: 'linear' | 'ease-in' | 'ease-out' | 'ease-in-out' | null
  ) {
    this.set('animateEasing', value)
  }

  /**
   * Desplazar, escalar y girar.
   *
   * No entran en el layout a propósito: una vista movida o escalada sigue
   * ocupando el mismo sitio que ocupaba. Por eso son baratas —no hay nada que
   * recalcular— y por eso son las que hay que usar para seguir a un dedo.
   * Para mover algo *y* que lo de al lado se aparte, hay que cambiar el
   * layout, no esto.
   */
  @Input() set translateX(value: number | null) {
    this.set('translateX', value)
  }

  @Input() set translateY(value: number | null) {
    this.set('translateY', value)
  }

  @Input() set scale(value: number | null) {
    this.set('scale', value)
  }

  @Input() set scaleX(value: number | null) {
    this.set('scaleX', value)
  }

  @Input() set scaleY(value: number | null) {
    this.set('scaleY', value)
  }

  /** En radianes, como lo que manda el gesto de girar. */
  @Input() set rotate(value: number | null) {
    this.set('rotate', value)
  }

  @Input() set borderRadius(value: number | null) {
    this.set('borderRadius', value)
  }

  // Radios por esquina. UIKit solo sabe de un radio único, así que cuando
  // difieren el host dibuja el contorno y lo usa de máscara; Android lo
  // resuelve con `setCornerRadii`.
  @Input() set borderTopLeftRadius(value: number | null) {
    this.set('borderTopLeftRadius', value)
  }

  @Input() set borderTopRightRadius(value: number | null) {
    this.set('borderTopRightRadius', value)
  }

  @Input() set borderBottomRightRadius(value: number | null) {
    this.set('borderBottomRightRadius', value)
  }

  @Input() set borderBottomLeftRadius(value: number | null) {
    this.set('borderBottomLeftRadius', value)
  }

  @Input() set borderWidth(value: number | null) {
    this.set('borderWidth', value)
  }

  @Input() set borderColor(value: string | null) {
    this.set('borderColor', value)
  }

  @Input() set opacity(value: number | null) {
    this.set('opacity', value)
  }

  /** Identificador para pruebas de interfaz; acaba en accessibilityIdentifier. */
  @Input() set testID(value: string | null) {
    this.set('testID', value)
  }
}

@Directive({ selector: 'View' })
export class View extends NativeVisual {}

/**
 * Un control del sistema: algo que se toca y que se puede apagar.
 *
 * `enabled` está aquí y no repetido en cada uno porque significa lo mismo en
 * los ocho y en las dos plataformas —`UIControl.isEnabled` y
 * `View.setEnabled`—, incluido el gris y el que deje de responder al toque,
 * que lo pone el sistema y no nosotros.
 *
 * Los campos de texto no entran: ya tienen `editable`, que es la misma idea
 * con el nombre que usa un campo.
 */
@Directive()
export abstract class NativeControl extends NativeVisual {
  @Input() set enabled(value: boolean | null) {
    this.set('enabled', value ?? true)
  }
}

/**
 * Pila de pantallas.
 *
 * Sus hijos se superponen y ocupan todo —eso lo impone el core, no el estilo—
 * y el host anima la entrada y la salida según `transition`. Rara vez se usa
 * a pelo: lo normal es `NativeStack`, que la conecta con el router.
 */
@Directive({ selector: 'StackView' })
export class StackView extends NativeVisual {
  /**
   * Sentido de la próxima transición. Lo decide quien navega, que es el
   * único que sabe si se avanza o se retrocede.
   */
  @Input() set transition(value: 'push' | 'pop' | 'none' | null) {
    this.set('transition', value ?? 'none')
  }

  /** Gesto de borde en iOS, botón físico en Android. */
  readonly back = outputFromObservable(this.nativeEvent<void>('back'))
}

/** Lo que el `UIScrollView` tiene y el de Android no. */
export type IosScrollViewProps = {
  /**
   * El desplazamiento se para en múltiplos del tamaño de la vista.
   * `UIScrollView.isPagingEnabled`. Android no lo trae: lo suyo es
   * `ViewPager2`, que es otra vista con su adaptador, no una prop.
   */
  pagingEnabled?: boolean
  /**
   * Qué hace el teclado al desplazarse. `keyboardDismissMode`. En Android el
   * teclado no se esconde al desplazar y no hay nada que pedirle.
   */
  keyboardDismissMode?: 'none' | 'onDrag' | 'interactive'
}

const SCROLL_VIEW_IOS = platformKeys('ScrollView', 'ios', [
  'pagingEnabled',
  'keyboardDismissMode'
])

@Directive({ selector: 'ScrollView' })
export class ScrollView extends NativeVisual {
  @Input() set showsScrollIndicator(value: boolean | null) {
    this.set('showsScrollIndicator', value)
  }

  /**
   * Si el dedo mueve el contenido.
   *
   * Apagado, la vista sigue recortando y el contenido sigue pudiendo
   * desplazarse desde el código: lo que se quita es el gesto.
   */
  @Input() set scrollEnabled(value: boolean | null) {
    this.set('scrollEnabled', value ?? true)
  }

  @Input() set ios(value: IosScrollViewProps | null) {
    this.platform(SCROLL_VIEW_IOS, value)
  }

  /** El rebote de iOS al llegar al final. */
  @Input() set bounces(value: boolean | null) {
    this.set('bounces', value)
  }

  /**
   * Si está recargando. Ponerlo a `false` cierra la ruedecilla; la abre el
   * propio gesto, no esta prop.
   */
  @Input() set refreshing(value: boolean | null) {
    this.set('refreshing', value ?? false)
  }

  /**
   * Tirar para recargar.
   *
   * En iOS lo dibuja el sistema con un `UIRefreshControl`. Android no trae uno
   * en la plataforma —`SwipeRefreshLayout` vive en AndroidX— así que se dibuja
   * el mismo arco que hace el sistema.
   */
  readonly refresh = outputFromObservable(this.nativeEvent<void>('refresh'))

  /**
   * Se emite en cada frame de desplazamiento. El `contentSize` lo calcula el
   * layout solo: es el tamaño que ocupan los hijos, y el core lo manda al
   * `UIScrollView` cuando cambia.
   */
  readonly scroll = outputFromObservable(this.nativeEvent<NativeScrollEvent>('scroll'))
}

/** Tamaño real de una imagen ya cargada, en puntos. */
export interface NativeImageLoadEvent {
  width: number
  height: number
}

@Directive({ selector: 'Image' })
export class Image extends NativeVisual {
  constructor() {
    super()
    // Este oyente no es opcional: el layout no puede colocar algo cuyo tamaño
    // no conoce, y solo la imagen sabe cuánto mide. Se registra siempre, aunque
    // la plantilla no escuche `load`.
    const unlisten = this.renderer.listen(this.node, 'load', (payload) => {
      const size = payload as NativeImageLoadEvent
      this.set('intrinsicWidth', size.width)
      this.set('intrinsicHeight', size.height)
    })
    inject(DestroyRef).onDestroy(unlisten)
  }

  /**
   * Ruta de la imagen. Sin esquema es un recurso del bundle de la app; con
   * `http` o `https` se baja por red y aparece cuando llegue.
   */
  @Input() set source(value: string | null) {
    this.set('source', value)
  }

  /** `contain` por defecto; también `cover`, `stretch` y `center`. */
  @Input() set resizeMode(value: 'contain' | 'cover' | 'stretch' | 'center' | null) {
    this.set('resizeMode', value)
  }

  /**
   * Tamaño intrínseco. Se rellena solo al cargar la imagen; fijarlo a mano
   * sirve para reservar el hueco antes de que llegue y evitar el salto.
   */
  @Input() set intrinsicWidth(value: number | null) {
    this.set('intrinsicWidth', value)
  }

  @Input() set intrinsicHeight(value: number | null) {
    this.set('intrinsicHeight', value)
  }

  readonly load = outputFromObservable(this.nativeEvent<NativeImageLoadEvent>('load'))
}

/** Lo que el `TextView` de Android tiene y el `UILabel` de iOS no. */
export type AndroidTextProps = {
  /**
   * Deja seleccionar y copiar el texto.
   *
   * `UILabel` no lo hace: en iOS un texto seleccionable es un `UITextView`
   * apagado, que es otra vista y otra medición, así que aquí no se imita.
   */
  selectable?: boolean
}

const TEXT_ANDROID = platformKeys('Text', 'android', ['selectable'])

@Directive({ selector: 'Text' })
export class Text extends NativeVisual {
  @Input() set color(value: string | null) {
    this.set('color', value)
  }

  @Input() set fontSize(value: number | null) {
    this.set('fontSize', value)
  }

  /** `'bold'`, `'normal'` o la escala numérica de CSS (100..900). */
  @Input() set fontWeight(value: string | number | null) {
    this.set('fontWeight', value)
  }

  @Input() set fontStyle(value: 'normal' | 'italic' | null) {
    this.set('fontStyle', value)
  }

  @Input() set fontFamily(value: string | null) {
    this.set('fontFamily', value)
  }

  @Input() set letterSpacing(value: number | null) {
    this.set('letterSpacing', value)
  }

  @Input() set lineHeight(value: number | null) {
    this.set('lineHeight', value)
  }

  @Input() set textAlign(value: 'left' | 'center' | 'right' | 'justify' | null) {
    this.set('textAlign', value)
  }

  /** 0 o nulo = sin límite. */
  @Input() set numberOfLines(value: number | null) {
    this.set('numberOfLines', value)
  }

  /** Subrayado o tachado. Una raya sencilla, que es lo que se pide siempre. */
  @Input() set textDecoration(value: 'none' | 'underline' | 'lineThrough' | null) {
    this.set('textDecoration', value ?? 'none')
  }

  @Input() set android(value: AndroidTextProps | null) {
    this.platform(TEXT_ANDROID, value)
  }
}

/** Lo que el campo de UIKit tiene y el de Android no. */
export type IosTextInputProps = {
  /**
   * La equis para vaciar el campo. `UITextField.clearButtonMode`. Android no
   * la tiene: ahí la convención es borrar con el teclado.
   */
  clearButtonMode?: 'never' | 'whileEditing' | 'always'
  /**
   * El marco que dibuja UIKit alrededor del campo.
   * `UITextField.borderStyle`. En Android el fondo de un `EditText` lo pone el
   * tema, y aquí se quita a propósito para que el marco lo ponga la plantilla.
   */
  borderStyle?: 'none' | 'line' | 'bezel' | 'roundedRect'
}

/** Lo que el campo de Android tiene y el de UIKit no. */
export type AndroidTextInputProps = {
  /** Al recibir el foco, todo el texto queda seleccionado. */
  selectAllOnFocus?: boolean
  /** Esconde el cursor. `EditText.setCursorVisible`. */
  cursorVisible?: boolean
}

const TEXT_INPUT_IOS = platformKeys('TextInput', 'ios', ['clearButtonMode', 'borderStyle'])
const TEXT_INPUT_ANDROID = platformKeys('TextInput', 'android', [
  'selectAllOnFocus',
  'cursorVisible'
])

@Directive({ selector: 'TextInput' })
export class TextInput extends NativeVisual {
  @Input() set placeholder(value: string | null) {
    this.set('placeholder', value)
  }

  /**
   * El host solo escribe en el campo si el texto difiere de verdad: asignarlo
   * en cada tecla movería el cursor al final.
   */
  @Input() set value(value: string | null) {
    this.set('value', value)
  }

  @Input() set secureTextEntry(value: boolean | null) {
    this.set('secureTextEntry', value)
  }

  @Input() set editable(value: boolean | null) {
    this.set('editable', value)
  }

  @Input() set color(value: string | null) {
    this.set('color', value)
  }

  @Input() set fontSize(value: number | null) {
    this.set('fontSize', value)
  }

  /** `'bold'`, `'normal'` o la escala numérica de CSS (100..900). */
  @Input() set fontWeight(value: string | number | null) {
    this.set('fontWeight', value)
  }

  @Input() set fontFamily(value: string | null) {
    this.set('fontFamily', value)
  }

  @Input() set textAlign(value: 'left' | 'center' | 'right' | null) {
    this.set('textAlign', value)
  }

  /**
   * Qué teclado sale.
   *
   * No es un adorno: un campo de correo con el teclado de texto obliga a
   * buscar la arroba, y uno de teléfono con letras deja escribir cosas que no
   * son un teléfono. En iOS es `keyboardType`; en Android, el `inputType`, que
   * además cambia lo que el campo acepta.
   */
  @Input() set keyboardType(
    value: 'default' | 'numeric' | 'decimal' | 'email' | 'phone' | 'url' | null
  ) {
    this.set('keyboardType', value ?? 'default')
  }

  /**
   * Qué pone la tecla de retorno. Cambia el rótulo y, con él, lo que la
   * persona espera que pase al pulsarla.
   */
  @Input() set returnKeyType(
    value: 'default' | 'done' | 'go' | 'next' | 'search' | 'send' | null
  ) {
    this.set('returnKeyType', value ?? 'default')
  }

  @Input() set autoCapitalize(
    value: 'none' | 'sentences' | 'words' | 'characters' | null
  ) {
    this.set('autoCapitalize', value ?? 'sentences')
  }

  /** El corrector del sistema. Apagarlo es lo normal en un usuario o un código. */
  @Input() set autoCorrect(value: boolean | null) {
    this.set('autoCorrect', value ?? true)
  }

  /** Color del texto de ayuda, que no tiene por qué ser el del texto. */
  @Input() set placeholderColor(value: string | null) {
    this.set('placeholderColor', value)
  }

  @Input() set ios(value: IosTextInputProps | null) {
    this.platform(TEXT_INPUT_IOS, value)
  }

  @Input() set android(value: AndroidTextInputProps | null) {
    this.platform(TEXT_INPUT_ANDROID, value)
  }

  /** Emparejado con `value`, habilita `[(value)]` en la plantilla. */
  readonly valueChange = outputFromObservable(
    this.nativeEvent<{ value: string }>('change').pipe(map((event) => event.value))
  )

  readonly focus = outputFromObservable(this.nativeEvent<{ value: string }>('focus'))
  readonly blur = outputFromObservable(this.nativeEvent<{ value: string }>('blur'))
  /** La tecla de retorno del teclado. */
  readonly submit = outputFromObservable(
    this.nativeEvent<{ value: string }>('submit').pipe(map((event) => event.value))
  )
}

/** Pestaña seleccionada. */
export interface NativeTabSelectEvent {
  index: number
}

/**
 * Barra de pestañas del sistema.
 *
 * Es la barra de verdad —`UITabBar` en iOS— no una fila de vistas imitándola:
 * hereda su tipografía, su fondo translúcido y su comportamiento con el texto
 * grande de accesibilidad.
 */
/** Lo que la barra de iOS tiene y la de Material no. */
export type IosTabBarProps = {
  /**
   * Si se ve lo que pasa por detrás. `UITabBar.isTranslucent`. La barra de
   * Material es opaca por diseño y no tiene un interruptor para esto.
   */
  translucent?: boolean
}

const TAB_BAR_IOS = platformKeys('TabBar', 'ios', ['translucent'])

@Directive({ selector: 'TabBar' })
export class TabBar extends NativeVisual {
  /** Títulos, en orden. */
  @Input() set items(value: readonly string[] | null) {
    // El protocolo no lleva listas y una barra de pestañas no justifica
    // añadirlas: viajan como JSON.
    this.set('items', JSON.stringify(value ?? []))
  }

  /**
   * Iconos, en el mismo orden que los títulos.
   *
   * Los nombres son los de `<Icon>`, así que valen los comunes —`home`,
   * `search`, `settings`— y también los nativos de cada plataforma. Una barra
   * de pestañas sin iconos es legal, pero no es lo que espera nadie.
   */
  @Input() set icons(value: readonly string[] | null) {
    this.set('icons', JSON.stringify(value ?? []))
  }

  @Input() set selectedIndex(value: number | null) {
    this.set('selectedIndex', value ?? 0)
  }

  /** Color de la pestaña activa. */
  @Input() set color(value: string | null) {
    this.set('color', value)
  }

  /**
   * Color de las demás.
   *
   * Sin esto salía el activo rebajado, que en una barra clara puede acabar
   * siendo casi el color del fondo: los rótulos están ahí y no se leen.
   */
  @Input() set unselectedColor(value: string | null) {
    this.set('unselectedColor', value)
  }

  @Input() set ios(value: IosTabBarProps | null) {
    this.platform(TAB_BAR_IOS, value)
  }

  readonly select = outputFromObservable(
    this.nativeEvent<NativeTabSelectEvent>('select').pipe(map((event) => event.index))
  )
}

/** Lo que el interruptor de Material tiene y el de UIKit no. */
export type AndroidSwitchProps = {
  /**
   * Color de la vía con el interruptor apagado.
   *
   * `UISwitch` no lo expone: lo que circula por ahí es ponerle un
   * `backgroundColor` y un radio de esquina a un control del sistema para que
   * se le vea el fondo por detrás, y eso se rompe en cuanto Apple cambia el
   * alto del control. En iOS se queda con el color del sistema.
   */
  trackColor?: string
}

const SWITCH_ANDROID = platformKeys('Switch', 'android', ['trackColor'])

/** Interruptor del sistema. */
@Directive({ selector: 'Switch' })
export class Switch extends NativeControl {
  @Input() set on(value: boolean | null) {
    this.set('on', value ?? false)
  }

  /** Color cuando está encendido. */
  @Input() set color(value: string | null) {
    this.set('color', value)
  }

  /** Color del pulgar, el que se mueve. */
  @Input() set thumbColor(value: string | null) {
    this.set('thumbColor', value)
  }

  @Input() set android(value: AndroidSwitchProps | null) {
    this.platform(SWITCH_ANDROID, value)
  }

  /** Emparejado con `on`, habilita `[(on)]` en la plantilla. */
  readonly onChange = outputFromObservable(
    this.nativeEvent<{ value: boolean }>('change').pipe(map((event) => event.value))
  )
}

/** Lo que el deslizador de UIKit tiene y el de Material no. */
export type IosSliderProps = {
  /**
   * Si avisa mientras se arrastra o solo al soltar. `UISlider.isContinuous`.
   * El de Material siempre avisa mientras se arrastra y no se puede cambiar.
   */
  continuous?: boolean
}

/** Lo que el deslizador de Material tiene y el de UIKit no. */
export type AndroidSliderProps = {
  /**
   * Salto entre valores. `Slider.setStepSize`.
   *
   * No es una prop común porque `UISlider` es continuo y no tiene pasos.
   * Redondear el valor en el host se puede, pero entonces el dedo va por un
   * sitio y el valor por otro: el de Material se engancha a los pasos, y
   * prometer «pasos» dando dos comportamientos distintos es peor que decir
   * que solo lo tiene Android.
   *
   * Tiene que dividir el recorrido de forma exacta o Material se queja; si no
   * lo hace, el host lo dice por el registro y deja el deslizador continuo.
   */
  stepSize?: number
}

const SLIDER_IOS = platformKeys('Slider', 'ios', ['continuous'])
const SLIDER_ANDROID = platformKeys('Slider', 'android', ['stepSize'])

/** Deslizador del sistema. */
@Directive({ selector: 'Slider' })
export class Slider extends NativeControl {
  @Input() set value(value: number | null) {
    this.set('value', value ?? 0)
  }

  @Input() set minimumValue(value: number | null) {
    this.set('minimumValue', value ?? 0)
  }

  @Input() set maximumValue(value: number | null) {
    this.set('maximumValue', value ?? 1)
  }

  @Input() set color(value: string | null) {
    this.set('color', value)
  }

  /** El tramo recorrido, de la izquierda al pulgar. */
  @Input() set minimumTrackColor(value: string | null) {
    this.set('minimumTrackColor', value)
  }

  /** El que queda por recorrer. */
  @Input() set maximumTrackColor(value: string | null) {
    this.set('maximumTrackColor', value)
  }

  @Input() set thumbColor(value: string | null) {
    this.set('thumbColor', value)
  }

  @Input() set ios(value: IosSliderProps | null) {
    this.platform(SLIDER_IOS, value)
  }

  @Input() set android(value: AndroidSliderProps | null) {
    this.platform(SLIDER_ANDROID, value)
  }

  readonly valueChange = outputFromObservable(
    this.nativeEvent<{ value: number }>('change').pipe(map((event) => event.value))
  )
}

/** Ruedecilla de carga. Se esconde sola cuando se para. */
@Directive({ selector: 'ActivityIndicator' })
export class ActivityIndicator extends NativeVisual {
  @Input() set animating(value: boolean | null) {
    this.set('animating', value ?? true)
  }

  @Input() set color(value: string | null) {
    this.set('color', value)
  }
}

/** Barra de progreso determinada. `progress` va de 0 a 1. */
@Directive({ selector: 'ProgressBar' })
export class ProgressBar extends NativeVisual {
  @Input() set progress(value: number | null) {
    this.set('progress', value ?? 0)
  }

  @Input() set color(value: string | null) {
    this.set('color', value)
  }
}

/**
 * Lo que el botón de iOS tiene y el de Android no.
 *
 * Es un alias y no una interfaz a propósito: una interfaz no se puede pasar
 * por un `Record<string, unknown>` —TypeScript no le da firma de índice— y el
 * recorrido de claves que hace `platform()` la necesita. Un alias sí.
 */
export type IosButtonProps = {
  /**
   * Segunda línea, más pequeña, debajo del rótulo.
   *
   * `UIButtonConfiguration.subtitle`. Material no tiene nada equivalente: un
   * botón de dos líneas no es un botón de Material, así que no se imita.
   */
  subtitle?: string
}

/** Lo que el botón de Material tiene y el de UIKit no. */
export type AndroidButtonProps = {
  /** Color de la onda que sale del dedo. `MaterialButton.setRippleColor`. */
  rippleColor?: string
  /**
   * Rótulo en mayúsculas. Era lo normal en Material 2 y dejó de serlo en
   * Material 3, pero sigue estando y hay marcas que lo piden. En iOS un botón
   * nunca ha llevado el rótulo en mayúsculas.
   */
  allCaps?: boolean
}

const BUTTON_IOS = platformKeys('Button', 'ios', ['subtitle'])
const BUTTON_ANDROID = platformKeys('Button', 'android', ['rippleColor', 'allCaps'])

/** Botón del sistema, con su tipografía y su respuesta al toque. */
@Directive({ selector: 'Button' })
export class Button extends NativeControl {
  @Input() set title(value: string | null) {
    this.set('title', value ?? '')
  }

  @Input() set color(value: string | null) {
    this.set('color', value)
  }

  /**
   * Cómo se ve: solo el rótulo, relleno, con un fondo tenue del mismo color, o
   * con el contorno y nada dentro.
   *
   * `text` por defecto, que es lo que hace un botón sin más en iOS. Las otras
   * las dibuja la plataforma —`UIButtonConfiguration` en iOS—, salvo en
   * Android, donde los botones de Material 3 no están en la plataforma y la
   * píldora se dibuja a mano sobre un `Button` de verdad.
   *
   * No hay `elevated`: Material la tiene y UIKit no tiene nada parecido, así
   * que sería una variante que solo hace algo en media plataforma. Quien la
   * quiera, por `[android]`.
   */
  @Input() set variant(value: 'text' | 'filled' | 'tonal' | 'outlined' | null) {
    this.set('variant', value ?? 'text')
  }

  /**
   * Icono a un lado del rótulo, por nombre, igual que `<Icon>`.
   *
   * Un SF Symbol en iOS y un Material Symbol en Android, así que la misma
   * plantilla da el icono que le toca a cada plataforma.
   */
  @Input() set icon(value: string | null) {
    this.set('icon', value)
  }

  /** De qué lado del rótulo. `leading` por defecto. */
  @Input() set iconPosition(value: 'leading' | 'trailing' | null) {
    this.set('iconPosition', value ?? 'leading')
  }

  @Input() set fontSize(value: number | null) {
    this.set('fontSize', value)
  }

  /** `'bold'`, `'normal'` o la escala numérica de CSS (100..900). */
  @Input() set fontWeight(value: string | number | null) {
    this.set('fontWeight', value)
  }

  @Input() set ios(value: IosButtonProps | null) {
    this.platform(BUTTON_IOS, value)
  }

  @Input() set android(value: AndroidButtonProps | null) {
    this.platform(BUTTON_ANDROID, value)
  }
}

/**
 * Elegir una de varias opciones que están todas a la vista.
 *
 * `UISegmentedControl` en iOS. Android no trae equivalente en la plataforma
 * —el de Material vive en una librería aparte—, así que se dibuja con vistas
 * del sistema, como la barra de pestañas.
 */
@Directive({ selector: 'SegmentedControl' })
export class SegmentedControl extends NativeControl {
  @Input() set items(value: readonly string[] | null) {
    this.set('items', JSON.stringify(value ?? []))
  }

  @Input() set selectedIndex(value: number | null) {
    this.set('selectedIndex', value ?? 0)
  }

  @Input() set color(value: string | null) {
    this.set('color', value)
  }

  readonly change = outputFromObservable(this.nativeEvent<NativeIndexEvent>('change'))
}

/**
 * Subir y bajar de uno en uno.
 *
 * `UIStepper` en iOS. En Android no hay equivalente en la plataforma y se arma
 * con dos botones del sistema.
 */
@Directive({ selector: 'Stepper' })
export class Stepper extends NativeControl {
  @Input() set value(v: number | null) {
    this.set('value', v ?? 0)
  }

  @Input() set minimumValue(v: number | null) {
    this.set('minimumValue', v ?? 0)
  }

  @Input() set maximumValue(v: number | null) {
    this.set('maximumValue', v ?? 100)
  }

  /** Cuánto sube o baja cada toque. Uno por defecto. */
  @Input() set step(v: number | null) {
    this.set('stepValue', v ?? 1)
  }

  readonly change = outputFromObservable(this.nativeEvent<NativeValueEvent>('change'))
}

/**
 * Campo de búsqueda del sistema, con su lupa y su botón de borrar.
 *
 * Es un control aparte y no un `<TextInput>` con un icono al lado: el sistema
 * le da el teclado con la tecla de buscar, el comportamiento de cancelar y el
 * aspecto que la gente reconoce como "aquí se busca".
 */
@Directive({ selector: 'SearchBar' })
export class SearchBar extends NativeControl {
  @Input() set value(v: string | null) {
    this.set('value', v ?? '')
  }

  @Input() set placeholder(v: string | null) {
    this.set('placeholder', v)
  }

  readonly input = outputFromObservable(this.nativeEvent<NativeTextEvent>('input'))
  readonly submit = outputFromObservable(this.nativeEvent<NativeTextEvent>('submit'))
}

/**
 * Elegir una de varias opciones de una lista que se despliega.
 *
 * En iOS es un botón que abre un `UIMenu`: no hay un control de desplegable, y
 * `UIPickerView` es la rueda a pantalla completa, que es otra cosa y ya no es
 * lo que usa el sistema para una lista corta. En Android es un `Spinner`.
 */
@Directive({ selector: 'Picker' })
export class Picker extends NativeControl {
  @Input() set items(value: readonly string[] | null) {
    this.set('items', JSON.stringify(value ?? []))
  }

  @Input() set selectedIndex(value: number | null) {
    this.set('selectedIndex', value ?? 0)
  }

  readonly change = outputFromObservable(this.nativeEvent<NativeIndexEvent>('change'))
}

/**
 * Selector de fecha y hora del sistema.
 *
 * El valor va y viene en milisegundos desde 1970 —lo que da y toma `Date`—,
 * porque una fecha formateada depende del idioma y de la zona horaria del
 * dispositivo, y eso lo resuelve cada plataforma.
 */
@Directive({ selector: 'DatePicker' })
export class DatePicker extends NativeControl {
  @Input() set value(v: number | Date | null) {
    const millis = v instanceof Date ? v.getTime() : (v ?? Date.now())
    this.set('value', millis)
  }

  @Input() set mode(v: 'date' | 'time' | 'dateAndTime' | null) {
    this.set('mode', v ?? 'date')
  }

  readonly change = outputFromObservable(this.nativeEvent<NativeValueEvent>('change'))
}

/**
 * Cabecera con título y botón de atrás.
 *
 * `UINavigationBar` en iOS y `Toolbar` en Android. Fuera de un
 * `UINavigationController` no hay botón de atrás automático, así que se pone
 * uno con el mismo símbolo y en el mismo sitio; navegar sigue siendo cosa del
 * router, que es quien sabe a dónde se vuelve.
 */
@Directive({ selector: 'NavigationBar' })
export class NavigationBar extends NativeVisual {
  @Input() set title(value: string | null) {
    this.set('title', value ?? '')
  }

  @Input() set showsBack(value: boolean | null) {
    this.set('showsBack', value ?? false)
  }

  /**
   * Rótulo del botón de atrás. Solo en iOS: en Android la barra de
   * herramientas lleva únicamente la flecha, que es lo que hace cualquier app
   * de la plataforma.
   */
  @Input() set backTitle(value: string | null) {
    this.set('backTitle', value)
  }

  readonly back = outputFromObservable(this.nativeEvent<void>('back'))
}

/**
 * Campo de texto de varias líneas.
 *
 * Es una primitiva aparte y no una prop de `<TextInput>` porque en iOS son dos
 * controles distintos —`UITextField` y `UITextView`— y cambiar de uno a otro
 * con la vista ya montada no es posible.
 */
@Directive({ selector: 'TextEditor' })
export class TextEditor extends NativeVisual {
  @Input() set value(v: string | null) {
    this.set('value', v ?? '')
  }

  @Input() set editable(v: boolean | null) {
    this.set('editable', v ?? true)
  }

  @Input() set color(v: string | null) {
    this.set('color', v)
  }

  readonly change = outputFromObservable(this.nativeEvent<NativeTextEvent>('change'))
}

/**
 * Navegador embebido: `WKWebView` en iOS, `WebView` en Android.
 *
 * Se le da una dirección o un HTML suelto. Es una vista más del árbol: ocupa
 * el sitio que le dé el layout y se puede poner al lado de cualquier otra.
 */
@Directive({ selector: 'WebView' })
export class WebView extends NativeVisual {
  @Input() set url(value: string | null) {
    this.set('url', value)
  }

  @Input() set html(value: string | null) {
    this.set('html', value)
  }
}

/**
 * Mapa.
 *
 * En iOS es `MKMapView`, el del sistema. En Android no hay ninguno en la
 * plataforma —el de Google vive en Play Services, que pide clave de API y una
 * dependencia que este build no puede traer—, así que ahí se dibujan teselas
 * de OpenStreetMap sobre un `Canvas`: es una vista nativa, pero no es el mapa
 * del sistema y no trae rutas ni búsqueda.
 */
@Directive({ selector: 'MapView' })
export class MapView extends NativeVisual {
  @Input() set latitude(value: number | null) {
    this.set('latitude', value ?? 0)
  }

  @Input() set longitude(value: number | null) {
    this.set('longitude', value ?? 0)
  }

  /**
   * Nivel de zoom al estilo de las teselas: 0 es el mundo entero y cada nivel
   * es el doble de cerca. MapKit no trabaja así —trabaja con cuántos grados se
   * ven— y la conversión la hace el host, para que la misma cifra signifique
   * lo mismo en las dos plataformas.
   */
  @Input() set zoom(value: number | null) {
    this.set('zoom', value ?? 12)
  }

  /** El punto de dónde estás. Solo en iOS: el mapa de Android no lo sabe. */
  @Input() set showsUser(value: boolean | null) {
    this.set('showsUser', value ?? false)
  }
}

/**
 * Vídeo.
 *
 * `VideoView` en Android. En iOS no hay una vista de vídeo: hay una capa
 * —`AVPlayerLayer`— que se cuelga de cualquier vista, así que el host la
 * cuelga y le ajusta el marco. Una capa no se estira con su vista.
 */
@Directive({ selector: 'VideoView' })
export class VideoView extends NativeVisual {
  @Input() set url(value: string | null) {
    this.set('url', value)
  }

  @Input() set playing(value: boolean | null) {
    this.set('playing', value ?? false)
  }

  /** Solo en iOS: `VideoView` no entrega el reproductor de dentro. */
  @Input() set muted(value: boolean | null) {
    this.set('muted', value ?? false)
  }
}

/**
 * Icono del sistema.
 *
 * No se dibuja nada ni se empaqueta ningún juego de iconos: en iOS es un SF
 * Symbol y en Android un drawable del sistema, pedidos por nombre. Un icono
 * así envejece con la plataforma —cambia cuando cambia el sistema— en vez de
 * quedarse anclado al día en que se metió en el proyecto, y ya viene con el
 * peso y el trazo que le tocan a esa versión.
 *
 * Los nombres comunes —`home`, `search`, `settings`, `back`, `close`, `add`,
 * `delete`, `edit`, `share`, `star`, `menu`, `check`…— se traducen al nombre
 * de cada plataforma, así que la misma plantilla vale para las dos. Para lo
 * específico se escribe el nombre nativo directamente: cualquier SF Symbol
 * (`square.and.arrow.up`) o cualquier drawable de Android.
 */
@Directive({ selector: 'Icon' })
export class Icon extends NativeVisual {
  constructor() {
    super()
    // Un `@Input` que no se enlaza no corre, así que sin esto un `<Icon>` sin
    // `[size]` se quedaría sin tamaño y sin configuración de símbolo. El
    // layout sí le da 24x24 por su cuenta; esto es para que el icono que se
    // dibuja dentro sea el de ese tamaño.
    this.size = null
  }

  @Input() set name(value: string | null) {
    this.set('name', value)
  }

  /**
   * Puntos. Además de fijar el tamaño de la vista, elige el trazo del
   * símbolo: en iOS un icono grande no es el pequeño escalado, es otro
   * dibujo.
   */
  @Input() set size(value: number | null) {
    const points = value ?? 24
    this.set('iconSize', points)
    this.renderer.setStyle(this.node, 'width', points)
    this.renderer.setStyle(this.node, 'height', points)
  }

  /** Grosor del trazo, en la escala de la tipografía: 100..900. */
  @Input() set weight(value: number | null) {
    this.set('iconWeight', value)
  }

  @Input() set color(value: string | null) {
    this.set('color', value)
  }
}

/**
 * Contenido que se presenta encima de todo.
 *
 * Se presenta de verdad: un `UIViewController` en iOS y un `Dialog` en
 * Android, no una vista puesta sobre las demás. Se ve parecido, pero la
 * diferencia importa: el sistema sabe que hay algo modal delante, así que
 * VoiceOver y TalkBack dejan de leer lo de detrás, el botón de atrás de
 * Android lo cierra, y no compite en orden de dibujo con los diálogos del
 * sistema.
 */
@Directive({ selector: 'Modal' })
export class Modal extends NativeVisual {
  @Input() set visible(value: boolean | null) {
    this.set('visible', value ?? false)
  }

  /**
   * `fullScreen` cubre la pantalla; `sheet` entra desde abajo con el tirador
   * y los topes del sistema.
   */
  @Input() set presentation(value: 'fullScreen' | 'sheet' | null) {
    this.set('presentation', value)
  }

  /**
   * Se cerró.
   *
   * Puede cerrarlo el usuario sin pasar por la plantilla —bajando la hoja en
   * iOS, con el botón de atrás en Android—, así que hay que escucharlo: si no,
   * la señal que lo abrió se queda diciendo que sigue abierto y volver a
   * ponerla a `true` no hace nada.
   */
  readonly dismiss = outputFromObservable(this.nativeEvent<void>('dismiss'))
}

/**
 * Diálogo del sistema.
 *
 * No es una capa dibujada por el framework: es un `UIAlertController` y un
 * `AlertDialog` de verdad, con su aspecto, su animación y su comportamiento
 * con VoiceOver y TalkBack. No ocupa sitio en el layout.
 */
@Directive({ selector: 'Alert' })
export class Alert extends NativeVisual {
  /**
   * Hoja de acciones en vez de diálogo centrado.
   *
   * Es la forma de ofrecer varias acciones sobre algo que se acaba de tocar;
   * el diálogo centrado es para confirmar o avisar. En iOS sale desde abajo,
   * en Android es una lista.
   */
  @Input() set sheet(value: boolean | null) {
    this.set('sheet', value ?? false)
  }

  @Input() set visible(value: boolean | null) {
    this.set('visible', value ?? false)
  }

  @Input() set title(value: string | null) {
    this.set('title', value ?? '')
  }

  @Input() set message(value: string | null) {
    this.set('message', value ?? '')
  }

  /** Títulos de los botones, en orden. Sin ninguno, sale un «OK». */
  @Input() set buttons(value: readonly string[] | null) {
    this.set('buttons', JSON.stringify(value ?? []))
  }

  /** Índice del botón pulsado. */
  readonly select = outputFromObservable(
    this.nativeEvent<NativeTabSelectEvent>('select').pipe(map((event) => event.index))
  )
}

/** Para importar todas de golpe en un componente standalone. */
export const NATIVE_PRIMITIVES = [
  View,
  Text,
  Image,
  ScrollView,
  TextInput,
  StackView,
  TabBar,
  Switch,
  Slider,
  ActivityIndicator,
  ProgressBar,
  Button,
  DatePicker,
  Icon,
  Modal,
  MapView,
  NavigationBar,
  TextEditor,
  VideoView,
  WebView,
  SearchBar,
  SegmentedControl,
  Picker,
  Stepper,
  Alert
] as const
