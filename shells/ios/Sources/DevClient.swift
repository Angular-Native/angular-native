import Foundation

/// Cliente del servidor de desarrollo.
///
/// El `.app` trae un `dev-server.txt` cuando lo armó `an dev`. Si está, la app
/// abre un WebSocket y espera avisos de recarga; si no está, este objeto no
/// llega a existir y la app se comporta como una compilación normal.
final class DevClient: NSObject {
    private let baseURL: URL
    private let onReload: (String) -> Void
    private var socket: URLSessionWebSocketTask?
    private lazy var session = URLSession(configuration: .default)

    init?(bundle: Bundle, onReload: @escaping (String) -> Void) {
        guard let path = bundle.path(forResource: "dev-server", ofType: "txt"),
              let raw = try? String(contentsOfFile: path, encoding: .utf8),
              let url = URL(string: raw.trimmingCharacters(in: .whitespacesAndNewlines))
        else {
            return nil
        }
        self.baseURL = url
        self.onReload = onReload
        super.init()
    }

    func connect() {
        guard var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false) else {
            return
        }
        components.scheme = components.scheme == "https" ? "wss" : "ws"
        components.path = "/ws"
        guard let url = components.url else { return }

        let socket = session.webSocketTask(with: url)
        self.socket = socket
        socket.resume()
        NSLog("angular-native: conectado al servidor de desarrollo en \(baseURL)")
        receive()
    }

    private func receive() {
        socket?.receive { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let message):
                if case .string(let text) = message, text == "reload" {
                    self.fetchBundle()
                }
                // Un solo `receive` entrega un solo mensaje: hay que volver a
                // pedir turno o la conexión queda muda.
                self.receive()
            case .failure(let error):
                NSLog("angular-native: se cayó el servidor de desarrollo (\(error.localizedDescription))")
            }
        }
    }

    private func fetchBundle() {
        let url = baseURL.appendingPathComponent("bundle.js")
        session.dataTask(with: url) { [weak self] data, _, error in
            guard let self else { return }
            guard let data, let source = String(data: data, encoding: .utf8) else {
                NSLog("angular-native: no se pudo bajar el bundle (\(error?.localizedDescription ?? "sin datos"))")
                return
            }
            // El runtime solo se toca desde el hilo principal.
            DispatchQueue.main.async { self.onReload(source) }
        }.resume()
    }
}
