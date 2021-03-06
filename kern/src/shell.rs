use alloc::vec::Vec;
use alloc::string::String;
use core::fmt::Write;
use core::str;
use crate::hardware::keyboard::blocking_get_char;

/// Error type for `Command` parse failures.
#[derive(Debug)]
enum Error {
    Empty,
    TooManyArgs,
}

pub struct Shell {
    history: Vec<String>,
}

/// A structure representing a single shell command.
struct Command {
    args: Vec<String>,
}

impl Command {
    /// Parse a command from a string `s` using `buf` as storage for the
    /// arguments.
    ///
    /// # Errors
    ///
    /// If `s` contains no arguments, returns `Error::Empty`. If there are more
    /// arguments than `buf` can hold, returns `Error::TooManyArgs`.
    fn parse(s: &str) -> Result<Command, Error> {
        let mut args: Vec<String> = Vec::new();
        for arg in s.split(' ').filter(|a| !a.is_empty()) {
            args.push(String::from(arg));
        }

        if args.is_empty() {
            return Err(Error::Empty);
        }

        Ok(Command { args })
    }

    /// Returns this command's path. This is equivalent to the first argument.
    fn path(&self) -> &str {
        &self.args[0]
    }
}


impl Shell {
    pub fn new() -> Shell {
        Shell{
            history: Vec::default(),
        }
    }
    /// Starts a shell using `prefix` as the prefix for each line. This function
    /// never returns.
    pub fn shell(&mut self, prefix: &str) {
        let mut input_vec: Vec<u8> = Vec::new();
        loop {
            input_vec.truncate(0);
            {
                print!("{}", prefix);
                loop {
                    let chr = blocking_get_char();
                    match chr {
                        b'\r' | b'\n' => { // \r \n
                            break;
                        }
                        3 => { // EXT (Ctrl-C)
                            print!("\n");
                            return;
                        }
                        32..=126 => {
                            input_vec.push(chr);
                            print!("{}", chr as char);
                        }
                        8 | 127 => {
                            if input_vec.is_empty() {
                                print!("\u{7}"); // Ring Bell
                            } else {
                                input_vec.truncate(input_vec.len() - 1);
                                print!("\u{8} \u{8}"); // Erase 1 char
                            }
                        }
                        _ => {
                            print!("\u{7}"); // Ring Bell
                        } // Bell
                    };
                };
                print!("\n")
            }
            let user_command_input = str::from_utf8(&input_vec).unwrap();
            self.history.insert(0, String::from(user_command_input));
            self.history.truncate(10);
            let cmd_result =
                Command::parse(&user_command_input);
            match cmd_result {
                Ok(cmd) => {
                    let exec_result = self.process_command(&cmd);
                    match exec_result {
                        Ok(val) => {
                            if val >= 0 {
                                continue;
                            } else {
                                return;
                            }
                        }
                        Err(_) => {
                            println!("unknown command: {}", cmd.path());
                        }
                    }
                }
                Err(e) => {
                    match e {
                        Error::TooManyArgs => {
                            println!("error: too many arguments");
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn process_command(&mut self, command: &Command) -> Result<i8, ()> {
        match command.path() {
            "echo" => {
                for (i, v) in command.args.iter().enumerate() {
                    if i == 0 {
                        continue;
                    }
                    print!("{} ", v);
                }
                print!("\n");
                Ok(0)
            },
            "history" => {
                for (id, cmd) in self.history.iter().rev().enumerate() {
                    println!("{} {}", id, cmd);
                }
                Ok(0)
            },
            "exit" => {
                Ok(-1)
            },
            _ => Err(())
        }
    }

}

