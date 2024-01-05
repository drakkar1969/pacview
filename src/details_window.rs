use std::cell::RefCell;
use std::fs;

use gtk::{gio, glib, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use regex::Regex;
use lazy_static::lazy_static;
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
        pub tree_depth_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub tree_depth_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub tree_reverse_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub tree_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub tree_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub tree_view: TemplateChild<gtk::TextView>,
        #[template_child]
        pub tree_buffer: TemplateChild<gtk::TextBuffer>,

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

        pub tree_text: RefCell<String>,
        pub tree_rev_text: RefCell<String>,
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

            obj.setup_controllers();
            obj.setup_signals();
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
    pub fn new(parent: &gtk::Window, pkg: &PkgObject, custom_font: bool, monospace_font: &str, log_file: &str, cache_dirs: &Vec<String>, pkg_model: &gtk::FlattenListModel) -> Self {
        let win: Self = glib::Object::builder()
            .property("transient-for", parent)
            .build();

        let installed = pkg.flags().intersects(PkgFlags::INSTALLED);

        win.update_ui_banner(pkg);
        win.update_ui_stack(installed);

        win.update_ui_tree_page(pkg, custom_font, monospace_font);

        if installed {
            win.update_ui_files_page(pkg);
            win.update_ui_logs_page(pkg, log_file);
            win.update_ui_cache_page(pkg, cache_dirs, pkg_model);
            win.update_ui_backup_page(pkg);
        }

        win
    }

    //-----------------------------------
    // Setup controllers
    //-----------------------------------
    fn setup_controllers(&self) {
        // Key controller (close window on ESC)
        let controller = gtk::EventControllerKey::new();

        controller.set_propagation_phase(gtk::PropagationPhase::Capture);

        controller.connect_key_pressed(clone!(@weak self as obj => @default-return glib::Propagation::Proceed, move |_, key, _, state| {
            if key == gdk::Key::Escape && state.is_empty() {
                obj.close();

                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }

        }));

        self.add_controller(controller);
    }

    //-----------------------------------
    // Open file manager helper function
    //-----------------------------------
    fn open_file_manager(&self, path: &str) {
        if let Some(desktop) = gio::AppInfo::default_for_type("inode/directory", true) {
            let path = format!("file://{path}");

            let _res = desktop.launch_uris(&[&path], None::<&gio::AppLaunchContext>);
        }
    }

    //-----------------------------------
    // Filter dependency tree helper function
    //-----------------------------------
    fn filter_dependency_tree(&self) {
        let imp = self.imp();

        let depth = imp.tree_depth_scale.value();
        let reverse = imp.tree_reverse_button.is_active();

        let tree_text = if reverse {imp.tree_rev_text.borrow()} else {imp.tree_text.borrow()};

        // Filter tree text
        lazy_static! {
            static ref EXPR: Regex = Regex::new("[└|─|│|├| ]*").unwrap();
        }

        let filter_text = if depth == imp.tree_depth_scale.adjustment().upper() {
            tree_text.to_string()
        } else {
            tree_text.lines()
                .filter(|&s| {
                    EXPR.find(s)
                        .filter(|ascii| ascii.as_str().chars().count() as f64 <= depth * 2.0)
                        .is_some()
                })
                .collect::<Vec<&str>>()
                .join("\n")
        };

        // Set tree view text
        imp.tree_buffer.set_text(&filter_text);

        // Set tree header label
        let n_lines = imp.tree_buffer.line_count();

        if n_lines >= 1 {
            imp.tree_header_label.set_label(&format!("Dependency Tree ({})", n_lines - 1));
        } else {
            imp.tree_header_label.set_label("Dependency Tree (0)");
        }
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Stack button toggled signals
        let stack_buttons = [
            imp.tree_button.get(),
            imp.files_button.get(),
            imp.log_button.get(),
            imp.cache_button.get(),
            imp.backup_button.get()
        ];

        for button in stack_buttons {
            button.connect_toggled(clone!(@weak imp => move |button| {
                if button.is_active() {
                    imp.content_stack.set_visible_child_name(&button.text().to_lowercase());
                }
            }));
        }

        // Tree scale value changed signal
        imp.tree_depth_scale.connect_value_changed(clone!(@weak self as obj, @weak imp => move |scale| {
            if scale.value() == imp.tree_depth_scale.adjustment().upper() {
                imp.tree_depth_label.set_label("Default");
            } else {
                imp.tree_depth_label.set_label(&scale.value().to_string());
            }

            obj.filter_dependency_tree();
        }));

        // Tree reverse button toggled signal
        imp.tree_reverse_button.connect_toggled(clone!(@weak self as obj => move |_| {
            obj.filter_dependency_tree();
        }));

        // Tree copy button clicked signal
        imp.tree_copy_button.connect_clicked(clone!(@weak self as obj, @weak imp => move |_| {
            obj.clipboard().set_text(&imp.tree_buffer.text(&imp.tree_buffer.start_iter(), &imp.tree_buffer.end_iter(), false));
        }));

        // Files search entry search changed signal
        imp.files_search_entry.connect_search_changed(clone!(@weak imp => move |entry| {
            imp.files_filter.set_search(Some(&entry.text()));
        }));

        // Files open button clicked signal
        imp.files_open_button.connect_clicked(clone!(@weak self as obj, @weak imp => move |_| {
            let item = imp.files_selection.selected_item()
                .and_downcast::<gtk::StringObject>()
                .expect("Must be a 'StringObject'");

            obj.open_file_manager(&item.string());
        }));

        // Files copy button clicked signal
        imp.files_copy_button.connect_clicked(clone!(@weak self as obj, @weak imp => move |_| {
            let copy_text = imp.files_selection.iter::<glib::Object>().flatten()
                .map(|item| {
                    let s = item
                        .downcast::<gtk::StringObject>()
                        .expect("Must be a 'StringObject'");

                    s.string()
                })
                .collect::<Vec<glib::GString>>()
                .join("\n");

            obj.clipboard().set_text(&copy_text);
        }));

        // Files listview activate signal
        imp.files_view.connect_activate(clone!(@weak imp => move |_, _| {
            imp.files_open_button.emit_clicked();
        }));

        // Log copy button clicked signal
        imp.log_copy_button.connect_clicked(clone!(@weak self as obj, @weak imp => move |_| {
            let copy_text = imp.log_model.iter::<gtk::StringObject>().flatten()
                .map(|item| item.string())
                .collect::<Vec<glib::GString>>()
                .join("\n");

            obj.clipboard().set_text(&copy_text);
        }));

        // Cache open button clicked signal
        imp.cache_open_button.connect_clicked(clone!(@weak self as obj, @weak imp => move |_| {
            let item = imp.cache_selection.selected_item()
                .and_downcast::<gtk::StringObject>()
                .expect("Must be a 'StringObject'");

            obj.open_file_manager(&item.string());
        }));

        // Cache copy button clicked signal
        imp.cache_copy_button.connect_clicked(clone!(@weak self as obj, @weak imp => move |_| {
            let copy_text = imp.cache_model.iter::<gtk::StringObject>().flatten()
                .map(|item| item.string())
                .collect::<Vec<glib::GString>>()
                .join("\n");

            obj.clipboard().set_text(&copy_text);
        }));

        // Cache listview activate signal
        imp.cache_view.connect_activate(clone!(@weak imp => move |_, _| {
            imp.cache_open_button.emit_clicked();
        }));

        // Backup open button clicked signal
        imp.backup_open_button.connect_clicked(clone!(@weak self as obj, @weak imp => move |_| {
            let item = imp.backup_selection.selected_item()
                .and_downcast::<BackupObject>()
                .expect("Must be a 'BackupObject'");

            obj.open_file_manager(&item.filename());
        }));

        // Backup copy button clicked signal
        imp.backup_copy_button.connect_clicked(clone!(@weak self as obj, @weak imp => move |_| {
            let copy_text = imp.backup_model.iter::<BackupObject>().flatten()
                .map(|item| {
                    format!("{filename} ({status})", filename=item.filename(), status=item.status())
                })
                .collect::<Vec<String>>()
                .join("\n");

            obj.clipboard().set_text(&copy_text);
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
    }

    //-----------------------------------
    // Update tree page
    //-----------------------------------
    fn update_ui_tree_page(&self, pkg: &PkgObject, custom_font: bool, monospace_font: &str) {
        let imp = self.imp();

        // Get monospace font
        let font_str = if custom_font {
            monospace_font.to_string()
        } else {
            let gsettings = gio::Settings::new("org.gnome.desktop.interface");

            gsettings.string("monospace-font-name").to_string()
        };

        // Set text view font
        let font_css = Utils::pango_font_string_to_css(&font_str);

        let css_provider = gtk::CssProvider::new();
        css_provider.load_from_string(&format!("textview.tree {{ {font_css}}}"));

        gtk::style_context_add_provider_for_display(&imp.tree_view.display(), &css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

        // Spawn thread to get tree text
        let pkg_name = pkg.name();
        let local_flag = if pkg.flags().intersects(PkgFlags::INSTALLED) {""} else {"-s"};

        let (sender, receiver) = glib::MainContext::channel::<(String, String)>(glib::Priority::DEFAULT);

        gio::spawn_blocking(move || {
            // Get dependecy tree
            let cmd = format!("/usr/bin/pactree {local_flag} {pkg_name}");

            let (_, mut deps) = Utils::run_command(&cmd);

            // Strip empty lines
            deps = deps.lines().into_iter()
                .filter(|&s| !s.is_empty())
                .collect::<Vec<&str>>()
                .join("\n");

            // Get reverse dependency tree
            let cmd = format!("/usr/bin/pactree -r {local_flag} {pkg_name}");

            let (_, mut rev_deps) = Utils::run_command(&cmd);

            // Strip empty lines
            rev_deps = rev_deps.lines().into_iter()
                .filter(|&s| !s.is_empty())
                .collect::<Vec<&str>>()
                .join("\n");

            // Return thread result
            sender.send((deps, rev_deps)).expect("Could not send through channel");
        });

        // Attach thread receiver
        receiver.attach(
            None,
            clone!(@weak self as obj, @weak imp => @default-return glib::ControlFlow::Break, move |(deps, rev_deps)| {
                imp.tree_text.replace(deps);

                imp.tree_rev_text.replace(rev_deps);

                // Set tree view text
                obj.filter_dependency_tree();

                imp.tree_stack.set_visible_child_name("deps");
    
                glib::ControlFlow::Break
            }),
        );
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

        imp.files_model.splice(0, 0, &files_list);

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

            imp.log_model.splice(0, 0, &log_lines);
        }

        // Set copy button state
        let n_items = imp.log_model.n_items();

        imp.log_copy_button.set_sensitive(n_items > 0);
    }

    //-----------------------------------
    // Update cache page
    //-----------------------------------
    fn update_ui_cache_page(&self, pkg: &PkgObject, cache_dirs: &Vec<String>, pkg_model: &gtk::FlattenListModel) {
        let imp = self.imp();

        let pkg_name = &pkg.name();

        // Get blacklist package names
        let blacklist: Vec<String> = pkg_model.iter::<glib::Object>().flatten()
            .map(|pkg| pkg.downcast::<PkgObject>().unwrap().name())
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

            imp.cache_model.splice(0, 0, &cache_list);
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
            .map(|backup| {
                if let Ok(file_hash) = alpm::compute_md5sum(backup.filename.to_string()) {
                    if file_hash == backup.hash {
                        BackupObject::new(&backup.filename, "backup-unmodified", "unmodified")
                    } else {
                        BackupObject::new(&backup.filename, "backup-modified", "modified")
                    }
                } else {
                    BackupObject::new(&backup.filename, "backup-error", "read error")
                }
            })
            .collect();

        imp.backup_model.splice(0, 0, &backup_list);

        // Set backup header label
        let n_items = imp.backup_model.n_items();

        imp.backup_header_label.set_label(&format!("Backup Files ({n_items})"));

        // Set open/copy button states
        imp.backup_open_button.set_sensitive(n_items > 0);
        imp.backup_copy_button.set_sensitive(n_items > 0);
    }
}
