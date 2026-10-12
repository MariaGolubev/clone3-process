#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Exited(u8),
    Signaled(i32),
}

impl Status {
    pub fn from_raw(status: libc::c_int) -> Self {
        if libc::WIFEXITED(status) {
            let code = libc::WEXITSTATUS(status);
            Status::Exited(code as u8)
        } else if libc::WIFSIGNALED(status) {
            let signal = libc::WTERMSIG(status);
            Status::Signaled(signal)
        } else {
            Status::Running
        }
    }

    pub fn success(&self) -> bool {
        matches!(self, Status::Exited(0))
    }

    pub fn code(&self) -> Option<i32> {
        match self {
            Status::Exited(code) => Some(*code as i32),
            _ => None,
        }
    }

    pub fn signal(&self) -> Option<i32> {
        match self {
            Status::Signaled(signal) => Some(*signal),
            _ => None,
        }
    }

    pub fn shell_code(&self) -> Option<i32> {
        match self {
            Status::Exited(code) => Some(*code as i32),
            Status::Signaled(signal) => Some(128 + *signal),
            _ => None,
        }
    }
}
