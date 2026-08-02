use std::cell::{Cell, RefCell};

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use adw::prelude::*;

use crate::utils::Pacman;

//------------------------------------------------------------------------------
// MODULE: RootDirDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::RootDirDialog)]
    #[template(resource = "/com/github/PacView/ui/rootdir_dialog.ui")]
    pub struct RootDirDialog {
        #[template_child]
        pub(super) body_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) rootdir_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) rootdir_reset_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) defaultconfig_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) configpath_row: TemplateChild<adw::ActionRow>,

        #[property(get, set)]
        root_dir: RefCell<String>,
        #[property(get, set)]
        default_config: Cell<bool>,
        #[property(get, set)]
        config_path: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for RootDirDialog {
        const NAME: &'static str = "RootDirDialog";
        type Type = super::RootDirDialog;
        type ParentType = adw::AlertDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            // Install actions
            Self::install_actions(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for RootDirDialog {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
            obj.setup_widgets();
        }
    }

    impl WidgetImpl for RootDirDialog {}
    impl AdwDialogImpl for RootDirDialog {}
    impl AdwAlertDialogImpl for RootDirDialog {}

    impl RootDirDialog {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Select root dir action
            klass.install_action_async("dialog.select-rootdir", None, async |dialog, _, _| {
                let folder_dialog = gtk::FileDialog::builder()
                    .initial_folder(&gio::File::for_path(dialog.root_dir()))
                    .build();

                let parent = dialog.root().and_downcast::<gtk::Window>();

                if let Ok(Some(path)) = folder_dialog.select_folder_future(parent.as_ref()).await
                    .map(|folder| folder.path()) {
                        let imp = dialog.imp();

                        let root_dir = path.display().to_string();

                        imp.rootdir_reset_box.set_sensitive(root_dir != "/");

                        dialog.set_root_dir(root_dir);

                        if dialog.default_config() {
                            let config_path = Pacman::default_config_path(&dialog.root_dir());

                            dialog.set_config_path(config_path);
                        }
                    }
            });

            // Reset root dir action
            klass.install_action("dialog.reset-rootdir", None, |dialog, _, _| {
                let imp = dialog.imp();

                dialog.set_root_dir("/");

                imp.rootdir_reset_box.set_sensitive(false);
                imp.rootdir_row.grab_focus();

                if dialog.default_config() {
                    dialog.set_config_path("/etc/pacman.conf");
                }
            });

            // Select config_path action
            klass.install_action_async("dialog.select-configpath", None, async |dialog, _, _| {
                let filters = gio::ListStore::new::<gtk::FileFilter>();

                let conf_filter = gtk::FileFilter::new();
                conf_filter.set_name(Some("Conf Files"));
                conf_filter.add_suffix("conf");

                filters.append(&conf_filter);

                let file_dialog = gtk::FileDialog::builder()
                    .filters(&filters)
                    .default_filter(&conf_filter)
                    .initial_file(&gio::File::for_path(dialog.config_path()))
                    .build();

                let parent = dialog.root().and_downcast::<gtk::Window>();

                if let Ok(Some(path)) = file_dialog.open_future(parent.as_ref()).await
                    .map(|file| file.path()) {
                        dialog.set_config_path(path.display().to_string());
                    }
            });
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: RootDirDialog
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct RootDirDialog(ObjectSubclass<imp::RootDirDialog>)
    @extends adw::AlertDialog, adw::Dialog, gtk::Widget,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl RootDirDialog {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(root_dir: &str, config_path: &str) -> Self {
        let default_config_path = Pacman::default_config_path(root_dir);

        glib::Object::builder()
            .property("root-dir", root_dir)
            .property("default-config", config_path == default_config_path)
            .property("config-path", config_path)
            .build()
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        // Default config property notify signal
        self.connect_default_config_notify(|dialog| {
            let imp = dialog.imp();

            let default = dialog.default_config();

            if default {
                let config_path = Pacman::default_config_path(&dialog.root_dir());

                dialog.set_config_path(config_path);
            }

            imp.configpath_row.set_sensitive(!default);
            imp.configpath_row.set_action_name((!default).then_some("dialog.select-configpath"));
        });
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind properties to widgets
        self.bind_property("root-dir", &imp.rootdir_row.get(), "title")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("default-config", &imp.defaultconfig_row.get(), "active")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("config-path", &imp.configpath_row.get(), "title")
            .sync_create()
            .bidirectional()
            .build();

        // Set body label text
        imp.body_label.set_markup("Set the default root directory for pacman. This option is used if you want to manage packages on a temporary mounted partition which is 'owned' by another system, or for a chroot install.\n\n<b>NOTE:</b> If database path or log file are not specified on either the command line or in pacman.conf(5), their default location will be inside this root path.");
    }
}
