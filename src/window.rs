use std::cell::{Cell, RefCell, OnceCell};
use std::sync::{OnceLock, LazyLock, Arc, Mutex};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use std::io::Read;
use std::str::FromStr;
use std::fs;

use gtk::{gio, glib, gdk};
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::{clone, closure_local};

use alpm_utils::DbListExt;
use titlecase::titlecase;
use regex::Regex;
use futures::join;
use flate2::read::GzDecoder;
use notify_debouncer_full::{notify::*, new_debouncer, Debouncer, DebounceEventResult, NoCache};

use crate::utils::{async_command, tokio_runtime};
use crate::APP_ID;
use crate::PacViewApplication;
use crate::pkg_data::{PkgFlags, PkgData};
use crate::pkg_object::{ALPM_HANDLE, PkgObject};
use crate::search_bar::{SearchBar, SearchMode, SearchProp};
use crate::package_view::{PackageView, PackageViewStatus, SortProp};
use crate::info_pane::InfoPane;
use crate::filter_row::{FilterRow, Updates};
use crate::stats_window::StatsWindow;
use crate::backup_window::BackupWindow;
use crate::groups_window::GroupsWindow;
use crate::log_window::LogWindow;
use crate::cache_window::CacheWindow;
use crate::config_dialog::ConfigDialog;
use crate::preferences_dialog::{PreferencesDialog, ColorScheme};
use crate::enum_traits::EnumExt;

//------------------------------------------------------------------------------
// GLOBAL VARIABLES
//------------------------------------------------------------------------------
thread_local! {
    pub static PKGS: RefCell<Vec<PkgObject>> = const { RefCell::new(vec![]) };
    pub static INSTALLED_PKGS: RefCell<Vec<PkgObject>> = const { RefCell::new(vec![]) };
    pub static INSTALLED_PKG_NAMES: RefCell<Arc<HashSet<String>>> = RefCell::new(Arc::new(HashSet::new()));
}

pub static PACMAN_CONFIG: OnceLock<pacmanconf::Config> = OnceLock::new();
pub static PACMAN_LOG: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));
pub static PACMAN_CACHE: LazyLock<Mutex<Vec<PathBuf>>> = LazyLock::new(|| Mutex::new(vec![]));

//------------------------------------------------------------------------------
// MODULE: PacViewWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/window.ui")]
    pub struct PacViewWindow {
        #[template_child]
        pub(super) sidebar_breakpoint: TemplateChild<adw::Breakpoint>,
        #[template_child]
        pub(super) main_breakpoint: TemplateChild<adw::Breakpoint>,

        #[template_child]
        pub(super) sidebar_split_view: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub(super) main_split_view: TemplateChild<adw::OverlaySplitView>,

        #[template_child]
        pub(super) sidebar_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) sort_button: TemplateChild<adw::SplitButton>,
        #[template_child]
        pub(super) search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) infopane_button: TemplateChild<gtk::ToggleButton>,

        #[template_child]
        pub(super) repo_listbox: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub(super) status_listbox: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub(super) status_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub(super) search_bar: TemplateChild<SearchBar>,
        #[template_child]
        pub(super) package_view: TemplateChild<PackageView>,

        #[template_child]
        pub(super) info_pane: TemplateChild<InfoPane>,

        #[template_child]
        pub(super) prefs_dialog: TemplateChild<PreferencesDialog>,
        #[template_child]
        pub(super) config_dialog: TemplateChild<ConfigDialog>,

        #[template_child]
        pub(super) backup_window: TemplateChild<BackupWindow>,
        #[template_child]
        pub(super) log_window: TemplateChild<LogWindow>,
        #[template_child]
        pub(super) cache_window: TemplateChild<CacheWindow>,
        #[template_child]
        pub(super) groups_window: TemplateChild<GroupsWindow>,
        #[template_child]
        pub(super) stats_window: TemplateChild<StatsWindow>,

        pub(super) gsettings: OnceCell<gio::Settings>,

        pub(super) aur_file: OnceCell<PathBuf>,

        pub(super) pacman_repos: OnceCell<Vec<String>>,

        pub(super) saved_repo_id: RefCell<Option<String>>,
        pub(super) saved_status_id: Cell<PkgFlags>,

        pub(super) all_repo_row: RefCell<FilterRow>,
        pub(super) all_status_row: RefCell<FilterRow>,
        pub(super) update_row: RefCell<FilterRow>,

        pub(super) notify_debouncer: OnceCell<Debouncer<INotifyWatcher, NoCache>>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PacViewWindow {
        const NAME: &'static str = "PacViewWindow";
        type Type = super::PacViewWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            // Search start/stop key bindings
            klass.add_binding(gdk::Key::F, gdk::ModifierType::CONTROL_MASK, |window| {
                window.imp().search_bar.set_enabled(true);

                glib::Propagation::Stop
            });

            klass.add_binding(gdk::Key::Escape, gdk::ModifierType::NO_MODIFIER_MASK, |window| {
                let imp = window.imp();

                if (imp.sidebar_split_view.is_collapsed() && imp.sidebar_split_view.shows_sidebar()) || (imp.main_split_view.is_collapsed() && imp.main_split_view.shows_sidebar()) {
                    glib::Propagation::Proceed
                } else {
                    window.imp().search_bar.set_enabled(false);

                    glib::Propagation::Stop
                }
            });

            // Show sidebar key binding
            klass.add_binding_action(gdk::Key::B, gdk::ModifierType::CONTROL_MASK, "win.show-sidebar");

            // Show infopane key binding
            klass.add_binding_action(gdk::Key::I, gdk::ModifierType::CONTROL_MASK, "win.show-infopane");

            // Show preferences key binding
            klass.add_binding_action(gdk::Key::comma, gdk::ModifierType::CONTROL_MASK, "win.show-preferences");

            // View refresh key binding
            klass.add_binding_action(gdk::Key::F5, gdk::ModifierType::NO_MODIFIER_MASK, "win.refresh");

            // View update AUR database key binding
            klass.add_binding_action(gdk::Key::F7, gdk::ModifierType::NO_MODIFIER_MASK, "win.update-aur-database");

            // View copy list key binding
            klass.add_binding_action(gdk::Key::C, gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK, "win.copy-package-list");

            // View show all packages key binding
            klass.add_binding_action(gdk::Key::A, gdk::ModifierType::ALT_MASK, "win.show-all-packages");

            // Stats window key binding
            klass.add_binding_action(gdk::Key::S, gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK, "win.show-stats");

            // Backup files window key binding
            klass.add_binding_action(gdk::Key::B, gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK, "win.show-backup-files");

            // Pacman log window key binding
            klass.add_binding_action(gdk::Key::L, gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK, "win.show-pacman-log");

            // Pacman cache window key binding
            klass.add_binding_action(gdk::Key::H, gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK, "win.show-pacman-cache");

            // Pacman groups window key binding
            klass.add_binding_action(gdk::Key::G, gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK, "win.show-pacman-groups");

            // Pacman config dialog key binding
            klass.add_binding_action(gdk::Key::P, gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK, "win.show-pacman-config");

            // Infopane set tab shortcuts
            klass.add_shortcut(&gtk::Shortcut::with_arguments(
                gtk::ShortcutTrigger::parse_string("<alt>I"),
                Some(gtk::NamedAction::new("win.infopane-set-tab")),
                &"info".to_variant()
            ));

            klass.add_shortcut(&gtk::Shortcut::with_arguments(
                gtk::ShortcutTrigger::parse_string("<alt>F"),
                Some(gtk::NamedAction::new("win.infopane-set-tab")),
                &"files".to_variant()
            ));

            klass.add_shortcut(&gtk::Shortcut::with_arguments(
                gtk::ShortcutTrigger::parse_string("<alt>L"),
                Some(gtk::NamedAction::new("win.infopane-set-tab")),
                &"log".to_variant()
            ));

            klass.add_shortcut(&gtk::Shortcut::with_arguments(
                gtk::ShortcutTrigger::parse_string("<alt>C"),
                Some(gtk::NamedAction::new("win.infopane-set-tab")),
                &"cache".to_variant()
            ));

            klass.add_shortcut(&gtk::Shortcut::with_arguments(
                gtk::ShortcutTrigger::parse_string("<alt>B"),
                Some(gtk::NamedAction::new("win.infopane-set-tab")),
                &"backup".to_variant()
            ));

            // Infopane previous/next key bindings
            klass.add_binding(gdk::Key::Left, gdk::ModifierType::ALT_MASK, |window| {
                window.imp().info_pane.display_prev();

                glib::Propagation::Stop
            });

            klass.add_binding(gdk::Key::Right, gdk::ModifierType::ALT_MASK, |window| {
                window.imp().info_pane.display_next();

                glib::Propagation::Stop
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PacViewWindow {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.init_gsettings();
            obj.load_gsettings();

            obj.init_cache_dir();

            obj.setup_widgets();
            obj.setup_actions();
            obj.setup_signals();

            obj.setup_alpm(true);

            obj.setup_inotify();
        }
    }

    impl WidgetImpl for PacViewWindow {}

    impl WindowImpl for PacViewWindow {
        //---------------------------------------
        // Window close handler
        //---------------------------------------
        fn close_request(&self) -> glib::Propagation {
            self.obj().save_gsettings();

            glib::Propagation::Proceed
        }
    }

    impl ApplicationWindowImpl for PacViewWindow {}
    impl AdwApplicationWindowImpl for PacViewWindow {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PacViewWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PacViewWindow(ObjectSubclass<imp::PacViewWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl PacViewWindow {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(app: &PacViewApplication) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    //---------------------------------------
    // Init gsettings
    //---------------------------------------
    fn init_gsettings(&self) {
        let gsettings = gio::Settings::new(APP_ID);

        self.imp().gsettings.set(gsettings).unwrap();
    }

    //---------------------------------------
    // Load gsettings
    //---------------------------------------
    fn load_gsettings(&self) {
        let imp = self.imp();

        let gsettings = imp.gsettings.get().unwrap();

        // Load window settings
        self.set_default_width(gsettings.int("window-width"));
        self.set_default_height(gsettings.int("window-height"));
        self.set_maximized(gsettings.boolean("window-maximized"));

        // Load preferences
        let color_scheme = ColorScheme::from_str(&gsettings.string("color-scheme")).unwrap();
        imp.prefs_dialog.set_color_scheme(color_scheme);

        imp.prefs_dialog.set_auto_refresh(gsettings.boolean("auto-refresh"));
        imp.prefs_dialog.set_aur_command(gsettings.string("aur-update-command"));
        imp.prefs_dialog.set_aur_check(gsettings.boolean("aur-package-check"));
        imp.prefs_dialog.set_sidebar_width(gsettings.double("sidebar-width"));
        imp.prefs_dialog.set_infopane_width(gsettings.double("infopane-width"));

        let search_mode = SearchMode::from_str(&gsettings.string("search-mode")).unwrap();
        imp.prefs_dialog.set_search_mode(search_mode);
        imp.search_bar.set_mode(search_mode);

        let search_prop = SearchProp::from_str(&gsettings.string("search-prop")).unwrap();
        imp.prefs_dialog.set_search_prop(search_prop);
        imp.search_bar.set_prop(search_prop);

        imp.prefs_dialog.set_search_delay(gsettings.double("search-delay"));
        imp.prefs_dialog.set_remember_sort(gsettings.boolean("remember-sorting"));
        imp.prefs_dialog.set_property_max_lines(gsettings.double("property-max-lines"));
        imp.prefs_dialog.set_property_line_spacing(gsettings.double("property-line-spacing"));
        imp.prefs_dialog.set_underline_links(gsettings.boolean("underline-links"));

        // Load package view sort prop/order
        if imp.prefs_dialog.remember_sort() {
            let sort_prop = SortProp::from_str(&gsettings.string("sort-prop")).unwrap();
            imp.package_view.set_sort_prop(sort_prop);

            imp.package_view.set_sort_ascending(gsettings.boolean("sort-ascending"));
        }
    }

    //---------------------------------------
    // Set gsetting helper function
    //---------------------------------------
    fn set_gsetting<T: FromVariant + ToVariant + PartialEq>(gsettings: &gio::Settings, key: &str, value: &T) {
        let default: T = gsettings.default_value(key)
            .expect("Failed to get gsettings default value")
            .get::<T>()
            .expect("Failed to retrieve value from variant");

        if !(default == *value && default == gsettings.get(key)) {
            gsettings.set(key, value.to_variant()).unwrap();
        }
    }

    //---------------------------------------
    // Save gsettings
    //---------------------------------------
    fn save_gsettings(&self) {
        let imp = self.imp();

        let gsettings = imp.gsettings.get().unwrap();

        // Save window settings
        let (width, height) = self.default_size();

        Self::set_gsetting(gsettings, "window-width", &width);
        Self::set_gsetting(gsettings, "window-height", &height);
        Self::set_gsetting(gsettings, "window-maximized", &self.is_maximized());

        // Save preferences
        Self::set_gsetting(gsettings, "color-scheme", &imp.prefs_dialog.color_scheme().nick());
        Self::set_gsetting(gsettings, "auto-refresh", &imp.prefs_dialog.auto_refresh());
        Self::set_gsetting(gsettings, "aur-update-command", &imp.prefs_dialog.aur_command());
        Self::set_gsetting(gsettings, "aur-package-check", &imp.prefs_dialog.aur_check());
        Self::set_gsetting(gsettings, "sidebar-width", &imp.prefs_dialog.sidebar_width());
        Self::set_gsetting(gsettings, "infopane-width", &imp.prefs_dialog.infopane_width());
        Self::set_gsetting(gsettings, "search-mode", &imp.prefs_dialog.search_mode().nick());
        Self::set_gsetting(gsettings, "search-prop", &imp.prefs_dialog.search_prop().nick());
        Self::set_gsetting(gsettings, "search-delay", &imp.prefs_dialog.search_delay());
        Self::set_gsetting(gsettings, "remember-sorting", &imp.prefs_dialog.remember_sort());
        Self::set_gsetting(gsettings, "property-max-lines", &imp.prefs_dialog.property_max_lines());
        Self::set_gsetting(gsettings, "property-line-spacing", &imp.prefs_dialog.property_line_spacing());
        Self::set_gsetting(gsettings, "underline-links", &imp.prefs_dialog.underline_links());

        // Save package view sort prop/order
        if imp.prefs_dialog.remember_sort() {
            Self::set_gsetting(gsettings, "sort-prop", &imp.package_view.sort_prop().nick());
            Self::set_gsetting(gsettings, "sort-ascending", &imp.package_view.sort_ascending());
        } else {
            Self::set_gsetting(gsettings, "sort-prop", &SortProp::default().nick());
            Self::set_gsetting(gsettings, "sort-ascending", &true);
        }
    }

    //---------------------------------------
    // Init cache dir
    //---------------------------------------
    fn init_cache_dir(&self) {
        if let Some(aur_file) = xdg::BaseDirectories::new().ok()
            .and_then(|xdg_dirs| xdg_dirs.create_cache_directory("pacview").ok())
            .map(|cache_dir| cache_dir.join("aur_packages"))
        {
            self.imp().aur_file.set(aur_file).unwrap();
        }
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind sidebar button state to sidebar visibility
        imp.sidebar_button.bind_property("active", &imp.sidebar_split_view.get(), "show-sidebar")
            .sync_create()
            .bidirectional()
            .build();

        // Bind package view sort order to sort button icon/tooltip
        imp.package_view.bind_property("sort-ascending", &imp.sort_button.get(), "icon-name")
            .transform_to(|_, sort_asc: bool| Some(if sort_asc { "view-sort-ascending-symbolic" } else { "view-sort-descending-symbolic" }))
            .sync_create()
            .build();

        imp.package_view.bind_property("sort-ascending", &imp.sort_button.get(), "tooltip-text")
            .transform_to(|_, sort_asc: bool| Some(if sort_asc { "Descending" } else { "Ascending" }))
            .sync_create()
            .build();

        // Bind search button state to search bar enabled state
        imp.search_button.bind_property("active", &imp.search_bar.get(), "enabled")
            .sync_create()
            .bidirectional()
            .build();

        // Bind infopane button state to infopane visibility
        imp.infopane_button.bind_property("active", &imp.main_split_view.get(), "show-sidebar")
            .sync_create()
            .bidirectional()
            .build();

        // Bind search bar default search mode preference
        imp.prefs_dialog.bind_property("search-mode", &imp.search_bar.get(), "default-mode")
            .sync_create()
            .build();

        // Bind search bar default search prop preference
        imp.prefs_dialog.bind_property("search-prop", &imp.search_bar.get(), "default-prop")
            .sync_create()
            .build();

        // Bind search bar delay preference
        imp.prefs_dialog.bind_property("search-delay", &imp.search_bar.get(), "delay")
            .sync_create()
            .build();

        // Set search bar key capture widget
        imp.search_bar.set_key_capture_widget(imp.package_view.view().upcast_ref());

        // Bind package view item count to status label text
        imp.package_view.bind_property("n-items", &imp.status_label.get(), "label")
            .transform_to(|_, n_items: u32| {
                Some(format!("{n_items} matching package{}", if n_items == 1 { "" } else { "s" }))
            })
            .sync_create()
            .build();

        // Bind info pane property preferences
        imp.prefs_dialog.bind_property("property-max-lines", &imp.info_pane.get(), "property-max-lines")
            .transform_to(|_, lines: f64| Some(lines as i32))
            .sync_create()
            .build();

        imp.prefs_dialog.bind_property("property-line-spacing", &imp.info_pane.get(), "property-line-spacing")
            .sync_create()
            .build();

        imp.prefs_dialog.bind_property("underline-links", &imp.info_pane.get(), "underline-links")
            .sync_create()
            .build();

        self.resize_window();
    }

    //---------------------------------------
    // Setup actions
    //---------------------------------------
    fn setup_actions(&self) {
        let imp = self.imp();

        // Pane visibility property actions
        let show_sidebar_action = gio::PropertyAction::new("show-sidebar", &imp.sidebar_split_view.get(), "show-sidebar");

        let show_infopane_action = gio::PropertyAction::new("show-infopane", &imp.main_split_view.get(), "show-sidebar");

        // Add pane visibility actions to window
        self.add_action(&show_sidebar_action);
        self.add_action(&show_infopane_action);

        // Package view refresh action
        let refresh_action = gio::ActionEntry::builder("refresh")
            .activate(|window: &Self, _, _| {
                let imp = window.imp();

                let repo_id = imp.repo_listbox.selected_row()
                    .and_downcast::<FilterRow>()
                    .and_then(|row| row.repo_id());

                imp.saved_repo_id.replace(repo_id);

                let status_id = imp.status_listbox.selected_row()
                    .and_downcast::<FilterRow>()
                    .map(|row| row.status_id())
                    .unwrap_or_default();

                imp.saved_status_id.set(status_id);

                window.setup_alpm(false);
            })
            .build();

        // Package view update AUR database action
        let aur_action = gio::ActionEntry::builder("update-aur-database")
            .activate(|window: &Self, _, _| {
                let imp = window.imp();

                if let Some(aur_file) = imp.aur_file.get() {
                    imp.update_row.borrow().set_status(Updates::Output(None, 0));
                    imp.package_view.set_status(PackageViewStatus::AURDownload);
                    imp.info_pane.set_pkg(None::<PkgObject>);

                    // Spawn tokio task to download AUR package names file
                    Self::download_aur_names_async(aur_file, clone!(
                        #[weak] window,
                        move || {
                            window.imp().package_view.set_status(PackageViewStatus::Normal);

                            // Refresh packages
                            ActionGroupExt::activate_action(&window, "refresh", None);
                        }
                    ));
                }
            })
            .build();

        // Package view copy list action
        let copy_action = gio::ActionEntry::builder("copy-package-list")
            .activate(|window: &Self, _, _| {
                window.clipboard().set_text(&window.imp().package_view.copy_list());
            })
            .build();

        // Package view all packages action
        let all_pkgs_action = gio::ActionEntry::builder("show-all-packages")
            .activate(|window: &Self, _, _| {
                let imp = window.imp();

                imp.all_repo_row.borrow().activate();
                imp.all_status_row.borrow().activate();
            })
            .build();

        // Package view reset sort action
        let reset_sort_action = gio::ActionEntry::builder("reset-package-sort")
            .activate(|window: &Self, _, _| {
                let imp = window.imp();

                imp.package_view.set_sort_prop(SortProp::default());
                imp.package_view.set_sort_ascending(true);
            })
            .build();

        // Package view sort prop property action
        let sort_prop_action = gio::PropertyAction::new("set-sort-prop", &imp.package_view.get(), "sort-prop");

        // Add package view actions to window
        self.add_action_entries([refresh_action, aur_action, copy_action, all_pkgs_action, reset_sort_action]);
        self.add_action(&sort_prop_action);

        // Bind package view item count to copy list action enabled state
        let copy_action = self.lookup_action("copy-package-list").unwrap();

        imp.package_view.bind_property("n-items", &copy_action, "enabled")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .sync_create()
            .build();

        // Info pane set tab action with parameter
        let visible_tab_action = gio::ActionEntry::builder("infopane-set-tab", )
            .parameter_type(Some(&String::static_variant_type()))
            .activate(|window: &Self, _, param| {
                let tab = param
                    .expect("Failed to retrieve Variant")
                    .get::<String>()
                    .expect("Failed to retrieve String from variant");

                window.imp().info_pane.set_visible_tab(&tab);
            })
            .build();

        // Add info pane actions to window
        self.add_action_entries([visible_tab_action]);

        // Show stats window action
        let stats_action = gio::ActionEntry::builder("show-stats")
            .activate(|window: &Self, _, _| {
                let imp = window.imp();

                imp.stats_window.show(imp.pacman_repos.get().unwrap());
            })
            .build();

        // Show backup files window action
        let backup_action = gio::ActionEntry::builder("show-backup-files")
            .activate(|window: &Self, _, _| {
                window.imp().backup_window.show();
            })
            .build();

        // Show pacman log window action
        let log_action = gio::ActionEntry::builder("show-pacman-log")
            .activate(|window: &Self, _, _| {
                window.imp().log_window.show();
            })
            .build();

        // Show pacman cache window action
        let cache_action = gio::ActionEntry::builder("show-pacman-cache")
            .activate(|window: &Self, _, _| {
                window.imp().cache_window.show();
            })
            .build();

        // Show pacman groups window action
        let groups_action = gio::ActionEntry::builder("show-pacman-groups")
            .activate(|window: &Self, _, _| {
                window.imp().groups_window.show();
            })
            .build();

        // Show pacman config window action
        let config_action = gio::ActionEntry::builder("show-pacman-config")
            .activate(|window: &Self, _, _| {
                window.imp().config_dialog.present(Some(window));
            })
            .build();

        // Add window actions to window
        self.add_action_entries([stats_action, backup_action, log_action, cache_action, groups_action, config_action]);

        // Show preferences action
        let prefs_action = gio::ActionEntry::builder("show-preferences")
            .activate(|window: &Self, _, _| {
                window.imp().prefs_dialog.present(Some(window));
            })
            .build();

        // Add preference actions to window
        self.add_action_entries([prefs_action]);
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Header sort button clicked signal
        imp.sort_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                imp.package_view.set_sort_ascending(!imp.package_view.sort_ascending());
            }
        ));

        // Repo listbox row activated signal
        imp.repo_listbox.connect_row_activated(clone!(
            #[weak] imp,
            move |_, row| {
                let repo_id = row
                    .downcast_ref::<FilterRow>()
                    .expect("Failed to downcast to 'FilterRow'")
                    .repo_id();

                imp.package_view.repo_filter_changed(repo_id.as_deref());

                if imp.sidebar_split_view.is_collapsed() {
                    imp.sidebar_split_view.set_show_sidebar(false);
                }

                imp.package_view.view().grab_focus();
            }
        ));

        // Status listbox row activated signal
        imp.status_listbox.connect_row_activated(clone!(
            #[weak] imp,
            move |_, row| {
                let status_id = row
                    .downcast_ref::<FilterRow>()
                    .expect("Failed to downcast to 'FilterRow'")
                    .status_id();

                imp.package_view.status_filter_changed(status_id);

                if imp.sidebar_split_view.is_collapsed() {
                    imp.sidebar_split_view.set_show_sidebar(false);
                }

                imp.package_view.view().grab_focus();
            }
        ));

        // Search bar enabled property notify signal
        imp.search_bar.connect_enabled_notify(clone!(
            #[weak] imp,
            move |bar| {
                if !bar.enabled() {
                    imp.package_view.cancel_aur_search();

                    imp.package_view.view().grab_focus();
                }
            }
        ));

        // Search bar changed signal
        imp.search_bar.connect_closure("changed", false, closure_local!(
            #[watch(rename_to = window)] self,
            move |_: SearchBar, search_term: &str, mode: SearchMode, prop: SearchProp| {
                window.imp().package_view.search_filter_changed(search_term, mode, prop);
            }
        ));

        // Search bar AUR Search signal
        imp.search_bar.connect_closure("aur-search", false, closure_local!(
            #[watch(rename_to = window)] self,
            move |search_bar: &SearchBar, search_term: &str, prop: SearchProp| {
                window.imp().package_view.search_in_aur(search_bar, search_term, prop);
            }
        ));

        // Package view selected signal
        imp.package_view.connect_closure("selected", false, closure_local!(
            #[watch(rename_to = window)] self,
            move |_: PackageView, pkg: Option<PkgObject>| {
                window.imp().info_pane.set_pkg(pkg.as_ref());
            }
        ));

        // Package view activate signal
        imp.package_view.connect_closure("activated", false, closure_local!(
            #[watch(rename_to = window)] self,
            move |_: PackageView, pkg: Option<PkgObject>| {
                let imp = window.imp();

                if pkg != imp.info_pane.pkg() {
                    imp.info_pane.set_pkg(pkg.as_ref());
                }
            }
        ));

        // Preferences aur check property notify signal
        imp.prefs_dialog.connect_aur_check_notify(clone!(
            #[weak(rename_to = window)] self,
            move |_| {
                ActionGroupExt::activate_action(&window, "refresh", None);
            }
        ));

        // Preferences sidebar width property notify signal
        imp.prefs_dialog.connect_sidebar_width_notify(clone!(
            #[weak(rename_to = window)] self,
            move |_| {
                window.resize_window();
            }
        ));

        // Preferences infopane width property notify signal
        imp.prefs_dialog.connect_infopane_width_notify(clone!(
            #[weak(rename_to = window)] self,
            move |_| {
                window.resize_window();
            }
        ));
    }

    //---------------------------------------
    // Resize window helper function
    //---------------------------------------
    fn resize_window(&self) {
        let imp = self.imp();

        let sidebar_width = imp.prefs_dialog.sidebar_width();
        let infopane_width = imp.prefs_dialog.infopane_width();

        let min_packageview_width = 500.0;

        self.set_width_request(infopane_width as i32);

        imp.sidebar_split_view.set_min_sidebar_width(sidebar_width);
        imp.sidebar_split_view.set_max_sidebar_width(sidebar_width);

        imp.main_split_view.set_min_sidebar_width(infopane_width);

        imp.main_breakpoint.set_condition(Some(
            &adw::BreakpointCondition::new_length(
                adw::BreakpointConditionLengthType::MaxWidth,
                sidebar_width + infopane_width + min_packageview_width,
                adw::LengthUnit::Sp
            )
        ));

        imp.sidebar_breakpoint.set_condition(Some(
            &adw::BreakpointCondition::new_length(
                adw::BreakpointConditionLengthType::MaxWidth,
                sidebar_width + infopane_width,
                adw::LengthUnit::Sp
            )
        ));
    }

    //---------------------------------------
    // Download AUR names helper function
    //---------------------------------------
    fn download_aur_names_async<F>(aur_file: &PathBuf, f: F)
    where F: Fn() + 'static {
        let (sender, receiver) = async_channel::bounded(1);

        let aur_file = aur_file.to_owned();

        tokio_runtime::runtime().spawn(
            async move {
                let url = "https://aur.archlinux.org/packages.gz";

                let response = reqwest::get(url).await?;

                let bytes = response.bytes().await?;

                let mut decoder = GzDecoder::new(&bytes[..]);

                let mut gz_string = String::new();

                if decoder.read_to_string(&mut gz_string).is_ok() {
                    fs::write(&aur_file, gz_string).unwrap_or_default();
                }

                sender.send(()).await.expect("Failed to send through channel");

                Ok::<(), reqwest::Error>(())
            }
        );

        glib::spawn_future_local(
            async move {
                while receiver.recv().await == Ok(()) {
                    f();
                }
            }
        );
    }

    //---------------------------------------
    // Setup alpm
    //---------------------------------------
    fn setup_alpm(&self, first_load: bool) {
        let imp = self.imp();

        if first_load {
            self.get_pacman_config();
        }

        self.populate_sidebar(first_load);

        // If first load, check AUR file
        if let Some(aur_file) = imp.aur_file.get().filter(|_| first_load) {
            if fs::exists(aur_file).is_ok_and(|exists| exists) {
                // AUR file exists: load packages and check AUR file age
                self.load_packages(true);
            } else {
                // If AUR file does not exist, download it
                imp.package_view.set_status(PackageViewStatus::AURDownload);
                imp.info_pane.set_pkg(None::<PkgObject>);

                // Spawn tokio task to download AUR package names file
                Self::download_aur_names_async(aur_file, clone!(
                    #[weak(rename_to = window)] self,
                    move || {
                        window.imp().package_view.set_status(PackageViewStatus::Normal);

                        // Load packages, no AUR file age check
                        window.load_packages(false);
                    }
                ));
            }
        } else {
            // Not first load or path of AUR file is invalid: load packages, no AUR file age check
            self.load_packages(false);
        }
    }

    //---------------------------------------
    // Setup alpm: get pacman config
    //---------------------------------------
    fn get_pacman_config(&self) {
        let imp = self.imp();

        // Get pacman config
        let pacman_config = pacmanconf::Config::new()
            .expect("Failed to get pacman config");

        // Get pacman repositories
        let pacman_repos: Vec<String> = pacman_config.repos.iter()
            .map(|r| r.name.clone())
            .chain([String::from("aur"), String::from("local")])
            .collect();

        // Init config dialog
        imp.config_dialog.init(&pacman_config);

        // Store pacman config
        PACMAN_CONFIG.set(pacman_config).unwrap();

        // Store pacman repos
        imp.pacman_repos.set(pacman_repos).unwrap();
    }

    //---------------------------------------
    // Setup alpm: populate sidebar
    //---------------------------------------
    fn populate_sidebar(&self, first_load: bool) {
        let imp = self.imp();

        // Add repository rows (enumerate pacman repositories)
        imp.repo_listbox.remove_all();

        let saved_repo_id = imp.saved_repo_id.take();

        let all_row = FilterRow::new("repository-symbolic", "All", None, PkgFlags::empty());

        imp.repo_listbox.append(&all_row);

        if saved_repo_id.is_none() {
            all_row.activate();
        }

        imp.all_repo_row.replace(all_row);

        let pacman_repos = imp.pacman_repos.get().unwrap();

        for repo in pacman_repos {
            let label = if repo == "aur" { repo.to_uppercase() } else { titlecase(repo) };

            let row = FilterRow::new("repository-symbolic", &label, Some(repo), PkgFlags::empty());

            imp.repo_listbox.append(&row);

            if saved_repo_id.as_ref() == Some(repo) {
                row.activate();
            }
        }

        // If first load, add package status rows (enumerate PkgStatusFlags)
        if first_load {
            let saved_status_id = imp.saved_status_id.replace(PkgFlags::empty());

            let flags = glib::FlagsClass::new::<PkgFlags>();

            for f in flags.values() {
                let flag = PkgFlags::from_bits_truncate(f.value());
                let nick = f.nick();

                let row = FilterRow::new(&format!("status-{nick}-symbolic"), f.name(), None, flag);

                imp.status_listbox.append(&row);

                if (saved_status_id == PkgFlags::empty() && flag == PkgFlags::INSTALLED) ||
                    saved_status_id == flag
                {
                    row.activate();
                }

                match flag {
                    PkgFlags::ALL => { imp.all_status_row.replace(row); },
                    PkgFlags::UPDATES => { imp.update_row.replace(row); },
                    _ => {}
                }
            }
        }
    }

    //---------------------------------------
    // Check AUR file age helper function
    //---------------------------------------
    fn check_aur_file_age(&self) {
        let imp = self.imp();

        if let Some(aur_file) = imp.aur_file.get() {
            // Get AUR package names file age
            let file_time = fs::metadata(aur_file).ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|file_time| {
                    let now = std::time::SystemTime::now();

                    now.duration_since(file_time).ok()
                });

            // Spawn tokio task to download AUR package names file if does not exist or older than 1 day
            if file_time.is_none() || file_time.unwrap() >= Duration::from_secs(24 * 60 * 60) {
                Self::download_aur_names_async(aur_file, || {});
            }
        }
    }

    //---------------------------------------
    // Setup alpm: load alpm packages
    //---------------------------------------
    fn load_packages(&self, check_aur_file: bool) {
        let imp = self.imp();

        // Hide update count in sidebar
        imp.update_row.borrow().set_status(Updates::Output(None, 0));

        // Reset AUR search
        imp.package_view.reset_aur_search();

        // Clear windows
        imp.backup_window.remove_all();
        imp.log_window.remove_all();
        imp.cache_window.remove_all();
        imp.groups_window.remove_all();
        imp.stats_window.remove_all();

        // Get pacman config
        let pacman_config = PACMAN_CONFIG.get().unwrap();

        // Load pacman log
        *PACMAN_LOG.lock().unwrap() = fs::read_to_string(&pacman_config.log_file).ok();

        // Load pacman cache
        let mut cache_files: Vec<PathBuf> = pacman_config.cache_dir.iter()
            .flat_map(|dir| {
                fs::read_dir(dir).map_or(vec![], |read_dir| {
                    read_dir.into_iter()
                        .flatten()
                        .map(|entry| entry.path())
                        .collect()
                })
            })
            .collect();

        cache_files.sort_unstable();

        *PACMAN_CACHE.lock().unwrap() = cache_files;

        // Load AUR package names from file if AUR check is enabled in preferences
        let aur_names: Option<Vec<String>> = imp.prefs_dialog.aur_check().then(|| {
            imp.aur_file.get()
                .and_then(|aur_file| fs::read(aur_file).ok())
                .map(|bytes| {
                    String::from_utf8_lossy(&bytes).lines()
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default()
        });

        // Show loading spinner
        imp.package_view.set_status(PackageViewStatus::PackageLoad);

        // Clear info pane package
        imp.info_pane.set_pkg(None::<PkgObject>);

        // Spawn task to load packages
        let (sender, receiver) = async_channel::bounded(1);

        gio::spawn_blocking(move || {
            match alpm_utils::alpm_with_conf(pacman_config) {
                Ok(handle) => {
                    let localdb = handle.localdb();
                    let syncdbs = handle.syncdbs();

                    // Load pacman sync packages
                    let mut pkg_data: Vec<PkgData> = syncdbs.iter()
                        .flat_map(|db| {
                            db.pkgs().iter()
                                .map(|sync_pkg| {
                                    let local_pkg = localdb.pkg(sync_pkg.name()).ok();

                                    PkgData::from_alpm(sync_pkg, local_pkg, aur_names.as_deref())
                                })
                        })
                        .collect();

                    // Load pacman local packages not in sync databases
                    pkg_data.extend(localdb.pkgs().iter()
                        .filter(|&pkg| syncdbs.pkg(pkg.name()).is_err())
                        .map(|pkg| PkgData::from_alpm(pkg, Some(pkg), aur_names.as_deref()))
                    );

                    sender.send_blocking(Ok(pkg_data)).expect("Failed to send through channel");
                },
                Err(error) => {
                    sender.send_blocking(Err(error)).expect("Failed to send through channel");
                }
            }
        });

        // Attach channel receiver
        glib::spawn_future_local(clone!(
            #[weak(rename_to = window)] self,
            #[weak] imp,
            async move {
                while let Ok(pkg_data) = receiver.recv().await {
                    match pkg_data {
                        Ok(pkg_data) => {
                            // Get alpm handle
                            let handle_ref = alpm_utils::alpm_with_conf(pacman_config)
                                .map(Rc::new)
                                .ok();

                            // Get package lists
                            let len = pkg_data.len();

                            let mut pkgs: Vec<PkgObject> = Vec::with_capacity(len);
                            let mut installed_pkgs: Vec<PkgObject> = Vec::with_capacity(len/10);
                            let mut installed_pkg_names: HashSet<String> = HashSet::with_capacity(len/10);

                            for data in pkg_data {
                                let pkg = PkgObject::new(data,handle_ref.as_ref().map(Rc::clone));

                                if pkg.flags().intersects(PkgFlags::INSTALLED) {
                                    installed_pkg_names.insert(pkg.name());
                                    installed_pkgs.push(pkg.clone());
                                }

                                pkgs.push(pkg);
                            }

                            // Add packages to package view
                            imp.package_view.splice_packages(&pkgs);

                            // Store alpm handle
                            ALPM_HANDLE.replace(handle_ref);

                            // Store package lists
                            PKGS.replace(pkgs);
                            INSTALLED_PKGS.replace(installed_pkgs);
                            INSTALLED_PKG_NAMES.replace(Arc::new(installed_pkg_names));

                            // Get package updates
                            window.get_package_updates();

                            // Check AUR package names file age
                            if check_aur_file {
                                window.check_aur_file_age();
                            }
                        },
                        Err(error) => {
                            let mut error = error.to_string();

                            let warning_dialog = adw::AlertDialog::builder()
                                .heading("Alpm Error")
                                .body(error.remove(0).to_uppercase().to_string() + &error)
                                .default_response("ok")
                                .build();

                            warning_dialog.add_responses(&[("ok", "_Ok")]);

                            warning_dialog.present(Some(&window));
                        }
                    }

                    // Hide loading spinner
                    imp.package_view.set_status(PackageViewStatus::Normal);

                    // Set focus on package view
                    imp.package_view.view().grab_focus();
                }
            }
        ));
    }

    //---------------------------------------
    // Setup alpm: get package updates
    //---------------------------------------
    fn get_package_updates(&self) {
        let imp = self.imp();

        let update_row = imp.update_row.borrow().clone();
        update_row.set_status(Updates::Checking);

        // Spawn async process to check for updates
        glib::spawn_future_local(clone!(
            #[weak] imp,
            async move {
                let mut update_str = String::new();
                let mut error_msg: Option<String> = None;

                let aur_command = imp.prefs_dialog.aur_command();

                // Check for pacman updates async
                let pacman_handle = async_command::run("/usr/bin/checkupdates");

                let (pacman_res, aur_res) = if aur_command.is_empty() {
                    (pacman_handle.await, Ok((None, String::new())))
                } else {
                    // Check for AUR updates async
                    let aur_handle = async_command::run(&aur_command);

                    join!(pacman_handle, aur_handle)
                };

                // Get pacman update results
                match pacman_res {
                    Ok((Some(0), stdout)) => {
                        update_str.push_str(&stdout);
                    },
                    Ok((Some(1), _)) => {
                        error_msg = Some(String::from("Failed to retrieve pacman updates: checkupdates error"));
                    },
                    Err(error) => {
                        error_msg = Some(format!("Failed to retrieve pacman updates: {error}"));
                    }
                    _ => {}
                }

                // Get AUR update results
                match aur_res {
                    Ok((Some(0), stdout)) => {
                        update_str.push_str(&stdout);
                    },
                    Err(error) if error_msg.is_none() => {
                        error_msg = Some(format!("Failed to retrieve AUR updates: {error}"));
                    },
                    _ => {}
                }

                // Create map with updates (name, version)
                static EXPR: LazyLock<Regex> = LazyLock::new(|| {
                    Regex::new(r"([a-zA-Z0-9@._+-]+)[ \t]+[a-zA-Z0-9@._+-:]+[ \t]+->[ \t]+([a-zA-Z0-9@._+-:]+)")
                        .expect("Failed to compile Regex")
                });

                let update_map: HashMap<String, String> = update_str.lines()
                    .filter_map(|s| {
                        EXPR.captures(s)
                            .map(|caps| (caps[1].to_string(), caps[2].to_string()))
                    })
                    .collect();

                // Update status of packages with updates
                if !update_map.is_empty() {
                    imp.package_view.show_updates(&update_map);
                }

                // Update info pane package if it has update
                if imp.info_pane.pkg().is_some_and(|pkg| update_map.contains_key(&pkg.name())) {
                    imp.info_pane.update_display();
                }

                // Show update status/count in sidebar
                update_row.set_status(Updates::Output(error_msg, update_map.len() as u32));

                // If update row is selected, refresh package status filter
                if update_row.is_selected() {
                    imp.package_view.status_filter_changed(update_row.status_id());
                }
            }
        ));
    }

    //---------------------------------------
    // Setup INotify
    //---------------------------------------
    fn setup_inotify(&self) {
        let imp = self.imp();

        // Create async channel
        let (sender, receiver) = async_channel::bounded(1);

        // Create new watcher
        let mut debouncer = new_debouncer(Duration::from_secs(2), None, move |result: DebounceEventResult| {
            if let Ok(events) = result {
                for event in events {
                    if event.kind.is_create() || event.kind.is_modify() || event.kind.is_remove() {
                        sender.send_blocking(())
                            .expect("Failed to send through channel");

                        break;
                    }
                }
            }
        })
        .expect("Failed to create debouncer");

        // Watch pacman local db path
        let db_path = &PACMAN_CONFIG.get().unwrap().db_path;

        let path = Path::new(db_path).join("local");

        if debouncer.watch(&path, RecursiveMode::Recursive).is_ok() {
            // Store watcher
            imp.notify_debouncer.set(debouncer).unwrap();

            // Attach receiver for async channel
            glib::spawn_future_local(clone!(
                #[weak(rename_to = window)] self,
                async move {
                    while receiver.recv().await == Ok(()) {
                        if window.imp().prefs_dialog.auto_refresh() {
                            ActionGroupExt::activate_action(&window, "refresh", None);
                        }
                    }
                }
            ));
        }
    }
}
