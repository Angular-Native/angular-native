import { Directive, ElementRef, inject, Input, Renderer2 } from '@angular/core'
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

/** Marco resuelto de una vista, relativo a su padre y en puntos. */
export interface NativeLayoutEvent {
  x: number
  y: number
  width: number
  height: number
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
   * El marco que le asignó el layout, cada vez que cambia.
   *
   * No lo produce ninguna plataforma: lo emite el core al terminar el commit,
   * porque es él quien calcula el marco. Sale gratis en iOS y en Android.
   */
  readonly layout = outputFromObservable(this.nativeEvent<NativeLayoutEvent>('layout'))

  @Input() set backgroundColor(value: string | null) {
    this.set('backgroundColor', value)
  }

  @Input() set borderRadius(value: number | null) {
    this.set('borderRadius', value)
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

@Directive({ selector: 'ScrollView' })
export class ScrollView extends NativeVisual {
  @Input() set showsScrollIndicator(value: boolean | null) {
    this.set('showsScrollIndicator', value)
  }

  /** El rebote de iOS al llegar al final. */
  @Input() set bounces(value: boolean | null) {
    this.set('bounces', value)
  }

  /**
   * Se emite en cada frame de desplazamiento. El `contentSize` lo calcula el
   * layout solo: es el tamaño que ocupan los hijos, y el core lo manda al
   * `UIScrollView` cuando cambia.
   */
  readonly scroll = outputFromObservable(this.nativeEvent<NativeScrollEvent>('scroll'))
}

@Directive({ selector: 'Image' })
export class Image extends NativeVisual {
  @Input() set source(value: string | null) {
    this.set('source', value)
  }

  /**
   * Tamaño intrínseco. Hasta que exista carga de imágenes, es lo que el layout
   * usa para medir; una imagen sin esto ocupa cero.
   */
  @Input() set intrinsicWidth(value: number | null) {
    this.set('intrinsicWidth', value)
  }

  @Input() set intrinsicHeight(value: number | null) {
    this.set('intrinsicHeight', value)
  }
}

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
}

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

/** Para importar todas de golpe en un componente standalone. */
export const NATIVE_PRIMITIVES = [View, Text, Image, ScrollView, TextInput] as const
