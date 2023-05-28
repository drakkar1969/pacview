use std::process::Command;

//------------------------------------------------------------------------------
// MODULE: Utils
//------------------------------------------------------------------------------
pub struct Utils;

impl Utils {
    //-----------------------------------
    // Public run command helper function
    //-----------------------------------
    pub fn run_command(cmd: &str) -> (Option<i32>, String) {
        let mut stdout: String = String::from("");
        let mut code: Option<i32> = None;

        if let Ok(params) = shell_words::split(cmd) {
            if !params.is_empty() {
                if let Ok(output) = Command::new(&params[0]).args(&params[1..]).output() {
                    code = output.status.code();
                    stdout = String::from_utf8(output.stdout).unwrap_or_default();
                }
            }
        }

        (code, stdout)
    }
}
