use clone3_process::{Command, pipe, stdio::Stdin};
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let (pipe_read, pipe_write) = pipe::new()?;
    let async_read = pipe_read.into_async()?;

    let mut child = Command::new("/bin/sh")
        .args([
            "-c",
            r#"
            for i in 1 2 3 4 5; do
                echo "Line $i"
                sleep 1
            done
            "#,
        ])
        .stdin(Stdin::Null)
        .stdout(pipe_write)
        .spawn()?;

    tokio::spawn(async move {
        let mut reader = BufReader::new(async_read);
        let mut line = String::new();

        loop {
            line.clear();

            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF: канал закрыт
                Ok(_) => {
                    print!("{line}");
                }
                Err(error) => {
                    eprintln!("Read error: {error}");
                    break;
                }
            }
        }
    });

    let status = child.wait().await?;

    println!("Child exited: {status:?}");
    Ok(())
}
