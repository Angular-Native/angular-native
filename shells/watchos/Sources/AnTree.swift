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
    /// Lo que el sistema presenta encima —`an-alert` y `an-modal`—, que llega
    /// fuera del árbol porque en SwiftUI no son vistas que se coloquen sino
    /// modificadores sobre la raíz.
    private(set) var overlays: [AnNode] = []
    /// La revisión que ya está reflejada aquí. Empieza fuera de rango para que
    /// el primer frame siempre entre.
    private var mirrored: UInt64 = .max

    /// Rust manda `border_radius`; Swift lo quiere como `borderRadius`. La
    /// conversión la hace el decodificador con una regla, no una tabla de
    /// `CodingKeys` de cuarenta líneas que habría que tocar en dos sitios cada
    /// vez que la foto crece.
    private static let decoder: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return decoder
    }()

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
            let snapshot = try Self.decoder.decode(AnSnapshot.self, from: data)
            root = snapshot.root
            overlays = snapshot.overlays ?? []
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
    let overlays: [AnNode]?
}

/// Un nodo tal y como lo manda Rust.
///
/// `Identifiable` con el id que asignó JS, que es estable entre frames: sin él
/// SwiftUI trataría cada foto como contenido nuevo y perdería, entre otras
/// cosas, la posición de scroll en cada cambio.
///
/// Todo es opcional salvo lo que tiene todo nodo. Rust omite lo que no aplica
/// al tipo —un `Text` no manda `minimum`— y así la foto de una pantalla se lee
/// de un vistazo cuando hay que depurarla.
struct AnNode: Decodable, Identifiable, Equatable {
    let id: UInt32
    let kind: String
    let x: Double
    let y: Double
    let width: Double
    let height: Double

    /// Por qué el reloj no pinta esto. Lo decide Rust, que es donde está la
    /// lista y el motivo; aquí solo se deja el hueco.
    let unsupported: String?

    let background: [Double]?
    let color: [Double]?
    let borderRadius: Double?
    let opacity: Double?
    let disabled: Bool?
    let testId: String?

    let text: String?
    let fontSize: Double?
    let fontWeight: Int?
    let italic: Bool?
    let fontFamily: String?
    let letterSpacing: Double?
    let textAlign: String?
    let textDecoration: String?
    let maxLines: Int?

    let on: Bool?
    let value: Double?
    let minimum: Double?
    let maximum: Double?
    let step: Double?
    let progress: Double?
    let animating: Bool?
    let field: String?
    let placeholder: String?
    let secure: Bool?
    let keyboard: String?
    let items: [String]?
    let selectedIndex: Int?
    let dateMode: String?
    let symbol: String?
    let symbolSize: Double?
    let symbolWeight: Int?
    let source: String?
    let resizeMode: String?

    let visible: Bool?
    let title: String?
    let message: String?
    let buttons: [String]?
    let presentation: String?
    let transition: String?

    let contentWidth: Double?
    let contentHeight: Double?

    /// Qué escucha la plantilla sobre este nodo. El shell solo engancha lo que
    /// está aquí: un reconocedor de más se comería los gestos del
    /// `ScrollView` de debajo, y en el reloj el sistema además realza lo que
    /// cree tocable.
    let listens: [String]?

    let children: [AnNode]?

    func listens(to event: String) -> Bool {
        listens?.contains(event) == true
    }
}
