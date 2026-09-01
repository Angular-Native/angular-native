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
struct AnNodeView: View {
    let node: AnNode
    /// Por dónde salen los toques hacia Rust.
    let dispatch: (UInt32, String) -> Void

    var body: some View {
        content
            .frame(width: node.width, height: node.height)
            .position(x: node.x + node.width / 2, y: node.y + node.height / 2)
    }

    @ViewBuilder
    private var content: some View {
        switch node.kind {
        case "Text":
            textView
        case "Button":
            buttonView
        case "ScrollView":
            scrollView
        default:
            // `View` y todo lo que el reloj todavía no sabe pintar. Un
            // contenedor con fondo y esquinas es lo que `View` es, y para lo
            // demás deja ver dónde está el hueco en vez de desaparecer.
            container
        }
    }

    /// Contenedor: fondo, esquinas e hijos colocados por marco.
    ///
    /// El `ZStack` con `topLeading` es el sistema de coordenadas del padre.
    /// Sin `alignment` explícito SwiftUI centraría, y los marcos de taffy son
    /// desde la esquina superior izquierda.
    private var container: some View {
        ZStack(alignment: .topLeading) {
            ForEach(node.children ?? []) { child in
                AnNodeView(node: child, dispatch: dispatch)
            }
        }
        .frame(width: node.width, height: node.height, alignment: .topLeading)
        .background(background)
        .clipShape(RoundedRectangle(cornerRadius: node.borderRadius ?? 0))
        .opacity(node.opacity ?? 1)
        .pressable(node, dispatch: dispatch)
    }

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
            .font(.system(size: node.fontSize ?? 16, weight: swiftWeight))
            .foregroundStyle(foreground)
            .multilineTextAlignment(alignment)
            .frame(width: node.width, height: node.height, alignment: blockAlignment)
            .fixedSize(horizontal: false, vertical: true)
            .opacity(node.opacity ?? 1)
            .pressable(node, dispatch: dispatch)
    }

    /// Botón del sistema. Se usa `Button` de SwiftUI y no un `View` con gesto
    /// para que traiga lo que trae en watchOS: el realce al tocar y el
    /// retorno háptico. El `buttonStyle(.plain)` quita el fondo de cápsula que
    /// pondría por defecto, porque el fondo lo decide la app.
    private var buttonView: some View {
        Button {
            dispatch(node.id, "press")
        } label: {
            Text(node.text ?? "")
                .font(.system(size: node.fontSize ?? 16, weight: swiftWeight))
                .foregroundStyle(foreground)
                .frame(width: node.width, height: node.height)
        }
        .buttonStyle(.plain)
        .background(background)
        .clipShape(RoundedRectangle(cornerRadius: node.borderRadius ?? 0))
        .opacity(node.opacity ?? 1)
    }

    /// El `ScrollView` sí es el de SwiftUI: el desplazamiento con la corona
    /// digital no se puede imitar, y es la única forma de que se sienta como el
    /// resto del reloj.
    ///
    /// Dentro sigue mandando taffy: el contenido es un `ZStack` del tamaño que
    /// dijo `contentSize`, con los hijos en sus marcos.
    private var scrollView: some View {
        ScrollView(.vertical) {
            ZStack(alignment: .topLeading) {
                ForEach(node.children ?? []) { child in
                    AnNodeView(node: child, dispatch: dispatch)
                }
            }
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

    private var background: Color {
        AnNodeView.color(node.background) ?? .clear
    }

    /// Sin `color` explícito manda el blanco: en el reloj el fondo del sistema
    /// es negro y heredar el color primario de SwiftUI daría lo mismo, pero
    /// dicho explícitamente no depende de qué haga SwiftUI mañana.
    private var foreground: Color {
        AnNodeView.color(node.color) ?? .white
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
    private var swiftWeight: Font.Weight {
        switch node.fontWeight ?? 400 {
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

private extension View {
    /// Un `(press)` sobre algo que no es un `Button`. Solo se engancha si la
    /// plantilla lo pidió: envolver todo en un gesto haría que SwiftUI se
    /// comiera toques que deberían llegar al `ScrollView` de debajo.
    @ViewBuilder
    func pressable(_ node: AnNode, dispatch: @escaping (UInt32, String) -> Void) -> some View {
        if node.pressable == true {
            // `contentShape` hace tocable el rectángulo entero, incluidas las
            // zonas transparentes: sin él solo respondería donde se pintó algo.
            self.contentShape(Rectangle())
                .onTapGesture { dispatch(node.id, "press") }
        } else {
            self
        }
    }
}
