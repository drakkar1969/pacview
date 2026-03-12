use std::cell::RefCell;
use std::fmt::Write as _;

use gtk::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{glib, gio};
use glib::clone;

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
        pub(super) files_filter_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) files_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) files_copy_button: TemplateChild<gtk::Button>,
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
        pub(super) files_paused_status: TemplateChild<adw::StatusPage>,

        #[template_child]
        pub(super) backup_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) backup_count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) backup_compare_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) backup_compare_image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) backup_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) backup_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) backup_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) backup_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) backup_selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) backup_paused_status: TemplateChild<adw::StatusPage>,

        #[property(get, set)]
        pkg_name: RefCell<String>,
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

        // Files filter button toggled signal
        imp.files_filter_button.connect_toggled(clone!(
            #[weak] imp,
            move |_| {
                imp.files_folder_filter.changed(gtk::FilterChange::Different);
            }
        ));

        // Files ppen button clicked signal
        imp.files_open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let file = imp.files_selection.selected_item()
                    .and_downcast::<gtk::StringObject>()
                    .expect("Failed to downcast to 'StringObject'")
                    .string();

                glib::spawn_future_local(async move {
                    AppInfoExt::open_with_default_app(&file).await;
                });
            }
        ));

        // Files copy button clicked signal
        imp.files_copy_button.connect_clicked(clone!(
            #[weak(rename_to = tab)] self,
            move |_| {
                let mut output = String::new();

                let _ = writeln!(output, "## {}\n|Files|\n|---|", tab.pkg_name());

                for obj in tab.imp().files_selection.iter::<glib::Object>()
                    .flatten()
                    .filter_map(|item| item.downcast::<gtk::StringObject>().ok()) {
                        let _ = writeln!(output, "{}", obj.string());
                    }

                tab.clipboard().set_text(&output);
            }
        ));

        // Files view activate signal
        imp.files_view.connect_activate(clone!(
            #[weak] imp,
            move |_, _| {
                if imp.files_open_button.is_sensitive() {
                    imp.files_open_button.emit_clicked();
                }
            }
        ));

        // Files selection items changed signal
        imp.files_selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.files_count_label.set_label(&n_items.to_string());
                imp.files_open_button.set_sensitive(n_items > 0);
                imp.files_copy_button.set_sensitive(n_items > 0);
            }
        ));

        // Backup compare button clicked signal
        imp.backup_compare_button.connect_clicked(clone!(
            #[weak] imp,
            move |button| {
                let spinner = adw::SpinnerPaintable::new(Some(button));

                imp.backup_compare_image.set_paintable(Some(&spinner));

                let item = imp.backup_selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .expect("Failed to downcast to 'BackupObject'");

                glib::spawn_future_local(
                    async move {
                        let _ = item.compare_with_original().await;

                        imp.backup_compare_image.set_icon_name(Some("info-compare-symbolic"));
                    }
                );
            }
        ));

        // Backup open button clicked signal
        imp.backup_open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let backup_file = imp.backup_selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .expect("Failed to downcast to 'BackupObject'")
                    .path();

                glib::spawn_future_local(async move {
                    AppInfoExt::open_with_default_app(&backup_file).await;
                });
            }
        ));

        // Backup copy button clicked signal
        imp.backup_copy_button.connect_clicked(clone!(
            #[weak(rename_to = tab)] self,
            move |_| {
                let mut output = String::new();

                let _ = writeln!(output, "## {}\n|Backup Files|Status|\n|---|---|", tab.pkg_name());

                for obj in tab.imp().backup_model.iter::<BackupObject>()
                    .flatten() {
                        let _ = writeln!(output, "{}|{}", obj.path(), obj.status_text());
                    }

                tab.clipboard().set_text(&output);
            }
        ));

        // Backup view activate signal
        imp.backup_view.connect_activate(clone!(
            #[weak] imp,
            move |_, _| {
                if imp.backup_open_button.is_sensitive() {
                    imp.backup_open_button.emit_clicked();
                }
            }
        ));

        // Backup selection items changed signal
        imp.backup_selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.backup_count_label.set_label(&n_items.to_string());
                imp.backup_compare_button.set_sensitive(n_items > 0);
                imp.backup_open_button.set_sensitive(n_items > 0);
                imp.backup_copy_button.set_sensitive(n_items > 0);
            }
        ));

        // Backup selection selected item property notify signal
        imp.backup_selection.connect_selected_item_notify(clone!(
            #[weak] imp,
            move |selection| {
                let status = selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .map_or(BackupStatus::Locked, |backup| backup.status());

                imp.backup_compare_button.set_sensitive(
                    imp.backup_compare_button.is_visible() && status == BackupStatus::Modified
                );

                imp.backup_open_button.set_sensitive(
                    status != BackupStatus::Locked && status != BackupStatus::All
                );
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
            #[weak] imp,
            #[upgrade_or] false,
            move |item| {
                if imp.files_filter_button.is_active() {
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
                #[upgrade_or] glib::Propagation::Proceed,
                move |_, _| {
                    imp.files_search_entry.set_text("");
                    imp.files_view.grab_focus();

                    glib::Propagation::Stop
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

        imp.files_paused_status.set_visible(true);
        imp.files_model.remove_all();

        imp.backup_paused_status.set_visible(true);
        imp.backup_model.remove_all();
    }

    //---------------------------------------
    // Update view function
    //---------------------------------------
    pub fn update_views(&self, pkg: &PkgObject) {
        let imp = self.imp();

        let pkg_name = pkg.name();

        imp.files_paused_status.set_visible(false);
        imp.backup_paused_status.set_visible(false);

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
