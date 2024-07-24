use std::process::{Command, Stdio, Child};
use std::io::{Write};
use std::fs::{File};
use std::error;
use rand::prelude::*;

type Result<T> = std::result::Result<T, Box<dyn error::Error>>;

/// This struct allows interacting with a live Milk session
pub struct Milk {
    milk_process: Child,
    fifo_pipe: File,
}

/// This allows the clean exiting of the milk session when the
/// Milk object goes out of scope
impl Drop for Milk {
    /// `drop(milk)` gracefully exits the Milk session by sending an exit command
    /// to the attached fifo pipe. Then there is a blocking wait before continuing
    /// execution. This is currently the cleanest way to synchronise the main rust
    /// process with the milk one - by dropping the Milk instance, e.g.,:
    /// ```
    /// let mut milk = Milk::new().unwrap();
    /// milk.cmd("writef2file \"/tmp/out.txt\" 0.5");
    /// // --- at this point we don't know if the above command has finished.
    /// drop(milk);
    /// // --- now we can be sure that the command has been executed.
    /// ``` 
    fn drop(&mut self) {
        // send exit signal to milk fifo
        self.cmd("exit");
        // if successfully exited then this next call will pass without stalling.
        self.milk_process.wait().expect("couldn't wait?");
    }
}

impl Milk {
    /// Creates a Milk session and associated fifo pipe.
    ///
    /// # Example
    /// ```
    /// use milkrs::Milk;
    /// 
    /// let milk = Milk::new().unwrap();
    /// ```
    pub fn new() -> Result<Self> {
        let mut rng = thread_rng();
        let fifo_name = format!("/tmp/.fifo.{:06}",rng.gen_range(0..=1_000_000));
        
        let mkfifo = Command::new("mkfifo")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .arg(fifo_name.clone())
            .status()?;
        
        match mkfifo.success() {
            false => return Err("Couldn't create pipe!".into()),
            _ => {}
        }
        
        let milk_process = Command::new("milk")
            .arg("-f")
            .arg("-F")
            .arg(fifo_name.clone())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn()
            .expect("Failed to spawn milk process");
        
        let fifo_pipe = File::options()
            .create(false)
            .read(false)
            .write(true)
            .append(true)
            .open(fifo_name.clone())?;
        
        let milk = Self {
            milk_process: milk_process,
            fifo_pipe: fifo_pipe,
        };
        Ok(milk)
    }

    /// Pass a command to the Milk session
    ///
    /// # Example
    /// ```
    /// let milk = Milk::new().unwrap();       // create milk instance
    /// milk.cmd("mk3Dim out1 512 512 512");   // make 512 x 512 x 512 image
    /// milk.cmd("imcp2shm out1 outs1");       // copy image to shm
    /// ```
    pub fn cmd(&mut self, command: &str) {
        write!(self.fifo_pipe, "{command}\n").expect("couldn't write commmand string");
    }

    /// Pass a vector of commands to the Milk session
    ///
    /// # Example
    /// ```
    /// let milk = Milk::new().unwrap();       // create milk instance
    /// milk.cmds(vec![
    ///     "mk3Dim out1 512 512 512",  // make 512 x 512 x 512 image
    ///     "imcp2shm out1 outs1",      // copy image to shm
    /// ]);
    /// ```
    pub fn cmds(&mut self, commands: Vec<&str>) {
        for command in commands {
            self.cmd(command);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Milk;
    use std::fs;
    use rand;
    
    #[test]
    fn milk_spawns(){
        Milk::new().expect("milk failed to start");
    }

    #[test]
    fn write_via_milk(){
        let mut milk = Milk::new().expect("Failed to start milk");
        let randint: u32 = rand::random::<u32>() % 1000; 
        milk.cmds(vec![
            &format!("writef2file \"/tmp/tmp.txt\" {randint}"),
        ]);
        // you usually don't need to drop milk, but doing so blocks the process
        // until all milk commands have finished.
        drop(milk);
        let contents = fs::read_to_string("/tmp/tmp.txt").expect("couldn't open");
        assert_eq!(contents, format!("{randint}\n"));
    }
}
