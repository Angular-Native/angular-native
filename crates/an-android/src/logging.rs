//! Diagnostic output on its way to logcat.
//!
//! Android throws away a process's standard output, so without this neither a
//! `console.log` nor an `eprintln!` from the core shows up anywhere: a failure
//! manifests as a blank screen and nothing else.

use std::io::{BufRead, BufReader};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::rc::Rc;

use an_bridge::runtime::LogSink;
use jni::objects::{Global, JObject, JValue};
use jni::JavaVM;

/// `console.*` coming from JavaScript.
pub struct AndroidLog {
    vm: JavaVM,
    /// Shared rather than copied: since jni 0.22 a global reference cannot be
    /// duplicated without the environment, while using it from another thread
    /// is perfectly legal.
    host: std::sync::Arc<Global<JObject<'static>>>,
}

impl AndroidLog {
    pub fn new(vm: JavaVM, host: Global<JObject<'static>>) -> Rc<Self> {
        Rc::new(AndroidLog { vm, host: std::sync::Arc::new(host) })
    }

    /// Another reference to the same JavaVM. Since jni 0.22 `JavaVM` is
    /// `Clone`, which is what used to be done by hand by wrapping the pointer.
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

/// Redirects the process's standard error to logcat.
///
/// This is what makes an `eprintln!` from the core, or the message of a
/// `panic!`, appear anywhere at all. A pipe is set up, descriptor 2 is pointed
/// at it, and a thread reads lines and forwards them.
pub fn redirect_stderr(sink: Rc<AndroidLog>) {
    let mut fds = [0; 2];
    // SAFETY: `pipe` writes two descriptors into the array it is handed.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return;
    }
    let (read_fd, write_fd) = (fds[0], fds[1]);
    // SAFETY: write_fd was just created and is valid.
    if unsafe { libc::dup2(write_fd, libc::STDERR_FILENO) } < 0 {
        return;
    }
    // SAFETY: read_fd is valid and from here on `OwnedFd` owns it.
    let read = unsafe { OwnedFd::from_raw_fd(read_fd) };

    // The sink uses JNI, which demands attaching to the thread: that is why
    // the reader thread builds its own instead of sharing the `Rc`, which does
    // not cross threads. The reference to the host does, inside an `Arc`.
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

/// The write descriptor is left open on purpose: closing it would close the
/// pipe, and with it the redirection.
const _: fn() = || {
    let _ = std::io::stderr().as_raw_fd();
};
