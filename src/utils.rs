use std::sync::OnceLock;
use std::io;

use gtk::{glib, gio};
use gtk::prelude::AppInfoExt;

use tokio::runtime::Runtime;

//------------------------------------------------------------------------------
// GLOBAL: Functions
//------------------------------------------------------------------------------
//---------------------------------------
// Tokio runtime function
//---------------------------------------
pub fn tokio_runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();

    RUNTIME.get_or_init(|| {
        Runtime::new().expect("Setting up tokio runtime needs to succeed.")
    })
}

//---------------------------------------
// Run command async function
//---------------------------------------
pub async fn run_command_async(cmd: &str) -> io::Result<(Option<i32>, String)> {
    // Run external command
    let params = shlex::split(cmd)
        .filter(|params| !params.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Error parsing parameters"))?;

    let output = async_process::Command::new(&params[0]).args(&params[1..]).output().await?;

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;

    let code = output.status.code();

    Ok((code, stdout))
}

//---------------------------------------
// Size to string function
//---------------------------------------
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

//---------------------------------------
// Date to string function
//---------------------------------------
pub fn date_to_string(date: i64, format: &str) -> String {
    if date == 0 {
        String::default()
    } else {
        glib::DateTime::from_unix_local(date)
            .and_then(|datetime| datetime.format(format))
            .expect("Datetime error")
            .to_string()
    }
}

//---------------------------------------
// Open with default app function
//---------------------------------------
pub fn open_with_default_app(path: &str) {
    let uri = format!("file://{path}");

    if gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>).is_err() {
        if let Some(desktop) = gio::AppInfo::default_for_type("inode/directory", true) {
            let _res = desktop.launch_uris(&[&uri], None::<&gio::AppLaunchContext>);
        }
    }
}
