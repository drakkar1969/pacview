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
    pub async fn run(cmd_line: &str) -> io::Result<(Option<i32>, String)> {
        // Parse command line
        let params = shlex::split(cmd_line);

        let (cmd, args) = params.as_ref()
            .and_then(|params| params.split_first())
            .ok_or(io::Error::new(io::ErrorKind::InvalidInput, "Failed to parse command"))?;

        // Run external command
        let output = async_process::Command::new(cmd).args(args).output().await?;

        let stdout = String::from_utf8(output.stdout)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        Ok((output.status.code(), stdout))
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

//------------------------------------------------------------------------------
// MODULE: AUR File
//------------------------------------------------------------------------------
pub mod aur_file {
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;
	use std::io::Read;

    use flate2::read::GzDecoder;
    use tokio::task::JoinHandle as TokioJoinHandle;

    use crate::utils::tokio_runtime;

    //---------------------------------------
    // Check file age function
    //---------------------------------------
    pub fn check_file_age(aur_file: &PathBuf, update_interval: u64) {
        // Get AUR package names file age
        let file_time = fs::metadata(aur_file).ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|file_time| {
                let now = std::time::SystemTime::now();

                now.duration_since(file_time).ok()
            });

        // Spawn tokio task to download AUR package names file if does not exist or older than x hours
        if file_time.is_none() || file_time.unwrap() >= Duration::from_secs(update_interval * 60 * 60) {
            let aur_file = aur_file.to_owned();

            glib::spawn_future_local(async move {
                let _ = download_future(&aur_file).await
                    .expect("Failed to complete tokio task");
            });
        }
    }

    //---------------------------------------
    // Download AUR names future function
    //---------------------------------------
    pub fn download_future(aur_file: &PathBuf) -> TokioJoinHandle<Result<(), reqwest::Error>> {
        let aur_file = aur_file.to_owned();

        // Spawn tokio task to download AUR file
        tokio_runtime::runtime().spawn(
            async move {
                let response = reqwest::Client::new()
                    .get("https://aur.archlinux.org/packages.gz")
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await?;

                let bytes = response.bytes().await?;

                let mut decoder = GzDecoder::new(bytes.as_ref());

                let mut gz_string = String::new();

                if decoder.read_to_string(&mut gz_string).is_ok() {
                    fs::write(&aur_file, gz_string).unwrap_or_default();
                }

                Ok::<(), reqwest::Error>(())
            }
        )
    }
}
