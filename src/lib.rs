#[cfg(not(target_os = "linux"))]
compile_error!("clone3-process is only supported on Linux");

mod child;
mod command;
pub mod pipe;
mod spawn;
mod status;
pub mod stdio;

pub use child::Child;
pub use command::Command;
pub use pipe::{PipeRead, PipeWrite};
// #[cfg(feature = "tokio")]
// pub use spawn_async::CommandAsyncExt;
pub use status::Status;

#[derive(Debug, Clone, Copy, Default)]
#[repr(C, align(8))]
pub(crate) struct CloneArgs {
    pub(crate) flags: u64,
    pub(crate) pidfd: u64,
    pub(crate) child_tid: u64,
    pub(crate) parent_tid: u64,
    pub(crate) exit_signal: u64,
    pub(crate) stack: u64,
    pub(crate) stack_size: u64,
    pub(crate) tls: u64,
    pub(crate) set_tid: u64,
    pub(crate) set_tid_size: u64,
    pub(crate) cgroup: u64,
}
