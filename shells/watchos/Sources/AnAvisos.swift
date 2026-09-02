import Foundation

/// Avisos que se dicen una vez.
///
/// El árbol se rehace treinta veces por segundo, así que un aviso puesto donde
/// se pinta se diría treinta veces por segundo y el registro dejaría de servir
/// para nada. La parte de Rust tiene lo mismo, por lo mismo, en `snapshot.rs`.
///
/// Es `@MainActor` y no un `Mutex` porque todo lo que avisa —pintar, decodificar
/// la foto, reconciliar los controles— pasa en el hilo principal: un candado
/// aquí sería un candado que nadie disputa.
@MainActor
enum AnAvisos {
    private static var dichos: Set<String> = []

    static func unaVez(_ clave: String, _ decir: () -> Void) {
        guard dichos.insert(clave).inserted else { return }
        decir()
    }
}
