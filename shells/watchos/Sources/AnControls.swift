import Foundation
import Observation
import SwiftUI

/// Lo que un control tiene dentro mientras se toca.
enum AnControlValue: Equatable {
    case number(Double)
    case text(String)
    case flag(Bool)
    case index(Int)
}

/// El estado de los controles, que es lo único que el shell guarda por su
/// cuenta.
///
/// Hace falta porque los dos lados llevan el mismo dato y no van al mismo
/// ritmo. Un `Slider` de SwiftUI necesita un `Binding` que pueda escribir en el
/// acto —el dedo está encima— mientras que el valor de verdad vive en una
/// señal de Angular, al otro lado de QuickJS, y no vuelve hasta el frame
/// siguiente. Sin este intermedio el deslizador saltaría hacia atrás en cada
/// arrastre, porque cada foto lo devolvería al valor viejo.
///
/// La regla de reconciliación es una sola y es la que evita que el shell y la
/// app se peleen:
///
/// > si el valor que llega de Rust **es distinto del que llegó la última vez**,
/// > lo cambió la app, y manda la app. Si es el mismo, manda lo que haya tocado
/// > el dedo.
///
/// Así una señal que cambia el valor desde código se ve enseguida, y un
/// arrastre no se pisa con el eco de su propio evento.
///
/// Y por eso mismo el estado sobrevive a la recarga en caliente: `an dev`
/// mantiene el árbol montado y los ids, así que aquí no se toca nada; lo que
/// desaparece del árbol se limpia en `reconcile`, que es también lo que vacía
/// esto cuando una recarga en frío tira el árbol entero.
@MainActor
@Observable
final class AnControls {
    /// Lo que se está tocando ahora mismo.
    private var locals: [UInt32: AnControlValue] = [:]
    /// Lo último que dijo Rust, para saber si un valor nuevo viene de la app.
    private var mirrored: [UInt32: AnControlValue] = [:]
    /// Presentaciones que el sistema ya ha cerrado y cuya plantilla todavía
    /// no se ha enterado.
    ///
    /// Sin esto un diálogo se reabriría solo: al pulsar un botón SwiftUI lo
    /// cierra, pero `[visible]` sigue valiendo `true` hasta que JS reacciona al
    /// `(select)`, y el frame de en medio lo volvería a presentar.
    private var cerrados: Set<UInt32> = []

    /// Por dónde salen los cambios hacia JS.
    @ObservationIgnored var dispatch: (UInt32, String, [String: Any]) -> Void = { _, _, _ in }

    /// Reconcilia el estado con la foto recién llegada y olvida los nodos que
    /// ya no están.
    func reconcile(root: AnNode?, overlays: [AnNode]) {
        var vivos: Set<UInt32> = []
        if let root {
            walk(root, &vivos)
        }
        for overlay in overlays {
            walk(overlay, &vivos)
        }
        locals = locals.filter { vivos.contains($0.key) }
        mirrored = mirrored.filter { vivos.contains($0.key) }
        cerrados = cerrados.filter { vivos.contains($0) }
    }

    private func walk(_ node: AnNode, _ vivos: inout Set<UInt32>) {
        vivos.insert(node.id)
        // La plantilla ya se enteró de que estaba cerrado: la próxima vez que
        // lo abra hay que dejarla.
        if node.visible == false {
            cerrados.remove(node.id)
        }
        if let incoming = Self.declared(node) {
            // Un valor distinto del anterior solo puede venir de la app: el
            // eco del dedo llegó ya en la foto pasada.
            if mirrored[node.id] != incoming {
                locals[node.id] = incoming
            }
            mirrored[node.id] = incoming
        }
        for child in node.children ?? [] {
            walk(child, &vivos)
        }
    }

    /// El valor que la plantilla declara para este nodo, si es un control.
    private static func declared(_ node: AnNode) -> AnControlValue? {
        switch node.kind {
        case "Switch": return .flag(node.on ?? false)
        case "Slider", "Stepper", "DatePicker": return .number(node.value ?? 0)
        case "TextInput": return .text(node.field ?? "")
        case "Picker": return .index(node.selectedIndex ?? 0)
        default: return nil
        }
    }

    private func current(_ node: AnNode) -> AnControlValue? {
        locals[node.id] ?? Self.declared(node)
    }

    // ------------------------------------------------------------- bindings

    /// Un número: `an-slider`, `an-stepper`.
    ///
    /// El evento sale con la misma clave que en iOS —`change` con `value`—
    /// porque el que lo recibe es el mismo `outputFromObservable` de la
    /// directiva, no un código de reloj.
    func number(_ node: AnNode) -> Binding<Double> {
        Binding(
            get: {
                if case .number(let value)? = self.current(node) { return value }
                return node.value ?? 0
            },
            set: { nuevo in
                self.locals[node.id] = .number(nuevo)
                self.dispatch(node.id, "change", ["value": nuevo])
            }
        )
    }

    /// Una fecha. Viaja en milisegundos desde 1970 en los dos sentidos, que es
    /// lo que da y toma `Date` en JS: formatearla depende del idioma y de la
    /// zona del dispositivo, y de eso ya se encarga el sistema al pintarla.
    func date(_ node: AnNode) -> Binding<Date> {
        let numero = number(node)
        return Binding(
            get: { Date(timeIntervalSince1970: numero.wrappedValue / 1000.0) },
            set: { numero.wrappedValue = $0.timeIntervalSince1970 * 1000.0 }
        )
    }

    func flag(_ node: AnNode) -> Binding<Bool> {
        Binding(
            get: {
                if case .flag(let value)? = self.current(node) { return value }
                return node.on ?? false
            },
            set: { nuevo in
                self.locals[node.id] = .flag(nuevo)
                self.dispatch(node.id, "change", ["value": nuevo])
            }
        )
    }

    func text(_ node: AnNode) -> Binding<String> {
        Binding(
            get: {
                if case .text(let value)? = self.current(node) { return value }
                return node.field ?? ""
            },
            set: { nuevo in
                self.locals[node.id] = .text(nuevo)
                self.dispatch(node.id, "change", ["value": nuevo])
            }
        )
    }

    /// La opción elegida de un `an-select`. El evento lleva `index`, no
    /// `value`: es lo que espera `NativeIndexEvent` en la directiva.
    func index(_ node: AnNode) -> Binding<Int> {
        Binding(
            get: {
                if case .index(let value)? = self.current(node) { return value }
                return node.selectedIndex ?? 0
            },
            set: { nuevo in
                self.locals[node.id] = .index(nuevo)
                self.dispatch(node.id, "change", ["index": nuevo])
            }
        )
    }

    // --------------------------------------------------- lo que se presenta

    /// Si un `an-alert` o un `an-modal` está a la vista.
    ///
    /// Escribir `false` es el sistema diciendo que lo ha cerrado —un botón, el
    /// gesto de bajar la hoja—, y de eso hay que avisar: si no, la señal que lo
    /// abrió se queda diciendo que sigue abierto y volver a ponerla a `true` no
    /// hace nada.
    func presented(_ node: AnNode) -> Binding<Bool> {
        Binding(
            get: { node.visible == true && !self.cerrados.contains(node.id) },
            set: { abierto in
                guard !abierto else { return }
                self.cerrados.insert(node.id)
                self.dispatch(node.id, "dismiss", [:])
            }
        )
    }

    // ----------------------------------------------------------- la corona

    /// Cuánto lleva girada la corona sobre este nodo.
    ///
    /// La corona de SwiftUI se pide con un `Binding` sobre un número que ella
    /// mueve; el evento que interesa —cuánto y a qué velocidad— llega aparte,
    /// por `onChange`. Este acumulador existe porque sin un sitio donde
    /// guardarlo el giro se reiniciaría en cada foto, y con él la corona
    /// arrastraría el valor de vuelta al que tenía.
    func crown(_ id: UInt32) -> Binding<Double> {
        Binding(
            get: {
                if case .number(let value)? = self.locals[id] { return value }
                return 0
            },
            set: { self.locals[id] = .number($0) }
        )
    }
}
