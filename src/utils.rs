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
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Failed to parse command"))?;

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
// MODULE: AURFile
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
        if file_time.is_none_or(|time| time >= Duration::from_secs(update_interval * 60 * 60)) {
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

//------------------------------------------------------------------------------
// MODULE: PangoUtils
//------------------------------------------------------------------------------
pub mod pango_utils {
    use std::fmt::Write as _;

    use gtk::pango;
    use gtk::prelude::ToValue;

    //-----------------------------------
    // Pango font str to CSS function
    //-----------------------------------
    pub fn font_str_to_css(font_str: &str) -> String {
        let mut css = String::new();
        
        let font_desc = pango::FontDescription::from_string(font_str);

        let mask = font_desc.set_fields();

        if mask.contains(pango::FontMask::FAMILY) {
            if let Some(family) = font_desc.family() {
                write!(css, "font-family: {family}; ").unwrap();
            }
        }

        if mask.contains(pango::FontMask::SIZE) {
            let font_size = font_desc.size()/pango::SCALE;

            write!(css, "font-size: {}pt; ", font_size.max(0)).unwrap();
        }

        if mask.contains(pango::FontMask::WEIGHT) {
            let weight = match font_desc.weight() {
                pango::Weight::Normal => "normal",
                pango::Weight::Bold => "bold",
                pango::Weight::Thin => "100",
                pango::Weight::Ultralight => "200",
                pango::Weight::Light | pango::Weight::Semilight => "300",
                pango::Weight::Book => "400",
                pango::Weight::Medium => "500",
                pango::Weight::Semibold => "600",
                pango::Weight::Ultrabold => "800",
                pango::Weight::Heavy | pango::Weight::Ultraheavy => "900",
                _ => unreachable!()
            };

            write!(css, "font-weight: {weight}; ").unwrap();
        }

        if mask.contains(pango::FontMask::STYLE) {
            if let Some((_, value)) = glib::EnumValue::from_value(&font_desc.style().to_value()) {
                write!(css, "font-style: {}; ", value.nick()).unwrap();
            }
        }

        css
    }
}

//------------------------------------------------------------------------------
// MODULE: StyleSchemes
//------------------------------------------------------------------------------
pub mod style_schemes {
    //-----------------------------------
    // Is variant dark functions
    //-----------------------------------
    pub fn is_variant_dark(scheme: &sourceview5::StyleScheme) -> bool {
        scheme.metadata("variant").is_some_and(|variant| variant == "dark")
    }

    pub fn is_variant_dark_by_id(id: &str) -> bool {
        sourceview5::StyleSchemeManager::default()
            .scheme(id)
            .is_some_and(|scheme| is_variant_dark(&scheme))
    }

    //-----------------------------------
    // Variant function
    //-----------------------------------
    pub fn variant_id(id: &str) -> Option<glib::GString> {
        let scheme_manager = sourceview5::StyleSchemeManager::default();

        let scheme = scheme_manager.scheme(id)?;

        let variant = scheme.metadata("variant")?;

        if variant == "dark" {
            scheme.metadata("light-variant")
        } else {
            scheme.metadata("dark-variant")
        }
    }

    //-----------------------------------
    // Schemes function
    //-----------------------------------
    pub fn schemes(dark: bool) -> Vec<sourceview5::StyleScheme> {
        let scheme_manager = sourceview5::StyleSchemeManager::default();

        scheme_manager
            .scheme_ids()
            .iter()
            .filter_map(|id| scheme_manager.scheme(id))
            .filter(|scheme| is_variant_dark(&scheme) == dark)
            .collect::<Vec<sourceview5::StyleScheme>>()
    }
}
