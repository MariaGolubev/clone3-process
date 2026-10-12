use clone3_process::{Command, stdio::Stdin};

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let file = std::fs::File::create("child_output.txt")?;
    let mut child = Command::new("/bin/sh")
        .args(["-c", "echo Hello from child process!"])
        .stdin(Stdin::Null)
        .stdout(file)
        .spawn()?;

    let status = child.wait().await?;

    println!("Child exited: {status:?}");
    Ok(())
}
