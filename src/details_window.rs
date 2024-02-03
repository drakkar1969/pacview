use std::cell::{RefCell, OnceCell};
use std::fs;
use std::collections::HashMap;

use gtk::{gio, glib};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use regex::Regex;
use glob::glob;

use crate::pkg_object::{PkgObject, PkgFlags};
use crate::toggle_button::ToggleButton;
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
        pub tree_button: TemplateChild<ToggleButton>,
        #[template_child]
        pub files_button: TemplateChild<ToggleButton>,
        #[template_child]
        pub log_button: TemplateChild<ToggleButton>,
        #[template_child]
        pub cache_button: TemplateChild<ToggleButton>,
        #[template_child]
        pub backup_button: TemplateChild<ToggleButton>,

        #[template_child]
        pub tree_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub tree_search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub tree_reverse_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub tree_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub tree_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub tree_filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub tree_selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub tree_name_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub tree_depth_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub tree_depth_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub tree_depth_scale: TemplateChild<gtk::Scale>,

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

        pub tree_model: OnceCell<gtk::TreeListModel>,
        pub tree_dep_map: RefCell<HashMap<String, Option<String>>>
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
            ToggleButton::ensure_type();
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
    pub fn new(parent: &gtk::Window, pkg: &PkgObject, pacman_config: &pacmanconf::Config, pkg_model: &gio::ListStore, aur_model: &gio::ListStore) -> Self {
        let win: Self = glib::Object::builder()
            .property("transient-for", parent)
            .build();

        let installed = pkg.flags().intersects(PkgFlags::INSTALLED);

        win.update_ui_banner(pkg);
        win.update_ui_stack(installed);

        win.update_ui_tree_page(pkg, pkg_model, aur_model);

        if installed {
            win.update_ui_files_page(pkg);
            win.update_ui_logs_page(pkg, &pacman_config.log_file);
            win.update_ui_cache_page(pkg, &pacman_config.cache_dir, pkg_model);
            win.update_ui_backup_page(pkg);
        }

        win
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Tree search entry search changed signal
        imp.tree_search_entry.connect_search_changed(clone!(@weak imp => move |entry| {
            imp.tree_name_filter.set_search(Some(&entry.text()));
        }));

        // Tree reverse button toggled signal
        imp.tree_reverse_button.connect_toggled(clone!(@weak imp => move |_| {
            let tree_model = imp.tree_model.get().unwrap();

            let root_model = tree_model.model()
                .downcast::<gio::ListStore>()
                .expect("Must be a 'ListStore'");

            if let Some(obj) = root_model.item(0)
                .and_downcast::<gtk::StringObject>()
            {
                imp.tree_dep_map.replace(HashMap::from([(obj.string().to_string(), None)]));

                root_model.splice(0, 1, &[obj]);
            }
        }));

        // Tree copy button clicked signal
        imp.tree_copy_button.connect_clicked(clone!(@weak self as window, @weak imp => move |_| {
            let copy_text = imp.tree_selection.iter::<glib::Object>().flatten()
                .map(|item| {
                    let row = item
                        .downcast::<gtk::TreeListRow>()
                        .expect("Must be a 'TreeListRow'");

                    let obj = row.item()
                        .and_downcast::<gtk::StringObject>()
                        .expect("Must be a 'StringObject'");

                    format!("{}{}{}",
                        format!("{:width$}", "", width=(row.depth() as usize) * 2),
                        if row.children().is_some() { "\u{25BE} " } else { "  " },
                        obj.string()
                    )
                })
                .collect::<Vec<String>>()
                .join("\n");

            window.clipboard().set_text(&copy_text);
        }));

        // Tree scale value changed signal
        imp.tree_depth_scale.connect_value_changed(clone!(@weak self as window, @weak imp => move |scale| {
            if scale.value() == imp.tree_depth_scale.adjustment().upper() {
                imp.tree_depth_label.set_label("All");
            } else {
                imp.tree_depth_label.set_label(&scale.value().to_string());
            }

            imp.tree_depth_filter.changed(gtk::FilterChange::Different);
        }));

        // Files search entry search changed signal
        imp.files_search_entry.connect_search_changed(clone!(@weak imp => move |entry| {
            imp.files_filter.set_search(Some(&entry.text()));
        }));

        // Files open button clicked signal
        imp.files_open_button.connect_clicked(clone!(@weak imp => move |_| {
            let item = imp.files_selection.selected_item()
                .and_downcast::<gtk::StringObject>()
                .expect("Must be a 'StringObject'");

            Utils::open_file_manager(&item.string());
        }));

        // Files copy button clicked signal
        imp.files_copy_button.connect_clicked(clone!(@weak self as window, @weak imp => move |_| {
            let copy_text = imp.files_selection.iter::<glib::Object>().flatten()
                .map(|item| {
                    let obj = item
                        .downcast::<gtk::StringObject>()
                        .expect("Must be a 'StringObject'");

                    obj.string()
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
                .expect("Must be a 'StringObject'");

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
                .expect("Must be a 'BackupObject'");

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
        let tab_action = gio::ActionEntry::<DetailsWindow>::builder("set-tab")
            .parameter_type(Some(&str::static_variant_type()))
            .state("tree".to_variant())
            .change_state(|window, action, state| {
                let state = state
                    .expect("Must be a 'Variant'");

                let state_str = state
                    .get::<String>()
                    .expect("Must be a 'String'");

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
            Some(gtk::CallbackAction::new(clone!(@weak self as window => @default-return true, move |_, _| {
                window.close();

                true
            })))
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

        imp.files_button.set_sensitive(installed);
        imp.log_button.set_sensitive(installed);
        imp.cache_button.set_sensitive(installed);
        imp.backup_button.set_sensitive(installed);
    }

    //-----------------------------------
    // Update tree page
    //-----------------------------------
    fn update_ui_tree_page(&self, pkg: &PkgObject, pkg_model: &gio::ListStore, aur_model: &gio::ListStore) {
        let imp = self.imp();

        // Set files search entry key capture widget
        imp.tree_search_entry.set_key_capture_widget(Some(&imp.tree_view.get().upcast::<gtk::Widget>()));

        // Build and store dependency hash map (avoid duplicates)
        imp.tree_dep_map.replace(HashMap::from([(pkg.name(), None)]));

        // Build package hash map
        let mut pkg_map: HashMap<String, PkgObject> = HashMap::new();

        pkg_map.extend(pkg_model.iter::<PkgObject>().flatten().map(|pkg| (pkg.name(), pkg)));
        pkg_map.extend(aur_model.iter::<PkgObject>().flatten().map(|pkg| (pkg.name(), pkg)));

        // Create and store tree model
        let root_model = gio::ListStore::from_iter([gtk::StringObject::new(&pkg.name())]);

        let tree_model = gtk::TreeListModel::new(root_model, false, true, clone!(@weak imp => @default-return None, move |item| {
            let obj = item.downcast_ref::<gtk::StringObject>()
                .expect("Must be a 'StringObject'");

            let mut dep_map = imp.tree_dep_map.borrow_mut();

            if let Some(pkg) = pkg_map.get(&obj.string().to_string()) {
                let mut deps: Vec<String> = if imp.tree_reverse_button.is_active() {
                    pkg.required_by()
                } else {
                    pkg.depends()
                };

                deps.retain(|dep| {
                    !dep_map.contains_key(dep) ||
                    dep_map.get(dep).filter(|&parent| parent == &Some(pkg.name())).is_some()
                });

                dep_map.extend(deps.iter().map(|dep| (dep.to_string(), Some(pkg.name()))));

                if !deps.is_empty() {
                    return Some(gio::ListStore::from_iter(deps.iter()
                        .map(|dep| gtk::StringObject::new(dep)))
                        .upcast::<gio::ListModel>())
                }
            }

            None
        }));

        imp.tree_filter_model.set_model(Some(&tree_model));

        imp.tree_model.set(tree_model).unwrap();

        // Set tree model filter function
        imp.tree_depth_filter.set_filter_func(clone!(@weak imp => @default-return false, move |item| {
            let row = item
                .downcast_ref::<gtk::TreeListRow>()
                .expect("Must be a 'TreeListRow'");

            let depth = imp.tree_depth_scale.value();

            if depth == imp.tree_depth_scale.adjustment().upper() {
                true
            } else {
                (row.depth() as f64) <= depth
            }
        }));

        // Bind reverse toggle button state to tree header label
        imp.tree_reverse_button.bind_property("active", &imp.tree_header_label.get(), "label")
            .transform_to(move |_, active: bool| if active { Some("Required By") } else { Some("Dependencies") })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
    }

    //-----------------------------------
    // Update files page
    //-----------------------------------
    fn update_ui_files_page(&self, pkg: &PkgObject) {
        let imp = self.imp();

        // Set files search entry key capture widget
        imp.files_search_entry.set_key_capture_widget(Some(&imp.files_view.get().upcast::<gtk::Widget>()));

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
            let match_expr = Regex::new(&format!("\\[(.+?)T(.+?)\\+.+?\\] \\[ALPM\\] (installed|removed|upgraded|downgraded) ({}) (.+)", pkg.name())).unwrap();

            let log_lines: Vec<gtk::StringObject> = log.lines().rev()
                .filter_map(|s|
                    if match_expr.is_match(s) {
                        Some(gtk::StringObject::new(&match_expr.replace(s, "[$1  $2] : $3 $4 $5")))
                    } else {
                        None
                    }
                )
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
    fn update_ui_cache_page(&self, pkg: &PkgObject, cache_dirs: &Vec<String>, pkg_model: &gio::ListStore) {
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
            // Find cache files that include package name
            let cache_list: Vec<gtk::StringObject> = glob(&format!("{dir}{pkg_name}*.zst"))
                .unwrap()
                .flatten()
                .filter_map(|entry| {
                    let cache_file = entry.display().to_string();

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
            .map(|backup| BackupObject::new(backup, None))
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
