use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};

#[cfg(feature = "tokio")]
use std::{
    pin::Pin,
    task::{Context, Poll, ready},
};
#[cfg(feature = "tokio")]
use tokio::io::unix::AsyncFd;

pub struct PipeRead {
    fd: OwnedFd,
}

pub struct PipeWrite {
    fd: OwnedFd,
}

#[cfg(feature = "tokio")]
pub struct AsyncPipeRead {
    fd: AsyncFd<OwnedFd>,
}

#[cfg(feature = "tokio")]
pub struct AsyncPipeWrite {
    fd: Option<AsyncFd<OwnedFd>>,
}

pub fn new() -> io::Result<(PipeRead, PipeWrite)> {
    let mut fds = [-1; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } == -1 {
        return Err(io::Error::last_os_error());
    }

    Ok((
        PipeRead {
            fd: unsafe { OwnedFd::from_raw_fd(fds[0]) },
        },
        PipeWrite {
            fd: unsafe { OwnedFd::from_raw_fd(fds[1]) },
        },
    ))
}

#[cfg(feature = "tokio")]
fn into_async_fd(fd: OwnedFd) -> io::Result<AsyncFd<OwnedFd>> {
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    if flags == -1 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
        return Err(io::Error::last_os_error());
    }

    AsyncFd::new(fd)
}

impl PipeRead {
    #[cfg(feature = "tokio")]
    pub fn into_async(self) -> io::Result<AsyncPipeRead> {
        Ok(AsyncPipeRead {
            fd: into_async_fd(self.fd)?,
        })
    }

    pub(crate) fn into_owned_fd(self) -> OwnedFd {
        self.fd
    }
}

impl PipeWrite {
    #[cfg(feature = "tokio")]
    pub fn into_async(self) -> io::Result<AsyncPipeWrite> {
        Ok(AsyncPipeWrite {
            fd: Some(into_async_fd(self.fd)?),
        })
    }

    pub(crate) fn into_owned_fd(self) -> OwnedFd {
        self.fd
    }
}

impl io::Read for PipeRead {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let result =
                unsafe { libc::read(self.fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
            if result >= 0 {
                return Ok(result as usize);
            }

            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}

impl io::Write for PipeWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        loop {
            let result =
                unsafe { libc::write(self.fd.as_raw_fd(), buf.as_ptr().cast(), buf.len()) };
            if result >= 0 {
                return Ok(result as usize);
            }

            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(feature = "tokio")]
impl tokio::io::AsyncRead for AsyncPipeRead {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        loop {
            let mut guard = ready!(this.fd.poll_read_ready(cx))?;
            match guard.try_io(|inner| {
                loop {
                    let result = unsafe {
                        libc::read(
                            inner.get_ref().as_raw_fd(),
                            buf.initialize_unfilled().as_mut_ptr().cast(),
                            buf.remaining(),
                        )
                    };
                    if result >= 0 {
                        return Ok(result as usize);
                    }

                    let error = io::Error::last_os_error();
                    if error.kind() != io::ErrorKind::Interrupted {
                        return Err(error);
                    }
                }
            }) {
                Ok(Ok(read)) => {
                    buf.advance(read);
                    return Poll::Ready(Ok(()));
                }
                Ok(Err(error)) => return Poll::Ready(Err(error)),
                Err(_would_block) => continue,
            }
        }
    }
}

#[cfg(feature = "tokio")]
impl tokio::io::AsyncWrite for AsyncPipeWrite {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        loop {
            let Some(fd) = this.fd.as_ref() else {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "pipe is shut down",
                )));
            };
            let mut guard = ready!(fd.poll_write_ready(cx))?;
            match guard.try_io(|inner| {
                loop {
                    let result = unsafe {
                        libc::write(inner.get_ref().as_raw_fd(), buf.as_ptr().cast(), buf.len())
                    };
                    if result >= 0 {
                        return Ok(result as usize);
                    }

                    let error = io::Error::last_os_error();
                    if error.kind() != io::ErrorKind::Interrupted {
                        return Err(error);
                    }
                }
            }) {
                Ok(result) => return Poll::Ready(result),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.get_mut().fd.take();
        Poll::Ready(Ok(()))
    }
}
