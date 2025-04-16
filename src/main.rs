use dotenv::dotenv;
use homeboy::runner::runner::CommandRunner;
use std::{env, error::Error, io::{stdin, stdout, Write}};

#[allow(dead_code)]
const INPUT_VOICE: i32 = 0;
#[allow(dead_code)]
const INPUT_CONSOLE_TYPING: i32 = 1;

const INPUT: i32 = INPUT_CONSOLE_TYPING;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    let _vars = env::vars();

    let mut runner = CommandRunner::new()?;
    println!("Initializing...");
    runner.init("./templates.txt")?;
    println!("Initialized");

    if INPUT == INPUT_VOICE {
        let devices = runner.get_input_devices()?;
        println!("Select Input Device:");
        for (i, dev) in devices.iter().enumerate() {
            println!("{}. {}", i + 1, dev.1);
        }
        let dev_idx = read_number(1, devices.len()) - 1;
        let device = devices.get(dev_idx).unwrap().0;
        runner.set_input_device(device);
    
        runner.run_loop()?;
    } else {
        loop {
            print!("Input: ");
            let line = read_line();
            runner.run(&line)?;
        }
    }

    Ok(())
}

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
fn read_number(min: usize, max: usize) -> usize {
    loop {
        let input = read_line();

        match input.trim().parse::<usize>() {
            Ok(num) => {
                if num >= min && num <= max {
                    return num
                }
                println!("Number must be between {} and {} (inclusive)", min, max);
            },
            Err(_) => println!("Invalid input. Please enter a valid positive integer."),
        }
    }
}
