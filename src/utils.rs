use std::process::Command;

use gtk::glib;

//------------------------------------------------------------------------------
// MODULE: Utils
//------------------------------------------------------------------------------
pub struct Utils;

impl Utils {
    //-----------------------------------
    // Run command function
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

    //-----------------------------------
    // Size to string function
    //-----------------------------------
    pub fn size_to_string(size: i64, decimals: usize) -> String {
        let mut size = size as f64;

        if size == 0.0 {
            String::from("0 B")
        } else {
            let mut unit = "";
    
            for u in ["B", "KiB", "MiB", "GiB", "TiB", "PiB"] {
                unit = u;
    
                if size < 1024.0 || u == "PiB" {
                    break;
                }
    
                size /= 1024.0;
            }
    
            format!("{size:.prec$} {unit}", size=size, prec=decimals, unit=unit)
        }
    }

    //-----------------------------------
    // Date to string function
    //-----------------------------------
    pub fn date_to_string(date: i64, format: &str) -> String {
        if date == 0 {
            String::from("")
        } else {
            let datetime = glib::DateTime::from_unix_local(date).expect("error");

            datetime.format(format).expect("error").to_string()
        }
    }
}
