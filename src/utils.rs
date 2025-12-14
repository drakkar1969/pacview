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
    use std::ffi::OsStr;
    use std::io::{Result, Error};

    //---------------------------------------
    // Run function
    //---------------------------------------
    pub async fn run(cmd: impl AsRef<OsStr>, args: &[&str]) -> Result<(Option<i32>, String)> {
        let output = async_process::Command::new(cmd)
            .args(args)
            .output()
            .await?;

        let stdout = String::from_utf8(output.stdout)
            .map_err(Error::other)?;

        Ok((output.status.code(), stdout))
    }

    //---------------------------------------
    // Spawn function
    //---------------------------------------
    pub fn spawn(cmd: impl AsRef<OsStr>, args: &[&str]) -> Result<()> {
        async_process::Command::new(cmd)
            .args(args)
            .spawn()?;

        Ok(())
    }
}

//------------------------------------------------------------------------------
// MODULE: AppInfo
//------------------------------------------------------------------------------
pub mod app_info {
    use gtk::gio;
    use gtk::prelude::AppInfoExt;
    use gio::{AppInfo, AppLaunchContext};

    //---------------------------------------
    // Open containing folder function
    //---------------------------------------
    pub fn open_containing_folder(path: &str) {
        let uri = format!("file://{path}");

        if let Some(desktop) = AppInfo::default_for_type("inode/directory", true) {
            let _res = desktop.launch_uris(&[&uri], None::<&AppLaunchContext>);
        }
    }

    //---------------------------------------
    // Open with default app function
    //---------------------------------------
    pub fn open_with_default_app(path: &str) {
        let uri = format!("file://{path}");

        if AppInfo::launch_default_for_uri(&uri, None::<&AppLaunchContext>).is_err()
            && let Some(desktop) = AppInfo::default_for_type("inode/directory", true) {
                let _res = desktop.launch_uris(&[&uri], None::<&AppLaunchContext>);
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

    use crate::utils::tokio_runtime;

    //---------------------------------------
    // Check file age function
    //---------------------------------------
    pub fn check_file_age(aur_file: &PathBuf, update_interval: u64) -> bool {
        // Get AUR package names file age
        let file_time = fs::metadata(aur_file).ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|file_time| {
                let now = std::time::SystemTime::now();

                now.duration_since(file_time).ok()
            });

        file_time.is_none_or(|time| time >= Duration::from_secs(update_interval * 60 * 60))
    }

    //---------------------------------------
    // Download async function
    //---------------------------------------
    pub async fn download(aur_file: &PathBuf) -> Result<(), reqwest::Error> {
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
        .await
        .expect("Failed to complete tokio task")
    }
}

//------------------------------------------------------------------------------
// MODULE: PangoUtils
//------------------------------------------------------------------------------
pub mod pango_utils {
    use std::fmt::Write as _;

    use gtk::{glib, pango};
    use gtk::prelude::{ToValue, WidgetExt};
    use pango::{FontDescription, FontMask, Weight};

    //---------------------------------------
    // Pango color from style
    //---------------------------------------
    pub fn color_from_style(style: &str) -> (u16, u16, u16, u16) {
        let fc = |color: f32| -> u16 {
            (color * f32::from(u16::MAX)) as u16
        };

        let label = gtk::Label::builder()
            .css_name("texttag")
            .css_classes([style])
            .build();

        let color = label.color();

        (fc(color.red()), fc(color.green()), fc(color.blue()), fc(color.alpha()))
    }

    //-----------------------------------
    // Pango font str to CSS function
    //-----------------------------------
    pub fn font_str_to_css(font_str: &str) -> String {
        let mut css = String::new();

        let font_desc = FontDescription::from_string(font_str);

        let mask = font_desc.set_fields();

        if mask.contains(FontMask::FAMILY)
            && let Some(family) = font_desc.family() {
                write!(css, "font-family: {family}; ").unwrap();
            }

        if mask.contains(FontMask::SIZE) {
            let font_size = font_desc.size()/pango::SCALE;

            write!(css, "font-size: {}pt; ", font_size.max(0)).unwrap();
        }

        if mask.contains(FontMask::WEIGHT) {
            let weight = match font_desc.weight() {
                Weight::Normal => "normal",
                Weight::Bold => "bold",
                Weight::Thin => "100",
                Weight::Ultralight => "200",
                Weight::Light | Weight::Semilight => "300",
                Weight::Book => "400",
                Weight::Medium => "500",
                Weight::Semibold => "600",
                Weight::Ultrabold => "800",
                Weight::Heavy | Weight::Ultraheavy => "900",
                _ => unreachable!()
            };

            write!(css, "font-weight: {weight}; ").unwrap();
        }

        if mask.contains(FontMask::STYLE)
            && let Some((_, value)) = glib::EnumValue::from_value(&font_desc.style()
                .to_value()) {
                write!(css, "font-style: {}; ", value.nick()).unwrap();
            }

        css
    }
}

//------------------------------------------------------------------------------
// MODULE: StyleSchemes
//------------------------------------------------------------------------------
pub mod style_schemes {
    use gtk::glib;
    use sourceview5::{StyleScheme, StyleSchemeManager};

    //-----------------------------------
    // Is variant dark functions
    //-----------------------------------
    pub fn is_variant_dark(scheme: &StyleScheme) -> bool {
        scheme.metadata("variant").is_some_and(|variant| variant == "dark")
    }

    pub fn is_variant_dark_by_id(id: &str) -> bool {
        StyleSchemeManager::default()
            .scheme(id)
            .is_some_and(|scheme| is_variant_dark(&scheme))
    }

    //-----------------------------------
    // Variant function
    //-----------------------------------
    pub fn variant_id(id: &str) -> Option<glib::GString> {
        let scheme_manager = StyleSchemeManager::default();

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
    pub fn schemes(dark: bool) -> Vec<StyleScheme> {
        let scheme_manager = StyleSchemeManager::default();

        scheme_manager
            .scheme_ids()
            .iter()
            .filter_map(|id| {
                scheme_manager.scheme(id)
                    .filter(|scheme| is_variant_dark(scheme) == dark)
            })
            .collect::<Vec<StyleScheme>>()
    }
}
