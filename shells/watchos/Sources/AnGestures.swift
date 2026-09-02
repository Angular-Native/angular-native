import SwiftUI

/// Los gestos que el reloj sí tiene, enganchados solo donde la plantilla los
/// pidió.
///
/// Se engancha por lista y no siempre por dos motivos que se ven en pantalla:
/// un reconocedor de más se come el arrastre del `ScrollView` que hay debajo, y
/// watchOS realza lo que cree tocable, así que envolver todo en un gesto haría
/// que parpadease media pantalla al rozarla.
///
/// Lo que no está aquí no está en el reloj, y se dice desde Rust —una vez por
/// gesto— en vez de engancharse a algo que no responde nunca:
///
/// - `(pinch)` y `(rotation)`: `MagnifyGesture` y `RotateGesture` están
///   marcados `@available(watchOS, unavailable)`. Una pantalla de 40 mm no
///   admite dos dedos.
/// - `(back)`: fuera de un `NavigationStack` el reloj no da el arrastre desde
///   el borde, y montar uno metería la navegación de SwiftUI —con su barra y
///   su layout— dentro de un árbol que coloca taffy.
/// - `(scroll)` y `(refresh)`: el `ScrollView` de SwiftUI no publica su
///   desplazamiento por ningún sitio que valga en watchOS 11, que es el mínimo
///   del shell.
struct AnGestures: ViewModifier {
    let node: AnNode
    let dispatch: (UInt32, String, [String: Any]) -> Void
    /// `true` cuando el propio control ya manda el `(press)` —un `Button`
    /// de SwiftUI lo hace, y con su realce y su háptico— y engancharle
    /// además un toque lo mandaría dos veces.
    let dueñoDelToque: Bool

    /// Dónde estaba el dedo la última vez.
    ///
    /// SwiftUI no da la posición ni en `onTapGesture` ni en
    /// `onLongPressGesture`, y el `(press)` de iOS la lleva. Se saca de un
    /// arrastre de distancia cero, que es un gesto de verdad y no una
    /// coordenada inventada: si no ha habido contacto, no se manda nada.
    @State private var punto: CGPoint?
    /// Si el arrastre en curso ya avisó de que empezaba.
    @State private var arrastrando = false

    /// Cuánto hay que recorrer para que cuente como deslizar. `UIKit` usa un
    /// umbral parecido en su reconocedor de swipe; aquí se escribe porque en
    /// SwiftUI no hay reconocedor de swipe y el umbral lo pone quien lo monta.
    private static let umbralSwipe: CGFloat = 24

    func body(content: Content) -> some View {
        var vista = AnyView(content)
        if quiereToque {
            vista = AnyView(vista.contentShape(Rectangle()))
        }
        if (quierePress || node.listens(to: "longPress")) && !quiereArrastre {
            // Distancia cero: no mueve nada, solo apunta dónde está el dedo.
            // Solo se pone cuando no hay arrastre de verdad: dos `DragGesture`
            // sobre la misma vista se disputan el dedo y gana uno de los dos,
            // así que cuando lo hay, la posición sale de él.
            vista = AnyView(
                vista.simultaneousGesture(
                    DragGesture(minimumDistance: 0).onChanged { punto = $0.location }
                )
            )
        }
        if quierePress {
            vista = AnyView(vista.onTapGesture { manda("press", posicion()) })
        }
        if node.listens(to: "doublePress") {
            vista = AnyView(vista.onTapGesture(count: 2) { manda("doublePress", posicion()) })
        }
        if node.listens(to: "longPress") {
            // `simultaneousGesture` y no `.onLongPressGesture`: este último se
            // instala como gesto normal y el arrastre de abajo se lo come, así
            // que sobre una vista que además escucha `(pan)` o `(swipe*)` no
            // llegaba nunca.
            vista = AnyView(
                vista.simultaneousGesture(
                    LongPressGesture(minimumDuration: 0.5).onEnded { _ in
                        manda("longPress", posicion())
                    }
                )
            )
        }
        if quiereArrastre {
            vista = AnyView(vista.gesture(arrastre))
        }
        return vista
    }

    private var quierePress: Bool {
        node.listens(to: "press") && !dueñoDelToque
    }

    private var quiereToque: Bool {
        quierePress || node.listens(to: "doublePress")
            || node.listens(to: "longPress") || quiereArrastre
    }

    private var quiereArrastre: Bool {
        node.listens(to: "pan") || quiereDeslizar
    }

    private var quiereDeslizar: Bool {
        node.listens(to: "swipeLeft") || node.listens(to: "swipeRight")
            || node.listens(to: "swipeUp") || node.listens(to: "swipeDown")
    }

    /// El arrastre, que alimenta a la vez `(pan)` y los cuatro `(swipe*)`.
    ///
    /// Un solo reconocedor para los dos porque son el mismo gesto mirado de dos
    /// maneras: `pan` cuenta el recorrido mientras pasa y `swipe` mira el saldo
    /// al soltar. Dos `DragGesture` encima de la misma vista se pelearían.
    private var arrastre: some Gesture {
        DragGesture(minimumDistance: 1)
            .onChanged { valor in
                // La posición se apunta siempre, la escuche quien la escuche:
                // es la que lleva una pulsación larga sobre esta misma vista.
                punto = valor.location
                guard node.listens(to: "pan") else { return }
                let fase = arrastrando ? "move" : "begin"
                arrastrando = true
                manda("pan", carga(valor, fase))
            }
            .onEnded { valor in
                if node.listens(to: "pan") {
                    manda("pan", carga(valor, "end"))
                }
                arrastrando = false
                deslizar(valor)
            }
    }

    private func deslizar(_ valor: DragGesture.Value) {
        guard quiereDeslizar else { return }
        let dx = valor.translation.width
        let dy = valor.translation.height
        // Manda el eje dominante: un arrastre en diagonal es un deslizamiento
        // en una dirección, no en dos.
        let nombre: String
        if abs(dx) >= abs(dy) {
            guard abs(dx) >= Self.umbralSwipe else { return }
            nombre = dx < 0 ? "swipeLeft" : "swipeRight"
        } else {
            guard abs(dy) >= Self.umbralSwipe else { return }
            nombre = dy < 0 ? "swipeUp" : "swipeDown"
        }
        guard node.listens(to: nombre) else { return }
        manda(nombre, ["x": valor.location.x, "y": valor.location.y])
    }

    private func carga(_ valor: DragGesture.Value, _ fase: String) -> [String: Any] {
        [
            "x": valor.location.x,
            "y": valor.location.y,
            "translationX": valor.translation.width,
            "translationY": valor.translation.height,
            "velocityX": valor.velocity.width,
            "velocityY": valor.velocity.height,
            "state": fase
        ]
    }

    /// La posición del último contacto, si la hubo.
    ///
    /// Sin contacto se manda vacío en vez de un cero: un cero es una esquina
    /// concreta de la vista y quien lo reciba se lo creería.
    private func posicion() -> [String: Any] {
        guard let punto else { return [:] }
        return ["x": punto.x, "y": punto.y]
    }

    private func manda(_ nombre: String, _ carga: [String: Any]) {
        dispatch(node.id, nombre, carga)
    }
}

extension View {
    func anGestures(
        _ node: AnNode,
        dispatch: @escaping (UInt32, String, [String: Any]) -> Void,
        dueñoDelToque: Bool = false
    ) -> some View {
        modifier(AnGestures(node: node, dispatch: dispatch, dueñoDelToque: dueñoDelToque))
    }
}
