//! Plugins: módulos nativos escritos fuera del repo.
//!
//! Un módulo de los de [`crate::modules`] se implementa en Rust y se compila
//! dentro del core. Un *plugin* no: lo escribe alguien de fuera, en Swift o en
//! Java, y el core solo tiene que llevarle la llamada y traerse la respuesta.
//!
//! Por eso aquí no hay una implementación por plugin sino una sola,
//! [`HostPlugin`], que hace de cartero. Lo que cambia entre un plugin y otro
//! está al otro lado de la frontera, no aquí.
//!
//! El reparto de hilos es la razón de que esto sea una cola y no una llamada
//! directa. `NativeModule::call` corre en el hilo del motor JS; `UIPasteboard`
//! y `ClipboardManager` quieren el hilo de UI. Así que la llamada se encola,
//! el hilo de UI la recoge en su frame —que ya pasa por ahí una vez por
//! vsync— y contesta cuando puede: en el acto, o tres segundos después si lo
//! que hay detrás es una cámara. El motor no espera a nadie.
//!
//! ```text
//!   hilo del motor                         hilo de UI
//!   ──────────────────                     ─────────────────────
//!   HostPlugin::call
//!        │ encola con su Responder
//!        ▼
//!   PluginBridge ──────── take_calls() ──▶ AnPluginRegistry (Swift/Java)
//!        ▲                                        │
//!        └────────── resolve(id, json) ◀──────────┘
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::modules::{NativeModule, Responder};

/// Una llamada que espera a que la plataforma la atienda.
pub struct PluginCall {
    /// Identificador con el que la plataforma contestará.
    pub id: u64,
    /// El nombre del módulo, tal como lo escribió JS.
    pub module: String,
    pub method: String,
    /// Los argumentos ya serializados: al otro lado hay Swift o Java, no serde.
    pub args: String,
}

/// El buzón compartido entre el hilo del motor y el de UI.
///
/// Se crea una vez por proceso y vive detrás de un `Arc`: los [`HostPlugin`]
/// se quedan con una copia y el shell de la plataforma con otra.
#[derive(Default)]
pub struct PluginBridge {
    next: AtomicU64,
    /// Llamadas que el hilo de UI todavía no ha recogido.
    pending: Mutex<Vec<PluginCall>>,
    /// Llamadas recogidas y aún sin contestar. El `Responder` guardado aquí es
    /// lo que mantiene viva la promesa del lado JS; si el buzón se destruye,
    /// su `Drop` las rechaza en vez de dejarlas colgadas para siempre.
    waiting: Mutex<HashMap<u64, Responder>>,
}

impl PluginBridge {
    pub fn new() -> Arc<Self> {
        Arc::new(PluginBridge::default())
    }

    /// Recoge lo que haya llegado desde la última vez. La llama el hilo de UI
    /// una vez por frame.
    pub fn take_calls(&self) -> Vec<PluginCall> {
        std::mem::take(&mut *self.pending.lock().expect("buzón de plugins envenenado"))
    }

    /// Contesta a una llamada. `json` es el valor de vuelta ya serializado;
    /// `"null"` para un método que no devuelve nada.
    ///
    /// El `Err` no es un fallo del plugin sino del shell: o contestó dos veces
    /// a la misma llamada, o contestó a una que no existe. Se devuelve para
    /// que la plataforma lo pueda registrar; tragárselo dejaría un plugin roto
    /// pareciendo uno lento.
    pub fn resolve(&self, id: u64, json: &str) -> Result<(), String> {
        let responder = self.take_waiting(id)?;
        match serde_json::from_str::<Value>(json) {
            Ok(value) => responder.resolve(value),
            // El plugin sí contestó; lo que no se entiende es su respuesta. La
            // promesa se rechaza con el motivo exacto en vez de resolverse con
            // un `undefined` que nadie sabría de dónde salió.
            Err(error) => responder
                .reject(format!("el plugin contestó con algo que no es JSON válido: {error}")),
        }
        Ok(())
    }

    pub fn reject(&self, id: u64, message: &str) -> Result<(), String> {
        self.take_waiting(id)?.reject(message.to_owned());
        Ok(())
    }

    /// Cuántas llamadas siguen en vuelo. Para diagnóstico y para los tests.
    pub fn in_flight(&self) -> usize {
        self.waiting.lock().expect("buzón de plugins envenenado").len()
    }

    fn take_waiting(&self, id: u64) -> Result<Responder, String> {
        self.waiting.lock().expect("buzón de plugins envenenado").remove(&id).ok_or_else(|| {
            format!("no hay ninguna llamada a plugin con el id {id} esperando respuesta")
        })
    }

    fn enqueue(&self, module: &str, method: &str, args: Value, respond: Responder) {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.waiting.lock().expect("buzón de plugins envenenado").insert(id, respond);
        self.pending.lock().expect("buzón de plugins envenenado").push(PluginCall {
            id,
            module: module.to_owned(),
            method: method.to_owned(),
            args: args.to_string(),
        });
    }
}

/// El módulo nativo que representa a un plugin dentro del registro.
///
/// Uno por nombre declarado. No sabe qué métodos tiene el plugin ni le
/// corresponde saberlo: quien rechaza un método que no existe es la
/// implementación, que es la única que conoce su propia lista.
pub struct HostPlugin {
    name: &'static str,
    bridge: Arc<PluginBridge>,
}

impl HostPlugin {
    /// El nombre se filtra a propósito. `NativeModule::name` devuelve
    /// `&'static str` porque los módulos compilados dentro son literales, y
    /// aquí el nombre llega en tiempo de ejecución. Son unas decenas de bytes
    /// por plugin, una sola vez en la vida del proceso.
    pub fn new(name: &str, bridge: Arc<PluginBridge>) -> Self {
        HostPlugin { name: Box::leak(name.to_owned().into_boxed_str()), bridge }
    }
}

impl NativeModule for HostPlugin {
    fn name(&self) -> &'static str {
        self.name
    }

    fn call(&mut self, method: &str, args: Value, respond: Responder) {
        self.bridge.enqueue(self.name, method, args, respond);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::ModuleRegistry;

    fn registro() -> (ModuleRegistry, Arc<PluginBridge>) {
        let bridge = PluginBridge::new();
        let mut registry = ModuleRegistry::new();
        registry.register(Box::new(HostPlugin::new("clipboard", bridge.clone())));
        (registry, bridge)
    }

    #[test]
    fn la_llamada_viaja_y_la_respuesta_vuelve() {
        let (mut registry, bridge) = registro();
        registry.invoke("clipboard", "read", Value::Null);

        // Nada resuelto todavía: el motor no bloquea esperando a la plataforma.
        assert!(registry.drain().is_empty());

        let calls = bridge.take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].module, "clipboard");
        assert_eq!(calls[0].method, "read");
        assert_eq!(calls[0].args, "null");
        // Y ya no está pendiente: recoger es consumir.
        assert!(bridge.take_calls().is_empty());
        assert_eq!(bridge.in_flight(), 1);

        bridge.resolve(calls[0].id, "\"hola\"").expect("la llamada estaba esperando");
        assert_eq!(bridge.in_flight(), 0);
        let answers = registry.drain();
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].1, Ok(Value::String("hola".to_owned())));
    }

    #[test]
    fn los_argumentos_llegan_serializados() {
        let (mut registry, bridge) = registro();
        registry.invoke("clipboard", "write", serde_json::json!({ "text": "ñandú" }));
        let calls = bridge.take_calls();
        assert_eq!(calls[0].args, r#"{"text":"ñandú"}"#);
    }

    #[test]
    fn el_rechazo_llega_con_su_motivo() {
        let (mut registry, bridge) = registro();
        registry.invoke("clipboard", "read", Value::Null);
        let calls = bridge.take_calls();
        bridge.reject(calls[0].id, "el portapapeles está vacío").expect("estaba esperando");
        assert_eq!(registry.drain()[0].1, Err("el portapapeles está vacío".to_owned()));
    }

    #[test]
    fn contestar_dos_veces_se_nota() {
        let (mut registry, bridge) = registro();
        registry.invoke("clipboard", "read", Value::Null);
        let id = bridge.take_calls()[0].id;
        bridge.resolve(id, "null").expect("la primera sí");
        assert!(bridge.resolve(id, "null").is_err(), "la segunda tiene que dar la cara");
        // Y a JS le llegó una sola respuesta, no dos.
        assert_eq!(registry.drain().len(), 1);
    }

    #[test]
    fn una_respuesta_ilegible_rechaza_la_promesa() {
        let (mut registry, bridge) = registro();
        registry.invoke("clipboard", "read", Value::Null);
        let id = bridge.take_calls()[0].id;
        bridge.resolve(id, "{esto no es json}").expect("la llamada existía");
        let answers = registry.drain();
        assert!(matches!(&answers[0].1, Err(message) if message.contains("JSON")));
    }

    #[test]
    fn un_modulo_que_no_esta_registrado_no_se_traga_la_llamada() {
        let (mut registry, bridge) = registro();
        registry.invoke("biometria", "authenticate", Value::Null);
        // No llegó a la cola de la plataforma…
        assert!(bridge.take_calls().is_empty());
        // …y la promesa se rechaza en el acto, diciendo cuál faltaba.
        assert!(matches!(&registry.drain()[0].1, Err(message) if message.contains("biometria")));
    }
}
