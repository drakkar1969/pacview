use gtk::glib;
use adw::{subclass::prelude::*, prelude::ActionRowExt};
use gtk::prelude::*;

//------------------------------------------------------------------------------
// MODULE: ConfigWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/config_window.ui")]
    pub struct ConfigWindow {
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
    impl ObjectSubclass for ConfigWindow {
        const NAME: &'static str = "ConfigWindow";
        type Type = super::ConfigWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.add_shortcut(&gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string("Escape"),
                Some(gtk::CallbackAction::new(|widget, _| {
                    let window = widget
                        .downcast_ref::<crate::config_window::ConfigWindow>()
                        .expect("Could not downcast to 'ConfigWindow'");

                    window.close();

                    glib::Propagation::Proceed
                }))
            ))
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ConfigWindow {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            // let obj = self.obj();

            // obj.setup_widgets();
            // obj.setup_signals();
        }
    }

    impl WidgetImpl for ConfigWindow {}
    impl WindowImpl for ConfigWindow {}
    impl AdwWindowImpl for ConfigWindow {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: ConfigWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct ConfigWindow(ObjectSubclass<imp::ConfigWindow>)
    @extends adw::Window, gtk::Window, gtk::Widget,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl ConfigWindow {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(parent: &impl IsA<gtk::Window>) -> Self {
        glib::Object::builder()
            .property("transient-for", parent)
            .build()
    }

    // //-----------------------------------
    // // Setup widgets
    // //-----------------------------------
    // fn setup_widgets(&self) {
    //     let imp = self.imp();
    // }

    // //-----------------------------------
    // // Setup signals
    // //-----------------------------------
    // fn setup_signals(&self) {
    //     let imp = self.imp();
    // }

    //-----------------------------------
    // Show window
    //-----------------------------------
    pub fn show(&self, config: &pacmanconf::Config) {
        let imp = self.imp();

        self.present();

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
    }
}
