use std::fs;

use gtk::{gio, glib};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use fancy_regex::Regex;
use glob::glob;

use crate::pkg_object::{PkgObject, PkgFlags};
use crate::backup_object::BackupObject;
use crate::utils::Utils;

//------------------------------------------------------------------------------
// MODULE: DetailsWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/details_window.ui")]
    pub struct DetailsWindow {
        #[template_child]
        pub pkg_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub content_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub files_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub log_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub cache_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub backup_button: TemplateChild<gtk::ToggleButton>,

        #[template_child]
        pub files_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub files_search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub files_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub files_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub files_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub files_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub files_selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub files_filter: TemplateChild<gtk::StringFilter>,

        #[template_child]
        pub log_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub log_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub log_selection: TemplateChild<gtk::NoSelection>,

        #[template_child]
        pub cache_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub cache_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub cache_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub cache_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub cache_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub cache_selection: TemplateChild<gtk::SingleSelection>,

        #[template_child]
        pub backup_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub backup_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub backup_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub backup_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub backup_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub backup_selection: TemplateChild<gtk::SingleSelection>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for DetailsWindow {
        const NAME: &'static str = "DetailsWindow";
        type Type = super::DetailsWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            BackupObject::ensure_type();

            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for DetailsWindow {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
            obj.setup_actions();
            obj.setup_shortcuts();
        }
    }

    impl WidgetImpl for DetailsWindow {}
    impl WindowImpl for DetailsWindow {}
    impl ApplicationWindowImpl for DetailsWindow {}
    impl AdwApplicationWindowImpl for DetailsWindow {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: DetailsWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct DetailsWindow(ObjectSubclass<imp::DetailsWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl DetailsWindow {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(parent: &impl IsA<gtk::Window>, pkg: &PkgObject, pacman_config: &pacmanconf::Config, pkg_model: &gio::ListStore) -> Self {
        let win: Self = glib::Object::builder()
            .property("transient-for", parent)
            .build();

        win.update_ui_banner(pkg);
        win.update_ui_stack(pkg.flags().intersects(PkgFlags::INSTALLED));

        win.update_ui_files_page(pkg);
        win.update_ui_logs_page(pkg, &pacman_config.log_file);
        win.update_ui_cache_page(pkg, &pacman_config.cache_dir, pkg_model);
        win.update_ui_backup_page(pkg);

        win
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Files search entry search changed signal
        imp.files_search_entry.connect_search_changed(clone!(@weak imp => move |entry| {
            imp.files_filter.set_search(Some(&entry.text()));
        }));

        // Files open button clicked signal
        imp.files_open_button.connect_clicked(clone!(@weak imp => move |_| {
            let item = imp.files_selection.selected_item()
                .and_downcast::<gtk::StringObject>()
                .expect("Could not downcast to 'StringObject'");

            Utils::open_file_manager(&item.string());
        }));

        // Files copy button clicked signal
        imp.files_copy_button.connect_clicked(clone!(@weak self as window, @weak imp => move |_| {
            let copy_text = imp.files_selection.iter::<glib::Object>().flatten()
                .map(|item| {
                    item
                        .downcast::<gtk::StringObject>()
                        .expect("Could not downcast to 'StringObject'")
                        .string()
                })
                .collect::<Vec<glib::GString>>()
                .join("\n");

            window.clipboard().set_text(&copy_text);
        }));

        // Files listview activate signal
        imp.files_view.connect_activate(clone!(@weak imp => move |_, _| {
            imp.files_open_button.emit_clicked();
        }));

        // Log copy button clicked signal
        imp.log_copy_button.connect_clicked(clone!(@weak self as window, @weak imp => move |_| {
            let copy_text = imp.log_model.iter::<gtk::StringObject>().flatten()
                .map(|item| item.string())
                .collect::<Vec<glib::GString>>()
                .join("\n");

            window.clipboard().set_text(&copy_text);
        }));

        // Cache open button clicked signal
        imp.cache_open_button.connect_clicked(clone!(@weak imp => move |_| {
            let item = imp.cache_selection.selected_item()
                .and_downcast::<gtk::StringObject>()
                .expect("Could not downcast to 'StringObject'");

            Utils::open_file_manager(&item.string());
        }));

        // Cache copy button clicked signal
        imp.cache_copy_button.connect_clicked(clone!(@weak self as window, @weak imp => move |_| {
            let copy_text = imp.cache_model.iter::<gtk::StringObject>().flatten()
                .map(|item| item.string())
                .collect::<Vec<glib::GString>>()
                .join("\n");

            window.clipboard().set_text(&copy_text);
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

            Utils::open_file_manager(&item.filename());
        }));

        // Backup copy button clicked signal
        imp.backup_copy_button.connect_clicked(clone!(@weak self as window, @weak imp => move |_| {
            let copy_text = imp.backup_model.iter::<BackupObject>().flatten()
                .map(|item| {
                    format!("{filename} ({status})", filename=item.filename(), status=item.status_text())
                })
                .collect::<Vec<String>>()
                .join("\n");

            window.clipboard().set_text(&copy_text);
        }));

        // Backup listview activate signal
        imp.backup_view.connect_activate(clone!(@weak imp => move |_, _| {
            imp.backup_open_button.emit_clicked();
        }));
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        // Add set tab action
        let tab_action = gio::ActionEntry::builder("set-tab")
            .parameter_type(Some(&str::static_variant_type()))
            .state("none".to_variant())
            .change_state(|window: &Self, action, state| {
                let state = state
                    .expect("Could not retrieve Variant");

                let state_str = state
                    .get::<String>()
                    .expect("Could not retrieve String from variant");

                action.set_state(state);

                window.imp().content_stack.set_visible_child_name(&state_str);
                
            })
            .build();

        // Add actions to window
        self.add_action_entries([tab_action]);
    }

    //-----------------------------------
    // Setup shortcuts
    //-----------------------------------
    fn setup_shortcuts(&self) {
        // Create shortcut controller
        let controller = gtk::ShortcutController::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);

        // Add close window shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::CallbackAction::new(|widget, _| {
                let window = widget
                    .downcast_ref::<DetailsWindow>()
                    .expect("Could not downcast to 'DetailsWindow'");

                window.close();

                glib::Propagation::Proceed
            }))
        ));

        // Add shortcut controller to window
        self.add_controller(controller);
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

        if installed {
            imp.files_button.emit_clicked();
        }

        imp.files_button.set_sensitive(installed);
        imp.log_button.set_sensitive(installed);
        imp.cache_button.set_sensitive(installed);
        imp.backup_button.set_sensitive(installed);
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
        if let Ok(log) = fs::read_to_string(log_file) {
            let expr = Regex::new(&format!("\\[(.+?)T(.+?)\\+.+?\\] \\[ALPM\\] (installed|removed|upgraded|downgraded) ({}) (.+)", pkg.name())).expect("Regex error");

            let log_lines: Vec<gtk::StringObject> = log.lines().rev()
                .filter_map(|s| {
                    let is_match = expr.is_match(s);

                    if is_match.is_ok_and(|is_match| is_match) {
                        Some(gtk::StringObject::new(&expr.replace(s, "[$1  $2] : $3 $4 $5")))
                    } else {
                        None
                    }
                })
                .collect();

            imp.log_model.extend_from_slice(&log_lines);
        }

        // Set copy button state
        let n_items = imp.log_model.n_items();

        imp.log_copy_button.set_sensitive(n_items > 0);
    }

    //-----------------------------------
    // Update cache page
    //-----------------------------------
    fn update_ui_cache_page(&self, pkg: &PkgObject, cache_dirs: &[String], pkg_model: &gio::ListStore) {
        let imp = self.imp();

        let pkg_name = &pkg.name();

        // Get blacklist package names
        let blacklist: Vec<String> = pkg_model.iter::<PkgObject>().flatten()
            .map(|pkg| pkg.name())
            .filter(|name| {
                name.starts_with(pkg_name) &&
                name != pkg_name
            })
            .collect();

        // Populate cache files list
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
            }
        }

        // Set cache header label
        let n_items = imp.cache_model.n_items();

        imp.cache_header_label.set_label(&format!("Cache Files ({n_items})"));

        // Set open/copy button states
        imp.cache_open_button.set_sensitive(n_items > 0);
        imp.cache_copy_button.set_sensitive(n_items > 0);
    }

    //-----------------------------------
    // Update backup page
    //-----------------------------------
    fn update_ui_backup_page(&self, pkg: &PkgObject) {
        let imp = self.imp();

        // Populate backup list
        let backup_list: Vec<BackupObject> = pkg.backup().iter()
            .map(|(filename, hash)| BackupObject::new(filename, hash, None))
            .collect();

        imp.backup_model.extend_from_slice(&backup_list);

        // Set backup header label
        let n_items = imp.backup_model.n_items();

        imp.backup_header_label.set_label(&format!("Backup Files ({n_items})"));

        // Set open/copy button states
        imp.backup_open_button.set_sensitive(n_items > 0);
        imp.backup_copy_button.set_sensitive(n_items > 0);
    }
}
