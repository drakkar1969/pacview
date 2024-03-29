use std::process::Command;

use gtk::{glib, gio};
use gtk::prelude::AppInfoExt;


//------------------------------------------------------------------------------
// MODULE: Utils
//------------------------------------------------------------------------------
pub struct Utils;

//------------------------------------------------------------------------------
// IMPLEMENTATION: Utils
//------------------------------------------------------------------------------
impl Utils {
    //-----------------------------------
    // Run command function
    //-----------------------------------
    pub fn run_command(cmd: &str) -> (Option<i32>, String) {
        let mut code: Option<i32> = None;
        let mut stdout: String = String::from("");

        if let Some(params) = shlex::split(cmd).filter(|params| !params.is_empty()) {
            if let Ok(output) = Command::new(&params[0]).args(&params[1..]).output() {
                code = output.status.code();
                stdout = String::from_utf8(output.stdout).unwrap_or_default();
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

            format!("{size:.decimals$}\u{202F}{unit}")
        }
    }

    //-----------------------------------
    // Date to string function
    //-----------------------------------
    pub fn date_to_string(date: i64, format: &str) -> String {
        if date == 0 {
            String::from("")
        } else {
            glib::DateTime::from_unix_local(date)
                .and_then(|datetime| datetime.format(format))
                .expect("Datetime error")
                .to_string()
        }
    }

    //-----------------------------------
    // Open file manager function
    //-----------------------------------
    pub fn open_file_manager(path: &str) {
        if let Some(desktop) = gio::AppInfo::default_for_type("inode/directory", true) {
            let path = format!("file://{path}");

            let _res = desktop.launch_uris(&[&path], None::<&gio::AppLaunchContext>);
        }
    }
}
