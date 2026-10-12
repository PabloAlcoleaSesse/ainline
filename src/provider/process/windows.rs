use super::INTERRUPTED;
use std::os::windows::{io::AsRawHandle, process::CommandExt};
use std::{
    io,
    process::{Child, Command},
    sync::atomic::Ordering,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT, SetConsoleCtrlHandler},
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
        },
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject,
        },
        Threading::{CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
    },
};
unsafe extern "system" fn interrupted(event: u32) -> i32 {
    if event == CTRL_C_EVENT || event == CTRL_BREAK_EVENT {
        INTERRUPTED.store(true, Ordering::Relaxed);
        1
    } else {
        0
    }
}
pub fn install_signal_handler() -> io::Result<()> {
    // SAFETY: callback has static lifetime and only performs an atomic store.
    if unsafe { SetConsoleCtrlHandler(Some(interrupted), 1) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub struct Tree {
    pub child: Child,
    job: Handle,
}
impl Tree {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        // SAFETY: all pointers reference initialized structures for the duration of each call.
        unsafe {
            let job = Handle(CreateJobObjectW(std::ptr::null(), std::ptr::null()));
            if job.0.is_null() {
                return Err(io::Error::last_os_error());
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            // Suspend before job assignment, preventing a race with child/grandchild creation.
            let child = command.creation_flags(CREATE_SUSPENDED).spawn()?;
            let mut tree = Self { child, job };
            if AssignProcessToJobObject(tree.job.0, tree.child.as_raw_handle()) == 0 {
                let error = io::Error::last_os_error();
                let _ = tree.child.kill();
                return Err(error);
            }
            let snapshot = Handle(CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0));
            if snapshot.0 == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            let mut entry: THREADENTRY32 = std::mem::zeroed();
            entry.dwSize = std::mem::size_of_val(&entry) as u32;
            let mut available = Thread32First(snapshot.0, &mut entry);
            while available != 0 {
                if entry.th32OwnerProcessID == tree.child.id() {
                    let thread = Handle(OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID));
                    if thread.0.is_null() || ResumeThread(thread.0) == u32::MAX {
                        return Err(io::Error::last_os_error());
                    }
                    return Ok(tree);
                }
                available = Thread32Next(snapshot.0, &mut entry);
            }
            Err(io::Error::other("could not find suspended provider thread"))
        }
    }
    pub fn terminate(&mut self) {
        unsafe {
            TerminateJobObject(self.job.0, 1);
        }
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        self.terminate();
        let _ = self.child.wait();
    }
}
