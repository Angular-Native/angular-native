import SwiftUI
import UIKit

/// Una imagen del bundle o de la red, y su tamaño natural de vuelta.
///
/// El tamaño no es un adorno: el layout no puede colocar algo cuyo tamaño no
/// conoce, y solo la imagen lo sabe. Por eso la directiva de `an-image`
/// registra el oyente `load` siempre, aunque la plantilla no lo escuche, y por
/// eso este host lo manda en cuanto tiene la imagen. Sin ese aviso una imagen
/// sin medidas se queda en cero, que es lo que pasaba antes en el reloj.
///
/// No se usa `AsyncImage`: solo sabe de URLs, y aquí la mitad de los casos son
/// recursos del bundle. Y hace falta el `UIImage` de todas formas, porque el
/// tamaño en puntos sale de él.
struct AnImageView: View {
    let node: AnNode
    let dispatch: (UInt32, String, [String: Any]) -> Void

    @State private var imagen: UIImage?

    var body: some View {
        contenido
            // `task(id:)` se rehace cuando cambia la ruta y se cancela sola al
            // desaparecer la vista: una descarga que ya no le importa a nadie
            // no se queda corriendo.
            .task(id: node.source) {
                imagen = await AnImageStore.shared.load(node.source)
                guard let imagen else { return }
                let tamaño = imagen.size
                dispatch(node.id, "load", ["width": tamaño.width, "height": tamaño.height])
            }
    }

    @ViewBuilder
    private var contenido: some View {
        if let imagen {
            Image(uiImage: imagen)
                .resizable()
                .aspectRatio(contentMode: modo)
                .frame(width: node.width, height: node.height, alignment: .center)
                .clipped()
        } else {
            // Mientras no hay imagen no se pinta nada. Un marcador gris sería
            // una imagen que la plantilla no pidió.
            Color.clear
        }
    }

    /// `contain` por defecto, como en iOS. `stretch` y `center` no son modos de
    /// relación de aspecto y se resuelven abajo, en `body`, con el marco.
    private var modo: ContentMode {
        node.resizeMode == "cover" ? .fill : .fit
    }
}

/// Las imágenes ya cargadas, para no volver a leerlas ni a bajarlas en cada
/// foto. El árbol se rehace treinta veces por segundo; las imágenes, no.
actor AnImageStore {
    static let shared = AnImageStore()

    private var cache: [String: UIImage] = [:]

    func load(_ source: String?) async -> UIImage? {
        guard let source, !source.isEmpty else { return nil }
        if let hit = cache[source] {
            return hit
        }
        let imagen: UIImage?
        if source.hasPrefix("http://") || source.hasPrefix("https://") {
            imagen = await descarga(source)
        } else {
            // Sin esquema es un recurso del bundle de la app, igual que en iOS.
            imagen = UIImage(named: source)
            if imagen == nil {
                NSLog("angular-native: no hay ninguna imagen llamada \(source) en el bundle")
            }
        }
        if let imagen {
            cache[source] = imagen
        }
        return imagen
    }

    private func descarga(_ source: String) async -> UIImage? {
        guard let url = URL(string: source) else {
            NSLog("angular-native: \(source) no es una URL")
            return nil
        }
        do {
            let (data, _) = try await URLSession.shared.data(from: url)
            guard let imagen = UIImage(data: data) else {
                NSLog("angular-native: lo que llegó de \(source) no es una imagen")
                return nil
            }
            return imagen
        } catch {
            NSLog("angular-native: no se pudo bajar \(source): \(error.localizedDescription)")
            return nil
        }
    }
}
