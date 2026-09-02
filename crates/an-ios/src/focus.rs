//! El motor de foco de tvOS.
//!
//! En una tele no hay toques. El mando mueve un cursor invisible entre las
//! vistas que se declaran enfocables y el botón central pulsa la que esté
//! enfocada en ese momento; ese recorrido lo decide UIKit por geometría, con
//! los marcos de las vistas. Nuestros marcos son absolutos y los calcula taffy,
//! así que el motor de foco recibe exactamente la retícula que describe la
//! plantilla y no hay nada que traducir.
//!
//! Lo que sí hay que hacer es declararse. `-[UIView canBecomeFocused]` devuelve
//! `NO` de fábrica, y una vista que devuelve `NO` **no se puede pulsar en una
//! tele**: el reconocedor de toque se engancha, no falla nada, y el botón
//! sencillamente no responde nunca. `UIButton`, `UITextField`, `UISegmentedControl`
//! y `UISearchBar` traen su `YES` de serie porque son controles; una `UIView`
//! con `(press)` no, y esa es la diferencia entre iOS y tvOS que más código
//! rompe.
//!
//! `canBecomeFocused` solo se puede cambiar heredando, así que aquí hay una
//! subclase de `UIView`. El host la usa para `an-view`, que es la primitiva a
//! la que la gente le cuelga `(press)`; `an-text` y `an-image` son `UILabel` y
//! `UIImageView` y siguen sin poder enfocarse, así que en una tele hay que
//! envolverlos. Se dice en `docs/tvos.md` y `events::attach` lo avisa en el
//! log en cuanto alguien lo intenta.
//!
//! No se dibuja ningún realce. tvOS no tiene `UIFocusEffect` —está marcado
//! `API_UNAVAILABLE(tvos)`, es de iOS—, y el sistema no pinta nada por su
//! cuenta sobre una vista normal: en tvOS el resalte es cosa de cada control,
//! que se levanta y proyecta sombra porque lo dibuja UIKit. Inventar aquí un
//! borde o una escala sería justo la imitación a mano que este proyecto no
//! hace. En su lugar la vista emite `focus` y `blur` hacia JavaScript, y la
//! plantilla decide con las props que ya tiene: `[backgroundColor]`, `[scale]`
//! y `[animate]`.

use std::cell::Cell;

use an_core::NodeId;
use an_host::{push_event, EventQueue, HostEvent};
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, Bool};
use objc2::{define_class, msg_send, ClassType, DefinedClass};
use objc2_foundation::{NSArray, NSNumber, NSObjectProtocol};
use objc2_ui_kit::{
    UIFocusAnimationCoordinator, UIFocusUpdateContext, UIGestureRecognizer, UIPressType, UIView,
};

pub struct FocusIvars {
    node: NodeId,
    queue: EventQueue,
    /// Lo enciende `events::attach` cuando la plantilla pide `(focus)` o
    /// `(blur)` sin pedir ninguna pulsación. Es `Cell` porque llega después de
    /// construir la vista: al crear el nodo todavía no se sabe qué eventos
    /// trae.
    quiere_foco: Cell<bool>,
}

define_class!(
    // SAFETY:
    // - UIView admite subclases y esta no toca su inicialización.
    // - AnFocusableView no implementa Drop.
    #[unsafe(super(UIView))]
    #[name = "AnFocusableView"]
    #[ivars = FocusIvars]
    pub struct FocusableView;

    impl FocusableView {
        /// Lo que el motor de foco pregunta antes de considerar esta vista.
        ///
        /// La respuesta se calcula en el momento en vez de guardarse en un
        /// contador: la vista es enfocable si tiene algún reconocedor de
        /// gestos encima, y de esa lista ya lleva la cuenta UIKit. Así, cuando
        /// el core quita el último oyente y `detach` retira el reconocedor, la
        /// vista deja de ser enfocable sola. Un contador propio habría que
        /// cuadrarlo con cada alta y cada baja, y el día que se descuadre lo
        /// que queda es una caja que roba el foco y no hace nada.
        #[unsafe(method(canBecomeFocused))]
        fn can_become_focused(&self) -> Bool {
            if self.ivars().quiere_foco.get() {
                return Bool::YES;
            }
            Bool::new(self.gestureRecognizers().is_some_and(|gestos| !gestos.is_empty()))
        }

        /// El foco entró o salió. UIKit manda esto a las dos vistas
        /// implicadas, así que hay que mirar cuál es cuál en el contexto.
        #[unsafe(method(didUpdateFocusInContext:withAnimationCoordinator:))]
        fn did_update_focus(
            &self,
            context: &UIFocusUpdateContext,
            coordinator: &UIFocusAnimationCoordinator,
        ) {
            // Al super lo primero: UIKit se apoya en su propia implementación
            // para mantener el estado del entorno de foco, y saltársela deja
            // el sistema creyendo cosas que no son.
            unsafe {
                let _: () = msg_send![
                    super(self),
                    didUpdateFocusInContext: context,
                    withAnimationCoordinator: coordinator,
                ];
            }

            let ivars = self.ivars();
            let yo: *const UIView = self.as_ref();

            let entra = unsafe { context.nextFocusedView() }
                .is_some_and(|view| Retained::as_ptr(&view) == yo);
            let sale = unsafe { context.previouslyFocusedView() }
                .is_some_and(|view| Retained::as_ptr(&view) == yo);

            // Nada que decir si esta vista no es ninguna de las dos: el aviso
            // llega también a los contenedores del camino.
            if entra {
                emit(&ivars.queue, ivars.node, "focus");
            }
            if sale {
                emit(&ivars.queue, ivars.node, "blur");
            }
        }
    }
);

fn emit(queue: &EventQueue, target: NodeId, name: &str) {
    push_event(queue, HostEvent { target, name: name.to_owned(), payload: Vec::new() });
}

impl FocusableView {
    pub fn new(mtm: objc2::MainThreadMarker, node: NodeId, queue: EventQueue) -> Retained<Self> {
        let this = mtm
            .alloc::<Self>()
            .set_ivars(FocusIvars { node, queue, quiere_foco: Cell::new(false) });
        unsafe { msg_send![super(this), init] }
    }

    /// Para `(focus)` y `(blur)` sin pulsación: una vista que solo quiere
    /// saber cuándo la miran no lleva reconocedor ninguno, así que el cálculo
    /// de `canBecomeFocused` no la vería.
    ///
    /// `setNeedsFocusUpdate` no se llama aquí: el motor vuelve a mirar el
    /// entorno cuando cambia la jerarquía, y forzarlo desde el montaje de un
    /// nodo movería el foco del usuario a mitad de pantalla.
    pub fn set_wants_focus(&self, quiere: bool) {
        self.ivars().quiere_foco.set(quiere);
        if quiere {
            self.setUserInteractionEnabled(true);
        }
    }

    /// Una vista enfocable tiene que recibir eventos. Una `UIView` con la
    /// interacción apagada no entra en el recorrido del mando.
    pub fn allow_interaction(&self) {
        self.setUserInteractionEnabled(true);
    }
}

/// La misma vista, si es una de las nuestras.
///
/// Se comprueba la clase antes de convertir, que es lo que separa esto de un
/// `as any`: si el nodo no se creó como enfocable, aquí sale `None` y quien
/// llama lo dice en voz alta en vez de escribir sobre una vista que no es.
pub fn focusable(view: &UIView) -> Option<&FocusableView> {
    let class: &AnyClass = FocusableView::class();
    if !view.isKindOfClass(class) {
        return None;
    }
    // SAFETY: la clase se acaba de comprobar.
    Some(unsafe { &*(view as *const UIView).cast::<FocusableView>() })
}

/// El mando no manda toques: manda pulsaciones de botón.
///
/// `allowedPressTypes` decide a cuál responde un reconocedor, y su valor por
/// defecto lo documenta el SDK como «platform dependent». Se fija a mano para
/// no depender de eso. Los de dirección no se piden nunca: son del motor de
/// foco, y quitárselos deja el mando sin poder moverse por la pantalla.
pub fn allow_press(recognizer: &UIGestureRecognizer, press: UIPressType) {
    let tipos = NSArray::from_retained_slice(&[NSNumber::new_isize(press.0)]);
    recognizer.setAllowedPressTypes(&tipos);
}

/// El botón central: el que pulsa lo que esté enfocado.
pub const SELECT: UIPressType = UIPressType::Select;

/// El botón de menú: el «atrás» del mando. Es lo que en el teléfono es el
/// arrastre desde el borde izquierdo, que en tvOS no existe porque no hay
/// borde que arrastrar.
pub const MENU: UIPressType = UIPressType::Menu;
