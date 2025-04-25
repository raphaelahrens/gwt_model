use std::path::Path;

use clap::Parser;

use miette::Result;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the gwt source file
    source: Box<Path>,
}

fn main() -> Result<()>{
    let args = Args::parse();
    let model = parser::parse_file(&args.source)?;
    println!("{}", model);
    Ok(())
}
