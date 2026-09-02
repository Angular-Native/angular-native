import SwiftUI

/// La corona digital como fuente de eventos.
///
/// Es el gesto propio del reloj y el único que no tiene equivalente en el
/// teléfono: un mando analógico, con inercia y con háptico, que se usa sin
/// tapar la pantalla con el dedo. Hasta ahora solo movía el `ScrollView`, que
/// es lo que hace SwiftUI sola; con esto una plantilla puede pedirla:
///
/// ```html
/// <an-view (crown)="giro($event)"> … </an-view>
/// ```
///
/// El evento lleva lo que da `DigitalCrownEvent` más el acumulado:
///
/// | clave | qué es |
/// |---|---|
/// | `delta` | cuánto ha girado desde el aviso anterior |
/// | `offset` | el acumulado desde que la vista tomó la corona |
/// | `velocity` | vueltas por segundo, con signo |
///
/// `delta` no lo da SwiftUI: se calcula aquí restando, porque lo que casi
/// siempre se quiere es «súbeme el valor lo que ha girado», y hacer esa resta
/// en cada plantilla sería repetirla en cada plantilla.
///
/// **Solo una vista puede tener la corona a la vez.** No es una limitación del
/// shell: en watchOS la corona va a lo que tenga el foco, y el foco es uno.
/// Cuando hay varias vistas con `(crown)`, se la queda la primera en orden de
/// pintado; tocar un `Slider`, un `Stepper` o un `Picker` le pasa el foco a
/// ese control, que es lo que espera cualquiera que use un reloj, y al volver
/// hay que tocar la vista para recuperarla.
struct AnCrown: ViewModifier {
    let node: AnNode
    let controls: AnControls
    let dispatch: (UInt32, String, [String: Any]) -> Void
    let focus: FocusState<UInt32?>.Binding

    /// Lo que se había girado la vez anterior, para poder mandar el paso.
    @State private var anterior: Double = 0

    func body(content: Content) -> some View {
        if node.listens(to: "crown") {
            content
                // Sin `focusable` la corona no llega: watchOS la manda a lo que
                // tenga el foco, y una vista normal no lo puede tomar.
                .focusable(true)
                .focused(focus, equals: node.id)
                // El foco de salida. En el reloj la corona va con el foco, y
                // una vista que la pide sin tenerlo no recibe absolutamente
                // nada; `defaultFocus` es la forma de decir «si nadie lo tiene,
                // que sea esta» sin quitárselo a quien lo tenga. Asignar el
                // `FocusState` a mano no vale: SwiftUI se lo traga sin avisar si
                // la vista todavía no está en pantalla.
                .defaultFocus(focus, node.id)
                .digitalCrownRotation(
                    controls.crown(node.id),
                    onChange: { evento in
                        let delta = evento.offset - anterior
                        anterior = evento.offset
                        dispatch(
                            node.id,
                            "crown",
                            ["delta": delta, "offset": evento.offset, "velocity": evento.velocity]
                        )
                    },
                    onIdle: {
                        // Se paró. Va como su propio evento y no como un `crown`
                        // con delta cero, porque «ha dejado de girar» es una
                        // cosa distinta de «ha girado nada».
                        dispatch(node.id, "crownIdle", [:])
                    }
                )
        } else {
            content
        }
    }
}

extension View {
    func anCrown(
        _ node: AnNode,
        controls: AnControls,
        dispatch: @escaping (UInt32, String, [String: Any]) -> Void,
        focus: FocusState<UInt32?>.Binding
    ) -> some View {
        modifier(AnCrown(node: node, controls: controls, dispatch: dispatch, focus: focus))
    }
}
