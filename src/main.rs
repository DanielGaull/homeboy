use dotenv::dotenv;
use homeboy::runner::runner::CommandRunner;
use std::{env, error::Error, io::{stdin, stdout, Write}};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    let _vars = env::vars();

    let mut runner = CommandRunner::new();
    println!("Initializing...");
    runner.init("./templates.txt")?;
    println!("Initialized");
    runner.run_loop()?;
    Ok(())
    // loop {
    //     print!("Input: ");
    //     let line = read_line();
    //     runner.run(&line)?;
    // }
    // Ok(())
}

#[allow(dead_code)]
fn read_line() -> String {
    let mut s = String::new();
    let _ = stdout().flush();
    stdin().read_line(&mut s).expect("Did not enter a correct string");
    if let Some('\n') = s.chars().next_back() {
        s.pop();
    }
    if let Some('\r') = s.chars().next_back() {
        s.pop();
    }
    s
}
