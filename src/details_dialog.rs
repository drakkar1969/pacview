use std::fs;

use gtk::{gio, glib};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use adw::prelude::AdwDialogExt;

use regex::Regex;
use glob::glob;

use crate::pkg_object::{PkgObject, PkgFlags};
use crate::backup_object::BackupObject;
use crate::utils::open_file_manager;

//------------------------------------------------------------------------------
// MODULE: DetailsDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/details_dialog.ui")]
    pub struct DetailsDialog {
        #[template_child]
        pub(super) pkg_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub(super) content_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) files_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) log_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) cache_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) backup_button: TemplateChild<gtk::ToggleButton>,

        #[template_child]
        pub(super) files_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) files_search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) files_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) files_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) files_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) files_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) files_selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) files_filter: TemplateChild<gtk::StringFilter>,

        #[template_child]
        pub(super) log_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) log_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) log_selection: TemplateChild<gtk::NoSelection>,
        #[template_child]
        pub(super) log_overlay_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub(super) cache_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) cache_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) cache_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) cache_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) cache_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) cache_selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) cache_overlay_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub(super) backup_header_label: TemplateChild<gtk::Label>,
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
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for DetailsDialog {
        const NAME: &'static str = "DetailsDialog";
        type Type = super::DetailsDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            BackupObject::ensure_type();

            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for DetailsDialog {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
        }
    }

    impl WidgetImpl for DetailsDialog {}
    impl AdwDialogImpl for DetailsDialog {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: DetailsDialog
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct DetailsDialog(ObjectSubclass<imp::DetailsDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl DetailsDialog {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn activate_tab_button(&self, button: &gtk::ToggleButton) {
        if button.is_active() {
            let content = button.child()
                .and_downcast::<adw::ButtonContent>()
                .expect("Could not downcast to 'ButtonContent'");

            self.imp().content_stack.set_visible_child_name(&content.label().to_lowercase());
        }
    }

    fn setup_signals(&self) {
        let imp = self.imp();

        // Tab buttons toggled signals
        imp.files_button.connect_toggled(clone!(@weak self as dialog => move |button| {
            dialog.activate_tab_button(button);
        }));

        imp.log_button.connect_toggled(clone!(@weak self as dialog => move |button| {
            dialog.activate_tab_button(button);
        }));

        imp.cache_button.connect_toggled(clone!(@weak self as dialog => move |button| {
            dialog.activate_tab_button(button);
        }));

        imp.backup_button.connect_toggled(clone!(@weak self as dialog => move |button| {
            dialog.activate_tab_button(button);
        }));

        // Files search entry search changed signal
        imp.files_search_entry.connect_search_changed(clone!(@weak imp => move |entry| {
            imp.files_filter.set_search(Some(&entry.text()));
        }));

        // Files open button clicked signal
        imp.files_open_button.connect_clicked(clone!(@weak imp => move |_| {
            let item = imp.files_selection.selected_item()
                .and_downcast::<gtk::StringObject>()
                .expect("Could not downcast to 'StringObject'");

            open_file_manager(&item.string());
        }));

        // Files copy button clicked signal
        imp.files_copy_button.connect_clicked(clone!(@weak self as dialog, @weak imp => move |_| {
            let copy_text = imp.files_selection.iter::<glib::Object>().flatten()
                .map(|item| {
                    item
                        .downcast::<gtk::StringObject>()
                        .expect("Could not downcast to 'StringObject'")
                        .string()
                })
                .collect::<Vec<glib::GString>>()
                .join("\n");

            dialog.clipboard().set_text(&copy_text);
        }));

        // Files listview activate signal
        imp.files_view.connect_activate(clone!(@weak imp => move |_, _| {
            imp.files_open_button.emit_clicked();
        }));

        // Log copy button clicked signal
        imp.log_copy_button.connect_clicked(clone!(@weak self as dialog, @weak imp => move |_| {
            let copy_text = imp.log_model.iter::<gtk::StringObject>().flatten()
                .map(|item| item.string())
                .collect::<Vec<glib::GString>>()
                .join("\n");

            dialog.clipboard().set_text(&copy_text);
        }));

        // Cache open button clicked signal
        imp.cache_open_button.connect_clicked(clone!(@weak imp => move |_| {
            let item = imp.cache_selection.selected_item()
                .and_downcast::<gtk::StringObject>()
                .expect("Could not downcast to 'StringObject'");

            open_file_manager(&item.string());
        }));

        // Cache copy button clicked signal
        imp.cache_copy_button.connect_clicked(clone!(@weak self as dialog, @weak imp => move |_| {
            let copy_text = imp.cache_model.iter::<gtk::StringObject>().flatten()
                .map(|item| item.string())
                .collect::<Vec<glib::GString>>()
                .join("\n");

            dialog.clipboard().set_text(&copy_text);
        }));

        // Cache listview activate signal
        imp.cache_view.connect_activate(clone!(@weak imp => move |_, _| {
            imp.cache_open_button.emit_clicked();
        }));

        // Backup open button clicked signal
        imp.backup_open_button.connect_clicked(clone!(@weak imp => move |_| {
            let item = imp.backup_selection.selected_item()
                .and_downcast::<BackupObject>()
                .expect("Could not downcast to 'BackupObject'");

            open_file_manager(&item.filename());
        }));

        // Backup copy button clicked signal
        imp.backup_copy_button.connect_clicked(clone!(@weak self as dialog, @weak imp => move |_| {
            let copy_text = imp.backup_model.iter::<BackupObject>().flatten()
                .map(|item| {
                    format!("{filename} ({status})", filename=item.filename(), status=item.status_text())
                })
                .collect::<Vec<String>>()
                .join("\n");

            dialog.clipboard().set_text(&copy_text);
        }));

        // Backup listview activate signal
        imp.backup_view.connect_activate(clone!(@weak imp => move |_, _| {
            imp.backup_open_button.emit_clicked();
        }));
    }

    //-----------------------------------
    // Update banner
    //-----------------------------------
    fn update_ui_banner(&self, pkg: &PkgObject) {
        // Set package name in banner
        self.imp().pkg_label.set_label(&format!("{repo}/{name}", repo=pkg.repository(), name=pkg.name()));
    }

    //-----------------------------------
    // Update stack
    //-----------------------------------
    fn update_ui_stack(&self, installed: bool) {
        let imp = self.imp();

        imp.files_button.set_sensitive(installed);
        imp.log_button.set_sensitive(installed);
        imp.cache_button.set_sensitive(installed);
        imp.backup_button.set_sensitive(installed);

        if installed {
            imp.files_button.set_active(true);
        }
    }

    //-----------------------------------
    // Update files page
    //-----------------------------------
    fn update_ui_files_page(&self, pkg: &PkgObject) {
        let imp = self.imp();

        // Set files search entry key capture widget
        imp.files_search_entry.set_key_capture_widget(Some(&imp.files_view.get()));

        // Populate files list
        let files_list: Vec<gtk::StringObject> = pkg.files().iter()
            .map(|s| gtk::StringObject::new(s))
            .collect();

        imp.files_model.extend_from_slice(&files_list);

        // Bind files count to files header label
        let files_len = files_list.len();

        imp.files_selection.bind_property("n-items", &imp.files_header_label.get(), "label")
            .transform_to(move |_, n_items: u32| Some(format!("Files ({n_items} of {files_len})")))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Bind files count to files open/copy button states
        imp.files_selection.bind_property("n-items", &imp.files_open_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        imp.files_selection.bind_property("n-items", &imp.files_copy_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
    }

    //-----------------------------------
    // Update logs page
    //-----------------------------------
    fn update_ui_logs_page(&self, pkg: &PkgObject, log_file: &str) {
        let imp = self.imp();

        // Populate log messages
        let mut log_error = false;

        if let Ok(log) = fs::read_to_string(log_file) {
            let expr = Regex::new(&format!("\\[(.+?)T(.+?)\\+.+?\\] \\[ALPM\\] (installed|removed|upgraded|downgraded) ({}) (.+)", pkg.name())).expect("Regex error");

            let log_lines: Vec<gtk::StringObject> = log.lines().rev()
                .filter_map(|s| {
                    if expr.is_match(s) {
                        Some(gtk::StringObject::new(&expr.replace(s, "[$1  $2] : $3 $4 $5")))
                    } else {
                        None
                    }
                })
                .collect();

            imp.log_model.extend_from_slice(&log_lines);
        } else {
            log_error = true;
        }

        // Set copy button state
        let n_items = imp.log_model.n_items();

        imp.log_copy_button.set_sensitive(n_items > 0);

        // Set overlay label visibility
        imp.log_overlay_label.set_visible(log_error);
    }

    //-----------------------------------
    // Update cache page
    //-----------------------------------
    fn update_ui_cache_page(&self, pkg: &PkgObject, cache_dirs: &[String], pkg_snapshot: &[PkgObject]) {
        let imp = self.imp();

        let pkg_name = &pkg.name();

        // Get blacklist package names
        let blacklist: Vec<String> = pkg_snapshot.iter()
            .map(|pkg| pkg.name())
            .filter(|name| {
                name.starts_with(pkg_name) &&
                name != pkg_name
            })
            .collect();

        // Populate cache files list
        let mut cache_error = false;

        for dir in cache_dirs {
            if let Ok(paths) = glob(&format!("{dir}{pkg_name}*.zst")) {
                // Find cache files that include package name
                let cache_list: Vec<gtk::StringObject> = paths
                    .flatten()
                    .filter_map(|path| {
                        let cache_file = path.display().to_string();

                        // Exclude cache files that include blacklist package names
                        if blacklist.iter().any(|s| cache_file.contains(s)) {
                            None
                        } else {
                            Some(gtk::StringObject::new(&cache_file))
                        }
                    })
                    .collect();

                imp.cache_model.extend_from_slice(&cache_list);
            } else {
                cache_error = true;
            }
        }

        // Set cache header label
        let n_items = imp.cache_model.n_items();

        imp.cache_header_label.set_label(&format!("Cache Files ({n_items})"));

        // Set open/copy button states
        imp.cache_open_button.set_sensitive(n_items > 0);
        imp.cache_copy_button.set_sensitive(n_items > 0);

        // Set overlay label visibility
        imp.cache_overlay_label.set_visible(cache_error);
    }

    //-----------------------------------
    // Update backup page
    //-----------------------------------
    fn update_ui_backup_page(&self, pkg: &PkgObject) {
        let imp = self.imp();

        // Populate backup list
        let backup_list: Vec<BackupObject> = pkg.backup().iter()
            .map(|(filename, hash)| BackupObject::new(filename, hash, None, alpm::compute_md5sum(filename.as_str()).ok().as_deref()))
            .collect();

        imp.backup_model.extend_from_slice(&backup_list);

        // Set backup header label
        let n_items = imp.backup_model.n_items();

        imp.backup_header_label.set_label(&format!("Backup Files ({n_items})"));

        // Set open/copy button states
        imp.backup_open_button.set_sensitive(n_items > 0);
        imp.backup_copy_button.set_sensitive(n_items > 0);
    }

    //-----------------------------------
    // Public show function
    //-----------------------------------
    pub fn show(&self, parent: &impl IsA<gtk::Widget>, pkg: &PkgObject, log_file: &str, cache_dirs: &[String], pkg_snapshot: &[PkgObject]) {
        self.update_ui_banner(pkg);
        self.update_ui_stack(pkg.flags().intersects(PkgFlags::INSTALLED));

        self.update_ui_files_page(pkg);
        self.update_ui_logs_page(pkg, log_file);
        self.update_ui_cache_page(pkg, cache_dirs, pkg_snapshot);
        self.update_ui_backup_page(pkg);

        self.present(parent);
    }
}

impl Default for DetailsDialog {
    //-----------------------------------
    // Default constructor
    //-----------------------------------
    fn default() -> Self {
        Self::new()
    }
}
