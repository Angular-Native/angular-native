import SwiftUI

/// Las seis props de accesibilidad del contrato, sobre SwiftUI.
///
/// Este host es el único de los cuatro de Apple que es declarativo, y eso
/// cambia el sitio donde se aplican. En UIKit y en AppKit hay una vista viva y
/// un setter al que llamar cuando llega la prop; aquí hay una vista que se
/// construye entera en cada foto, así que la accesibilidad no se «pone»: se
/// declara al construirla, y por eso está en un `ViewModifier` que cuelga de
/// `AnNodeView.body` y no en ningún equivalente de `set_prop`.
///
/// **La decisión no está aquí.** Qué rol es qué trait, y qué parte del
/// contrato no tiene forma en SwiftUI, lo resuelve Rust en
/// `snapshot::accessibility_traits_of`, que es además quien lo dice por el
/// registro. Lo que llega aquí ya viene traducido: una lista de nombres de
/// trait y tres cadenas. Así la tabla vive en un sitio y no en dos, igual que
/// pasa con los iconos.
///
/// Lo único que este fichero decide es cómo se escribe cada cosa en SwiftUI, y
/// un nombre de trait que no reconozca sale por el registro en vez de
/// perderse: es la única forma de que un cambio en Rust que aquí no se hubiera
/// seguido se vea, en vez de dejar una vista muda.
struct AnAccessibility: ViewModifier {
    let node: AnNode

    func body(content: Content) -> some View {
        content
            // Primero, si esto es **un** elemento o un contenedor. Va antes que
            // el resto porque es lo que decide sobre qué se pega la etiqueta:
            // con `.combine`, la fila entera es una sola parada y la etiqueta
            // es la de la fila.
            .modifier(Agrupado(accessible: node.accessible))
            .modifier(Cadena(texto: node.accessibilityLabel, cual: .etiqueta))
            .modifier(Cadena(texto: node.accessibilityHint, cual: .pista))
            .modifier(Cadena(texto: node.accessibilityValue, cual: .valor))
            // Un juego vacío de traits no hace nada, así que este sí se puede
            // poner siempre: cuando no hay rol no cambia la vista.
            .accessibilityAddTraits(traits)
            // Y esconder la rama, que es lo contrario de agrupar: `false` en el
            // contrato es «esto es decorativo, no lo leas ni a mí ni a los
            // míos».
            .accessibilityHidden(node.accessible == false)
    }

    /// Los traits que pidió Rust, ya sumados.
    private var traits: AccessibilityTraits {
        var suma = AccessibilityTraits()
        for nombre in node.accessibilityTraits ?? [] {
            guard let trait = Self.trait(nombre) else {
                AnAccessibility.avisar(
                    "trait:\(nombre)",
                    "angular-native: Rust mandó el AccessibilityTraits «\(nombre)» y el shell "
                        + "del reloj no lo conoce; ese rasgo no se aplicó"
                )
                continue
            }
            suma.formUnion(trait)
        }
        return suma
    }

    /// El `AccessibilityTraits` que se llama así.
    ///
    /// Es la única tabla de este lado, y es a propósito una traducción tonta:
    /// los nombres son los de SwiftUI tal cual, así que quien lea
    /// `swiftui_trait()` en Rust está leyendo ya el nombre de la constante.
    private static func trait(_ nombre: String) -> AccessibilityTraits? {
        switch nombre {
        case "isButton": return .isButton
        case "isLink": return .isLink
        case "isHeader": return .isHeader
        case "isImage": return .isImage
        case "isStaticText": return .isStaticText
        case "isSearchField": return .isSearchField
        case "isSummaryElement": return .isSummaryElement
        case "isSelected": return .isSelected
        case "isToggle": return .isToggle
        default: return nil
        }
    }

    /// Convierte la vista en una sola parada del lector, o la deja como el
    /// contenedor que era.
    ///
    /// Va en un `ViewModifier` suyo, y no en un `if` dentro del de arriba, para
    /// que el tipo de lo que sale de `AnAccessibility` no dependa de si la
    /// plantilla puso `[accessible]`: con la rama encerrada aquí, `body` tiene
    /// un tipo y no dos.
    private struct Agrupado: ViewModifier {
        let accessible: Bool?

        @ViewBuilder
        func body(content: Content) -> some View {
            if accessible == true {
                // `.combine` y no `.ignore`: lo que hace falta es que la fila
                // —icono, título y subtítulo— se lea de una vez, no que se
                // quede muda.
                content.accessibilityElement(children: .combine)
            } else {
                content
            }
        }
    }

    /// Una de las tres cadenas, puesta solo si vino.
    ///
    /// Las tres tienen el mismo problema y por eso comparten modificador:
    /// SwiftUI no tiene un valor que signifique «no pongas ninguna».
    /// `.accessibilityLabel("")` no es no poner etiqueta, es poner una vacía, y
    /// encima de un `Toggle` —que trae la suya del sistema— eso deja el control
    /// sin nombre. Vacías no llegan nunca: Rust las filtra antes de meterlas en
    /// la foto.
    private struct Cadena: ViewModifier {
        enum Cual {
            case etiqueta
            case pista
            case valor
        }

        let texto: String?
        let cual: Cual

        @ViewBuilder
        func body(content: Content) -> some View {
            if let texto {
                switch cual {
                case .etiqueta: content.accessibilityLabel(Text(verbatim: texto))
                case .pista: content.accessibilityHint(Text(verbatim: texto))
                case .valor: content.accessibilityValue(Text(verbatim: texto))
                }
            } else {
                content
            }
        }
    }

    /// Lo ya dicho. Igual que en Rust: un aviso por frame a 30 Hz no se lee.
    ///
    /// `@MainActor` porque SwiftUI construye las vistas ahí y porque un estado
    /// mutable global sin dueño no compila en Swift 6.
    @MainActor private static var dicho = Set<String>()

    @MainActor
    private static func avisar(_ clave: String, _ mensaje: String) {
        guard !dicho.contains(clave) else { return }
        dicho.insert(clave)
        NSLog("%@", mensaje)
    }
}

extension View {
    func anAccessibility(_ node: AnNode) -> some View {
        modifier(AnAccessibility(node: node))
    }
}
