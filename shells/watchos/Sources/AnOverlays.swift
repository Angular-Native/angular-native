import SwiftUI

/// Lo que el sistema presenta encima: `an-alert` y `an-modal`.
///
/// No son vistas que se coloquen. En SwiftUI son modificadores sobre la raíz
/// —`.alert`, `.sheet`, `.fullScreenCover`— y quien decide dónde van y cómo
/// entran es el sistema, no taffy. Por eso Rust los saca del árbol y los manda
/// aparte, en `overlays`, y por eso este modificador se cuelga de la vista raíz
/// y no de un nodo.
///
/// Es lo mismo que hace iOS presentando un `UIAlertController` de verdad en vez
/// de dibujar una capa parecida: el sistema sabe que hay algo modal delante,
/// así que VoiceOver deja de leer lo de detrás y no compite en orden de dibujo
/// con los diálogos del propio reloj.
struct AnOverlays: ViewModifier {
    let overlays: [AnNode]
    let controls: AnControls
    let dispatch: (UInt32, String, [String: Any]) -> Void
    let crownFocus: FocusState<UInt32?>.Binding

    func body(content: Content) -> some View {
        // Uno a uno y en el orden en que aparecen en el árbol: apilar varios
        // `.alert` sobre la misma vista es legal y el sistema los encola.
        overlays.reduce(AnyView(content)) { vista, overlay in
            switch overlay.kind {
            case "Alert":
                AnyView(
                    vista.modifier(
                        AnAlert(node: overlay, controls: controls, dispatch: dispatch)
                    )
                )
            case "Modal":
                AnyView(
                    vista.modifier(
                        AnModal(
                            node: overlay,
                            controls: controls,
                            dispatch: dispatch,
                            crownFocus: crownFocus
                        )
                    )
                )
            default: vista
            }
        }
    }
}

/// El diálogo del sistema.
///
/// El `Binding` de `isPresented` se escribe también cuando lo cierra el
/// usuario —tocando fuera, o el botón de atrás del reloj—: si no se avisara,
/// la señal que lo abrió seguiría diciendo `true` y volver a ponerla no haría
/// nada. Es el mismo motivo por el que `an-modal` tiene `(dismiss)`.
private struct AnAlert: ViewModifier {
    let node: AnNode
    let controls: AnControls
    let dispatch: (UInt32, String, [String: Any]) -> Void

    func body(content: Content) -> some View {
        content.alert(
            node.title ?? "",
            isPresented: controls.presented(node),
            actions: {
                // Sin botones sale un «OK», como en iOS: un diálogo del que no
                // se puede salir no es un diálogo.
                let rótulos = node.buttons?.isEmpty == false ? node.buttons! : ["OK"]
                ForEach(Array(rótulos.enumerated()), id: \.offset) { posicion, rótulo in
                    Button(rótulo) { dispatch(node.id, "select", ["index": posicion]) }
                }
            },
            message: {
                if let mensaje = node.message, !mensaje.isEmpty {
                    Text(mensaje)
                }
            }
        )
    }
}

/// Contenido presentado encima de todo.
///
/// En el reloj no hay diferencia visible entre `sheet` y `fullScreen` —una hoja
/// ocupa la pantalla igual—, pero sí en cómo se cierra: la hoja se puede bajar
/// con el dedo y la de pantalla completa no. Se respeta lo que diga
/// `[presentation]` en vez de elegir una por las dos.
private struct AnModal: ViewModifier {
    let node: AnNode
    let controls: AnControls
    let dispatch: (UInt32, String, [String: Any]) -> Void
    let crownFocus: FocusState<UInt32?>.Binding

    func body(content: Content) -> some View {
        if node.presentation == "fullScreen" {
            content.fullScreenCover(isPresented: presentado) { dentro }
        } else {
            content.sheet(isPresented: presentado) { dentro }
        }
    }

    private var presentado: Binding<Bool> {
        controls.presented(node)
    }

    /// Dentro sigue mandando taffy: los hijos del `an-modal` llevan su marco
    /// relativo a él, así que se colocan igual que en cualquier contenedor.
    private var dentro: some View {
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
        .frame(width: node.width, height: node.height, alignment: .topLeading)
        .ignoresSafeArea()
    }
}

extension View {
    func anOverlays(
        _ overlays: [AnNode],
        controls: AnControls,
        dispatch: @escaping (UInt32, String, [String: Any]) -> Void,
        crownFocus: FocusState<UInt32?>.Binding
    ) -> some View {
        modifier(
            AnOverlays(
                overlays: overlays,
                controls: controls,
                dispatch: dispatch,
                crownFocus: crownFocus
            )
        )
    }
}
