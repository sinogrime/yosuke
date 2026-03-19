use shared::commands::Response;

fn disconnect() -> Result<(), Box<dyn std::error::Error>> {
    // exit process with return code 0
    std::process::exit(0); // it's literally that simple
    // disconnecting tcp would just reconnect us so we need to exit the process
}

pub fn main() -> Result<Response, Box<dyn std::error::Error>> {
    let _ = disconnect();
    Ok(Response::Success) // we wont ever get here, like elevate command
}
