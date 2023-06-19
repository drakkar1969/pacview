use std::cell::{Cell, RefCell};
use std::fs;
use std::borrow::Cow;

use gtk::{gio, glib, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use gtk::pango::AttrList;
use glib::clone;

use fancy_regex::Regex;
use lazy_static::lazy_static;

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
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::DetailsWindow)]
    #[template(resource = "/com/github/PacView/ui/details_window.ui")]
    pub struct DetailsWindow {
        #[template_child]
        pub pkg_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub content_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub files_button: TemplateChild<ToggleButton>,
        #[template_child]
        pub tree_button: TemplateChild<ToggleButton>,
        #[template_child]
        pub log_button: TemplateChild<ToggleButton>,
        #[template_child]
        pub cache_button: TemplateChild<ToggleButton>,
        #[template_child]
        pub backup_button: TemplateChild<ToggleButton>,

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
        pub files_model: TemplateChild<gtk::StringList>,
        #[template_child]
        pub files_selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub files_filter: TemplateChild<gtk::StringFilter>,

        #[template_child]
        pub tree_depth_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub tree_depth_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub tree_reverse_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub tree_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub tree_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub log_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub log_model: TemplateChild<gtk::StringList>,

        #[template_child]
        pub cache_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub cache_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub cache_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub cache_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub cache_model: TemplateChild<gtk::StringList>,
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

        #[property(get, set)]
        pkg: RefCell<PkgObject>,

        #[property(get, set)]
        default_tree_depth: Cell<f64>,
        #[property(get, set)]
        tree_text: RefCell<String>,
        #[property(get, set)]
        tree_rev_text: RefCell<String>,
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
        // Default property functions
        //-----------------------------------
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

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
    pub fn new(parent: &gtk::Window, pkg: &PkgObject, custom_font: bool, monospace_font: &str, log_file: &str) -> Self {
        let win: Self = glib::Object::builder()
            .property("transient-for", parent)
            .property("pkg", pkg)
            .build();

        let installed = pkg.flags().intersects(PkgFlags::INSTALLED);

        win.init_banner();
        win.init_stack(installed);

        if installed { win.init_files_page(); }
        win.init_tree_page(custom_font, monospace_font);
        if installed { win.init_logs_page(log_file); }
        if installed { win.init_cache_page(); }
        if installed { win.init_backup_page(); }

        win
    }

    //-----------------------------------
    // Setup controllers
    //-----------------------------------
    fn setup_controllers(&self) {
        // Key controller (close window on ESC)
        let controller = gtk::EventControllerKey::new();

        controller.set_propagation_phase(gtk::PropagationPhase::Capture);

        controller.connect_key_pressed(clone!(@weak self as obj => @default-return gtk::Inhibit(false), move |_, key, _, state| {
            if key == gdk::Key::Escape && state.is_empty() {
                obj.close();

                gtk::Inhibit(true)
            } else {
                gtk::Inhibit(false)
            }

        }));

        self.add_controller(controller);
    }

    //-----------------------------------
    // Open file manager helper function
    //-----------------------------------
    fn open_file_manager(&self, path: &str) {
        if let Some(desktop) = gio::AppInfo::default_for_type("inode/directory", true) {
            let path = format!("file://{}", path);

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

        let tree_text = if reverse {self.tree_rev_text()} else {self.tree_text()};

        lazy_static! {
            static ref EXPR: Regex = Regex::new("([└|─|│|├| ]+)?(.+)").unwrap();
        }

        let filter_text = if depth == self.default_tree_depth() {
            tree_text
        } else {
            tree_text.lines()
            .filter_map(|s| {
                let ascii = EXPR.replace(s, "$1");

                if ascii.chars().count() as f64 > depth * 2.0 {
                    None
                } else {
                    Some(s)
                }
            })
            .collect::<Vec<&str>>()
            .join("\n")
        };

        imp.tree_label.set_label(&filter_text);
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Stack button toggled signals
        let stack_buttons = [
            imp.files_button.get(),
            imp.tree_button.get(),
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
        imp.files_view.connect_activate(clone!(@weak self as obj, @weak imp => move |_, _| {
            let item = imp.files_selection.selected_item()
                .and_downcast::<gtk::StringObject>()
                .expect("Must be a 'StringObject'");

            obj.open_file_manager(&item.string());
        }));

        // Tree scale value changed signal
        imp.tree_depth_scale.connect_value_changed(clone!(@weak self as obj, @weak imp => move |scale| {
            if scale.value() == obj.default_tree_depth() {
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
            obj.clipboard().set_text(&imp.tree_label.label());
        }));

        // Log copy button clicked signal
        imp.log_copy_button.connect_clicked(clone!(@weak self as obj, @weak imp => move |_| {
            let copy_text = imp.log_model.iter::<glib::Object>().flatten()
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

        // Cache open button clicked signal
        imp.cache_open_button.connect_clicked(clone!(@weak self as obj, @weak imp => move |_| {
            let item = imp.cache_selection.selected_item()
                .and_downcast::<gtk::StringObject>()
                .expect("Must be a 'StringObject'");

            obj.open_file_manager(&item.string());
        }));

        // Cache copy button clicked signal
        imp.cache_copy_button.connect_clicked(clone!(@weak self as obj, @weak imp => move |_| {
            let copy_text = imp.cache_model.iter::<glib::Object>().flatten()
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

        // Cache listview activate signal
        imp.cache_view.connect_activate(clone!(@weak self as obj, @weak imp => move |_, _| {
            let item = imp.cache_selection.selected_item()
                .and_downcast::<gtk::StringObject>()
                .expect("Must be a 'StringObject'");

            obj.open_file_manager(&item.string());
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
            let copy_text = imp.backup_model.iter::<glib::Object>().flatten()
                .map(|item| {
                    let bck = item
                        .downcast::<BackupObject>()
                        .expect("Must be a 'BackupObject'");

                    format!("{filename} ({status})", filename=bck.filename(), status=bck.status())
                })
                .collect::<Vec<String>>()
                .join("\n");

            obj.clipboard().set_text(&copy_text);
        }));

        // Backup listview activate signal
        imp.backup_view.connect_activate(clone!(@weak self as obj, @weak imp => move |_, _| {
            let item = imp.backup_selection.selected_item()
                .and_downcast::<BackupObject>()
                .expect("Must be a 'BackupObject'");

            obj.open_file_manager(&item.filename());
        }));
    }

    //-----------------------------------
    // Initialize banner
    //-----------------------------------
    fn init_banner(&self) {
        // Set package name in banner
        self.imp().pkg_label.set_label(&format!("{repo}/{name}", repo=self.pkg().repo_show(), name=self.pkg().name()));
    }

    //-----------------------------------
    // Initialize stack
    //-----------------------------------
    fn init_stack(&self, installed: bool) {
        let imp = self.imp();

        imp.files_button.set_sensitive(installed);
        imp.log_button.set_sensitive(installed);
        imp.cache_button.set_sensitive(installed);
        imp.backup_button.set_sensitive(installed);

        if !installed {
            imp.tree_button.set_active(true);
        }
    }

    //-----------------------------------
    // Initialize files page
    //-----------------------------------
    fn init_files_page(&self) {
        let imp = self.imp();

        // Set files search entry key capture widget
        imp.files_search_entry.set_key_capture_widget(Some(&imp.files_view.get().upcast::<gtk::Widget>()));

        // Populate files list
        let files = self.pkg().files();
        let files_len = files.len();

        imp.files_model.splice(0, 0, &files.iter().map(|s| s.as_str()).collect::<Vec<&str>>());

        // Bind files count to files header label
        imp.files_selection.bind_property("n-items", &imp.files_header_label.get(), "label")
            .transform_to(move |_, n_items: u32| Some(format!("Files ({} of {})", n_items, files_len)))
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
    // Initialize tree page
    //-----------------------------------
    fn init_tree_page(&self, custom_font: bool, monospace_font: &str) {
        let imp = self.imp();

        // Get monospace font
        let font_str = if custom_font {
            monospace_font.to_string()
        } else {
            let gsettings = gio::Settings::new("org.gnome.desktop.interface");

            gsettings.string("monospace-font-name").to_string()
        };

        // Set tree label font
        let format_str = format!("0 -1 font-desc \"{}\"", font_str);

        if let Ok(attr) = AttrList::from_string(&format_str) {
            imp.tree_label.set_attributes(Some(&attr));
        }

        // Set default tree depth
        self.set_default_tree_depth(imp.tree_depth_scale.adjustment().upper());

        // Get dependency tree text
        let pkg = self.pkg();

        let local_flag = if pkg.flags().intersects(PkgFlags::INSTALLED) {""} else {"-s"};

        let cmd = format!("/usr/bin/pactree {local_flag} {name}",
            local_flag=local_flag,
            name=pkg.name()
        );

        let (_code, stdout) = Utils::run_command(&cmd);

        self.set_tree_text(stdout);

        // Get dependency tree reverse text
        let cmd = format!("/usr/bin/pactree -r {local_flag} {name}",
            local_flag=local_flag,
            name=pkg.name()
        );

        let (_code, stdout) = Utils::run_command(&cmd);

        self.set_tree_rev_text(stdout);

        // Set tree label text
        imp.tree_label.set_label(&self.tree_text());
    }

    //-----------------------------------
    // Initialize logs page
    //-----------------------------------
    fn init_logs_page(&self, log_file: &str) {
        let imp = self.imp();

        // Populate log messages
        if let Ok(log) = fs::read_to_string(log_file) {
            let match_expr = Regex::new(&format!("\\[(.+)T(.+)\\+.+\\] \\[ALPM\\] (installed|removed|upgraded|downgraded) ({}) (.+)", self.pkg().name())).unwrap();

            let log_lines: Vec<Cow<str>> = log.lines().rev()
                .filter_map(|s|
                    if match_expr.is_match(s).unwrap_or_default() {
                        Some(match_expr.replace_all(s, "[$1 $2]  $3 $4 $5"))
                    } else {
                        None
                    }
                )
                .collect();

            imp.log_model.splice(0, 0, &log_lines.iter().map(|s| s.as_ref()).collect::<Vec<&str>>());
        }

        // Set copy button state
        let n_files = imp.log_model.n_items();

        imp.log_copy_button.set_sensitive(n_files > 0);
    }

    //-----------------------------------
    // Initialize cache page
    //-----------------------------------
    fn init_cache_page(&self) {
        let imp = self.imp();

        // Populate cache files list
        let cmd = format!("/usr/bin/paccache -vvdk0 {}", self.pkg().name());

        let (_code, stdout) = Utils::run_command(&cmd);

        let cache_lines: Vec<&str> = stdout.lines()
            .filter(|s| !s.is_empty() && !s.starts_with("==>") && !s.ends_with(".sig"))
            .collect();

        imp.cache_model.splice(0, 0, &cache_lines);

        // Set cache header label
        let n_files = imp.cache_model.n_items();

        imp.cache_header_label.set_label(&format!("Cache Files ({})", n_files));

        // Set open/copy button states
        imp.cache_open_button.set_sensitive(n_files > 0);
        imp.cache_copy_button.set_sensitive(n_files > 0);
    }

    //-----------------------------------
    // Initialize backup page
    //-----------------------------------
    fn init_backup_page(&self) {
        let imp = self.imp();

        // Populate backup list
        let backup = self.pkg().backup();

        let backup_list: Vec<BackupObject> = backup.iter()
            .map(|backup| {
                let (name, hash) = backup.split_once(" || ").unwrap();

                if let Ok(file_hash) = alpm::compute_md5sum(name) {
                    if file_hash == hash {
                        BackupObject::new(name, "backup-unmodified", "unmodified")
                    } else {
                        BackupObject::new(name, "backup-modified", "modified")
                    }
                } else {
                    BackupObject::new(name, "backup-error", "read error")
                }
            })
            .collect();

        imp.backup_model.splice(0, 0, &backup_list);

        // Set backup header label
        let n_files = imp.backup_model.n_items();

        imp.backup_header_label.set_label(&format!("Backup Files ({})", n_files));

        // Set open/copy button states
        imp.backup_open_button.set_sensitive(n_files > 0);
        imp.backup_copy_button.set_sensitive(n_files > 0);
    }
}
