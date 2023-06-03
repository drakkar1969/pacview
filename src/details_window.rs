use std::cell::{Cell, RefCell};
use std::fs;
use std::io::{BufReader, BufRead};

use gtk::{gio, glib, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use gtk::pango::AttrList;

use fancy_regex::Regex;
use md5;

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
        pub log_selection: TemplateChild<gtk::NoSelection>,

        #[template_child]
        pub cache_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub cache_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub cache_copy_button: TemplateChild<gtk::Button>,
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
        pub backup_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub backup_selection: TemplateChild<gtk::SingleSelection>,

        #[property(get, set)]
        pkg: RefCell<PkgObject>,

        #[property(get, set)]
        tree_font: RefCell<Option<String>>,
        #[property(get, set)]
        log_file: RefCell<String>,
        #[property(get, set)]
        cache_dir: RefCell<String>,

        #[property(get, set)]
        default_tree_depth: Cell<f64>,
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
            ToggleButton::static_type();
            BackupObject::static_type();

            klass.bind_template();
            klass.bind_template_callbacks();
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
        }
    }

    impl WidgetImpl for DetailsWindow {}
    impl WindowImpl for DetailsWindow {}
    impl ApplicationWindowImpl for DetailsWindow {}
    impl AdwApplicationWindowImpl for DetailsWindow {}

    #[gtk::template_callbacks]
    impl DetailsWindow {
        //-----------------------------------
        // Stack button signal handler
        //-----------------------------------
        #[template_callback]
        fn on_stack_button_toggled(&self, button: ToggleButton) {
            if button.is_active() {
                self.content_stack.set_visible_child_name(&button.text().to_lowercase());
            }
        }

        //-----------------------------------
        // Open file manager helper function
        //-----------------------------------
        fn open_file_manager(&self, path: &str) {
            if let Some(desktop) = gio::AppInfo::default_for_type("inode/directory", true) {
                let path = format!("file://{}", path);

                desktop.launch_uris(&[&path], None::<&gio::AppLaunchContext>).unwrap();
            }
        }

        //-----------------------------------
        // Files page signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_files_search_changed(&self, entry: &gtk::SearchEntry) {
            self.files_filter.set_search(Some(&entry.text()));
        }

        #[template_callback]
        fn on_files_view_activated(&self) {
            self.on_files_open_button_clicked();
        }

        #[template_callback]
        fn on_files_open_button_clicked(&self) {
            let item = self.files_selection.selected_item()
                .and_downcast::<gtk::StringObject>()
                .expect("Must be a 'StringObject'");

            self.open_file_manager(&item.string());
        }

        #[template_callback]
        fn on_files_copy_button_clicked(&self) {
            let copy_text = (0..self.files_selection.n_items()).into_iter()
                .map(|i| {
                    let item = self.files_selection.item(i)
                        .and_downcast::<gtk::StringObject>()
                        .expect("Must be a 'StringObject'");

                    item.string()
                })
                .collect::<Vec<glib::GString>>()
                .join("\n");

            self.obj().clipboard().set_text(&copy_text);
        }

        //-----------------------------------
        // Populate dependency tree helper function
        //-----------------------------------
        pub fn populate_dependency_tree(&self, depth: f64, reverse: bool) {
            let depth_str = format!("-d{}", depth);

            let pkg = self.obj().pkg();

            let cmd = format!("/usr/bin/pactree {local_flag} {depth_flag} {reverse_flag} {name}", 
                local_flag=if pkg.flags().intersects(PkgFlags::INSTALLED) {""} else {"-s"},
                depth_flag=if depth == self.obj().default_tree_depth() {""} else {&depth_str},
                reverse_flag=if reverse {"-r"} else {""},
                name=pkg.name()
            );

            let (_code, stdout) = Utils::run_command(&cmd);

            self.tree_label.set_label(&stdout);
        }

        //-----------------------------------
        // Tree page signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_tree_depth_changed(&self, scale: gtk::Scale) {
            let depth = scale.value();

            self.populate_dependency_tree(depth, self.tree_reverse_button.is_active());

            let depth_str = depth.to_string();

            self.tree_depth_label.set_label(if depth == self.obj().default_tree_depth() {"Default"} else {&depth_str});
        }

        #[template_callback]
        fn on_tree_reverse_toggled(&self, button: gtk::ToggleButton) {
            self.populate_dependency_tree(self.tree_depth_scale.value(), button.is_active());
        }

        #[template_callback]
        fn on_tree_copy_button_clicked(&self) {
            self.obj().clipboard().set_text(&self.tree_label.label());
        }

        //-----------------------------------
        // Log page signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_log_copy_button_clicked(&self) {
            let copy_text = (0..self.log_selection.n_items()).into_iter()
                .map(|i| {
                    let item = self.log_selection.item(i)
                        .and_downcast::<gtk::StringObject>()
                        .expect("Must be a 'StringObject'");

                    item.string()
                })
                .collect::<Vec<glib::GString>>()
                .join("\n");

            self.obj().clipboard().set_text(&copy_text);
        }

        //-----------------------------------
        // Cache page signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_cache_view_activated(&self) {
            self.on_cache_open_button_clicked();
        }

        #[template_callback]
        fn on_cache_open_button_clicked(&self) {
            let item = self.cache_selection.selected_item()
                .and_downcast::<gtk::StringObject>()
                .expect("Must be a 'StringObject'");

            self.open_file_manager(&format!("{}{}", self.obj().cache_dir(), item.string()));
        }

        #[template_callback]
        fn on_cache_copy_button_clicked(&self) {
            let copy_text = (0..self.cache_selection.n_items()).into_iter()
                .map(|i| {
                    let item = self.cache_selection.item(i)
                        .and_downcast::<gtk::StringObject>()
                        .expect("Must be a 'StringObject'");

                    item.string()
                })
                .collect::<Vec<glib::GString>>()
                .join("\n");

            self.obj().clipboard().set_text(&copy_text);
        }

        //-----------------------------------
        // Backup page signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_backup_view_activated(&self) {
            self.on_backup_open_button_clicked();
        }

        #[template_callback]
        fn on_backup_open_button_clicked(&self) {
            let item = self.backup_selection.selected_item()
                .and_downcast::<BackupObject>()
                .expect("Must be a 'BackupObject'");

            self.open_file_manager(&item.filename());
        }

        #[template_callback]
        fn on_backup_copy_button_clicked(&self) {
            let copy_text = (0..self.backup_selection.n_items()).into_iter()
                .map(|i| {
                    let item = self.backup_selection.item(i)
                        .and_downcast::<BackupObject>()
                        .expect("Must be a 'BackupObject'");

                    format!("{filename} ({status})", filename=item.filename(), status=item.status())
                })
                .collect::<Vec<String>>()
                .join("\n");

            self.obj().clipboard().set_text(&copy_text);
        }

        //-----------------------------------
        // Key press signal handler
        //-----------------------------------
        #[template_callback]
        fn on_key_pressed(&self, key: u32, _: u32, state: gdk::ModifierType) -> bool {
            if key == 65307 && state.is_empty() {
                self.obj().close();

                true
            } else {
                false
            }
        }
    }
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION: DetailsWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct DetailsWindow(ObjectSubclass<imp::DetailsWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl DetailsWindow {
    //-----------------------------------
    // Public new function
    //-----------------------------------
    pub fn new(pkg: &PkgObject, tree_font: Option<String>, log_file: &str, cache_dir: &str) -> Self {
        let win: Self = glib::Object::builder()
            .property("pkg", pkg)
            .property("tree-font", tree_font)
            .property("log-file", log_file)
            .property("cache-dir", cache_dir)
            .build();

        win.setup_banner();
        win.setup_files();
        win.setup_tree();
        win.setup_logs();
        win.setup_cache();
        win.setup_backup();

        win
    }

    //-----------------------------------
    // Setup banner
    //-----------------------------------
    fn setup_banner(&self) {
        // Set package name in banner
        self.imp().pkg_label.set_label(&format!("{repo}/{name}", repo=self.pkg().repo_show(), name=self.pkg().name()));
    }

    //-----------------------------------
    // Setup files page
    //-----------------------------------
    fn setup_files(&self) {
        let imp = self.imp();

        // Set files search entry key capture widget
        imp.files_search_entry.set_key_capture_widget(Some(&imp.files_view.get().upcast::<gtk::Widget>()));

        // Bind files count to files header label
        imp.files_selection.bind_property("n-items", &imp.files_header_label.get(), "label")
            .transform_to(|_, n_items: u32| Some(format!("Files ({})", n_items)))
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

        // Populate files list
        let files = self.pkg().files();

        imp.files_model.splice(0, 0, &files.iter().map(|s| s.as_str()).collect::<Vec<&str>>());
    }

    //-----------------------------------
    // Setup tree page
    //-----------------------------------
    fn setup_tree(&self) {
        let imp = self.imp();

        let font = self.tree_font();

        // Get monospace font
        let font_str = if font.is_none() {
            let gsettings = gio::Settings::new("org.gnome.desktop.interface");

            let font_str = gsettings.string("monospace-font-name");

            font_str.to_string()
        } else {
            font.unwrap()
        };

        // Set tree label font
        let format_str = format!("0 -1 font-desc \"{}\"", font_str);

        if let Ok(attr) = AttrList::from_string(&format_str) {
            imp.tree_label.set_attributes(Some(&attr));
        }

        // Set default tree depth
        self.set_default_tree_depth(imp.tree_depth_scale.adjustment().upper());

        // Populate dependency tree
        imp.populate_dependency_tree(self.default_tree_depth(), false);
    }

    //-----------------------------------
    // Setup logs page
    //-----------------------------------
    fn setup_logs(&self) {
        let imp = self.imp();

        // Populate log messages
        if let Ok(log) = fs::read_to_string(self.log_file()) {
            let match_expr = Regex::new(&format!("\\[(.+)T(.+)\\+.+\\] \\[ALPM\\] (installed|removed|upgraded|downgraded) ({}) (.+)", self.pkg().name())).unwrap();

            let log_lines: Vec<String> = log.lines().rev()
                .filter_map(|s|
                    if match_expr.is_match(s).unwrap_or_default() {
                        Some(match_expr.replace_all(s, "[$1 $2]  $3 $4 $5").to_string())
                    } else {
                        None
                    }
                )
                .collect();

            imp.log_model.splice(0, 0, &log_lines.iter().map(|s| s.as_str()).collect::<Vec<&str>>());
        }

        // Set copy button state
        let n_files = imp.log_selection.n_items();

        imp.log_copy_button.set_sensitive(n_files > 0);
    }

    //-----------------------------------
    // Setup cache page
    //-----------------------------------
    fn setup_cache(&self) {
        let imp = self.imp();

        // Populate cache files list
        let cmd = format!("/usr/bin/paccache -vdk0 {}", self.pkg().name());

        let (_code, stdout) = Utils::run_command(&cmd);

        let cache_lines: Vec<&str> = stdout.lines()
            .filter(|s| !s.is_empty() && !s.starts_with("==>") && !s.ends_with(".sig"))
            .collect();

        imp.cache_model.splice(0, 0, &cache_lines);

        // Set cache header label
        let n_files = imp.cache_selection.n_items();

        imp.cache_header_label.set_label(&format!("Cache ({})", n_files));

        // Set open/copy button states
        imp.cache_open_button.set_sensitive(n_files > 0);
        imp.cache_copy_button.set_sensitive(n_files > 0);
    }

    //-----------------------------------
    // Setup backup page
    //-----------------------------------
    fn setup_backup(&self) {
        let imp = self.imp();

        // Populate backup list
        let backup = self.pkg().backup();

        let backup_list: Vec<BackupObject> = backup.iter()
            .map(|backup| {
                let (name, hash) = backup.split_once(" || ").unwrap();

                let mut status_icon = "backup-error";
                let mut status = "read error";

                // Open backup file
                if let Ok(file) = fs::File::open(name) {
                    // Get file size
                    let file_len = file.metadata().unwrap().len();

                    // Define buffer size
                    let buffer_len = file_len.min(4096) as usize;

                    // Create read buffer
                    let mut buffer = BufReader::with_capacity(buffer_len, file);

                    // Create new MD5 context
                    let mut context = md5::Context::new();

                    loop {
                        // Get a chunk of the file
                        let chunk = buffer.fill_buf().unwrap();

                        // Break if chunk is empty (EOF reached)
                        if chunk.is_empty() {
                            break;
                        }
                        // Add chunk to the MD5 context
                        context.consume(chunk);

                        // Tell the buffer that the chunk is consumed
                        let chunk_len = chunk.len();
                        buffer.consume(chunk_len);
                    }

                    // Compute MD5 hash for file
                    let u8_hash = context.compute();

                    // Convert MD5 hash to string
                    let file_hash = format!("{:x}", u8_hash);

                    status_icon = if file_hash == hash {"backup-unchanged"} else {"backup-changed"};
                    status = if file_hash == hash {"unchanged"} else {"changed"};
                } else {
                    
                }

                BackupObject::new(name, status_icon, status)
            })
            .collect();

        imp.backup_model.splice(0, 0, &backup_list);

        // Set backup header label
        let n_files = imp.backup_selection.n_items();

        imp.backup_header_label.set_label(&format!("Backup Files ({})", n_files));

        // Set open/copy button states
        imp.backup_open_button.set_sensitive(n_files > 0);
        imp.backup_copy_button.set_sensitive(n_files > 0);
    }
}
