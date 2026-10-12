use std::{collections::HashMap, ffi::OsString, path::Path};

use crate::stdio::{Stderr, Stdin, Stdout};

pub struct Command {
    pub(crate) program: OsString,
    pub(crate) arg0: Option<OsString>,
    pub(crate) args: Vec<OsString>,
    pub(crate) env: HashMap<OsString, OsString>,
    pub(crate) env_clear: bool,
    pub(crate) current_dir: Option<OsString>,
    pub(crate) stdin: Stdin,
    pub(crate) stdout: Stdout,
    pub(crate) stderr: Stderr,
    pub(crate) kill_on_drop: bool,
}

impl Command {
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            arg0: None,
            args: Vec::new(),
            env: HashMap::new(),
            env_clear: false,
            current_dir: None,
            stdin: Stdin::Inherit,
            stdout: Stdout::Inherit,
            stderr: Stderr::Inherit,
            kill_on_drop: false,
        }
    }

    pub fn arg0(mut self, arg0: impl Into<OsString>) -> Self {
        self.arg0 = Some(arg0.into());
        self
    }

    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn envs<I, K, V>(mut self, envs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<OsString>,
        V: Into<OsString>,
    {
        self.env.extend(
            envs.into_iter()
                .map(|(key, value)| (key.into(), value.into())),
        );
        self
    }

    pub fn env_clear(mut self) -> Self {
        self.env_clear = true;
        self.env.clear();
        self
    }

    pub fn env_remove(mut self, key: impl Into<OsString>) -> Self {
        self.env.remove(&key.into());
        self
    }

    pub fn current_dir(mut self, dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref().as_os_str().to_owned();
        self.current_dir = Some(dir);
        self
    }

    pub fn stdin(mut self, stdin: impl Into<Stdin>) -> Self {
        self.stdin = stdin.into();
        self
    }

    pub fn stdout(mut self, stdout: impl Into<Stdout>) -> Self {
        self.stdout = stdout.into();
        self
    }

    pub fn stderr(mut self, stderr: impl Into<Stderr>) -> Self {
        self.stderr = stderr.into();
        self
    }

    pub fn kill_on_drop(mut self, kill_on_drop: bool) -> Self {
        self.kill_on_drop = kill_on_drop;
        self
    }
}
