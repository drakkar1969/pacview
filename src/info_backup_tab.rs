use std::cell::RefCell;
use std::fmt::Write as _;

use gtk::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{glib, gio};
use glib::clone;

use crate::window::{PACCAT_PATH, MELD_PATH};
use crate::pkg_object::PkgObject;
use crate::backup_object::{BackupObject, BackupStatus};
use crate::utils::app_info;

//------------------------------------------------------------------------------
// MODULE: InfoBackupTab
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::InfoBackupTab)]
    #[template(resource = "/com/github/PacView/ui/info_backup_tab.ui")]
    pub struct InfoBackupTab {
        #[template_child]
        pub(super) header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) compare_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) compare_image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) paused_status: TemplateChild<adw::StatusPage>,

        #[property(get, set)]
        pkg_name: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for InfoBackupTab {
        const NAME: &'static str = "InfoBackupTab";
        type Type = super::InfoBackupTab;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for InfoBackupTab {
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
    impl WidgetImpl for InfoBackupTab {}
    impl BoxImpl for InfoBackupTab {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: InfoBackupTab
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct InfoBackupTab(ObjectSubclass<imp::InfoBackupTab>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl InfoBackupTab {
    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Compare button clicked signal
        imp.compare_button.connect_clicked(clone!(
            #[weak] imp,
            move |button| {
                let spinner = adw::SpinnerPaintable::new(Some(button));

                imp.compare_image.set_paintable(Some(&spinner));

                let item = imp.selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .expect("Failed to downcast to 'BackupObject'");

                glib::spawn_future_local(
                    async move {
                        let _ = item.compare_with_original().await;

                        imp.compare_image.set_icon_name(Some("info-compare-symbolic"));
                    }
                );
            }
        ));

        // Open button clicked signal
        imp.open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let backup_file = imp.selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .expect("Failed to downcast to 'BackupObject'")
                    .filename();

                glib::spawn_future_local(async move {
                    app_info::open_with_default_app(&backup_file).await;
                });
            }
        ));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = tab)] self,
            move |_| {
                let mut output = String::new();

                let _ = writeln!(output, "## {}\n|Backup Files|Status|\n|---|---|", tab.pkg_name());

                for obj in tab.imp().model.iter::<BackupObject>()
                    .flatten() {
                        let _ = writeln!(output, "{}|{}", obj.filename(), obj.status_text());
                    }

                tab.clipboard().set_text(&output);
            }
        ));

        // View activate signal
        imp.view.connect_activate(clone!(
            #[weak] imp,
            move |_, _| {
                if imp.open_button.is_sensitive() {
                    imp.open_button.emit_clicked();
                }
            }
        ));

        // Selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.count_label.set_label(&n_items.to_string());
                imp.compare_button.set_sensitive(n_items > 0);
                imp.open_button.set_sensitive(n_items > 0);
                imp.copy_button.set_sensitive(n_items > 0);
            }
        ));

        // Selection selected item property notify signal
        imp.selection.connect_selected_item_notify(clone!(
            #[weak] imp,
            move |selection| {
                let status = selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .map_or(BackupStatus::Locked, |backup| backup.status());

                imp.compare_button.set_sensitive(
                    imp.compare_button.is_visible() && status == BackupStatus::Modified
                );

                imp.open_button.set_sensitive(
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

        // Set compare button visibility
        imp.compare_button.set_visible(PACCAT_PATH.is_ok() && MELD_PATH.is_ok());
    }

    //---------------------------------------
    // Pause view function
    //---------------------------------------
    pub fn pause_view(&self) {
        let imp = self.imp();

        imp.paused_status.set_visible(true);
        imp.model.remove_all();
    }

    //---------------------------------------
    // Update view function
    //---------------------------------------
    pub fn update_view(&self, pkg: &PkgObject) {
        let imp = self.imp();

        self.set_pkg_name(pkg.name());

        imp.paused_status.set_visible(false);

        // Populate view
        let backup_list: Vec<BackupObject> = pkg.backup().iter()
            .map(BackupObject::new)
            .collect();

        imp.model.splice(0, imp.model.n_items(), &backup_list);
    }
}
