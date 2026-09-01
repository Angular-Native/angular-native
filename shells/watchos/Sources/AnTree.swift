import Foundation
import Observation

/// El modelo observable que SwiftUI mira.
///
/// Es el reflejo del árbol que mantiene Rust. No se construye desde Swift: se
/// decodifica de la foto que devuelve `an_watch_runtime_snapshot`, y solo
/// cuando la revisión cambia. En un frame quieto no se decodifica nada y
/// SwiftUI no recompone.
@Observable
final class AnTree {
    private(set) var root: AnNode?
    /// La revisión que ya está reflejada aquí. Empieza fuera de rango para que
    /// el primer frame siempre entre.
    private var mirrored: UInt64 = .max

    /// Vuelca la foto si Rust dice que cambió. Devuelve si hubo cambio, que es
    /// lo que el shell mira para saber si tiene sentido registrar el frame.
    @discardableResult
    func sync(runtime: OpaquePointer?) -> Bool {
        guard let runtime else { return false }
        let revision = an_watch_runtime_revision(runtime)
        guard revision != mirrored else { return false }
        mirrored = revision

        guard let raw = an_watch_runtime_snapshot(runtime) else {
            NSLog("angular-native: el árbol no se pudo serializar")
            return false
        }
        // `String(cString:)` copia. El puntero de Rust solo vale hasta la
        // siguiente llamada, así que no se guarda.
        let json = String(cString: raw)
        guard let data = json.data(using: .utf8) else { return false }
        do {
            let snapshot = try JSONDecoder().decode(AnSnapshot.self, from: data)
            root = snapshot.root
            return true
        } catch {
            NSLog("angular-native: la foto del árbol no se entiende: \(error)")
            return false
        }
    }
}

struct AnSnapshot: Decodable {
    let revision: UInt64
    let root: AnNode?
}

/// Un nodo tal y como lo manda Rust.
///
/// `Identifiable` con el id que asignó JS, que es estable entre frames: sin él
/// SwiftUI trataría cada foto como contenido nuevo y perdería, entre otras
/// cosas, la posición de scroll en cada cambio.
struct AnNode: Decodable, Identifiable, Equatable {
    let id: UInt32
    let kind: String
    let x: Double
    let y: Double
    let width: Double
    let height: Double

    let background: [Double]?
    let color: [Double]?
    let borderRadius: Double?
    let opacity: Double?

    let text: String?
    let fontSize: Double?
    let fontWeight: Int?
    let textAlign: String?

    let pressable: Bool?

    let contentWidth: Double?
    let contentHeight: Double?

    let children: [AnNode]?

    enum CodingKeys: String, CodingKey {
        case id, kind, x, y, width, height
        case background, color, opacity, text
        case borderRadius = "border_radius"
        case fontSize = "font_size"
        case fontWeight = "font_weight"
        case textAlign = "text_align"
        case pressable
        case contentWidth = "content_width"
        case contentHeight = "content_height"
        case children
    }
}
