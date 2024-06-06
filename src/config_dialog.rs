use gtk::glib;
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::clone;

use crate::utils::open_file_manager;

//------------------------------------------------------------------------------
// MODULE: ConfigDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/config_dialog.ui")]
    pub struct ConfigDialog {
        #[template_child]
        pub(super) rootdir_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) dbpath_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) cachedir_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) logfile_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) gpgdir_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) hookdir_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) xfercommand_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) cleanmethod_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) architecture_row: TemplateChild<adw::ActionRow>,

        #[template_child]
        pub(super) rootdir_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) dbpath_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) cachedir_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) logfile_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) gpgdir_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) hookdir_open_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) holdpkg_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) ignorepkg_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) ignoregroup_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) noupgrade_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) noextract_row: TemplateChild<adw::ActionRow>,

        #[template_child]
        pub(super) usesyslog_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) color_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) checkspace_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) verbosepkglists_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) paralleldownloads_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) totaldownload_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) disabledownloadtimeout_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) ilovecandy_row: TemplateChild<adw::ActionRow>,

        #[template_child]
        pub(super) siglevel_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) localfilesiglevel_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) remotefilesiglevel_row: TemplateChild<adw::ActionRow>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for ConfigDialog {
        const NAME: &'static str = "ConfigDialog";
        type Type = super::ConfigDialog;
        type ParentType = adw::PreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ConfigDialog {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
        }
    }

    impl WidgetImpl for ConfigDialog {}
    impl AdwDialogImpl for ConfigDialog {}
    impl PreferencesDialogImpl for ConfigDialog {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: ConfigDialog
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct ConfigDialog(ObjectSubclass<imp::ConfigDialog>)
    @extends adw::Dialog, gtk::Widget,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ConfigDialog {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder()
            .build()
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // RootDir open button clicked signal
        imp.rootdir_open_button.connect_clicked(clone!(@weak imp => move |_| {
            open_file_manager(&imp.rootdir_row.subtitle().unwrap_or_default());
        }));

        // DBPath open button clicked signal
        imp.dbpath_open_button.connect_clicked(clone!(@weak imp => move |_| {
            open_file_manager(&imp.dbpath_row.subtitle().unwrap_or_default());
        }));

        // CacheDir open button clicked signal
        imp.cachedir_open_button.connect_clicked(clone!(@weak imp => move |_| {
            for item in imp.cachedir_row.subtitle().unwrap_or_default().split('\n') {
                open_file_manager(item);
            }
        }));

        // LogFile open button clicked signal
        imp.logfile_open_button.connect_clicked(clone!(@weak imp => move |_| {
            open_file_manager(&imp.logfile_row.subtitle().unwrap_or_default());
        }));

        // GPGDir open button clicked signal
        imp.gpgdir_open_button.connect_clicked(clone!(@weak imp => move |_| {
            open_file_manager(&imp.gpgdir_row.subtitle().unwrap_or_default());
        }));

        // HookDir open button clicked signal
        imp.hookdir_open_button.connect_clicked(clone!(@weak imp => move |_| {
            for item in imp.hookdir_row.subtitle().unwrap_or_default().split('\n') {
                open_file_manager(item);
            }
        }));
    }

    //-----------------------------------
    // Init dialog
    //-----------------------------------
    pub fn init(&self, config: &pacmanconf::Config) {
        let imp = self.imp();

        // Populate config rows
        imp.rootdir_row.set_subtitle(&config.root_dir);
        imp.dbpath_row.set_subtitle(&config.db_path);
        imp.cachedir_row.set_subtitle(&config.cache_dir.join("\n"));
        imp.logfile_row.set_subtitle(&config.log_file);
        imp.gpgdir_row.set_subtitle(&config.gpg_dir);
        imp.hookdir_row.set_subtitle(&config.hook_dir.join("\n"));
        imp.xfercommand_row.set_subtitle(&config.xfer_command);
        imp.cleanmethod_row.set_subtitle(&config.clean_method.join(" | "));
        imp.architecture_row.set_subtitle(&config.architecture.join(" | "));

        imp.holdpkg_row.set_subtitle(&config.hold_pkg.join(" | "));
        imp.ignorepkg_row.set_subtitle(&config.ignore_pkg.join(" | "));
        imp.ignoregroup_row.set_subtitle(&config.ignore_group.join(" | "));
        imp.noupgrade_row.set_subtitle(&config.no_upgrade.join(" | "));
        imp.noextract_row.set_subtitle(&config.no_extract.join(" | "));

        imp.usesyslog_row.set_subtitle(&config.use_syslog.to_string());
        imp.color_row.set_subtitle(&config.color.to_string());
        imp.checkspace_row.set_subtitle(&config.check_space.to_string());
        imp.verbosepkglists_row.set_subtitle(&config.verbose_pkg_lists.to_string());
        imp.paralleldownloads_row.set_subtitle(&config.parallel_downloads.to_string());
        imp.totaldownload_row.set_subtitle(&config.total_download.to_string());
        imp.disabledownloadtimeout_row.set_subtitle(&config.disable_download_timeout.to_string());
        imp.ilovecandy_row.set_subtitle(&config.chomp.to_string());

        imp.siglevel_row.set_subtitle(&config.sig_level.join(" | "));
        imp.localfilesiglevel_row.set_subtitle(&config.local_file_sig_level.join(" | "));
        imp.remotefilesiglevel_row.set_subtitle(&config.remote_file_sig_level.join(" | "));

        // Set open button sensitivity
        imp.rootdir_open_button.set_sensitive(!config.root_dir.is_empty());
        imp.dbpath_open_button.set_sensitive(!config.db_path.is_empty());
        imp.cachedir_open_button.set_sensitive(!config.cache_dir.is_empty());
        imp.logfile_open_button.set_sensitive(!config.log_file.is_empty());
        imp.gpgdir_open_button.set_sensitive(!config.gpg_dir.is_empty());
        imp.hookdir_open_button.set_sensitive(!config.hook_dir.is_empty());
    }
}

impl Default for ConfigDialog {
    //-----------------------------------
    // Default constructor
    //-----------------------------------
    fn default() -> Self {
        Self::new()
    }
}
    