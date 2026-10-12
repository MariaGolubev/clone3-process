use clone3_process::{
    Command, pipe,
    stdio::{Stdin, Stdout},
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let (pipe_read, pipe_write) = pipe::new()?;

    let mut producer = Command::new("/bin/sh")
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

    let mut consumer = Command::new("/usr/bin/tr")
        .args(["a-z", "A-Z"])
        .stdin(pipe_read)
        .stdout(Stdout::Inherit)
        .spawn()?;

    let (producer_status, consumer_status) = tokio::join!(producer.wait(), consumer.wait());

    println!("Producer exited: {producer_status:?}");
    println!("Consumer exited: {consumer_status:?}");

    Ok(())
}
