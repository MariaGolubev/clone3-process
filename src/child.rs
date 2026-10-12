use std::{
    io,
    os::fd::{AsRawFd, OwnedFd},
};

use libc::pid_t;
#[cfg(feature = "tokio")]
use tokio::io::unix::AsyncFd;

use crate::Status;

pub struct Child {
    pid: libc::pid_t,
    #[cfg(feature = "tokio")]
    pidfd: AsyncFd<OwnedFd>,
    #[cfg(not(feature = "tokio"))]
    pidfd: OwnedFd,
    status: Status,
    kill_on_drop: bool,
}

impl Child {
    pub fn from_pid(pid: pid_t, pidfd: OwnedFd, kill_on_drop: bool) -> io::Result<Self> {
        let status = Status::Running;
        #[cfg(feature = "tokio")]
        let pidfd = Self::async_pidfd(pidfd)?;

        Ok(Self {
            pid,
            pidfd,
            status,
            kill_on_drop,
        })
    }

    pub fn pid(&self) -> libc::pid_t {
        self.pid
    }

    pub fn status(&self) -> Status {
        self.status
    }

    #[cfg(feature = "tokio")]
    pub async fn wait(&mut self) -> io::Result<Status> {
        if self.status != Status::Running {
            return Ok(self.status);
        }

        let mut guard = self.pidfd.readable().await?;
        let status = loop {
            match guard.try_io(|_| {
                let mut status: libc::c_int = 0;
                let result = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };
                if result == 0 {
                    Err(io::Error::from(io::ErrorKind::WouldBlock))
                } else if result == -1 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(Status::from_raw(status))
                }
            }) {
                Ok(result) => break result?,
                Err(_would_block) => continue,
            }
        };

        self.status = status;
        Ok(status)
    }

    #[cfg(not(feature = "tokio"))]
    pub fn wait(&mut self) -> io::Result<Status> {
        if self.status != Status::Running {
            return Ok(self.status);
        }

        loop {
            let mut raw_status = 0;
            match unsafe { libc::waitpid(self.pid, &mut raw_status, 0) } {
                pid if pid == self.pid => {
                    self.status = Status::from_raw(raw_status);
                    return Ok(self.status);
                }
                -1 => {
                    let error = io::Error::last_os_error();
                    if error.kind() != io::ErrorKind::Interrupted {
                        return Err(error);
                    }
                }
                _ => return Err(io::Error::other("waitpid returned an unexpected PID")),
            }
        }
    }

    pub fn kill(&self) -> io::Result<()> {
        self.send_signal(libc::SIGKILL)
    }

    pub fn send_signal(&self, signal: i32) -> io::Result<()> {
        loop {
            let result = unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    self.pidfd_raw_fd(),
                    signal,
                    std::ptr::null::<libc::siginfo_t>(),
                    0,
                )
            };
            if result == 0 {
                return Ok(());
            }

            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }

    #[inline]
    fn pidfd_raw_fd(&self) -> std::os::fd::RawFd {
        #[cfg(feature = "tokio")]
        {
            self.pidfd.get_ref().as_raw_fd()
        }
        #[cfg(not(feature = "tokio"))]
        {
            self.pidfd.as_raw_fd()
        }
    }

    #[inline]
    #[cfg(feature = "tokio")]
    fn async_pidfd(pidfd: OwnedFd) -> io::Result<AsyncFd<OwnedFd>> {
        unsafe {
            let flags = libc::fcntl(pidfd.as_raw_fd(), libc::F_GETFL);
            if flags == -1 {
                return Err(io::Error::last_os_error());
            }
            libc::fcntl(pidfd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
        AsyncFd::new(pidfd).map_err(|error| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to create AsyncFd for pidfd: {error}"),
            )
        })
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        if !self.kill_on_drop || self.status != Status::Running {
            return;
        }

        if let Err(error) = self.kill()
            && error.raw_os_error() != Some(libc::ESRCH)
        {
            eprintln!("failed to kill child {} on drop: {error}", self.pid);
            return;
        }
    }
}
