import {
  DestroyRef,
  Directive,
  effect,
  ElementRef,
  inject,
  input,
  Renderer2,
  type Signal
} from '@angular/core'
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
 * Primitivas nativas como directivas.
 *
 * La alternativa era `CUSTOM_ELEMENTS_SCHEMA`, que además de exigir un guion en
 * el nombre apaga la comprobación de propiedades: `[bakcgroundColor]` con
 * errata pasaría el compilador y fallaría en silencio en el dispositivo.
 *
 * Con directivas, cada prop es una entrada declarada: el compilador de
 * plantillas la comprueba, el editor la autocompleta, y la directiva es el
 * sitio natural donde convertir el valor antes de mandarlo al core.
 *
 * Todas son `input()` de señales. Un `set` de `@Input` corría en el momento
 * exacto en que Angular escribía la entrada; una señal se lee cuando alguien
 * la lee, así que quien manda las propiedades al núcleo es el `effect` de
 * `forwardInputs`, uno por vista.
 */
/**
 * Los nombres de las entradas de una directiva, sacados de la definición que
 * compila Angular.
 *
 * Se recorre la cadena de herencia en vez de fiarse de que la definición de la
 * hija ya traiga las de la madre: eso lo hace una `feature` de Angular, y
 * depender de cuándo corre para algo que se puede sumar aquí no compensa.
 *
 * Se calcula una vez por clase, no por vista: son las mismas para todas.
 */
const inputsByClass = new WeakMap<Function, readonly string[]>()

function inputNames(type: Function): readonly string[] {
  const found = inputsByClass.get(type)
  if (found) {
    return found
  }
  const names = new Set<string>()
  for (let current: Function | null = type; current; current = Object.getPrototypeOf(current)) {
    const def = Reflect.get(current, 'ɵdir') as { inputs?: Record<string, unknown> } | undefined
    // `Reflect.get` sube por el prototipo, así que la definición de la madre
    // aparecería otra vez en la hija; solo cuenta la suya.
    if (def && Object.hasOwn(current, 'ɵdir')) {
      for (const name of Object.keys(def.inputs ?? {})) {
        names.add(name)
      }
    }
  }
  const list = [...names]
  inputsByClass.set(type, list)
  return list
}

@Directive()
export abstract class NativeVisual {
  protected readonly node = inject(ElementRef).nativeElement
  protected readonly renderer = inject(Renderer2)

  constructor() {
    this.forwardInputs()
  }

  /**
   * Manda al núcleo lo que cambie de las entradas.
   *
   * Un `effect` por vista y no uno por entrada: una `<View>` declara veinte y
   * lo normal es que no haya ninguna puesta, y veinte nodos reactivos por
   * vista se notan cuando hay setenta en pantalla.
   *
   * La lista de entradas sale de la definición que compila Angular, no de una
   * escrita a mano: una entrada nueva que se olvidara de esa lista no haría
   * nada, y nadie lo diría.
   *
   * `null` es «no lo toques»: una entrada sin poner vale `null` y no se manda,
   * que es lo que hacía un `set` que no se llamaba nunca. En cuanto se manda
   * una vez, se sigue mandando aunque vuelva a `null`, porque entonces `null`
   * sí quiere decir «quítalo».
   */
  private forwardInputs(): void {
    const names = inputNames(this.constructor)
    const renamed = this.nativeNames()
    const sent = new Map<string, unknown>()
    effect(() => {
      for (const name of names) {
        const prop = name in renamed ? renamed[name] : name
        // `null` es la forma de decir que esa entrada se maneja a mano.
        if (prop === null) {
          continue
        }
        const source: unknown = Reflect.get(this, name)
        if (typeof source !== 'function') {
          continue
        }
        const value = (source as Signal<unknown>)()
        if (value === null && !sent.has(name)) {
          continue
        }
        if (sent.has(name) && sent.get(name) === value) {
          continue
        }
        sent.set(name, value)
        this.set(prop, value)
      }
    })
  }

  /**
   * Entradas cuyo nombre en el núcleo no es el de la plantilla, y entradas que
   * no se reenvían —esas van con `null`—.
   */
  protected nativeNames(): Readonly<Record<string, string | null>> {
    return {}
  }

  protected set(name: string, value: unknown): void {
    this.renderer.setProperty(this.node, name, value ?? null)
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

  readonly backgroundColor = input<string | null>(null)

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
  readonly animate = input<number | null>(null)

  readonly animateDelay = input<number | null>(null)

  /** Por defecto `ease-out`: sale rápido y frena al llegar. */
  readonly animateEasing = input<'linear' | 'ease-in' | 'ease-out' | 'ease-in-out' | null>(null)

  /**
   * Desplazar, escalar y girar.
   *
   * No entran en el layout a propósito: una vista movida o escalada sigue
   * ocupando el mismo sitio que ocupaba. Por eso son baratas —no hay nada que
   * recalcular— y por eso son las que hay que usar para seguir a un dedo.
   * Para mover algo *y* que lo de al lado se aparte, hay que cambiar el
   * layout, no esto.
   */
  readonly translateX = input<number | null>(null)

  readonly translateY = input<number | null>(null)

  readonly scale = input<number | null>(null)

  readonly scaleX = input<number | null>(null)

  readonly scaleY = input<number | null>(null)

  /** En radianes, como lo que manda el gesto de girar. */
  readonly rotate = input<number | null>(null)

  readonly borderRadius = input<number | null>(null)

  // Radios por esquina. UIKit solo sabe de un radio único, así que cuando
  // difieren el host dibuja el contorno y lo usa de máscara; Android lo
  // resuelve con `setCornerRadii`.
  readonly borderTopLeftRadius = input<number | null>(null)

  readonly borderTopRightRadius = input<number | null>(null)

  readonly borderBottomRightRadius = input<number | null>(null)

  readonly borderBottomLeftRadius = input<number | null>(null)

  readonly borderWidth = input<number | null>(null)

  readonly borderColor = input<string | null>(null)

  readonly opacity = input<number | null>(null)

  /** Identificador para pruebas de interfaz; acaba en accessibilityIdentifier. */
  readonly testID = input<string | null>(null)
}

@Directive({ selector: 'View' })
export class View extends NativeVisual {}

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
  readonly transition = input<'push' | 'pop' | 'none' | null, 'push' | 'pop' | 'none' | null>(null, {
    transform: (value) => value ?? 'none'
  })

  /** Gesto de borde en iOS, botón físico en Android. */
  readonly back = outputFromObservable(this.nativeEvent<void>('back'))
}

@Directive({ selector: 'ScrollView' })
export class ScrollView extends NativeVisual {
  readonly showsScrollIndicator = input<boolean | null>(null)

  /** El rebote de iOS al llegar al final. */
  readonly bounces = input<boolean | null>(null)

  /**
   * Si está recargando. Ponerlo a `false` cierra la ruedecilla; la abre el
   * propio gesto, no esta prop.
   */
  readonly refreshing = input<boolean | null, boolean | null>(null, {
    transform: (value) => value ?? false
  })

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
  readonly source = input<string | null>(null)

  /** `contain` por defecto; también `cover`, `stretch` y `center`. */
  readonly resizeMode = input<'contain' | 'cover' | 'stretch' | 'center' | null>(null)

  /**
   * Tamaño intrínseco. Se rellena solo al cargar la imagen; fijarlo a mano
   * sirve para reservar el hueco antes de que llegue y evitar el salto.
   */
  readonly intrinsicWidth = input<number | null>(null)

  readonly intrinsicHeight = input<number | null>(null)

  readonly load = outputFromObservable(this.nativeEvent<NativeImageLoadEvent>('load'))
}

@Directive({ selector: 'Text' })
export class Text extends NativeVisual {
  readonly color = input<string | null>(null)

  readonly fontSize = input<number | null>(null)

  /** `'bold'`, `'normal'` o la escala numérica de CSS (100..900). */
  readonly fontWeight = input<string | number | null>(null)

  readonly fontStyle = input<'normal' | 'italic' | null>(null)

  readonly fontFamily = input<string | null>(null)

  readonly letterSpacing = input<number | null>(null)

  readonly lineHeight = input<number | null>(null)

  readonly textAlign = input<'left' | 'center' | 'right' | 'justify' | null>(null)

  /** 0 o nulo = sin límite. */
  readonly numberOfLines = input<number | null>(null)
}

@Directive({ selector: 'TextInput' })
export class TextInput extends NativeVisual {
  readonly placeholder = input<string | null>(null)

  /**
   * El host solo escribe en el campo si el texto difiere de verdad: asignarlo
   * en cada tecla movería el cursor al final.
   */
  readonly value = input<string | null>(null)

  readonly secureTextEntry = input<boolean | null>(null)

  readonly editable = input<boolean | null>(null)

  readonly color = input<string | null>(null)

  readonly fontSize = input<number | null>(null)

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
@Directive({ selector: 'TabBar' })
export class TabBar extends NativeVisual {
  /** Títulos, en orden. */
  // El protocolo no lleva listas y una barra de pestañas no justifica
  // añadirlas: viajan como JSON.
  readonly items = input<string | null, readonly string[] | null>(null, {
    transform: (value) => JSON.stringify(value ?? [])
  })

  /**
   * Iconos, en el mismo orden que los títulos.
   *
   * Los nombres son los de `<Icon>`, así que valen los comunes —`home`,
   * `search`, `settings`— y también los nativos de cada plataforma. Una barra
   * de pestañas sin iconos es legal, pero no es lo que espera nadie.
   */
  readonly icons = input<string | null, readonly string[] | null>(null, {
    transform: (value) => JSON.stringify(value ?? [])
  })

  readonly selectedIndex = input<number | null, number | null>(null, {
    transform: (value) => value ?? 0
  })

  /** Color de la pestaña activa. */
  readonly color = input<string | null>(null)

  readonly select = outputFromObservable(
    this.nativeEvent<NativeTabSelectEvent>('select').pipe(map((event) => event.index))
  )
}

/** Interruptor del sistema. */
@Directive({ selector: 'Switch' })
export class Switch extends NativeVisual {
  readonly on = input<boolean | null, boolean | null>(null, {
    transform: (value) => value ?? false
  })

  /** Color cuando está encendido. */
  readonly color = input<string | null>(null)

  /** Emparejado con `on`, habilita `[(on)]` en la plantilla. */
  readonly onChange = outputFromObservable(
    this.nativeEvent<{ value: boolean }>('change').pipe(map((event) => event.value))
  )
}

/** Deslizador del sistema. */
@Directive({ selector: 'Slider' })
export class Slider extends NativeVisual {
  readonly value = input<number | null, number | null>(null, {
    transform: (value) => value ?? 0
  })

  readonly minimumValue = input<number | null, number | null>(null, {
    transform: (value) => value ?? 0
  })

  readonly maximumValue = input<number | null, number | null>(null, {
    transform: (value) => value ?? 1
  })

  readonly color = input<string | null>(null)

  readonly valueChange = outputFromObservable(
    this.nativeEvent<{ value: number }>('change').pipe(map((event) => event.value))
  )
}

/** Ruedecilla de carga. Se esconde sola cuando se para. */
@Directive({ selector: 'ActivityIndicator' })
export class ActivityIndicator extends NativeVisual {
  readonly animating = input<boolean | null, boolean | null>(null, {
    transform: (value) => value ?? true
  })

  readonly color = input<string | null>(null)
}

/** Barra de progreso determinada. `progress` va de 0 a 1. */
@Directive({ selector: 'ProgressBar' })
export class ProgressBar extends NativeVisual {
  readonly progress = input<number | null, number | null>(null, {
    transform: (value) => value ?? 0
  })

  readonly color = input<string | null>(null)
}

/** Botón del sistema, con su tipografía y su respuesta al toque. */
@Directive({ selector: 'Button' })
export class Button extends NativeVisual {
  readonly title = input<string | null, string | null>(null, {
    transform: (value) => value ?? ''
  })

  readonly color = input<string | null>(null)

  /**
   * Cómo se ve: solo el rótulo, relleno, o con un fondo tenue del mismo color.
   *
   * `text` por defecto, que es lo que hace un botón sin más en iOS. Las otras
   * dos las dibuja la plataforma —`UIButtonConfiguration` en iOS—, salvo en
   * Android, donde los botones de Material 3 no están en la plataforma y la
   * píldora se dibuja a mano sobre un `Button` de verdad.
   */
  readonly variant = input<'text' | 'filled' | 'tonal' | null, 'text' | 'filled' | 'tonal' | null>(null, {
    transform: (value) => value ?? 'text'
  })
}

/**
 * Elegir una de varias opciones que están todas a la vista.
 *
 * `UISegmentedControl` en iOS. Android no trae equivalente en la plataforma
 * —el de Material vive en una librería aparte—, así que se dibuja con vistas
 * del sistema, como la barra de pestañas.
 */
@Directive({ selector: 'SegmentedControl' })
export class SegmentedControl extends NativeVisual {
  readonly items = input<string | null, readonly string[] | null>(null, {
    transform: (value) => JSON.stringify(value ?? [])
  })

  readonly selectedIndex = input<number | null, number | null>(null, {
    transform: (value) => value ?? 0
  })

  readonly color = input<string | null>(null)

  readonly change = outputFromObservable(this.nativeEvent<NativeIndexEvent>('change'))
}

/**
 * Subir y bajar de uno en uno.
 *
 * `UIStepper` en iOS. En Android no hay equivalente en la plataforma y se arma
 * con dos botones del sistema.
 */
@Directive({ selector: 'Stepper' })
export class Stepper extends NativeVisual {
  readonly value = input<number | null, number | null>(null, {
    transform: (v) => v ?? 0
  })

  readonly minimumValue = input<number | null, number | null>(null, {
    transform: (v) => v ?? 0
  })

  readonly maximumValue = input<number | null, number | null>(null, {
    transform: (v) => v ?? 100
  })

  /** Cuánto sube o baja cada toque. Uno por defecto. */
  readonly step = input<number | null, number | null>(null, { transform: (v) => v ?? 1 })

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
export class SearchBar extends NativeVisual {
  readonly value = input<string | null, string | null>(null, {
    transform: (v) => v ?? ''
  })

  readonly placeholder = input<string | null>(null)

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
export class Picker extends NativeVisual {
  readonly items = input<string | null, readonly string[] | null>(null, {
    transform: (value) => JSON.stringify(value ?? [])
  })

  readonly selectedIndex = input<number | null, number | null>(null, {
    transform: (value) => value ?? 0
  })

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
export class DatePicker extends NativeVisual {
  /** Milisegundos o `Date`; al núcleo siempre van milisegundos. */
  readonly value = input<number | null, number | Date | null>(null, {
    transform: (v) => (v instanceof Date ? v.getTime() : (v ?? Date.now()))
  })

  readonly mode = input<'date' | 'time' | 'dateAndTime' | null, 'date' | 'time' | 'dateAndTime' | null>(null, {
    transform: (v) => v ?? 'date'
  })

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
  readonly title = input<string | null, string | null>(null, {
    transform: (value) => value ?? ''
  })

  readonly showsBack = input<boolean | null, boolean | null>(null, {
    transform: (value) => value ?? false
  })

  /**
   * Rótulo del botón de atrás. Solo en iOS: en Android la barra de
   * herramientas lleva únicamente la flecha, que es lo que hace cualquier app
   * de la plataforma.
   */
  readonly backTitle = input<string | null>(null)

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
  readonly value = input<string | null, string | null>(null, {
    transform: (v) => v ?? ''
  })

  readonly editable = input<boolean | null, boolean | null>(null, {
    transform: (v) => v ?? true
  })

  readonly color = input<string | null>(null)

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
  readonly url = input<string | null>(null)

  readonly html = input<string | null>(null)
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
  readonly latitude = input<number | null, number | null>(null, {
    transform: (value) => value ?? 0
  })

  readonly longitude = input<number | null, number | null>(null, {
    transform: (value) => value ?? 0
  })

  /**
   * Nivel de zoom al estilo de las teselas: 0 es el mundo entero y cada nivel
   * es el doble de cerca. MapKit no trabaja así —trabaja con cuántos grados se
   * ven— y la conversión la hace el host, para que la misma cifra signifique
   * lo mismo en las dos plataformas.
   */
  readonly zoom = input<number | null, number | null>(null, {
    transform: (value) => value ?? 12
  })

  /** El punto de dónde estás. Solo en iOS: el mapa de Android no lo sabe. */
  readonly showsUser = input<boolean | null, boolean | null>(null, {
    transform: (value) => value ?? false
  })
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
  readonly url = input<string | null>(null)

  readonly playing = input<boolean | null, boolean | null>(null, {
    transform: (value) => value ?? false
  })

  /** Solo en iOS: `VideoView` no entrega el reproductor de dentro. */
  readonly muted = input<boolean | null, boolean | null>(null, {
    transform: (value) => value ?? false
  })
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
    // El tamaño no viaja como los demás porque no es una propiedad, son tres:
    // la del símbolo, y el ancho y el alto de la vista. Y corre aunque nadie
    // ponga `[size]`, o un `<Icon>` pelado se quedaría sin tamaño de símbolo
    // —el layout sí le da 24x24 por su cuenta, pero el dibujo de dentro no—.
    effect(() => {
      const points = this.size() ?? 24
      this.set('iconSize', points)
      this.renderer.setStyle(this.node, 'width', points)
      this.renderer.setStyle(this.node, 'height', points)
    })
  }

  readonly name = input<string | null>(null)

  /**
   * Puntos. Además de fijar el tamaño de la vista, elige el trazo del
   * símbolo: en iOS un icono grande no es el pequeño escalado, es otro
   * dibujo.
   */
  readonly size = input<number | null>(null)

  /** Grosor del trazo, en la escala de la tipografía: 100..900. */
  readonly weight = input<number | null>(null)

  readonly color = input<string | null>(null)
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
  readonly visible = input<boolean | null, boolean | null>(null, {
    transform: (value) => value ?? false
  })

  /**
   * `fullScreen` cubre la pantalla; `sheet` entra desde abajo con el tirador
   * y los topes del sistema.
   */
  readonly presentation = input<'fullScreen' | 'sheet' | null>(null)

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
  readonly sheet = input<boolean | null, boolean | null>(null, {
    transform: (value) => value ?? false
  })

  readonly visible = input<boolean | null, boolean | null>(null, {
    transform: (value) => value ?? false
  })

  readonly title = input<string | null, string | null>(null, {
    transform: (value) => value ?? ''
  })

  readonly message = input<string | null, string | null>(null, {
    transform: (value) => value ?? ''
  })

  /** Títulos de los botones, en orden. Sin ninguno, sale un «OK». */
  readonly buttons = input<string | null, readonly string[] | null>(null, {
    transform: (value) => JSON.stringify(value ?? [])
  })

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
