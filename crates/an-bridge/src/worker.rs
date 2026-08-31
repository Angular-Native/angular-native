//! El motor JS y el árbol, en su propio hilo.
//!
//! No es por paralelismo: es por la pila. QuickJS necesita unos 4 MB para que
//! el router de Angular complete una navegación —diecisiete operadores de RxJS
//! encadenados y una recursión de suscripción profunda— y el hilo principal de
//! iOS tiene 1 MB que no se pueden cambiar. Un hilo propio sí admite la pila
//! que se le pida.
//!
//! El hilo de UI sigue mandando: pide un frame y espera la respuesta. No hay
//! concurrencia real todavía, solo la pila. Convertirlo en asíncrono, con el
//! hilo de sombra trabajando por delante del de UI, es el paso siguiente y la
//! frontera ya está donde tiene que estar: entre las dos mitades solo viaja un
//! `Frame`.

use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;

use an_core::{Frame, TextMeasurer};
use an_host::{HostEvent, ShadowSide};

use crate::protocol::apply;
use crate::quickjs::QuickJsRuntime;
use crate::runtime::{JsError, JsRuntime};

/// Lo que el hilo de UI le pide al worker.
pub enum Request {
    Eval { name: String, code: String },
    /// Vistas fuera, motor nuevo, árbol vacío, y a evaluar de cero.
    Reload { name: String, code: String },
    SetViewport(f32, f32),
    Tick { now_ms: f64, events: Vec<HostEvent> },
    Stop,
}

/// Lo que devuelve. Un `Tick` trae el frame; el resto solo dicen si hubo error.
#[derive(Default)]
pub struct Reply {
    pub frame: Frame,
    pub error: Option<String>,
}

pub struct RuntimeWorker {
    requests: Sender<Request>,
    replies: Receiver<Reply>,
    handle: Option<JoinHandle<()>>,
}

impl RuntimeWorker {
    /// `init` corre ya dentro del hilo: el runtime de QuickJS no es `Send`, así
    /// que no se puede construir fuera y mover. Lo que sí cruza son las piezas
    /// con las que se construye.
    pub fn spawn<M, F>(stack_size: usize, init: F) -> Result<Self, JsError>
    where
        M: TextMeasurer + 'static,
        F: FnOnce() -> Result<(QuickJsRuntime, ShadowSide<M>), JsError> + Send + 'static,
    {
        let (request_tx, request_rx) = std::sync::mpsc::channel::<Request>();
        let (reply_tx, reply_rx) = std::sync::mpsc::channel::<Reply>();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();

        let handle = std::thread::Builder::new()
            .name("an-runtime".to_owned())
            .stack_size(stack_size)
            .spawn(move || {
                let (mut js, mut shadow) = match init() {
                    Ok(parts) => {
                        let _ = ready_tx.send(Ok(()));
                        parts
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                // Los `layout` que produce un frame se despachan al principio
                // del siguiente, sin volver a cruzar al hilo de UI: el worker
                // tiene las dos puntas.
                let mut pending_layout: Vec<HostEvent> = Vec::new();

                while let Ok(request) = request_rx.recv() {
                    let reply = match request {
                        Request::Stop => break,
                        Request::Eval { name, code } => Reply {
                            error: js.eval(&name, &code).err().map(|e| e.to_string()),
                            ..Reply::default()
                        },
                        Request::Reload { name, code } => {
                            shadow.reset();
                            pending_layout.clear();
                            match QuickJsRuntime::new() {
                                Ok(fresh) => {
                                    js = fresh;
                                    Reply {
                                        error: js.eval(&name, &code).err().map(|e| e.to_string()),
                                        ..Reply::default()
                                    }
                                }
                                Err(error) => {
                                    Reply { error: Some(error.to_string()), ..Reply::default() }
                                }
                            }
                        }
                        Request::SetViewport(width, height) => {
                            shadow.set_viewport((width, height));
                            Reply::default()
                        }
                        Request::Tick { now_ms, mut events } => {
                            events.append(&mut pending_layout);
                            let mut error = None;
                            if !events.is_empty() {
                                error = js.dispatch_events(&events).err().map(|e| e.to_string());
                            }
                            if error.is_none() {
                                match js.tick(now_ms) {
                                    Ok(commands) if commands.is_empty() => {}
                                    Ok(commands) => {
                                        if let Err(protocol) = apply(&commands, &mut shadow.tree) {
                                            error = Some(format!(
                                                "búfer de comandos inválido: {protocol:?}"
                                            ));
                                        }
                                    }
                                    Err(js_error) => error = Some(js_error.to_string()),
                                }
                            }
                            match shadow.commit() {
                                Ok((frame, layout_events)) => {
                                    pending_layout = layout_events;
                                    Reply { frame, error }
                                }
                                Err(commit) => Reply {
                                    error: Some(
                                        error.unwrap_or_else(|| format!("el commit falló: {commit:?}")),
                                    ),
                                    ..Reply::default()
                                },
                            }
                        }
                    };
                    if reply_tx.send(reply).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| JsError::Engine(format!("no se pudo crear el hilo del runtime: {e}")))?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(RuntimeWorker {
                requests: request_tx,
                replies: reply_rx,
                handle: Some(handle),
            }),
            Ok(Err(message)) => Err(JsError::Engine(message)),
            Err(_) => Err(JsError::Engine("el hilo del runtime murió al arrancar".to_owned())),
        }
    }

    /// Manda y espera. El hilo de UI se bloquea, igual que cuando todo esto
    /// corría en él.
    pub fn request(&self, request: Request) -> Reply {
        if self.requests.send(request).is_err() {
            return Reply {
                error: Some("el hilo del runtime no responde".to_owned()),
                ..Reply::default()
            };
        }
        self.replies.recv().unwrap_or_else(|_| Reply {
            error: Some("el hilo del runtime murió".to_owned()),
            ..Reply::default()
        })
    }
}

impl Drop for RuntimeWorker {
    fn drop(&mut self) {
        let _ = self.requests.send(Request::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
