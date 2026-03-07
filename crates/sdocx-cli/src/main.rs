use clap::Parser;

#[derive(Parser)]
#[command(name = "sdocx", version, about = "sdocx CLI")]
struct Cli {
    #[arg(short, long)]
    name: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    if let Some(name) = cli.name {
        println!("Hello, {name}!");
    } else {
        println!("{}", sdocx::hello());
    }
}