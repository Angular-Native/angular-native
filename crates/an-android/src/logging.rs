//! Salida de diagnóstico hacia logcat.
//!
//! Android tira a la basura la salida estándar de un proceso, así que sin esto
//! ni `console.log` ni un `eprintln!` del core se ven en ninguna parte: un
//! fallo se manifiesta como una pantalla vacía y nada más.

use std::io::{BufRead, BufReader};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::rc::Rc;

use an_bridge::runtime::LogSink;
use jni::objects::{GlobalRef, JValue};
use jni::JavaVM;

/// `console.*` desde JavaScript.
pub struct AndroidLog {
    vm: JavaVM,
    host: GlobalRef,
}

impl AndroidLog {
    pub fn new(vm: JavaVM, host: GlobalRef) -> Rc<Self> {
        Rc::new(AndroidLog { vm, host })
    }

    /// Otra referencia a la misma JavaVM. El puntero es estable durante toda la
    /// vida del proceso; envolverlo otra vez es la forma soportada de
    /// compartirla.
    pub fn java_vm(&self) -> JavaVM {
        unsafe { JavaVM::from_raw(self.vm.get_java_vm_pointer()) }
            .expect("la JavaVM sigue viva mientras el proceso lo esté")
    }

    pub fn host_ref(&self) -> GlobalRef {
        self.host.clone()
    }
}

impl LogSink for AndroidLog {
    fn log(&self, level: u8, message: &str) {
        let Ok(mut env) = self.vm.attach_current_thread() else {
            return;
        };
        let Ok(text) = env.new_string(message) else { return };
        let result = env.call_method(
            self.host.as_obj(),
            "log",
            "(ILjava/lang/String;)V",
            &[JValue::Int(level as i32), JValue::Object(&text)],
        );
        if result.is_err() {
            let _ = env.exception_clear();
        }
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
    // lector construye el suyo propio en vez de compartir el `Rc`.
    // La JavaVM es válida durante toda la vida del proceso; envolver el
    // puntero otra vez es la forma soportada de compartirla entre hilos.
    let vm = Some(sink.java_vm());
    let host = sink.host.clone();
    std::thread::spawn(move || {
        let Some(vm) = vm else { return };
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
