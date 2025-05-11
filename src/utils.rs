//------------------------------------------------------------------------------
// MODULE: TokioRuntime
//------------------------------------------------------------------------------
pub mod tokio_runtime {
    use std::sync::OnceLock;
    use tokio::runtime::Runtime;

    //---------------------------------------
    // Runtime function
    //---------------------------------------
    pub fn runtime() -> &'static Runtime {
        static RUNTIME: OnceLock<Runtime> = OnceLock::new();

        RUNTIME.get_or_init(|| {
            Runtime::new().expect("Failed to set up tokio runtime")
        })
    }
}

//------------------------------------------------------------------------------
// MODULE: AsyncCommand
//------------------------------------------------------------------------------
pub mod async_command {
    use std::io;

    //---------------------------------------
    // Run function
    //---------------------------------------
    pub async fn run(cmd: &str) -> io::Result<(Option<i32>, String)> {
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
}

//------------------------------------------------------------------------------
// MODULE: AppInfo
//------------------------------------------------------------------------------
pub mod app_info {
    use gtk::gio;
    use gtk::prelude::AppInfoExt;

    //---------------------------------------
    // Open containing folder function
    //---------------------------------------
    pub fn open_containing_folder(path: &str) {
        let uri = format!("file://{path}");

        if let Some(desktop) = gio::AppInfo::default_for_type("inode/directory", true) {
            let _res = desktop.launch_uris(&[&uri], None::<&gio::AppLaunchContext>);
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
}
