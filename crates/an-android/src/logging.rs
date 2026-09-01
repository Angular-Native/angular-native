//! Salida de diagnóstico hacia logcat.
//!
//! Android tira a la basura la salida estándar de un proceso, así que sin esto
//! ni `console.log` ni un `eprintln!` del core se ven en ninguna parte: un
//! fallo se manifiesta como una pantalla vacía y nada más.

use std::io::{BufRead, BufReader};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::rc::Rc;

use an_bridge::runtime::LogSink;
use jni::objects::{Global, JObject, JValue};
use jni::JavaVM;

/// `console.*` desde JavaScript.
pub struct AndroidLog {
    vm: JavaVM,
    /// Compartida y no copiada: desde jni 0.22 una referencia global no se
    /// puede duplicar sin el entorno, y usarla desde otro hilo sí es legal.
    host: std::sync::Arc<Global<JObject<'static>>>,
}

impl AndroidLog {
    pub fn new(vm: JavaVM, host: Global<JObject<'static>>) -> Rc<Self> {
        Rc::new(AndroidLog { vm, host: std::sync::Arc::new(host) })
    }

    /// Otra referencia a la misma JavaVM. Desde jni 0.22 `JavaVM` es `Clone`,
    /// que es lo que antes había que hacer a mano envolviendo el puntero.
    pub fn java_vm(&self) -> JavaVM {
        self.vm.clone()
    }

}

impl LogSink for AndroidLog {
    fn log(&self, level: u8, message: &str) {
        let _ = self.vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
            let Ok(text) = env.new_string(message) else { return Ok(()) };
            crate::host::call_java(
                env,
                self.host.as_obj(),
                "log",
                "(ILjava/lang/String;)V",
                &[JValue::Int(level as i32), JValue::Object(&text)],
            );
            Ok(())
        });
    }
}

/// Redirige la salida estándar del proceso a logcat.
///
/// Es lo que hace que un `eprintln!` del core, o el mensaje de un `panic!`,
/// aparezcan en algún sitio. Se monta una tubería, se apunta ahí el descriptor
/// 2, y un hilo lee líneas y las reenvía.
pub fn redirect_stderr(sink: Rc<AndroidLog>) {
    let mut fds = [0; 2];
    // SAFETY: `pipe` escribe dos descriptores en el array que se le pasa.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return;
    }
    let (read_fd, write_fd) = (fds[0], fds[1]);
    // SAFETY: write_fd acaba de crearse y es válido.
    if unsafe { libc::dup2(write_fd, libc::STDERR_FILENO) } < 0 {
        return;
    }
    // SAFETY: read_fd es válido y a partir de aquí lo posee `OwnedFd`.
    let read = unsafe { OwnedFd::from_raw_fd(read_fd) };

    // El sumidero usa JNI, que exige engancharse al hilo: por eso el hilo
    // lector construye el suyo propio en vez de compartir el `Rc`, que no
    // cruza hilos. La referencia al host sí, dentro de un `Arc`.
    let vm = sink.java_vm();
    let host = sink.host.clone();
    std::thread::spawn(move || {
        let sink = AndroidLog { vm, host };
        let reader = BufReader::new(std::fs::File::from(read));
        for line in reader.lines().map_while(Result::ok) {
            sink.log(3, &line);
        }
    });
    let _ = read_fd;
    let _ = write_fd;
}

/// El descriptor de escritura se queda abierto a propósito: cerrarlo cerraría
/// la tubería y con ella el redireccionamiento.
const _: fn() = || {
    let _ = std::io::stderr().as_raw_fd();
};
