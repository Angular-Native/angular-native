import SwiftUI

/// Pinta un nodo y, recursivamente, los suyos.
///
/// La regla que gobierna todo este fichero: **el layout ya está hecho**. taffy
/// calculó en Rust el marco de cada nodo, relativo a su padre. Aquí solo se
/// coloca. Por eso no hay ni un `VStack` ni un `HStack` ni un `.padding` en
/// todo el shell: si los hubiera, habría dos motores de layout decidiendo lo
/// mismo, y el resultado sería el de ninguno de los dos.
///
/// El patrón de colocación es siempre el mismo:
///
///     .frame(width:height:)      el tamaño que dijo taffy
///     .position(x:y:)            el centro, dentro del ZStack del padre
///
/// Se usa `.position` y no `.offset` porque `.offset` desplaza desde donde
/// SwiftUI hubiera puesto la vista —el centro del contenedor—, y eso obligaría
/// a compensar el centrado en cada nodo. `.position` es absoluto respecto al
/// contenedor, que es justo lo que da taffy.
///
/// Los controles son los de SwiftUI, no dibujos que se les parezcan: un
/// `Toggle`, un `Slider`, un `Picker`. Traen lo que trae el sistema —el
/// háptico, el realce, el giro de la corona— y eso no se imita.
struct AnNodeView: View {
    let node: AnNode
    /// El estado que el dedo mueve y la app todavía no ha confirmado.
    let controls: AnControls
    /// Por dónde salen los eventos hacia Rust.
    let dispatch: (UInt32, String, [String: Any]) -> Void
    /// Quién tiene ahora mismo la corona. Se pasa a mano por todo el árbol
    /// porque un `FocusState` no viaja por el entorno de SwiftUI.
    let crownFocus: FocusState<UInt32?>.Binding

    var body: some View {
        content
            .frame(width: node.width, height: node.height)
            .position(x: node.x + node.width / 2, y: node.y + node.height / 2)
    }

    @ViewBuilder
    private var content: some View {
        if node.unsupported != nil {
            // El hueco del tamaño que dijo el layout. El porqué ya lo dijo el
            // host por el registro, una vez y con el motivo del SDK: pintar
            // aquí una caja con un rótulo sería inventarse un control.
            Color.clear
        } else {
            switch node.kind {
            case "Text": textView
            case "Button": buttonView
            case "ScrollView": scrollView
            case "Image": imageView
            case "Icon": iconView
            case "Switch": toggleView
            case "Slider": sliderView
            case "Stepper": stepperView
            case "ProgressBar": progressView
            case "ActivityIndicator": spinnerView
            case "TextInput": fieldView
            case "Picker": pickerView
            case "DatePicker": dateView
            case "StackView": stackView
            default: container
            }
        }
    }

    // ------------------------------------------------------------ contenedores

    /// Contenedor: fondo, esquinas e hijos colocados por marco.
    ///
    /// El `ZStack` con `topLeading` es el sistema de coordenadas del padre.
    /// Sin `alignment` explícito SwiftUI centraría, y los marcos de taffy son
    /// desde la esquina superior izquierda.
    private var container: some View {
        hijos
            .frame(width: node.width, height: node.height, alignment: .topLeading)
            .background(background)
            .clipShape(RoundedRectangle(cornerRadius: node.borderRadius ?? 0))
            .opacity(node.opacity ?? 1)
            .anGestures(node, dispatch: dispatch)
            .anCrown(node, controls: controls, dispatch: dispatch, focus: crownFocus)
    }

    private var hijos: some View {
        ZStack(alignment: .topLeading) {
            ForEach(node.children ?? []) { child in
                AnNodeView(
                    node: child,
                    controls: controls,
                    dispatch: dispatch,
                    crownFocus: crownFocus
                )
            }
        }
    }

    /// Pila de pantallas: solo se ve la de arriba, y entra o sale según el
    /// sentido que diga la plantilla.
    ///
    /// El core ya le puso a la pila el tamaño del padre y a sus hijos uno
    /// encima de otro; lo único que decide el shell es cuál se ve y cómo entra.
    private var stackView: some View {
        ZStack(alignment: .topLeading) {
            if let top = node.children?.last {
                AnNodeView(
                    node: top,
                    controls: controls,
                    dispatch: dispatch,
                    crownFocus: crownFocus
                )
                .transition(.asymmetric(insertion: .move(edge: .trailing), removal: .move(edge: .leading)))
            }
        }
        .frame(width: node.width, height: node.height, alignment: .topLeading)
        .clipped()
        .animation(.easeOut(duration: 0.25), value: node.children?.last?.id)
    }

    /// El `ScrollView` sí es el de SwiftUI: el desplazamiento con la corona
    /// digital no se puede imitar, y es la única forma de que se sienta como el
    /// resto del reloj.
    ///
    /// Dentro sigue mandando taffy: el contenido es un `ZStack` del tamaño que
    /// dijo `contentSize`, con los hijos en sus marcos.
    private var scrollView: some View {
        ScrollView(.vertical) {
            hijos
                .frame(
                    width: node.contentWidth ?? node.width,
                    height: node.contentHeight ?? node.height,
                    alignment: .topLeading
                )
        }
        .frame(width: node.width, height: node.height)
        .background(background)
        .clipShape(RoundedRectangle(cornerRadius: node.borderRadius ?? 0))
    }

    // ------------------------------------------------------------------ texto

    /// Texto ya medido por UIKit en Rust: el marco que trae es exactamente el
    /// que ocupa. `fixedSize` evita que SwiftUI decida truncarlo por su cuenta
    /// tras haberle dado ese marco.
    ///
    /// Hacen falta las dos alineaciones y no son la misma cosa:
    /// `multilineTextAlignment` reparte las líneas *dentro* del bloque de
    /// texto, y la del `frame` coloca ese bloque dentro del marco que dio
    /// taffy. Con solo la primera, un texto centrado en una caja más ancha que
    /// él se queda pegado a la izquierda con las líneas centradas entre sí.
    private var textView: some View {
        Text(node.text ?? "")
            .font(fuente)
            .tracking(node.letterSpacing ?? 0)
            .underline(node.textDecoration == "underline")
            .strikethrough(node.textDecoration == "lineThrough")
            .foregroundStyle(foreground)
            .multilineTextAlignment(alignment)
            .lineLimit(node.maxLines)
            .frame(width: node.width, height: node.height, alignment: blockAlignment)
            .fixedSize(horizontal: false, vertical: true)
            .opacity(node.opacity ?? 1)
            .anGestures(node, dispatch: dispatch)
    }

    /// Botón del sistema. Se usa `Button` de SwiftUI y no un `View` con gesto
    /// para que traiga lo que trae en watchOS: el realce al tocar y el
    /// retorno háptico. El `buttonStyle(.plain)` quita el fondo de cápsula que
    /// pondría por defecto, porque el fondo lo decide la app.
    private var buttonView: some View {
        Button {
            dispatch(node.id, "press", [:])
        } label: {
            Text(node.text ?? "")
                .font(fuente)
                .foregroundStyle(foreground)
                .frame(width: node.width, height: node.height)
        }
        .buttonStyle(.plain)
        .disabled(node.disabled == true)
        .background(background)
        .clipShape(RoundedRectangle(cornerRadius: node.borderRadius ?? 0))
        .opacity(node.opacity ?? 1)
        .anGestures(node, dispatch: dispatch, dueñoDelToque: true)
    }

    // -------------------------------------------------------------- imágenes

    /// Una imagen del bundle o de la red.
    ///
    /// El tamaño natural se devuelve por `(load)` en cuanto se sabe: el layout
    /// no puede colocar algo cuyo tamaño no conoce, y solo la imagen lo sabe.
    /// Es el mismo contrato que en iOS, donde la directiva registra ese oyente
    /// aunque la plantilla no lo escuche.
    private var imageView: some View {
        AnImageView(node: node, dispatch: dispatch)
            .frame(width: node.width, height: node.height)
            .clipShape(RoundedRectangle(cornerRadius: node.borderRadius ?? 0))
            .opacity(node.opacity ?? 1)
            .anGestures(node, dispatch: dispatch)
    }

    /// Un SF Symbol, pedido por su nombre. No se empaqueta ningún juego de
    /// iconos: lo dibuja el sistema, con el trazo que le toque a esa versión de
    /// watchOS.
    private var iconView: some View {
        Image(systemName: node.symbol ?? "questionmark")
            .font(.system(size: node.symbolSize ?? 24, weight: pesoDeSwift(node.symbolWeight)))
            .foregroundStyle(foreground)
            .frame(width: node.width, height: node.height)
            .opacity(node.opacity ?? 1)
            .anGestures(node, dispatch: dispatch)
    }

    // -------------------------------------------------------------- controles

    /// `Toggle` con el estilo de interruptor, que es el del sistema. Sin
    /// rótulo: el rótulo lo pone la plantilla al lado, porque quien decide el
    /// layout es taffy.
    private var toggleView: some View {
        Toggle("", isOn: controls.flag(node))
            .labelsHidden()
            .tint(colorPropio ?? .green)
            .disabled(node.disabled == true)
            .frame(width: node.width, height: node.height, alignment: .trailing)
            .opacity(node.opacity ?? 1)
    }

    private var sliderView: some View {
        Slider(
            value: controls.number(node),
            in: (node.minimum ?? 0)...max(node.maximum ?? 1, (node.minimum ?? 0) + 0.000_001)
        )
        .tint(colorPropio)
        .disabled(node.disabled == true)
        .frame(width: node.width, height: node.height)
        .opacity(node.opacity ?? 1)
    }

    /// `Stepper` del sistema: los dos botones con el más y el menos, y el giro
    /// de la corona cuando tiene el foco, que en el reloj es como se usa de
    /// verdad.
    private var stepperView: some View {
        Stepper(
            value: controls.number(node),
            in: (node.minimum ?? 0)...max(node.maximum ?? 100, node.minimum ?? 0),
            step: node.step ?? 1
        ) {
            EmptyView()
        }
        .disabled(node.disabled == true)
        .frame(width: node.width, height: node.height)
        .opacity(node.opacity ?? 1)
    }

    private var progressView: some View {
        ProgressView(value: node.progress ?? 0)
            .tint(colorPropio)
            .frame(width: node.width, height: node.height)
            .opacity(node.opacity ?? 1)
    }

    /// La ruedecilla. `animating` a `false` la esconde, igual que
    /// `hidesWhenStopped` en iOS: un indicador parado no dice nada.
    @ViewBuilder
    private var spinnerView: some View {
        if node.animating == false {
            Color.clear
        } else {
            ProgressView()
                .tint(colorPropio)
                .frame(width: node.width, height: node.height)
                .opacity(node.opacity ?? 1)
        }
    }

    /// Campo de texto.
    ///
    /// En el reloj un `TextField` no se escribe en su sitio: al tocarlo el
    /// sistema abre su propia pantalla —dictado, garabateo o teclado— y
    /// devuelve el texto. Eso es exactamente lo que hace este control, y por
    /// eso es el del sistema y no una caja con un cursor dibujado.
    private var fieldView: some View {
        campo
            .font(fuente)
            .foregroundStyle(foreground)
            .multilineTextAlignment(alignment)
            .disabled(node.disabled == true)
            .frame(width: node.width, height: node.height, alignment: blockAlignment)
            .opacity(node.opacity ?? 1)
            .onSubmit { dispatch(node.id, "submit", ["value": controls.text(node).wrappedValue]) }
    }

    @ViewBuilder
    private var campo: some View {
        if node.secure == true {
            SecureField(node.placeholder ?? "", text: controls.text(node))
        } else {
            TextField(node.placeholder ?? "", text: controls.text(node))
        }
    }

    /// Elegir una opción. En el reloj el `Picker` es la rueda que gira con la
    /// corona; no hay desplegable, y fabricar uno sería imitar un control que
    /// el sistema no tiene.
    private var pickerView: some View {
        Picker("", selection: controls.index(node)) {
            ForEach(Array((node.items ?? []).enumerated()), id: \.offset) { posicion, rótulo in
                Text(rótulo).tag(posicion)
            }
        }
        .labelsHidden()
        .disabled(node.disabled == true)
        .frame(width: node.width, height: node.height)
        .opacity(node.opacity ?? 1)
    }

    /// Fecha y hora. Al tocarlo, el reloj abre su propio selector —el de las
    /// esferas— y devuelve el resultado.
    private var dateView: some View {
        DatePicker("", selection: controls.date(node), displayedComponents: componentes)
            .labelsHidden()
            .disabled(node.disabled == true)
            .frame(width: node.width, height: node.height)
            .opacity(node.opacity ?? 1)
    }

    private var componentes: DatePickerComponents {
        switch node.dateMode {
        case "time": [.hourAndMinute]
        case "dateAndTime": [.date, .hourAndMinute]
        default: [.date]
        }
    }

    // ------------------------------------------------------------------ estilo

    private var background: Color {
        AnNodeView.color(node.background) ?? .clear
    }

    /// Sin `color` explícito manda el blanco: en el reloj el fondo del sistema
    /// es negro y heredar el color primario de SwiftUI daría lo mismo, pero
    /// dicho explícitamente no depende de qué haga SwiftUI mañana.
    private var foreground: Color {
        AnNodeView.color(node.color) ?? .white
    }

    /// El color de la plantilla, sin sustituto: un control sin `[color]` se
    /// queda con el suyo, que es el del sistema.
    private var colorPropio: Color? {
        AnNodeView.color(node.color)
    }

    /// La fuente de un texto o del rótulo de un botón.
    ///
    /// Con `[fontFamily]` se pide esa por su nombre; sin ella, la del sistema.
    /// Es la misma decisión que tomó `WatchMeasurer` al medir, y tiene que
    /// serlo: si aquí se pintase con otra, el marco no le vendría bien.
    private var fuente: Font {
        let puntos = node.fontSize ?? 16
        var base: Font
        if let familia = node.fontFamily, !familia.isEmpty {
            base = .custom(familia, size: puntos)
        } else {
            base = .system(size: puntos, weight: pesoDeSwift(node.fontWeight))
        }
        if node.italic == true {
            base = base.italic()
        }
        return base
    }

    private var alignment: TextAlignment {
        switch node.textAlign {
        case "center": .center
        case "right": .trailing
        default: .leading
        }
    }

    /// Dónde va el bloque de texto dentro de su marco.
    ///
    /// En vertical siempre al centro, que es lo que hace un `UILabel` y por
    /// tanto lo que se ve en iOS: cuando el marco es más alto que el texto
    /// —una fila de altura fija, un botón— pegarlo arriba se nota y no es lo
    /// que nadie espera.
    private var blockAlignment: Alignment {
        switch node.textAlign {
        case "center": .center
        case "right": .trailing
        default: .leading
        }
    }

    /// La escala CSS 100..900 a los pesos de SwiftUI. Es la misma tabla con la
    /// que Rust midió el texto: si divergieran, el marco no le vendría bien a
    /// la letra que se acaba dibujando.
    private func pesoDeSwift(_ peso: Int?) -> Font.Weight {
        switch peso ?? 400 {
        case ..<200: .ultraLight
        case ..<300: .thin
        case ..<400: .light
        case ..<500: .regular
        case ..<600: .medium
        case ..<700: .semibold
        case ..<800: .bold
        case ..<900: .heavy
        default: .black
        }
    }

    /// Los canales llegan ya resueltos a 0..1 desde Rust: aquí no se analiza
    /// ningún `#rrggbb`, para que no haya dos analizadores de color que
    /// mantener de acuerdo.
    static func color(_ channels: [Double]?) -> Color? {
        guard let c = channels, c.count == 4 else { return nil }
        return Color(.sRGB, red: c[0], green: c[1], blue: c[2], opacity: c[3])
    }
}
