use std::{
    collections::HashMap,
    ffi::{CString, OsString},
    io::{self, Read},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::ffi::OsStrExt,
    },
};

use super::CloneArgs;
use crate::{Child, Command};

impl Command {
    pub fn spawn(self) -> io::Result<Child> {
        let mut pidfd: RawFd = -1;
        let mut clone_args = CloneArgs {
            exit_signal: libc::SIGCHLD as u64,
            flags: libc::CLONE_PIDFD as u64,
            pidfd: &mut pidfd as *mut RawFd as u64,
            ..Default::default()
        };

        let (mut pipe_read, pipe_write) = crate::pipe::new()?;
        let write_fd = pipe_write.into_owned_fd();
        let write_raw = write_fd.as_raw_fd();

        let path = self.path()?;
        let current_dir = self.get_current_dir()?;
        let argv_storage = self.argv()?;
        let envp_storage = envp(self.env, self.env_clear)?;
        let argv = argv_storage
            .iter()
            .map(|arg| arg.as_ptr())
            .chain(std::iter::once(std::ptr::null()))
            .collect::<Vec<_>>();
        let envp = envp_storage
            .iter()
            .map(|entry| entry.as_ptr())
            .chain(std::iter::once(std::ptr::null()))
            .collect::<Vec<_>>();

        let pid = unsafe {
            libc::syscall(
                libc::SYS_clone3,
                &mut clone_args,
                std::mem::size_of::<CloneArgs>(),
            ) as libc::pid_t
        };

        match pid {
            -1 => return Err(io::Error::last_os_error()),

            0 => unsafe {
                if self.stdin.apply() == -1
                    || self.stdout.apply() == -1
                    || self.stderr.apply() == -1
                {
                    fail(write_raw);
                }
                if let Some(dir) = current_dir {
                    if libc::chdir(dir.as_ptr()) == -1 {
                        fail(write_raw);
                    }
                }
                libc::execve(path.as_ptr(), argv.as_ptr(), envp.as_ptr());
                fail(write_raw);
            },

            pid => {
                drop(write_fd);
                let pidfd = unsafe { OwnedFd::from_raw_fd(pidfd) };

                let mut buf = [0u8; 4];
                let mut filled = 0;
                while filled < buf.len() {
                    match pipe_read.read(&mut buf[filled..]) {
                        Ok(0) => break, // EOF: execve прошёл
                        Ok(n) => filled += n,
                        Err(error) => {
                            let _ = kill_and_wait(pid);
                            return Err(error);
                        }
                    }
                }

                if filled == buf.len() {
                    // ребёнок упал до exec: забрать зомби и вернуть причину
                    let _ = kill_and_wait(pid);
                    return Err(io::Error::from_raw_os_error(i32::from_ne_bytes(buf)));
                }
                match Child::from_pid(pid, pidfd, self.kill_on_drop) {
                    Ok(child) => return Ok(child),
                    Err(error) => {
                        if let Err(cleanup_error) = kill_and_wait(pid) {
                            eprintln!("failed to clean up child {pid}: {cleanup_error}");
                        }
                        return Err(error);
                    }
                }
            }
        }
    }

    #[inline]
    fn path(&self) -> io::Result<CString> {
        let path = CString::new(self.program.as_os_str().as_bytes())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        Ok(path)
    }

    #[inline]
    fn get_arg0(&self) -> io::Result<CString> {
        let arg0 = self.arg0.as_ref().unwrap_or(&self.program);

        CString::new(arg0.as_os_str().as_bytes())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
    }

    #[inline]
    fn argv(&self) -> io::Result<Vec<CString>> {
        let mut argv = Vec::with_capacity(self.args.len() + 2);
        argv.push(self.get_arg0()?);
        for arg in &self.args {
            let cstr = CString::new(arg.as_os_str().as_bytes())
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
            argv.push(cstr);
        }
        Ok(argv)
    }

    #[inline]
    fn get_current_dir(&self) -> io::Result<Option<CString>> {
        if let Some(dir) = &self.current_dir {
            let cstr = CString::new(dir.as_os_str().as_bytes())
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
            Ok(Some(cstr))
        } else {
            Ok(None)
        }
    }
}

#[inline]
fn envp(env: HashMap<OsString, OsString>, env_clear: bool) -> io::Result<Vec<CString>> {
    let env = if env_clear {
        env
    } else {
        let mut parent_env = std::env::vars_os().collect::<HashMap<OsString, OsString>>();
        parent_env.extend(env);
        parent_env
    };

    let mut envp = Vec::with_capacity(env.len() + 1);
    for (key, value) in env {
        let mut kv = key.clone().into_string().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid environment variable key",
            )
        })?;
        kv.push('=');
        kv.push_str(&value.to_string_lossy());
        let cstr =
            CString::new(kv).map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        envp.push(cstr);
    }
    Ok(envp)
}

#[inline]
unsafe fn fail(fd: RawFd) -> ! {
    unsafe {
        let errno = *libc::__errno_location();
        let bytes = errno.to_ne_bytes();
        libc::write(fd, bytes.as_ptr().cast(), bytes.len());
        libc::_exit(127)
    }
}

#[inline]
fn kill_and_wait(pid: libc::pid_t) -> io::Result<()> {
    loop {
        if unsafe { libc::kill(pid, libc::SIGKILL) } == 0 {
            break;
        }

        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error);
        }
        break;
    }

    loop {
        let result = unsafe { libc::waitpid(pid, std::ptr::null_mut(), 0) };
        if result == pid {
            return Ok(());
        }
        if result == -1 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if error.raw_os_error() == Some(libc::ECHILD) {
                return Ok(());
            }
            return Err(error);
        }

        return Err(io::Error::other("waitpid returned an unexpected PID"));
    }
}
