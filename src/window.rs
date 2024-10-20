use std::cell::{Cell, RefCell, OnceCell};
use std::sync::OnceLock;
use std::path::Path;
use std::rc::Rc;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use std::env;
use std::io;
use std::io::Read;
use std::str::FromStr;

use gtk::{gio, glib};
use adw::subclass::prelude::*;
use adw::prelude::AdwDialogExt;
use gtk::prelude::*;
use glib::{clone, closure_local};

use alpm_utils::DbListExt;
use titlecase::titlecase;
use regex::Regex;
use async_process::Command;
use futures::join;
use flate2::read::GzDecoder;
use notify_debouncer_full::{notify::*, new_debouncer, Debouncer, DebounceEventResult, FileIdMap};

use crate::utils::tokio_runtime;
use crate::APP_ID;
use crate::PacViewApplication;
use crate::pkg_object::{PkgObject, PkgData, PkgFlags};
use crate::search_bar::{SearchBar, SearchMode, SearchProp};
use crate::package_view::{PackageView, SortProp};
use crate::info_pane::InfoPane;
use crate::filter_row::FilterRow;
use crate::stats_window::StatsWindow;
use crate::backup_window::BackupWindow;
use crate::log_window::LogWindow;
use crate::config_dialog::ConfigDialog;
use crate::preferences_dialog::PreferencesDialog;
use crate::traits::EnumValueExt;

//------------------------------------------------------------------------------
// GLOBAL VARIABLES
//------------------------------------------------------------------------------
thread_local! {
    pub static PKG_SNAPSHOT: RefCell<Vec<PkgObject>> = const {RefCell::new(vec![])};
    pub static AUR_SNAPSHOT: RefCell<Vec<PkgObject>> = const {RefCell::new(vec![])};
    pub static INSTALLED_SNAPSHOT: RefCell<Vec<PkgObject>> = const {RefCell::new(vec![])};
    pub static INSTALLED_PKG_NAMES: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    pub static PACMAN_CONFIG: RefCell<pacmanconf::Config> = RefCell::new(pacmanconf::Config::default());
}

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
        pub(super) package_header_bar: TemplateChild<adw::HeaderBar>,
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

        pub(super) gsettings: OnceCell<gio::Settings>,

        pub(super) aur_file: RefCell<Option<gio::File>>,

        pub(super) pacman_repos: RefCell<Vec<String>>,

        pub(super) saved_repo_id: RefCell<Option<String>>,
        pub(super) saved_status_id: Cell<PkgFlags>,

        pub(super) all_repo_row: RefCell<FilterRow>,
        pub(super) all_status_row: RefCell<FilterRow>,
        pub(super) update_row: RefCell<FilterRow>,

        pub(super) notify_watcher: OnceCell<Debouncer<INotifyWatcher, FileIdMap>>,
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
            obj.setup_shortcuts();
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
        imp.prefs_dialog.set_auto_refresh(gsettings.boolean("auto-refresh"));
        imp.prefs_dialog.set_aur_command(gsettings.string("aur-update-command"));
        if let Ok(search_mode) = SearchMode::from_str(&gsettings.string("search-mode")) {
            imp.prefs_dialog.set_search_mode(search_mode);
            imp.search_bar.set_mode(search_mode);
        }
        if let Ok(search_prop) = SearchProp::from_str(&gsettings.string("search-prop")) {
            imp.prefs_dialog.set_search_prop(search_prop);
            imp.search_bar.set_prop(search_prop);
        }
        imp.prefs_dialog.set_search_delay(gsettings.double("search-delay"));
        imp.prefs_dialog.set_remember_sort(gsettings.boolean("remember-sorting"));

        // Load package view sort prop/order
        if imp.prefs_dialog.remember_sort() {
            if let Ok(sort_prop) = SortProp::from_str(&gsettings.string("sort-prop")) {
                imp.package_view.set_sort_prop(sort_prop);
            }

            imp.package_view.set_sort_ascending(gsettings.boolean("sort-ascending"));
        }
    }

    //---------------------------------------
    // Set gsetting helper function
    //---------------------------------------
    fn set_gsetting<T: FromVariant + ToVariant + PartialEq>(&self, gsettings: &gio::Settings, key: &str, value: T) {
        let default: T = gsettings.default_value(key)
            .expect("Could not get gsettings default value")
            .get::<T>()
            .expect("Could not retrieve value from variant");

        if !(default == value && default == gsettings.get(key)) {
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

        self.set_gsetting(gsettings, "window-width", width);
        self.set_gsetting(gsettings, "window-height", height);
        self.set_gsetting(gsettings, "window-maximized", self.is_maximized());

        // Save preferences
        self.set_gsetting(gsettings, "auto-refresh", imp.prefs_dialog.auto_refresh());
        self.set_gsetting(gsettings, "aur-update-command", imp.prefs_dialog.aur_command());
        self.set_gsetting(gsettings, "search-mode", imp.prefs_dialog.search_mode().nick());
        self.set_gsetting(gsettings, "search-prop", imp.prefs_dialog.search_prop().nick());
        self.set_gsetting(gsettings, "search-delay", imp.prefs_dialog.search_delay());
        self.set_gsetting(gsettings, "remember-sorting", imp.prefs_dialog.remember_sort());

        // Save package view sort prop/order
        if imp.prefs_dialog.remember_sort() {
            self.set_gsetting(gsettings, "sort-prop", imp.package_view.sort_prop().nick());
            self.set_gsetting(gsettings, "sort-ascending", imp.package_view.sort_ascending());
        } else {
            self.set_gsetting(gsettings, "sort-prop", SortProp::default().nick());
            self.set_gsetting(gsettings, "sort-ascending", true);
        }
    }

    //---------------------------------------
    // Init cache dir
    //---------------------------------------
    fn init_cache_dir(&self) {
        // Create cache dir
        let cache_dir = env::var("XDG_CACHE_HOME")
            .or_else(|_| env::var("HOME")
                .map(|var| Path::new(&var).join(".cache").display().to_string())
            )
            .map(|var| Path::new(&var).join("pacview").display().to_string())
            .map_or(None, |cache_dir| {
                let res = gio::File::for_path(Path::new(&cache_dir))
                    .make_directory_with_parents(None::<&gio::Cancellable>);

                if res.is_ok() || res.is_err_and(|error| error.matches(gio::IOErrorEnum::Exists)) {
                    Some(cache_dir)
                } else {
                    None
                }
            });

        // Store AUR package names file path
        let aur_file = cache_dir
            .map(|cache_dir| gio::File::for_path(Path::new(&cache_dir).join("aur_packages")));

        self.imp().aur_file.replace(aur_file);
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

        // Bind package view sort order to sort button icon
        imp.package_view.bind_property("sort-ascending", &imp.sort_button.get(), "icon-name")
            .transform_to(|_, sort_asc: bool| Some(if sort_asc { "view-sort-ascending-symbolic" } else { "view-sort-descending-symbolic" }))
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
        imp.search_bar.set_key_capture_widget(imp.package_view.view().upcast());

        // Bind package view item count to status label text
        imp.package_view.bind_property("n-items", &imp.status_label.get(), "label")
            .transform_to(|_, n_items: u32| {
                Some(format!("{n_items} matching package{}", if n_items != 1 {"s"} else {""}))
            })
            .sync_create()
            .build();

        // Set initial focus on package view
        imp.package_view.view().grab_focus();
    }

    //---------------------------------------
    // Setup actions
    //---------------------------------------
    fn setup_actions(&self) {
        let imp = self.imp();

        // Add start/stop search actions
        let start_action = gio::ActionEntry::builder("start-search")
            .activate(|window: &Self, _, _| {
                window.imp().search_bar.set_enabled(true);
            })
            .build();

        let stop_action = gio::ActionEntry::builder("stop-search")
            .activate(|window: &Self, _, _| {
                window.imp().search_bar.set_enabled(false);
            })
            .build();

        // Add search actions to window
        self.add_action_entries([start_action, stop_action]);

        // Add pane visibility property actions
        let show_sidebar_action = gio::PropertyAction::new("show-sidebar", &imp.sidebar_split_view.get(), "show-sidebar");

        let show_infopane_action = gio::PropertyAction::new("show-infopane", &imp.main_split_view.get(), "show-sidebar");

        // Add pane visibility actions to window
        self.add_action(&show_sidebar_action);
        self.add_action(&show_infopane_action);

        // Add package view refresh action
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

        // Add package view copy list action
        let copy_action = gio::ActionEntry::builder("copy-package-list")
            .activate(|window: &Self, _, _| {
                let imp = window.imp();

                let copy_text = imp.package_view.copy_list();

                window.clipboard().set_text(&copy_text);
            })
            .build();

        // Add package view all packages action
        let all_pkgs_action = gio::ActionEntry::builder("show-all-packages")
            .activate(|window: &Self, _, _| {
                let imp = window.imp();

                imp.all_repo_row.borrow().activate();
                imp.all_status_row.borrow().activate();
            })
            .build();

        // Add package view reset sort action
        let reset_sort_action = gio::ActionEntry::builder("reset-package-sort")
            .activate(|window: &Self, _, _| {
                let imp = window.imp();

                imp.package_view.set_sort_prop(SortProp::default());
                imp.package_view.set_sort_ascending(true);
            })
            .build();

        // Add package view sort prop property action
        let sort_prop_action = gio::PropertyAction::new("set-sort-prop", &imp.package_view.get(), "sort-prop");

        // Add package view actions to window
        self.add_action_entries([refresh_action, copy_action, all_pkgs_action, reset_sort_action]);
        self.add_action(&sort_prop_action);

        // Bind package view item count to copy list action enabled state
        let copy_action = self.lookup_action("copy-package-list").unwrap();

        imp.package_view.bind_property("n-items", &copy_action, "enabled")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .sync_create()
            .build();

        // Add info pane set tab action
        let visible_tab_action = gio::PropertyAction::new("infopane-set-tab", &imp.info_pane.get(), "visible-tab");

        // Add info pane prev/next actions
        let prev_action = gio::ActionEntry::builder("infopane-previous")
            .activate(|window: &Self, _, _| {
                window.imp().info_pane.display_prev();
            })
            .build();

        let next_action = gio::ActionEntry::builder("infopane-next")
            .activate(|window: &Self, _, _| {
                window.imp().info_pane.display_next();
            })
            .build();

        // Add info pane actions to window
        self.add_action(&visible_tab_action);
        self.add_action_entries([prev_action, next_action]);

        // Add show stats window action
        let stats_action = gio::ActionEntry::builder("show-stats")
            .activate(|window: &Self, _, _| {
                PKG_SNAPSHOT.with_borrow(|pkg_snapshot| {
                    let stats_window = StatsWindow::new(window);

                    stats_window.show(&window.imp().pacman_repos.borrow(), pkg_snapshot);
                });
            })
            .build();

        // Add show backup files window action
        let backup_action = gio::ActionEntry::builder("show-backup-files")
            .activate(|window: &Self, _, _| {
                INSTALLED_SNAPSHOT.with_borrow(|installed_snapshot| {
                    let backup_window = BackupWindow::new(window);

                    backup_window.show(installed_snapshot);
                });
            })
            .build();

        // Add show pacman lo&pacman_config.g window action
        let log_action = gio::ActionEntry::builder("show-pacman-log")
            .activate(|window: &Self, _, _| {
                PACMAN_CONFIG.with_borrow(|pacman_config| {
                    let log_window = LogWindow::new(window);

                    log_window.show(&pacman_config.log_file);
                });
            })
            .build();

        // Add show pacman config window action
        let config_action = gio::ActionEntry::builder("show-pacman-config")
            .activate(|window: &Self, _, _| {
                window.imp().config_dialog.present(Some(window));
            })
            .build();

        // Add window actions to window
        self.add_action_entries([stats_action, backup_action, log_action, config_action]);

        // Add show preferences action
        let prefs_action = gio::ActionEntry::builder("show-preferences")
            .activate(|window: &Self, _, _| {
                window.imp().prefs_dialog.present(Some(window));
            })
            .build();

        // Add preference actions to window
        self.add_action_entries([prefs_action]);
    }

    //---------------------------------------
    // Setup shortcuts
    //---------------------------------------
    fn setup_shortcuts(&self) {
        // Create shortcut controller
        let controller = gtk::ShortcutController::new();

        // Add search start shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>F"),
            Some(gtk::NamedAction::new("win.start-search"))
        ));

        // Add search stop shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::NamedAction::new("win.stop-search"))
        ));

        // Add show sidebar shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>B"),
            Some(gtk::NamedAction::new("win.show-sidebar"))
        ));

        // Add show infopane shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>I"),
            Some(gtk::NamedAction::new("win.show-infopane"))
        ));

        // Add show preferences shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>comma"),
            Some(gtk::NamedAction::new("win.show-preferences"))
        ));

        // Add view refresh shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("F5"),
            Some(gtk::NamedAction::new("win.refresh"))
        ));

        // Add view copy list shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl><shift>C"),
            Some(gtk::NamedAction::new("win.copy-package-list"))
        ));

        // Add view show all packages shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl><shift>W"),
            Some(gtk::NamedAction::new("win.show-all-packages"))
        ));

        // Add view show stats shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl><shift>S"),
            Some(gtk::NamedAction::new("win.show-stats"))
        ));

        // Add view show backup files shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl><shift>B"),
            Some(gtk::NamedAction::new("win.show-backup-files"))
        ));

        // Add view show pacman log shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl><shift>L"),
            Some(gtk::NamedAction::new("win.show-pacman-log"))
        ));

        // Add view show pacman config shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl><shift>P"),
            Some(gtk::NamedAction::new("win.show-pacman-config"))
        ));

        // Add infopane set tab shortcuts
        controller.add_shortcut(gtk::Shortcut::with_arguments(
            gtk::ShortcutTrigger::parse_string("<alt>I"),
            Some(gtk::NamedAction::new("win.infopane-set-tab")),
            &"info".to_variant()
        ));

        controller.add_shortcut(gtk::Shortcut::with_arguments(
            gtk::ShortcutTrigger::parse_string("<alt>F"),
            Some(gtk::NamedAction::new("win.infopane-set-tab")),
            &"files".to_variant()
        ));

        controller.add_shortcut(gtk::Shortcut::with_arguments(
            gtk::ShortcutTrigger::parse_string("<alt>L"),
            Some(gtk::NamedAction::new("win.infopane-set-tab")),
            &"log".to_variant()
        ));

        controller.add_shortcut(gtk::Shortcut::with_arguments(
            gtk::ShortcutTrigger::parse_string("<alt>C"),
            Some(gtk::NamedAction::new("win.infopane-set-tab")),
            &"cache".to_variant()
        ));

        controller.add_shortcut(gtk::Shortcut::with_arguments(
            gtk::ShortcutTrigger::parse_string("<alt>B"),
            Some(gtk::NamedAction::new("win.infopane-set-tab")),
            &"backup".to_variant()
        ));

        // Add infopane previous shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<alt>Left"),
            Some(gtk::NamedAction::new("win.infopane-previous"))
        ));

        // Add infopane next shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<alt>Right"),
            Some(gtk::NamedAction::new("win.infopane-next"))
        ));

        // Add shortcut controller to window
        self.add_controller(controller);
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
                    .expect("Could not downcast to 'FilterRow'")
                    .repo_id();

                imp.package_view.set_repo_filter(repo_id.as_deref());

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
                    .expect("Could not downcast to 'FilterRow'")
                    .status_id();

                imp.package_view.set_status_filter(status_id);

                if imp.sidebar_split_view.is_collapsed() {
                    imp.sidebar_split_view.set_show_sidebar(false);
                }

                imp.package_view.view().grab_focus();
            }
        ));

        // Search bar enabled signal
        imp.search_bar.connect_closure("enabled", false, closure_local!(
            #[watch(rename_to = window)] self,
            move |_: SearchBar, enabled: bool| {
                if !enabled {
                    window.imp().package_view.view().grab_focus();
                }
            }
        ));

        // Search bar changed signal
        imp.search_bar.connect_closure("changed", false, closure_local!(
            #[watch(rename_to = window)] self,
            move |_: SearchBar, search_term: &str, mode: SearchMode, prop: SearchProp| {
                window.imp().package_view.set_search_filter(search_term, mode, prop);
            }
        ));

        // Search bar AUR Search signal
        imp.search_bar.connect_closure("aur-search", false, closure_local!(
            #[watch(rename_to = window)] self,
            move |search_bar: SearchBar, search_term: &str, prop: SearchProp| {
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
    }

    //---------------------------------------
    // Download AUR names helper function
    //---------------------------------------
    fn download_aur_names(&self, file: &gio::File, sender: Option<async_channel::Sender<()>>) {
        tokio_runtime().spawn(clone!(
            #[strong] file,
            async move {
                let url = "https://aur.archlinux.org/packages.gz";

                if let Ok(response) = reqwest::get(url).await {
                    if let Ok(bytes) = response.bytes().await {
                        let mut decoder = GzDecoder::new(&bytes[..]);

                        let mut gz_string = String::new();

                        if decoder.read_to_string(&mut gz_string).is_ok() {
                            file.replace_contents(
                                gz_string.as_bytes(),
                                None,
                                false,
                                gio::FileCreateFlags::REPLACE_DESTINATION,
                                None::<&gio::Cancellable>
                            ).unwrap_or_default();
                        }
                    }
                }

                if let Some(sender) = sender {
                    sender.send(()).await.expect("Could not send through channel");
                }
            }
        ));
    }

    //---------------------------------------
    // Check AUR file helper function
    //---------------------------------------
    fn check_aur_file(&self) {
        let imp = self.imp();

        let aur_file = &*imp.aur_file.borrow();

        // If AUR package names file does not exist, download it
        if let Some(aur_file) = aur_file {
            if !aur_file.query_exists(None::<&gio::Cancellable>) {
                let (sender, receiver) = async_channel::bounded(1);

                self.download_aur_names(aur_file, Some(sender));

                glib::spawn_future_local(clone!(
                    #[weak(rename_to = window)] self,
                    async move {
                        while let Ok(()) = receiver.recv().await {
                            window.load_packages(false);
                        }
                    }
                ));
            } else {
                self.load_packages(true);
            }
        } else {
            self.load_packages(true);
        }
    }

    //---------------------------------------
    // Setup alpm
    //---------------------------------------
    fn setup_alpm(&self, is_init: bool) {
        self.get_pacman_config();
        self.populate_sidebar();

        if is_init {
            self.check_aur_file();
        } else {
            self.load_packages(false);
        }
    }

    //---------------------------------------
    // Setup alpm: get pacman config
    //---------------------------------------
    fn get_pacman_config(&self) {
        let imp = self.imp();

        // Get pacman config
        let pacman_config = pacmanconf::Config::new().unwrap();

        // Get pacman repositories
        let pacman_repos: Vec<String> = pacman_config.repos.iter()
            .map(|r| r.name.to_string())
            .chain([String::from("aur"), String::from("local")])
            .collect();

        // Init config dialog
        imp.config_dialog.init(&pacman_config);

        // Store pacman config
        PACMAN_CONFIG.replace(pacman_config);

        // Store pacman repos
        imp.pacman_repos.replace(pacman_repos);
    }

    //---------------------------------------
    // Setup alpm: populate sidebar
    //---------------------------------------
    fn populate_sidebar(&self) {
        let imp = self.imp();

        // Clear sidebar rows
        imp.repo_listbox.remove_all();
        imp.status_listbox.remove_all();

        // Add repository rows (enumerate pacman repositories)
        let saved_repo_id = imp.saved_repo_id.take();

        let row = FilterRow::new("repository-symbolic", "All", None, PkgFlags::empty());

        imp.repo_listbox.append(&row);

        if saved_repo_id.is_none() {
            row.activate();
        }

        imp.all_repo_row.replace(row);

        for repo in &*imp.pacman_repos.borrow() {
            let display_label = if repo == "aur" { repo.to_uppercase() } else { titlecase(repo) };

            let row = FilterRow::new("repository-symbolic", &display_label, Some(repo), PkgFlags::empty());

            imp.repo_listbox.append(&row);

            if saved_repo_id.as_ref() == Some(repo) {
                row.activate();
            }
        }

        // Add package status rows (enumerate PkgStatusFlags)
        let saved_status_id = imp.saved_status_id.replace(PkgFlags::empty());

        let flags = glib::FlagsClass::new::<PkgFlags>();

        for f in flags.values() {
            let flag = PkgFlags::from_bits_truncate(f.value());

            let row = FilterRow::new(
                &format!("status-{}-symbolic", f.nick()),
                f.name(),
                None,
                flag
            );

            imp.status_listbox.append(&row);

            if saved_status_id == PkgFlags::empty() {
                if flag == PkgFlags::INSTALLED {
                    row.activate();
                }
            } else if saved_status_id == flag {
                row.activate();
            }

            if flag == PkgFlags::ALL {
                imp.all_status_row.replace(row);
            }
            else if flag == PkgFlags::UPDATES {
                imp.update_row.replace(row);
            }
        }
    }

    //---------------------------------------
    // Setup alpm: load alpm packages
    //---------------------------------------
    fn load_packages(&self, update_aur_file: bool) {
        let imp = self.imp();

        let aur_file = imp.aur_file.borrow();

        // Spawn thread to load packages
        let (sender, receiver) = async_channel::bounded(1);

        PACMAN_CONFIG.with_borrow(|pacman_config| {
            gio::spawn_blocking(clone!(
                #[strong] aur_file,
                #[strong] pacman_config,
                move || {
                    let mut aur_names: HashSet<String> = HashSet::new();

                    // Load AUR package names from file
                    if let Some(aur_file) = aur_file {
                        if let Ok((bytes, _)) = aur_file.load_contents(None::<&gio::Cancellable>) {
                            aur_names = String::from_utf8_lossy(&bytes).lines()
                                .map(|line| line.to_string())
                                .collect();
                        };
                    }

                    // Load pacman database packages
                    let mut data_list: Vec<PkgData> = vec![];

                    if let Ok(handle) = alpm_utils::alpm_with_conf(&pacman_config) {
                        let localdb = handle.localdb();

                        // Load sync packages
                        data_list.extend(handle.syncdbs().iter()
                            .flat_map(|db| db.pkgs().iter()
                                .map(|syncpkg| {
                                    let localpkg = localdb.pkg(syncpkg.name());

                                    PkgData::from_pkg(syncpkg, localpkg, None)
                                })
                            )
                        );

                        // Load local packages not in sync databases
                        data_list.extend(localdb.pkgs().iter()
                            .filter(|pkg| handle.syncdbs().pkg(pkg.name()).is_err())
                            .map(|pkg| {
                                if aur_names.contains(pkg.name()) {
                                    PkgData::from_pkg(pkg, Ok(pkg), Some("aur"))
                                } else {
                                    PkgData::from_pkg(pkg, Ok(pkg), None)
                                }
                            })
                        );
                    }

                    sender.send_blocking(data_list).expect("Could not send through channel");
                }
            ));

            // Attach thread receiver
            glib::spawn_future_local(clone!(
                #[weak(rename_to = window)] self,
                #[weak] imp,
                #[strong] pacman_config,
                async move {
                    while let Ok(data_list) = receiver.recv().await {
                        if let Ok(handle) = alpm_utils::alpm_with_conf(&pacman_config) {
                            let handle_ref = Rc::new(handle);

                            // Get package lists (installed and non)
                            let (install_list, mut pkg_list): (Vec<_>, Vec<_>) = data_list
                                .into_iter()
                                .map(|data| PkgObject::new(Some(handle_ref.clone()), data))
                                .partition(|pkg| {
                                    pkg.flags().intersects(PkgFlags::INSTALLED)
                                });

                            pkg_list.extend_from_slice(&install_list);

                            // Add packages to package view
                            imp.package_view.splice_packages(&pkg_list);

                            // Store package lists in global variables
                            INSTALLED_PKG_NAMES.replace(install_list.iter()
                                .map(|pkg| pkg.name())
                                .collect()
                            );

                            PKG_SNAPSHOT.replace(pkg_list);
                            INSTALLED_SNAPSHOT.replace(install_list);

                            // Show package list
                            imp.package_view.hide_loading_spinner();

                            // Get package updates
                            window.get_package_updates();

                            // Update AUR package names file
                            if update_aur_file {
                                window.update_aur_file();
                            }
                        }
                    }
                }
            ));
        });
    }

    //---------------------------------------
    // Update helper function
    //---------------------------------------
    async fn run_update_command(&self, cmd: &str) -> io::Result<(Option<i32>, String)> {
        // Run external command
        let params = shlex::split(cmd)
            .filter(|params| !params.is_empty())
            .ok_or(io::Error::new(io::ErrorKind::Other, "Error parsing parameters"))?;

        let output = Command::new(&params[0]).args(&params[1..]).output().await?;

        let stdout = String::from_utf8(output.stdout)
            .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;

        let code = output.status.code();

        Ok((code, stdout))
    }

    //---------------------------------------
    // Setup alpm: get package updates
    //---------------------------------------
    fn get_package_updates(&self) {
        let imp = self.imp();

        let update_row = imp.update_row.borrow();
        update_row.set_updating(true);

        glib::spawn_future_local(clone!(
            #[weak(rename_to = window)] self,
            #[weak] imp,
            #[strong] update_row,
            async move {
                let mut update_str = String::from("");
                let mut error_msg: Option<String> = None;

                let aur_command = imp.prefs_dialog.aur_command();

                // Check for pacman updates async
                let pacman_handle = window.run_update_command("/usr/bin/checkupdates");

                let (pacman_res, aur_res) = if !aur_command.is_empty() {
                    // Check for AUR updates async
                    let aur_handle = window.run_update_command(&aur_command);

                    join!(pacman_handle, aur_handle)
                } else {
                    (pacman_handle.await, Ok((None, "".to_string())))
                };

                // Get pacman update results
                match pacman_res {
                    Ok((code, stdout)) => {
                        if code == Some(0) {
                            update_str += &stdout;
                        } else if code == Some(1) {
                            error_msg = Some("Error Retrieving Pacman Updates (checkupdates error)".to_string())
                        }
                    },
                    Err(error) => error_msg = Some(format!("Error Retrieving Pacman Updates ({})", error))
                }

                // Get AUR update results
                match aur_res {
                    Ok((code, stdout)) => {
                        if code == Some(0) {
                            update_str += &stdout;
                        }
                    },
                    Err(error) => {
                        if error_msg.is_none() {
                            error_msg = Some(format!("Error Retrieving AUR Updates ({})", error));
                        }
                    }
                }

                // Create map with updates (name, version)
                static EXPR: OnceLock<Regex> = OnceLock::new();

                let expr = EXPR.get_or_init(|| {
                    Regex::new(r"([a-zA-Z0-9@._+-]+?)[ \t]+?([a-zA-Z0-9@._+-:]+?)[ \t]+?->[ \t]+?([a-zA-Z0-9@._+-:]+)")
                        .expect("Regex error")
                });

                let update_map: HashMap<String, String> = update_str.lines()
                    .filter_map(|s| {
                        expr.captures(s)
                            .map(|caps| {
                                (caps[1].to_string(), format!("{} \u{2192} {}", &caps[2], &caps[3]))
                            })
                    })
                    .collect();

                // Update status of packages with updates
                if !update_map.is_empty() {
                    imp.package_view.update_packages(&update_map);
                }

                // Update info pane package if it has update
                if imp.info_pane.pkg().is_some_and(|pkg| update_map.contains_key(&pkg.name())) {
                    imp.info_pane.update_display();
                }

                // Show update status/count in sidebar
                update_row.set_updating(false);
                update_row.set_icon(if error_msg.is_some() {"status-updates-error-symbolic"} else {"status-updates-symbolic"});
                update_row.set_count(update_map.len() as u64);
                update_row.set_tooltip_text(error_msg.as_deref());

                // If update row is selected, refresh package status filter
                if update_row.is_selected() {
                    imp.package_view.set_status_filter(update_row.status_id());
                }
            }
        ));
    }

    //---------------------------------------
    // Setup alpm: update AUR file
    //---------------------------------------
    fn update_aur_file(&self) {
        let imp = self.imp();

        let aur_file = &*imp.aur_file.borrow();

        if let Some(aur_file) = aur_file {
            // Get AUR package names file age
            let file_days = aur_file.query_info("time::modified", gio::FileQueryInfoFlags::NONE, None::<&gio::Cancellable>)
                .ok()
                .and_then(|file_info| file_info.modification_date_time())
                .and_then(|file_time| {
                    glib::DateTime::now_local().ok()
                        .map(|current_time| current_time.difference(&file_time).as_days())
                });

            // Download AUR package names file if does not exist or older than 7 days
            if file_days.is_none() || file_days.unwrap() >= 7 {
                self.download_aur_names(aur_file, None);
            }
        }
    }

    //---------------------------------------
    // Setup INotify
    //---------------------------------------
    fn setup_inotify(&self) {
        let imp = self.imp();

        PACMAN_CONFIG.with_borrow(|pacman_config| {
            // Create async channel
            let (sender, receiver) = async_channel::bounded(1);

            // Create new watcher
            let mut watcher = new_debouncer(Duration::from_secs(1), None, move |result: DebounceEventResult| {
                if let Ok(events) = result {
                    for event in events {
                        if event.kind.is_create() || event.kind.is_modify() || event.kind.is_remove() {
                            sender.send_blocking(())
                                .expect("Could not send through channel");

                            break;
                        }
                    }
                }
            }).unwrap();

            // Watch pacman local db path
            let path = Path::new(&pacman_config.db_path).join("local");

            if watcher.watcher().watch(&path, RecursiveMode::Recursive).is_ok() {
                watcher.cache().add_root(&path, RecursiveMode::Recursive);

                // Store watcher
                imp.notify_watcher.set(watcher).unwrap();

                // Attach receiver for async channel
                glib::spawn_future_local(clone!(
                    #[weak(rename_to = window)] self,
                    #[weak] imp,
                    async move {
                        while let Ok(()) = receiver.recv().await {
                            if imp.prefs_dialog.auto_refresh() {
                                ActionGroupExt::activate_action(&window, "refresh", None);
                            }
                        }
                    }
                ));
            }
        });
    }
}
