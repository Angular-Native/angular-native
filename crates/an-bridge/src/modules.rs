//! Módulos nativos: cómo llama JavaScript a código Rust que no es el renderer.
//!
//! Todo lo que no sea pintar —leer el dispositivo, guardar un fichero, pedir
//! permisos, hablar por red— entra por aquí. El contrato es el mismo que el
//! resto del puente: JS no bloquea nunca, y las respuestas llegan en un frame,
//! el mismo o uno posterior.

use std::sync::{Arc, Mutex};

use serde_json::Value;

pub type ModuleResult = Result<Value, String>;

/// Un identificador de llamada. Lo asigna el puente y viaja de ida y vuelta
/// para casar la respuesta con su promesa en JS.
pub type CallId = u64;

/// Buzón de respuestas. Es `Arc<Mutex<..>>` y no `Rc<RefCell<..>>` a
/// propósito: un módulo puede resolver desde otro hilo —una descarga, una
/// consulta a disco— y esa es justamente la razón de que exista.
type Outbox = Arc<Mutex<Vec<(CallId, ModuleResult)>>>;

/// Lo que un módulo usa para contestar. Se puede resolver en el acto o
/// guardarse y resolver más tarde; si se tira sin contestar, la promesa del
/// lado JS se rechaza en vez de quedarse colgada para siempre.
pub struct Responder {
    id: CallId,
    outbox: Outbox,
    answered: bool,
}

impl Responder {
    pub fn resolve(mut self, value: Value) {
        self.answered = true;
        self.outbox.lock().expect("buzón envenenado").push((self.id, Ok(value)));
    }

    pub fn reject(mut self, message: impl Into<String>) {
        self.answered = true;
        self.outbox
            .lock()
            .expect("buzón envenenado")
            .push((self.id, Err(message.into())));
    }
}

impl Drop for Responder {
    fn drop(&mut self) {
        if self.answered {
            return;
        }
        // Un módulo que se olvida de contestar es un bug, pero una promesa
        // colgada para siempre es peor: al menos se ve.
        self.outbox.lock().expect("buzón envenenado").push((
            self.id,
            Err("el módulo nativo no contestó".to_owned()),
        ));
    }
}

pub trait NativeModule {
    /// El nombre por el que JS lo invoca.
    fn name(&self) -> &'static str;

    /// `respond` se puede consumir aquí mismo o guardarse para más tarde.
    fn call(&mut self, method: &str, args: Value, respond: Responder);
}

#[derive(Default)]
pub struct ModuleRegistry {
    modules: Vec<Box<dyn NativeModule>>,
    outbox: Outbox,
    next_id: CallId,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, module: Box<dyn NativeModule>) {
        self.modules.push(module);
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.modules.iter().map(|m| m.name()).collect()
    }

    /// Arranca una llamada y devuelve su identificador. No bloquea.
    pub fn invoke(&mut self, module: &str, method: &str, args: Value) -> CallId {
        self.next_id += 1;
        let id = self.next_id;
        let respond = Responder { id, outbox: self.outbox.clone(), answered: false };

        match self.modules.iter_mut().find(|m| m.name() == module) {
            Some(target) => target.call(method, args, respond),
            None => respond.reject(format!("no hay ningún módulo nativo llamado {module:?}")),
        }
        id
    }

    /// Respuestas listas desde la última vez. Las recoge el puente al cerrar
    /// el frame.
    pub fn drain(&mut self) -> Vec<(CallId, ModuleResult)> {
        std::mem::take(&mut *self.outbox.lock().expect("buzón envenenado"))
    }
}

/// Declara un módulo nativo sin escribir el despacho a mano.
///
/// Cada método recibe los argumentos ya deserializados y devuelve
/// `Result<T, String>` con `T: Serialize`. Un método que no exista se rechaza
/// con un mensaje que dice cuál se pidió.
///
/// ```ignore
/// native_module! {
///     Device as "device" {
///         fn info(&mut self, _: ()) -> Result<DeviceInfo, String> { ... }
///         fn vibrate(&mut self, args: VibrateArgs) -> Result<(), String> { ... }
///     }
/// }
/// ```
#[macro_export]
macro_rules! native_module {
    (
        $ty:ty as $name:literal {
            $(
                fn $method:ident (&mut $self_:ident, $arg:ident : $arg_ty:ty) -> Result<$ret:ty, String> $body:block
            )*
        }
    ) => {
        impl $ty {
            $(
                fn $method(&mut $self_, $arg: $arg_ty) -> Result<$ret, String> $body
            )*
        }

        impl $crate::modules::NativeModule for $ty {
            fn name(&self) -> &'static str {
                $name
            }

            fn call(
                &mut self,
                method: &str,
                args: ::serde_json::Value,
                respond: $crate::modules::Responder,
            ) {
                match method {
                    $(
                        stringify!($method) => {
                            let parsed: $arg_ty = match ::serde_json::from_value(args) {
                                Ok(parsed) => parsed,
                                Err(error) => {
                                    respond.reject(format!(
                                        concat!($name, ".", stringify!($method), ": argumentos inválidos: {}"),
                                        error
                                    ));
                                    return;
                                }
                            };
                            match self.$method(parsed) {
                                Ok(value) => match ::serde_json::to_value(value) {
                                    Ok(value) => respond.resolve(value),
                                    Err(error) => respond.reject(error.to_string()),
                                },
                                Err(message) => respond.reject(message),
                            }
                        }
                    )*
                    other => respond.reject(format!(
                        concat!("el módulo ", $name, " no tiene ningún método {:?}")
                        , other
                    )),
                }
            }
        }
    };
}
