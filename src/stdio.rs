use std::{
    fs::File,
    io,
    os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd},
};

#[derive(Debug, Default)]
pub enum Stdin {
    #[default]
    Inherit,
    Null,
    Fd(OwnedFd),
}

impl Stdin {
    #[inline]
    pub(crate) unsafe fn apply(&self) -> libc::c_int {
        match self {
            Self::Inherit => 0,
            Self::Null => unsafe { apply_null(libc::STDIN_FILENO, libc::O_RDONLY) },
            Self::Fd(fd) => unsafe { apply_fd(fd.as_raw_fd(), libc::STDIN_FILENO) },
        }
    }
}

#[derive(Debug, Default)]
pub enum Stdout {
    #[default]
    Inherit,
    Null,
    Fd(OwnedFd),
}

impl Stdout {
    #[inline]
    pub(crate) unsafe fn apply(&self) -> libc::c_int {
        match self {
            Self::Inherit => 0,
            Self::Null => unsafe { apply_null(libc::STDOUT_FILENO, libc::O_WRONLY) },
            Self::Fd(fd) => unsafe { apply_fd(fd.as_raw_fd(), libc::STDOUT_FILENO) },
        }
    }
}

#[derive(Debug, Default)]
pub enum Stderr {
    #[default]
    Inherit,
    Null,
    Fd(OwnedFd),
}

impl Stderr {
    #[inline]
    pub(crate) unsafe fn apply(&self) -> libc::c_int {
        match self {
            Self::Inherit => 0,
            Self::Null => unsafe { apply_null(libc::STDERR_FILENO, libc::O_WRONLY) },
            Self::Fd(fd) => unsafe { apply_fd(fd.as_raw_fd(), libc::STDERR_FILENO) },
        }
    }
}

impl From<crate::PipeRead> for Stdin {
    fn from(pipe: crate::PipeRead) -> Self {
        Self::Fd(pipe.into_owned_fd())
    }
}

impl From<crate::PipeWrite> for Stdout {
    fn from(pipe: crate::PipeWrite) -> Self {
        Self::Fd(pipe.into_owned_fd())
    }
}

impl From<crate::PipeWrite> for Stderr {
    fn from(pipe: crate::PipeWrite) -> Self {
        Self::Fd(pipe.into_owned_fd())
    }
}

macro_rules! impl_stdio_conversions {
    ($stdio:ty) => {
        impl From<OwnedFd> for $stdio {
            fn from(fd: OwnedFd) -> Self {
                Self::Fd(fd)
            }
        }

        impl From<File> for $stdio {
            fn from(file: File) -> Self {
                Self::from(OwnedFd::from(file))
            }
        }

        impl TryFrom<BorrowedFd<'_>> for $stdio {
            type Error = io::Error;

            fn try_from(fd: BorrowedFd<'_>) -> Result<Self, Self::Error> {
                duplicate_fd(fd).map(Self::from)
            }
        }
    };
}

impl_stdio_conversions!(Stdin);
impl_stdio_conversions!(Stdout);
impl_stdio_conversions!(Stderr);

#[inline]
fn duplicate_fd(fd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    let duplicate = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if duplicate == -1 {
        return Err(io::Error::last_os_error());
    }

    Ok(unsafe { OwnedFd::from_raw_fd(duplicate) })
}

#[inline]
unsafe fn apply_null(target: libc::c_int, flags: libc::c_int) -> libc::c_int {
    let fd = unsafe { libc::open(c"/dev/null".as_ptr(), flags) };
    if fd == -1 {
        return -1;
    }

    if fd == target {
        return 0;
    }

    let result = unsafe { libc::dup2(fd, target) };
    unsafe {
        libc::close(fd);
    }
    result
}

#[inline]
unsafe fn apply_fd(source: RawFd, target: libc::c_int) -> libc::c_int {
    if source == target {
        let descriptor_flags = unsafe { libc::fcntl(source, libc::F_GETFD) };
        if descriptor_flags == -1 {
            return -1;
        }
        return unsafe { libc::fcntl(source, libc::F_SETFD, descriptor_flags & !libc::FD_CLOEXEC) };
    }

    unsafe { libc::dup2(source, target) }
}
