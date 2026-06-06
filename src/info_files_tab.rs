use std::cell::{Cell, RefCell};
use std::fmt::Write as _;

use gtk::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{glib, gio};
use glib::{clone, Propagation};

use crate::{
    pkg_object::PkgObject,
    backup_object::{BackupObject, BackupStatus},
    utils::{Paths, AppInfoExt}
};

//------------------------------------------------------------------------------
// MODULE: InfoFilesTab
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::InfoFilesTab)]
    #[template(resource = "/com/github/PacView/ui/info_files_tab.ui")]
    pub struct InfoFilesTab {
        #[template_child]
        pub(super) files_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) files_count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) files_search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) files_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) files_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) files_filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub(super) files_selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) files_search_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) files_folder_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) files_spinner: TemplateChild<adw::Spinner>,

        #[template_child]
        pub(super) backup_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) backup_count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) backup_compare_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) backup_compare_image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) backup_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) backup_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) backup_selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) backup_spinner: TemplateChild<adw::Spinner>,

        #[property(get, set)]
        pkg_name: RefCell<String>,
        #[property(get, set)]
        files_show_folders: Cell<bool>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for InfoFilesTab {
        const NAME: &'static str = "InfoFilesTab";
        type Type = super::InfoFilesTab;
        type ParentType = gtk::Box;

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
    impl ObjectImpl for InfoFilesTab {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
            obj.setup_widgets();
        }
    }
    impl WidgetImpl for InfoFilesTab {}
    impl BoxImpl for InfoFilesTab {}

    impl InfoFilesTab {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Files show folders property action
            klass.install_property_action("info.files-show-folders", "files-show-folders");
            // Open file action
            klass.install_action_async("info.files-open", None, async |tab, _, _| {
                if let Some(file) = tab.imp().files_selection.selected_item()
                    .and_downcast::<gtk::StringObject>() {
                        AppInfoExt::open_with_default_app(&file.string()).await;
                    }
            });

            // Copy files action
            klass.install_action("info.files-copy", None, |tab, _, _| {
                let mut output = String::new();

                let _ = writeln!(output, "## {}\n|Files|\n|---|", tab.pkg_name());

                for obj in tab.imp().files_selection.iter::<glib::Object>()
                    .flatten()
                    .filter_map(|item| item.downcast::<gtk::StringObject>().ok()) {
                        let _ = writeln!(output, "{}", obj.string());
                    }

                tab.clipboard().set_text(&output);
            });

            // Compare backup file action
            klass.install_action_async("info.backup-compare", None, async |tab, _, _| {
                let imp = tab.imp();

                let spinner = adw::SpinnerPaintable::new(Some(&imp.backup_compare_button.get()));

                imp.backup_compare_image.set_paintable(Some(&spinner));

                if let Some(backup_file) = imp.backup_selection.selected_item()
                    .and_downcast::<BackupObject>() {
                        let _ = backup_file.compare_with_original().await;
                    }

                imp.backup_compare_image.set_icon_name(Some("info-compare-symbolic"));
            });

            // Open backup file action
            klass.install_action_async("info.backup-open", None, async |tab, _, _| {
                if let Some(backup_file) = tab.imp().backup_selection.selected_item()
                    .and_downcast::<BackupObject>() {
                        AppInfoExt::open_with_default_app(&backup_file.path()).await;
                    }
            });

            // Copy backup files action
            klass.install_action("info.backup-copy", None, |tab, _, _| {
                let mut output = String::new();

                let _ = writeln!(output, "## {}\n|Backup Files|Status|\n|---|---|", tab.pkg_name());

                for obj in tab.imp().backup_model.iter::<BackupObject>()
                    .flatten() {
                        let _ = writeln!(output, "{}|{}", obj.path(), obj.status_text());
                    }

                tab.clipboard().set_text(&output);
            });
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: InfoFilesTab
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct InfoFilesTab(ObjectSubclass<imp::InfoFilesTab>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl InfoFilesTab {
    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Files search entry search started signal
        imp.files_search_entry.connect_search_started(|entry| {
            if !entry.has_focus() {
                entry.grab_focus();
            }
        });

        // Files search entry search changed signal
        imp.files_search_entry.connect_search_changed(clone!(
            #[weak] imp,
            move |entry| {
                imp.files_search_filter.set_search(Some(&entry.text()));
            }
        ));

        // Files show folders property notify signal
        self.connect_files_show_folders_notify(|window| {
            window.imp().files_folder_filter.changed(gtk::FilterChange::Different);
        });

        // Files view activate signal
        imp.files_view.connect_activate(clone!(
            #[weak(rename_to = tab)] self,
            move |_, _| {
                tab.activate_action("info.files-open", None).unwrap();
            }
        ));

        // Files selection items changed signal
        imp.files_selection.connect_items_changed(clone!(
            #[weak(rename_to = tab)] self,
            move |selection, _, _, _| {
                let imp = tab.imp();

                let n_items = selection.n_items();

                imp.files_count_label.set_label(&n_items.to_string());

                tab.action_set_enabled("info.files-open", n_items > 0);
                tab.action_set_enabled("info.files-copy", n_items > 0);
            }
        ));

        // Backup view activate signal
        imp.backup_view.connect_activate(clone!(
            #[weak(rename_to = tab)] self,
            move |_, _| {
                tab.activate_action("info.backup-open", None).unwrap();
            }
        ));

        // Backup selection items changed signal
        imp.backup_selection.connect_items_changed(clone!(
            #[weak(rename_to = tab)] self,
            move |selection, _, _, _| {
                let imp = tab.imp();

                let n_items = selection.n_items();

                imp.backup_count_label.set_label(&n_items.to_string());

                tab.action_set_enabled("info.backup-compare", n_items > 0);
                tab.action_set_enabled("info.backup-open", n_items > 0);
                tab.action_set_enabled("info.backup-copy", n_items > 0);
            }
        ));

        // Backup selection selected item property notify signal
        imp.backup_selection.connect_selected_item_notify(clone!(
            #[weak(rename_to = tab)] self,
            move |selection| {
                let imp = tab.imp();

                let status = selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .map_or(BackupStatus::Locked, |backup| backup.status());

                tab.action_set_enabled("info.backup-compare", imp.backup_compare_button.is_visible() && status == BackupStatus::Modified);
                tab.action_set_enabled("info.backup-open", status != BackupStatus::Locked && status != BackupStatus::All);
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set files search entry key capture widget
        imp.files_search_entry.set_key_capture_widget(Some(&imp.files_view.get()));

        // Set files folder filter function
        imp.files_folder_filter.set_filter_func(clone!(
            #[weak(rename_to = tab)] self,
            #[upgrade_or] false,
            move |item| {
                if tab.files_show_folders() {
                    true
                } else {
                    let obj = item
                        .downcast_ref::<gtk::StringObject>()
                        .expect("Failed to downcast to 'StringObject'");

                    !obj.string().ends_with('/')
                }
            }
        ));

        // Add keyboard shortcut to cancel files search
        let shortcut = gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::CallbackAction::new(clone!(
                #[weak] imp,
                #[upgrade_or] Propagation::Proceed,
                move |_, _| {
                    imp.files_search_entry.set_text("");
                    imp.files_view.grab_focus();

                    Propagation::Stop
                }
            )))
        );

        let controller = gtk::ShortcutController::new();
        controller.add_shortcut(shortcut);

        imp.files_search_entry.add_controller(controller);

        // Set backup compare button visibility
        self.imp().backup_compare_button.set_visible(Paths::paccat().is_ok() && Paths::meld().is_ok());
    }

    //---------------------------------------
    // Pause view function
    //---------------------------------------
    pub fn pause_views(&self) {
        let imp = self.imp();

        imp.files_spinner.set_visible(true);
        imp.files_model.remove_all();

        imp.backup_spinner.set_visible(true);
        imp.backup_model.remove_all();
    }

    //---------------------------------------
    // Update view function
    //---------------------------------------
    pub fn update_views(&self, pkg: &PkgObject) {
        let imp = self.imp();

        let pkg_name = pkg.name();

        imp.files_spinner.set_visible(false);
        imp.backup_spinner.set_visible(false);

        // Populate files view
        let files_list: Vec<gtk::StringObject> = pkg.files().iter()
            .map(|file| gtk::StringObject::new(file))
            .collect();

        imp.files_model.splice(0, imp.files_model.n_items(), &files_list);

        self.set_pkg_name(pkg.name());

        // Populate backup view
        let backup_list: Vec<BackupObject> = pkg.backup().iter()
            .map(|backup| BackupObject::new(backup, &pkg_name))
            .collect();

        imp.backup_model.splice(0, imp.backup_model.n_items(), &backup_list);

        self.set_pkg_name(pkg_name);
    }
}
