use super::INTERRUPTED;
use std::os::unix::process::CommandExt;
use std::{
    io,
    process::{Child, Command},
    sync::atomic::Ordering,
};
extern "C" fn interrupted(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::Relaxed);
}

/// Installed by the CLI, once, before launching a provider. The handler only stores
/// an atomic flag; allocation, I/O, and child cleanup happen in the supervisor.
pub fn install_signal_handler() -> io::Result<()> {
    // SAFETY: initialized sigaction with a valid handler; libc copies the structure.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = interrupted as *const () as libc::sighandler_t;
        libc::sigemptyset(&mut action.sa_mask);
        for signal in [libc::SIGINT, libc::SIGTERM] {
            if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                return Err(io::Error::last_os_error());
            }
        }
    }
    Ok(())
}
pub struct Tree {
    pub child: Child,
    terminated: bool,
}
impl Tree {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        Ok(Self {
            child: command.process_group(0).spawn()?,
            terminated: false,
        })
    }
    pub fn terminate(&mut self) {
        if !self.terminated {
            // SAFETY: this child was spawned into its own group, identified by its PID.
            // Kill the group even after the leader exits, closing inherited pipe handles.
            unsafe {
                libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL);
            }
            self.terminated = true;
        }
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        self.terminate();
        let _ = self.child.wait();
    }
}
