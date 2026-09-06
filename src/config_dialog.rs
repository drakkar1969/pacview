use gtk::glib;
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::VariantTy;

use crate::{
    config_row::ConfigRow,
    utils::{AppInfoExt, Pacman}
};

//------------------------------------------------------------------------------
// MODULE: ConfigDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/config_dialog.ui")]
    pub struct ConfigDialog {
        #[template_child]
        pub(super) options_page: TemplateChild<adw::PreferencesPage>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for ConfigDialog {
        const NAME: &'static str = "ConfigDialog";
        type Type = super::ConfigDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            // Install actions
            Self::install_actions(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ConfigDialog {}

    impl WidgetImpl for ConfigDialog {}
    impl AdwDialogImpl for ConfigDialog {}

    impl ConfigDialog {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Open config action
            klass.install_action_async("conf.config", None, async |_, _, _| {
                let config_path = Pacman::config_path().read().unwrap().clone();

                AppInfoExt::open_with_default_app(&config_path).await;
            });

            // Open path action
            klass.install_action_async("conf.path", Some(VariantTy::STRING),
                async |_, _, param| {
                    let paths = param
                        .and_then(|param| param.get::<String>())
                        .expect("Failed to get string from variant");

                    for path in paths.split('\n') {
                        AppInfoExt::open_with_default_app(path).await;
                    }
                }
            );
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: ConfigDialog
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct ConfigDialog(ObjectSubclass<imp::ConfigDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl ConfigDialog {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new() -> Self {
        let dialog: Self = glib::Object::builder().build();

        // Add config rows
        let config = Pacman::config().read().unwrap();

        dialog.add_group("Paths", &[
            ("RootDir", &config.root_dir, Some("conf.path")),
            ("DBPath", &config.db_path, Some("conf.path")),
            ("CacheDir", &config.cache_dir.join("\n"), Some("conf.path")),
            ("LogFile", &config.log_file, Some("conf.path")),
            ("GPGDir", &config.gpg_dir, Some("conf.path")),
            ("HookDir", &config.hook_dir.join("\n"), Some("conf.path"))
        ]);

        dialog.add_group("Download", &[
            ("XferCommand", &config.xfer_command, None),
            ("ParallelDownloads", &config.parallel_downloads.to_string(), None),
            ("DisableDownloadTimeout", &config.disable_download_timeout.to_string(), None),
            ("DownloadUser", &config.download_user.clone().unwrap_or_else(|| "None".into()), None),
            ("Architecture", &config.architecture.join(" | "), None)
        ]);

        dialog.add_group("Sandbox", &[
            ("DisableSandBox", &config.disable_sandbox.to_string(), None),
            ("DisableSandBoxFilesystem", &config.disable_sandbox_filesystem.to_string(), None),
            ("DisableSandBoxSyscalls", &config.disable_sandbox_syscalls.to_string(), None)
        ]);

        dialog.add_group("Packages", &[
            ("HoldPkg", &config.hold_pkg.join(" | "), None),
            ("IgnorePkg", &config.ignore_pkg.join(" | "), None),
            ("IgnoreGroup", &config.ignore_group.join(" | "), None),
            ("NoUpgrade", &config.no_upgrade.join(" | "), None),
            ("NoExtract", &config.no_extract.join(" | "), None)
        ]);

        dialog.add_group("Miscellaneous", &[
            ("UseSyslog", &config.use_syslog.to_string(), None),
            ("Color", &config.color.to_string(), None),
            ("CheckSpace", &config.check_space.to_string(), None),
            ("CleanMethod", &config.clean_method.join(" | "), None),
            ("VerbosePkgLists", &config.verbose_pkg_lists.to_string(), None),
            ("ILoveCandy", &config.chomp.to_string(), None)
        ]);

        dialog.add_group("Signature Levels", &[
            ("SigLevel", &config.sig_level.join(" | "), None),
            ("LocalFileSigLevel", &config.local_file_sig_level.join(" | "), None),
            ("RemoteFileSigLevel", &config.remote_file_sig_level.join(" | "), None)
        ]);

        dialog
    }

    //---------------------------------------
    // Add group helper function
    //---------------------------------------
    fn add_group(&self, title: &str, rows: &[(&str, &str, Option<&str>)]) {
        let group = adw::PreferencesGroup::builder()
            .title(title)
            .build();

        for &(label, property, action_name) in rows {
            group.add(&ConfigRow::new(label, property, action_name));
        }

        self.imp().options_page.add(&group);
    }
}

impl Default for ConfigDialog {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
