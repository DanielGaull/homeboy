use dotenv::dotenv;
use std::env;

mod templating;

fn main() {
    dotenv().ok();
    let _vars = env::vars();

    println!("Hello, world!");
}
