use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use enigo::{Enigo, Settings};
use shared::commands::{BaseCommand, BaseResponse, CaptureCommand, CaptureType, Command, Response};
use smol::{Task, channel::Sender, lock::Mutex};

use crate::{handler, input};

pub struct CaptureTaskState {
    id: u64,
    active: Arc<AtomicBool>,
}
pub struct ActiveCommands {
    tasks: Arc<Mutex<HashMap<u64, Task<()>>>>,
    captures: Arc<Mutex<HashMap<CaptureType, CaptureTaskState>>>, // type, command id
    enigo: Enigo,
}
impl ActiveCommands {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())), // store a hashmap we can access from any thread, keeps commands
            captures: Arc::new(Mutex::new(HashMap::new())), // capture commands stored differently because they run in loops
            enigo: Enigo::new(&Settings::default()).unwrap(), // Enigo, the library, is used for simulating input
        }
    }
    // runs when a command is ran
    pub async fn spawn(&mut self, command: BaseCommand, tx: Sender<Vec<u8>>) {
        let id = command.id;
        let tasks = Arc::clone(&self.tasks); // clone the pointer to move it into the thread

        // if we are told to stop a capture...
        if let Command::Capture(CaptureCommand::Stop, capture_type) = &command.command {
            let mut captures = self.captures.lock().await; // get our current active captures
            if let Some(task_state) = captures.get(capture_type) { // if the capture we want to stop is active...
                task_state.active.store(false, Ordering::SeqCst); // we set the active bool to false, which tells the capture loop to stop

                let task = { 
                    let mut tasks = self.tasks.lock().await; // get the command's task
                    tasks.remove(&task_state.id) // remove it so we forget about it
                    // the return value of remove() is the task, so we can cancel it. which i do below.
                };

                if let Some(task) = task { // it is possible that the task will finish before we get here, making it null. check this
                    task.cancel().await; // wait for task to stop
                    captures.remove(capture_type); // remove it from the hash map of captures to show that we are no longer running it
                }
            }
            return;
        }

        // handle input sim before we try to refuse to reserve a thread
        // design choice: a whole new thread should not be needed to handle input simulation
        if input::main(&command, &mut self.enigo) { // input payload handles this elsewhere
            return; // stop running if we handled an input this cycle
        };

        let parallelism = thread::available_parallelism() // check how many threads we can use
            .map(|n| n.get()) // get the number
            .unwrap_or(1); // if we have trouble, assume we have 1 thread
        let current_tasks = {
            let lock = self.tasks.lock().await; // get our list of tasks, lock it because we want to read it. nobody else
            lock.len() // get the amount of tasks currently running
        };
        let max_tasks = if parallelism > 1 { parallelism - 1 } else { 1 }; // calculate how many tasks we can run
        if current_tasks >= max_tasks { // if we are running too many tasks, refuse to run another one. this is to prevent the client from becoming unresponsive 
            let refusal = bincode::encode_to_vec(
                BaseResponse {
                    id: command.id,
                    response: Response::Error("Client is fat!".to_string()), // we are running too many, can't afford to start another one
                },
                bincode::config::standard(),
            )
            .unwrap();
            let _ = tx.send(refusal).await;
            return;
        }

        let mut running: Option<Arc<AtomicBool>> = None; // dictates whether a capture loop should be running or not
        if let Command::Capture(CaptureCommand::Start(_, _), capture_type) = &command.command { // if we have been told to start a capture
            let mut captures = self.captures.lock().await; // we get our currently running captures,
            if captures.contains_key(capture_type) { //       and we see if one of the same type is already running
                let refusal = bincode::encode_to_vec(
                    BaseResponse {
                        id: command.id,              // if so, we refuse
                        response: Response::Error("Capture already started!".to_string()),
                    },
                    bincode::config::standard(),
                )
                .unwrap();
                let _ = tx.send(refusal).await;
                return;
            };
            // make a new capture state to keep track of the capture
            let capture_state = CaptureTaskState {
                id,
                active: Arc::new(AtomicBool::new(true)),
            };
            running = Some(capture_state.active.clone());
            captures.insert(capture_type.clone(), capture_state); // add to the hashmap of active captures for later access
        }

        let tx_clone = tx.clone(); // clone because it'll be moved into a new thread
        let handle = smol::spawn(async move { // another thread. whatever as long as it doesn't block the main thread
            smol::unblock(move || handler::main(command, tx_clone, running)).await; // this is our command handler

            let mut tasks = tasks.lock().await; // when the command is done running (^ see the await), we lock tasks
            tasks.remove(&id); //                  and then we remove our task/command from the list because it's done
        });
        let mut lock = self.tasks.lock().await; // in the meantime, while it runs above us,
        lock.insert(id, handle); //                we add the task to the list, which indicates that it's running
        println!("[*] spawned task");
    }
}
