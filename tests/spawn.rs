use clone3_process::{Command, stdio::Stdin};
use std::io::ErrorKind;

fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("clone3_{}_{name}", std::process::id()))
}

#[test]
fn spawn_fails_for_missing_program() {
    let err = Command::new("/no/such/prog")
        .spawn()
        .err()
        .expect("spawn must fail");
    assert_eq!(err.kind(), ErrorKind::NotFound);
}

#[test]
fn spawn_fails_for_missing_current_dir() {
    let err = Command::new("/bin/true")
        .current_dir("/no/such/dir")
        .spawn()
        .err()
        .expect("spawn must fail");
    assert_eq!(err.kind(), ErrorKind::NotFound);
}

#[tokio::test(flavor = "current_thread")]
async fn spawn_succeeds() -> std::io::Result<()> {
    let mut child = Command::new("/bin/true").stdin(Stdin::Null).spawn()?;
    let status = child.wait().await?;
    assert!(status.success()); // поправь, если у твоего статуса другой API
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn current_dir_is_applied() -> std::io::Result<()> {
    let dir = std::fs::canonicalize(std::env::temp_dir())?;
    let out = tmp("pwd.txt");

    let mut child = Command::new("/bin/sh")
        .args(["-c", "pwd"])
        .current_dir(&dir)
        .stdin(Stdin::Null)
        .stdout(std::fs::File::create(&out)?)
        .spawn()?;
    child.wait().await?;

    let got = std::fs::read_to_string(&out)?;
    assert_eq!(got.trim(), dir.to_str().unwrap());
    std::fs::remove_file(out)?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn env_clear_and_env() -> std::io::Result<()> {
    let out = tmp("env.txt");

    let mut child = Command::new("/usr/bin/env")
        .env_clear()
        .env("FOO", "bar")
        .stdin(Stdin::Null)
        .stdout(std::fs::File::create(&out)?)
        .spawn()?;
    child.wait().await?;

    // в окружении ровно одна переменная
    assert_eq!(std::fs::read_to_string(&out)?.trim(), "FOO=bar");
    std::fs::remove_file(out)?;
    Ok(())
}
