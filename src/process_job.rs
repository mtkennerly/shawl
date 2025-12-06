use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

use std::os::windows::io::AsRawHandle;

pub struct ProcessJob {
    handle: HANDLE,
}

impl ProcessJob {
    /// Create a process job that kills all child processes when closed
    pub fn create_kill_on_close() -> Result<Self, windows::core::Error> {
        unsafe {
            let job = CreateJobObjectW(None, None)?;
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )?;

            Ok(Self { handle: job })
        }
    }

    /// Assign a child process to this job
    pub fn assign(&self, child: &std::process::Child) -> Result<(), windows::core::Error> {
        unsafe {
            let process_handle = HANDLE(child.as_raw_handle());
            AssignProcessToJobObject(self.handle, process_handle)?;
        }
        Ok(())
    }
}

impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            // Closing the job handle terminates all child processes
            let _ = CloseHandle(self.handle);
        }
    }
}
