use gtk::glib;
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::VariantTy;

use crate::{
    config_row::ConfigRow,
    utils::AppInfoExt
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
        pub(super) config_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) paths_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub(super) download_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub(super) sandbox_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub(super) packages_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub(super) misc_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub(super) siglevel_group: TemplateChild<adw::PreferencesGroup>,
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
                AppInfoExt::open_with_default_app("/etc/pacman.conf").await;
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
    // Add row helper function
    //---------------------------------------
    fn add_row(&self, group: &adw::PreferencesGroup, label: &str, property: &str, action_name: Option<&str>) {
        group.add(&ConfigRow::new(label, property, action_name));
    }

    //---------------------------------------
    // Init dialog
    //---------------------------------------
    pub fn init(&self, config: &pacmanconf::Config) {
        let imp = self.imp();

        // Add config rows
        let mut group = &imp.paths_group;

        self.add_row(group, "RootDir", &config.root_dir, Some("conf.path"));
        self.add_row(group, "DBPath", &config.db_path, Some("conf.path"));
        self.add_row(group, "CacheDir", &config.cache_dir.join("\n"), Some("conf.path"));
        self.add_row(group, "LogFile", &config.log_file, Some("conf.path"));
        self.add_row(group, "GPGDir", &config.gpg_dir, Some("conf.path"));
        self.add_row(group, "HookDir", &config.hook_dir.join("\n"), Some("conf.path"));

        group = &imp.download_group;

        self.add_row(group, "XferCommand", &config.xfer_command, None);
        self.add_row(group, "ParallelDownloads", &config.parallel_downloads.to_string(), None);
        self.add_row(group, "DisableDownloadTimeout", &config.disable_download_timeout.to_string(), None);
        self.add_row(group, "DownloadUser", &config.download_user.clone().unwrap_or_else(|| String::from("None")), None);
        self.add_row(group, "Architecture", &config.architecture.join(" | "), None);

        group = &imp.sandbox_group;

        self.add_row(group, "DisableSandBox", &config.disable_sandbox.to_string(), None);
        self.add_row(group, "DisableSandBoxFilesystem", &config.disable_sandbox_filesystem.to_string(), None);
        self.add_row(group, "DisableSandBoxSyscalls", &config.disable_sandbox_syscalls.to_string(), None);

        group = &imp.packages_group;

        self.add_row(group, "HoldPkg", &config.hold_pkg.join(" | "), None);
        self.add_row(group, "IgnorePkg", &config.ignore_pkg.join(" | "), None);
        self.add_row(group, "IgnoreGroup", &config.ignore_group.join(" | "), None);
        self.add_row(group, "NoUpgrade", &config.no_upgrade.join(" | "), None);
        self.add_row(group, "NoExtract", &config.no_extract.join(" | "), None);

        group = &imp.misc_group;

        self.add_row(group, "UseSyslog", &config.use_syslog.to_string(), None);
        self.add_row(group, "Color", &config.color.to_string(), None);
        self.add_row(group, "CheckSpace", &config.check_space.to_string(), None);
        self.add_row(group, "CleanMethod", &config.clean_method.join(" | "), None);
        self.add_row(group, "VerbosePkgLists", &config.verbose_pkg_lists.to_string(), None);
        self.add_row(group, "ILoveCandy", &config.chomp.to_string(), None);

        group = &imp.siglevel_group;

        self.add_row(group, "SigLevel", &config.sig_level.join(" | "), None);
        self.add_row(group, "LocalFileSigLevel", &config.local_file_sig_level.join(" | "), None);
        self.add_row(group, "RemoteFileSigLevel", &config.remote_file_sig_level.join(" | "), None);
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
