use clap::Parser;
use std::thread;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "probe-service")]
#[command(about = "A test probe service for spindle app")]
struct Args {
    /// Service name to simulate
    #[arg(short, long, default_value = "probe-service")]
    name: String,

    /// Exit after specified seconds (0 means run forever)
    #[arg(short, long, default_value = "0")]
    exit_after: u64,
}

fn main() {
    let args = Args::parse();
    let start_time = chrono::Local::now();

    println!(
        "[{}] Service '{}' starting...",
        start_time.format("%Y-%m-%d %H:%M:%S"),
        args.name
    );

    if args.exit_after > 0 {
        println!(
            "  Service '{}' will exit after {} seconds",
            args.name, args.exit_after
        );
    } else {
        println!("  Service '{}' will run forever", args.name);
    }

    // Run the service
    if args.exit_after > 0 {
        thread::sleep(Duration::from_secs(args.exit_after));

        let exit_time = chrono::Local::now();
        println!();
        println!(
            "[{}] Service '{}' exiting after {} seconds",
            exit_time.format("%Y-%m-%d %H:%M:%S"),
            args.name,
            args.exit_after
        );
    } else {
        // Run forever
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }
}
