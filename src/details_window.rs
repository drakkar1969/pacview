use std::cell::{Cell, RefCell};
use std::fs;
use std::io::{BufReader, BufRead};
use std::borrow::Cow;

use gtk::{gio, glib, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use gtk::pango::AttrList;
use glib::clone;

use fancy_regex::Regex;
use lazy_static::lazy_static;
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
        default_tree_depth: Cell<f64>,
        #[property(get, set)]
        tree_text: RefCell<String>,
        #[property(get, set)]
        tree_rev_text: RefCell<String>,

        #[property(get, set)]
        cache_dir: RefCell<String>,
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

            let obj = self.obj();

            // Close window on ESC
            let controller = gtk::EventControllerKey::new();

            controller.set_propagation_phase(gtk::PropagationPhase::Capture);

            controller.connect_key_pressed(clone!(@weak obj => @default-return gtk::Inhibit(false), move |_, key, _, state| {
                if key == gdk::Key::Escape && state.is_empty() {
                    obj.close();

                    gtk::Inhibit(true)
                } else {
                    gtk::Inhibit(false)
                }

            }));

            obj.add_controller(controller);
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

                let _res = desktop.launch_uris(&[&path], None::<&gio::AppLaunchContext>);
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
        // Filter dependency tree helper function
        //-----------------------------------
        fn filter_dependency_tree(&self) {
            let obj = self.obj();

            let depth = self.tree_depth_scale.value();
            let reverse = self.tree_reverse_button.is_active();

            let tree_text = if reverse {obj.tree_rev_text()} else {obj.tree_text()};

            lazy_static! {
                static ref EXPR: Regex = Regex::new("([└|─|│|├| ]+)?(.+)").unwrap();
            }

            let filter_text = if depth == obj.default_tree_depth() {
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

            self.tree_label.set_label(&filter_text);
        }

        //-----------------------------------
        // Tree page signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_tree_depth_changed(&self, scale: gtk::Scale) {
            if scale.value() == self.obj().default_tree_depth() {
                self.tree_depth_label.set_label("Default");
            } else {
                self.tree_depth_label.set_label(&scale.value().to_string());
            }

            self.filter_dependency_tree();
        }

        #[template_callback]
        fn on_tree_reverse_toggled(&self) {
            self.filter_dependency_tree();
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
    pub fn new(pkg: &PkgObject, custom_font: bool, monospace_font: &str, log_file: &str, cache_dir: &str) -> Self {
        let win: Self = glib::Object::builder()
            .property("pkg", pkg)
            .property("cache-dir", cache_dir)
            .build();

        let installed = pkg.flags().intersects(PkgFlags::INSTALLED);

        win.setup_banner();
        win.setup_stack(installed);

        if installed { win.setup_files(); }
        win.setup_tree(custom_font, monospace_font);
        if installed { win.setup_logs(log_file); }
        if installed { win.setup_cache(); }
        if installed { win.setup_backup(); }

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
    // Setup stack
    //-----------------------------------
    fn setup_stack(&self, installed: bool) {
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
    // Setup files page
    //-----------------------------------
    fn setup_files(&self) {
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
    // Setup tree page
    //-----------------------------------
    fn setup_tree(&self, custom_font: bool, monospace_font: &str) {
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
    // Setup logs page
    //-----------------------------------
    fn setup_logs(&self, log_file: &str) {
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

        imp.cache_header_label.set_label(&format!("Cache Files ({})", n_files));

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
                    if let Ok(file_len) = file.metadata().and_then(|m| Ok(m.len())) {
                        // Define buffer size
                        let buffer_len = file_len.min(4096) as usize;

                        // Create read buffer
                        let mut buffer = BufReader::with_capacity(buffer_len, file);

                        // Create new MD5 context
                        let mut context = md5::Context::new();

                        let res = loop {
                            // Get a chunk of the file
                            if let Ok(chunk) = buffer.fill_buf() {
                                // Break with true if chunk is empty (EOF reached)
                                if chunk.is_empty() {
                                    break true;
                                }

                                // Add chunk to the MD5 context
                                context.consume(chunk);

                                // Tell the buffer that the chunk is consumed
                                let chunk_len = chunk.len();
                                buffer.consume(chunk_len);
                            } else {
                                // Break with false if buffer error
                                break false;
                            }
                        };

                        if res {
                            // Compute MD5 hash for file
                            let u8_hash = context.compute();

                            // Convert MD5 hash to string
                            let file_hash = format!("{:x}", u8_hash);

                            // Get item status icon and text
                            status_icon = if file_hash == hash {"backup-unmodified"} else {"backup-modified"};
                            status = if file_hash == hash {"unmodified"} else {"modified"};
                        }
                    }
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
