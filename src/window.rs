use std::cell::{Cell, RefCell, OnceCell};
use std::sync::{LazyLock, RwLock};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use std::fs;

use gtk::{gio, glib, gdk};
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::clone;
use gdk::{Key, ModifierType};

use alpm_utils::DbListExt;
use heck::ToTitleCase;
use rayon::slice::ParallelSliceMut;
use regex::Regex;
use futures::join;
use notify_debouncer_full::{notify::{INotifyWatcher, RecursiveMode}, new_debouncer, Debouncer, DebounceEventResult, NoCache};
use which::which_global;

use crate::APP_ID;
use crate::PacViewApplication;
use crate::pkg_data::{PkgFlags, PkgData};
use crate::pkg_object::{ALPM_HANDLE, PkgObject};
use crate::search_bar::SearchBar;
use crate::package_view::{PackageView, PackageViewState, SortProp};
use crate::info_pane::InfoPane;
use crate::filter_row::{FilterRow, FilterRowState};
use crate::stats_window::StatsWindow;
use crate::backup_window::BackupWindow;
use crate::groups_window::GroupsWindow;
use crate::log_window::LogWindow;
use crate::cache_window::CacheWindow;
use crate::config_dialog::ConfigDialog;
use crate::preferences_dialog::PreferencesDialog;
use crate::utils::{async_command, aur_file};

//------------------------------------------------------------------------------
// GLOBAL VARIABLES
//------------------------------------------------------------------------------
pub static PARU_PATH: LazyLock<which::Result<PathBuf>> = LazyLock::new(|| which_global("paru"));
pub static PACCAT_PATH: LazyLock<which::Result<PathBuf>> = LazyLock::new(|| which_global("paccat"));
pub static MELD_PATH: LazyLock<which::Result<PathBuf>> = LazyLock::new(|| which_global("meld"));

thread_local! {
    pub static PKGS: RefCell<Vec<PkgObject>> = const { RefCell::new(vec![]) };
    pub static INSTALLED_PKGS: RefCell<Vec<PkgObject>> = const { RefCell::new(vec![]) };
}

pub static PACMAN_CONFIG: LazyLock<pacmanconf::Config> = LazyLock::new(|| {
    pacmanconf::Config::new().expect("Failed to get pacman config")
});
pub static PACMAN_LOG: LazyLock<RwLock<Option<String>>> = LazyLock::new(|| RwLock::new(None));
pub static PACMAN_CACHE: LazyLock<RwLock<Vec<PathBuf>>> = LazyLock::new(|| RwLock::new(vec![]));

//------------------------------------------------------------------------------
// MODULE: PacViewWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PacViewWindow)]
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

        #[property(get, set)]
        show_sidebar: Cell<bool>,
        #[property(get, set)]
        show_infopane: Cell<bool>,

        #[property(get, set, builder(SortProp::default()))]
        package_sort_prop: Cell<SortProp>,

        pub(super) aur_file: RefCell<Option<PathBuf>>,

        pub(super) repo_names: RefCell<Vec<String>>,

        pub(super) saved_repo_id: RefCell<Option<String>>,
        pub(super) saved_status_id: Cell<PkgFlags>,

        pub(super) all_repo_row: RefCell<FilterRow>,
        pub(super) all_status_row: RefCell<FilterRow>,
        pub(super) update_row: RefCell<FilterRow>,

        pub(super) notify_debouncer: OnceCell<Debouncer<INotifyWatcher, NoCache>>,

        pub(super) prefs_dialog: RefCell<PreferencesDialog>,

        pub(super) backup_window: RefCell<BackupWindow>,
        pub(super) cache_window: RefCell<CacheWindow>,
        pub(super) groups_window: RefCell<GroupsWindow>,
        pub(super) log_window: RefCell<LogWindow>,
        pub(super) stats_window: RefCell<StatsWindow>,

        pub(super) config_dialog: RefCell<ConfigDialog>,
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

            // Install actions
            Self::install_actions(klass);

            // Add key bindings
            Self::bind_shortcuts(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PacViewWindow {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();

            obj.bind_gsettings();

            obj.setup_widgets();

            obj.setup_alpm(true);

            obj.setup_inotify();
        }
    }

    impl WidgetImpl for PacViewWindow {}
    impl WindowImpl for PacViewWindow {}
    impl ApplicationWindowImpl for PacViewWindow {}
    impl AdwApplicationWindowImpl for PacViewWindow {}

    impl PacViewWindow {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Pane visibility property actions
            klass.install_property_action("win.show-sidebar", "show-sidebar");
            klass.install_property_action("win.show-infopane", "show-infopane");

            // Refresh action
            klass.install_action("win.refresh", None, |window, _, _| {
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
            });

            // Check for updates action
            klass.install_action("win.check-updates", None, |window, _, _| {
                window.get_package_updates();
            });

            // Update AUR database action
            klass.install_action_async("win.update-aur-database", None, async |window, _, _| {
                let imp = window.imp();

                let aur_file = imp.aur_file.borrow().to_owned();

                if let Some(aur_file) = aur_file {
                    imp.update_row.borrow().set_state(FilterRowState::Reset);
                    imp.package_view.set_state(PackageViewState::AURDownload);
                    imp.info_pane.set_pkg(None::<PkgObject>);

                    // Spawn tokio task to download AUR package names file
                    let _ = aur_file::download(&aur_file).await;

                    // Refresh packages
                    gtk::prelude::WidgetExt::activate_action(&window, "win.refresh", None)
                        .unwrap();
                }
            });

            // Package view copy list action
            klass.install_action("view.copy-list", None, |window, _, _| {
                 window.clipboard().set_text(&window.imp().package_view.copy_list());
            });

            // Package view sort prop property action
            klass.install_property_action("view.set-sort-prop", "package-sort-prop");

            // Package view reset sort action
            klass.install_action("view.reset-sort", None, |window, _, _| {
                let imp = window.imp();

                imp.package_view.set_sort_prop(SortProp::default());
                imp.package_view.set_sort_ascending(true);
            });

            // Show window/dialog actions
            klass.install_action("win.show-backup-files", None, |window, _, _| {
                window.imp().backup_window.borrow().show(window);
            });

            klass.install_action("win.show-pacman-cache", None, |window, _, _| {
                window.imp().cache_window.borrow().show(window);
            });

            klass.install_action("win.show-pacman-groups", None, |window, _, _| {
                window.imp().groups_window.borrow().show(window);
            });

            klass.install_action("win.show-pacman-log", None, |window, _, _| {
                window.imp().log_window.borrow().show(window);
            });

            klass.install_action("win.show-stats", None, |window, _, _| {
                let imp = window.imp();

                imp.stats_window.borrow().show(window, &imp.repo_names.borrow());
            });

            klass.install_action("win.show-pacman-config", None, |window, _, _| {
                window.imp().config_dialog.borrow().present(Some(window));
            });

            klass.install_action("win.show-preferences", None, |window, _, _| {
                window.imp().prefs_dialog.borrow().present(Some(window));
            });
        }

        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Search start/stop key bindings
            klass.add_binding(Key::F, ModifierType::CONTROL_MASK, |window| {
                window.imp().search_bar.set_enabled(true);

                glib::Propagation::Stop
            });

            klass.add_binding(Key::Escape, ModifierType::NO_MODIFIER_MASK, |window| {
                let imp = window.imp();

                if (imp.sidebar_split_view.is_collapsed() && imp.sidebar_split_view.shows_sidebar()) || (imp.main_split_view.is_collapsed() && imp.main_split_view.shows_sidebar()) {
                    glib::Propagation::Proceed
                } else {
                    window.imp().search_bar.set_enabled(false);

                    glib::Propagation::Stop
                }
            });

            // Show sidebar key binding
            klass.add_binding_action(Key::B, ModifierType::CONTROL_MASK, "win.show-sidebar");

            // Show infopane key binding
            klass.add_binding_action(Key::I, ModifierType::CONTROL_MASK, "win.show-infopane");

            // Show preferences key binding
            klass.add_binding_action(Key::comma, ModifierType::CONTROL_MASK, "win.show-preferences");

            // View refresh key binding
            klass.add_binding_action(Key::F5, ModifierType::NO_MODIFIER_MASK, "win.refresh");

            // View check updates binding
            klass.add_binding_action(Key::F9, ModifierType::NO_MODIFIER_MASK, "win.check-updates");

            // View update AUR database key binding
            klass.add_binding_action(Key::F7, ModifierType::NO_MODIFIER_MASK, "win.update-aur-database");

            // View copy list key binding
            klass.add_binding_action(Key::C, ModifierType::CONTROL_MASK | ModifierType::ALT_MASK, "view.copy-list");

            // View show all packages key binding
            klass.add_binding(Key::A, ModifierType::ALT_MASK, |window| {
                let imp = window.imp();

                imp.all_repo_row.borrow().activate();
                imp.all_status_row.borrow().activate();

                glib::Propagation::Stop
            });

            // Stats window key binding
            klass.add_binding_action(Key::S, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "win.show-stats");

            // Backup files window key binding
            klass.add_binding_action(Key::B, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "win.show-backup-files");

            // Pacman log window key binding
            klass.add_binding_action(Key::L, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "win.show-pacman-log");

            // Pacman cache window key binding
            klass.add_binding_action(Key::C, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "win.show-pacman-cache");

            // Pacman groups window key binding
            klass.add_binding_action(Key::G, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "win.show-pacman-groups");

            // Pacman config dialog key binding
            klass.add_binding_action(Key::P, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "win.show-pacman-config");

            // Infopane set tab shortcuts
            klass.add_binding(Key::I, ModifierType::ALT_MASK, |window| {
                window.imp().info_pane.set_visible_tab("info");

                glib::Propagation::Stop
            });

            klass.add_binding(Key::F, ModifierType::ALT_MASK, |window| {
                window.imp().info_pane.set_visible_tab("files");

                glib::Propagation::Stop
            });

            klass.add_binding(Key::L, ModifierType::ALT_MASK, |window| {
                window.imp().info_pane.set_visible_tab("log");

                glib::Propagation::Stop
            });

            klass.add_binding(Key::C, ModifierType::ALT_MASK, |window| {
                window.imp().info_pane.set_visible_tab("cache");

                glib::Propagation::Stop
            });

            klass.add_binding(Key::B, ModifierType::ALT_MASK, |window| {
                window.imp().info_pane.set_visible_tab("backup");

                glib::Propagation::Stop
            });

            // Infopane previous/next key bindings
            klass.add_binding(Key::Left, ModifierType::ALT_MASK, |window| {
                window.imp().info_pane.display_prev();

                glib::Propagation::Stop
            });

            klass.add_binding(Key::Right, ModifierType::ALT_MASK, |window| {
                window.imp().info_pane.display_next();

                glib::Propagation::Stop
            });

            // Infopane show PKGBUILD key bindings
            klass.add_binding(Key::K, ModifierType::ALT_MASK | ModifierType::SHIFT_MASK, |window| {
                window.imp().info_pane.show_pkgbuild();

                glib::Propagation::Stop
            });

            // Infopane show hashes key bindings
            klass.add_binding(Key::H, ModifierType::ALT_MASK | ModifierType::SHIFT_MASK, |window| {
                window.imp().info_pane.show_hashes();

                glib::Propagation::Stop
            });
        }
    }
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
        glib::Object::builder()
            .property("application", app)
            .build()
    }

    //---------------------------------------
    // Resize window helper function
    //---------------------------------------
    fn resize_window(&self) {
        let imp = self.imp();

        let prefs_dialog = imp.prefs_dialog.borrow();

        let sidebar_width = prefs_dialog.sidebar_width();
        let infopane_width = prefs_dialog.infopane_width();

        // Helper closure to convert sp to px
        let to_px = |sp: f64| -> f64 {
            imp.main_split_view.sidebar_width_unit().to_px(sp, None)
        };

        let min_packageview_width = to_px(500.0);

        self.set_width_request(to_px(infopane_width) as i32);

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

        // Package view sort sort ascending property notify signal
        imp.package_view.connect_sort_ascending_notify(clone!(
            #[weak] imp,
            move |view| {
                let sort_asc = view.sort_ascending();

                imp.sort_button.set_icon_name(
                    if sort_asc {
                        "view-sort-ascending-symbolic"
                    } else {
                        "view-sort-descending-symbolic"
                    }
                );

                imp.sort_button.set_tooltip_text(
                    Some(if sort_asc { "Descending" } else { "Ascending" })
                );
            }
        ));

        // Package view n_items property notify signal
        imp.package_view.selection().connect_items_changed(clone!(
            #[weak(rename_to = window)] self,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                window.imp().status_label.set_label(
                    &format!("{n_items} matching package{}", if n_items == 1 { "" } else { "s" })
                );

                window.action_set_enabled("view.copy-list", n_items != 0);
            }
        ));

        let prefs_dialog = imp.prefs_dialog.borrow();

        // Preferences sidebar width property notify signal
        prefs_dialog.connect_sidebar_width_notify(clone!(
            #[weak(rename_to = window)] self,
            move |_| {
                window.resize_window();
            }
        ));

        // Preferences infopane width property notify signal
        prefs_dialog.connect_infopane_width_notify(clone!(
            #[weak(rename_to = window)] self,
            move |_| {
                window.resize_window();
            }
        ));

        // Preferences AUR database download property notify
        prefs_dialog.connect_aur_database_download_notify(clone!(
            #[weak(rename_to = window)] self,
            move |prefs_dialog| {
                window.action_set_enabled(
                    "win.update-aur-database",
                    prefs_dialog.aur_database_download()
                );
            }
        ));

        // Preferences search mode property notify signal
        prefs_dialog.connect_search_mode_notify(clone!(
            #[weak] imp,
            move |prefs_dialog| {
                imp.search_bar.set_default_mode(prefs_dialog.search_mode());
            }
        ));

        // Preferences search prop property notify signal
        prefs_dialog.connect_search_prop_notify(clone!(
            #[weak] imp,
            move |prefs_dialog| {
                imp.search_bar.set_default_prop(prefs_dialog.search_prop());
            }
        ));

        // Preferences search delay property notify signal
        prefs_dialog.connect_search_delay_notify(clone!(
            #[weak] imp,
            move |prefs_dialog| {
                imp.search_bar.set_delay(prefs_dialog.search_delay() as u64);
            }
        ));
    }

    //---------------------------------------
    // Bind gsettings
    //---------------------------------------
    fn bind_gsettings(&self) {
        let imp = self.imp();

        let settings = gio::Settings::new(APP_ID);

        // Bind window settings
        settings.bind("window-width", self, "default-width").build();
        settings.bind("window-height", self, "default-height").build();
        settings.bind("window-maximized", self, "maximized").build();

        settings.bind("show-infopane", &imp.infopane_button.get(), "active").build();
        settings.bind("show-sidebar", &imp.sidebar_button.get(), "active").build();

        // Load initial search bar settings
        settings.bind("search-mode", &imp.search_bar.get(), "mode")
            .get()
            .get_no_changes()
            .build();

        settings.bind("search-prop", &imp.search_bar.get(), "prop")
            .get()
            .get_no_changes()
            .build();

        // Bind preferences
        let prefs_dialog = &*imp.prefs_dialog.borrow();

        settings.bind("color-scheme", prefs_dialog, "color-scheme").build();
        settings.bind("sidebar-width", prefs_dialog, "sidebar-width").build();
        settings.bind("infopane-width", prefs_dialog, "infopane-width").build();
        settings.bind("aur-database-download", prefs_dialog, "aur-database-download").build();
        settings.bind("aur-database-age", prefs_dialog, "aur-database-age").build();
        settings.bind("auto-refresh", prefs_dialog, "auto-refresh").build();
        settings.bind("remember-sort", prefs_dialog, "remember-sort").build();
        settings.bind("search-mode", prefs_dialog, "search-mode").build();
        settings.bind("search-prop", prefs_dialog, "search-prop").build();
        settings.bind("search-delay", prefs_dialog, "search-delay").build();
        settings.bind("property-max-lines", prefs_dialog, "property-max-lines").build();
        settings.bind("property-line-spacing", prefs_dialog, "property-line-spacing").build();
        settings.bind("underline-links", prefs_dialog, "underline-links").build();
        settings.bind("pkgbuild-style-scheme", prefs_dialog, "pkgbuild-style-scheme").build();
        settings.bind("pkgbuild-use-system-font", prefs_dialog, "pkgbuild-use-system-font").build();
        settings.bind("pkgbuild-custom-font", prefs_dialog, "pkgbuild-custom-font").build();

        // Load/save package view sort properties
        if prefs_dialog.remember_sort() {
            settings.bind("sort-prop", &imp.package_view.get(), "sort-prop")
                .get()
                .get_no_changes()
                .build();

            settings.bind("sort-ascending", &imp.package_view.get(), "sort-ascending")
                .get()
                .get_no_changes()
                .build();
        }

        settings.bind("sort-prop", &imp.package_view.get(), "sort-prop")
            .set()
            .build();

        settings.bind("sort-ascending", &imp.package_view.get(), "sort-ascending")
            .set()
            .build();
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind properties to pane visibility
        self.bind_property("show-sidebar", &imp.sidebar_split_view.get(), "show-sidebar")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("show-infopane", &imp.main_split_view.get(), "show-sidebar")
            .sync_create()
            .bidirectional()
            .build();

        // Bind sidebar button state to sidebar visibility
        imp.sidebar_button.bind_property("active", &imp.sidebar_split_view.get(), "show-sidebar")
            .sync_create()
            .bidirectional()
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

        // Bind property to package view sort prop
        self.bind_property("package-sort-prop", &imp.package_view.get(), "sort-prop")
            .sync_create()
            .bidirectional()
            .build();
    }

    //---------------------------------------
    // Setup alpm
    //---------------------------------------
    fn setup_alpm(&self, first_load: bool) {
        let imp = self.imp();

        let pacman_config = &PACMAN_CONFIG;
        let user_cache_dir = glib::user_cache_dir();

        // Load pacman log
        *PACMAN_LOG.write().unwrap() = fs::read_to_string(&pacman_config.log_file).ok();

        // Load pacman cache
        let mut cache_files: Vec<PathBuf> = vec![];

        for dir in &pacman_config.cache_dir {
            if let Ok(read_dir) = fs::read_dir(dir) {
                let files = read_dir.into_iter().flatten().map(|entry| entry.path());

                cache_files.extend(files);
            }
        }

        cache_files.par_sort_unstable();

        *PACMAN_CACHE.write().unwrap() = cache_files;

        // Init config dialog
        imp.config_dialog.borrow().init(pacman_config);

        // Clear windows
        imp.backup_window.borrow().remove_all();
        imp.log_window.borrow().remove_all();
        imp.cache_window.borrow().remove_all();
        imp.groups_window.borrow().remove_all();
        imp.stats_window.borrow().remove_all();

        // Get paru repos
        let mut paru_repos: Vec<(String, PathBuf)> = vec![];

        if PARU_PATH.is_ok()
            && let Ok(read_dir) = fs::read_dir(user_cache_dir.join("paru/clone/repo")) {
                let repos = read_dir.into_iter()
                    .flatten()
                    .map(|entry| {
                        (entry.file_name().to_string_lossy().into_owned(), entry.path())
                    });

                paru_repos.extend(repos);
            }

        // Create repo names list
        let repo_names: Vec<String> = pacman_config.repos.iter()
            .map(|r| r.name.as_str())
            .chain(paru_repos.iter().map(|(name, _)| name.as_str()))
            .chain(["aur", "local"])
            .map(ToOwned::to_owned)
            .collect();

        // Populate sidebar
        self.alpm_populate_sidebar(&repo_names, first_load);

        // Store repo names
        imp.repo_names.replace(repo_names);

        // Get AUR file (create cache dir)
        let cache_dir = user_cache_dir.join("pacview");
        
        let aur_file = fs::create_dir_all(&cache_dir)
            .map(|_| cache_dir.join("aur_packages"))
            .ok();

        imp.aur_file.replace(aur_file.clone());

        // If AUR database download is enabled and AUR file does not exist, download it
        if let Some(file) = aur_file
            .filter(|file| {
                imp.prefs_dialog.borrow().aur_database_download()
                    && fs::metadata(file).is_err()
            }) {
                imp.package_view.set_state(PackageViewState::AURDownload);
                imp.info_pane.set_pkg(None::<PkgObject>);

                glib::spawn_future_local(clone!(
                    #[weak(rename_to = window)] self,
                    async move {
                        let _ = aur_file::download(&file).await;

                        window.alpm_load_packages(paru_repos);
                    }
                ));
            } else {
                self.alpm_load_packages(paru_repos);
            }
    }

    //---------------------------------------
    // Setup alpm: populate sidebar
    //---------------------------------------
    fn alpm_populate_sidebar(&self, repo_names: &[String], first_load: bool) {
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

        for repo in repo_names {
            let label = if repo == "aur" { repo.to_uppercase() } else { repo.to_title_case() };

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

                if (saved_status_id == PkgFlags::empty() && flag == PkgFlags::INSTALLED)
                    || saved_status_id == flag {
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
    // Setup alpm: load alpm packages
    //---------------------------------------
    fn alpm_load_packages(&self, paru_repos: Vec<(String, PathBuf)>) {
        let imp = self.imp();

        // Get pacman config
        let pacman_config = &PACMAN_CONFIG;

        // Get AUR package names file
        let aur_download = imp.prefs_dialog.borrow().aur_database_download();
        let aur_file = imp.aur_file.borrow().to_owned();

        // Create async channel
        let (sender, receiver) = async_channel::bounded(1);

        // Create task to load package data
        let alpm_future = gio::spawn_blocking(move || {
            // Get alpm handle
            let handle = alpm_utils::alpm_with_conf(pacman_config)?;

            // Load AUR package names from file if AUR download is enabled in preferences
            let aur_names: HashSet<String> = if aur_download {
                aur_file
                    .and_then(|aur_file| fs::read_to_string(aur_file).ok())
                    .map(|s| s.lines().map(ToOwned::to_owned).collect())
                    .unwrap_or_default()
            } else {
                HashSet::default()
            };

            // Get paru repo package map
            let mut paru_map: HashMap<String, Rc<String>> = HashMap::new();

            for (name, path) in paru_repos {
                let name_rc = Rc::new(name);

                if let Ok(read_dir) = fs::read_dir(path) {
                    let items = read_dir.into_iter()
                        .flatten()
                        .map(|entry| {
                            (
                                entry.file_name().to_string_lossy().into_owned(),
                                Rc::clone(&name_rc)
                            )
                        });

                    paru_map.extend(items);
                }
            }

            let syncdbs = handle.syncdbs();
            let localdb = handle.localdb();

            // Load pacman local packages
            let local_data: Vec<PkgData> = localdb.pkgs().iter()
                // .filter(|&pkg| syncdbs.pkg(pkg.name()).is_err())
                .map(|pkg| {
                    let repository = paru_map.get(pkg.name())
                        .map_or_else(|| {
                            if aur_names.contains(pkg.name()) {
                                "aur"
                            } else if let Ok(sync_pkg) = syncdbs.pkg(pkg.name()) {
                                sync_pkg.db().map(|db| db.name()).unwrap_or_default()
                            } else {
                                "local"
                            }
                        }, |paru_repo| paru_repo);

                    PkgData::from_alpm(pkg, true, repository)
                })
                .collect();

            sender.send_blocking((local_data, true))
                .expect("Failed to send through channel");

            // Load pacman sync packages
            let sync_data: Vec<PkgData> = syncdbs.iter()
                .flat_map(|db| {
                    db.pkgs().iter()
                        .filter(|pkg| localdb.pkg(pkg.name()).is_err())
                        .map(|pkg| PkgData::from_alpm(pkg, false, db.name()))
                })
                .collect();

            sender.send_blocking((sync_data, false))
                .expect("Failed to send through channel");

            Ok(())
        });

        glib::spawn_future_local(clone!(
            #[weak(rename_to = window)] self,
            async move {
                let imp = window.imp();

                // Hide update count in sidebar
                imp.update_row.borrow().set_state(FilterRowState::Reset);

                // Show package view loading spinner
                imp.package_view.set_state(PackageViewState::PackageLoad);

                // Clear package view
                imp.package_view.clear_packages();

                // Clear info pane package
                imp.info_pane.set_pkg(None::<PkgObject>);

                // Get alpm handle
                let handle_ref = alpm_utils::alpm_with_conf(pacman_config)
                    .map(Rc::new)
                    .ok();

                // Get package lists
                let mut pkgs: Vec<PkgObject> = Vec::new();
                let mut installed_pkgs: Vec<PkgObject> = Vec::new();

                while let Ok((pkg_data, local_data)) = receiver.recv().await {
                    // Resize package lists
                    let len = pkg_data.len();

                    pkgs.reserve(len);

                    if local_data {
                        installed_pkgs.reserve(len);
                    }

                    // Process package data
                    let mut pkg_chunk: Vec<PkgObject> = Vec::with_capacity(len);

                    for data in pkg_data {
                        let pkg = PkgObject::new(data, handle_ref.as_ref().map(Rc::clone));

                        if local_data {
                            installed_pkgs.push(pkg.clone());
                        }

                        pkg_chunk.push(pkg);
                    }

                    // Add packages to package view
                    imp.package_view.append_packages(&pkg_chunk);

                    pkgs.append(&mut pkg_chunk);

                    // Hide package view loading spinner
                    if local_data {
                        imp.package_view.set_state(PackageViewState::Normal);
                    }
                }

                // Await package load task
                let result: alpm::Result<()> = alpm_future.await
                    .expect("Failed to complete task");

                match result {
                    Ok(()) => {
                        // Store alpm handle
                        ALPM_HANDLE.replace(handle_ref);

                        // Store package lists
                        PKGS.replace(pkgs);
                        INSTALLED_PKGS.replace(installed_pkgs);

                        // Get package updates
                        window.get_package_updates();

                        // Check AUR package names file age
                        let (max_age, database_download) = {
                            let prefs_dialog = imp.prefs_dialog.borrow();

                            (prefs_dialog.aur_database_age() as u64, prefs_dialog.aur_database_download())
                        };

                        if database_download {
                            let aur_file = imp.aur_file.borrow().to_owned();

                            if let Some(aur_file) = aur_file
                                && aur_file::check_age(&aur_file, max_age) {
                                    let _ = aur_file::download(&aur_file).await;
                                }
                        }
                    },
                    Err(error) => {
                        let warning_dialog = adw::AlertDialog::builder()
                            .heading("Alpm Error")
                            .body(error.to_string().to_title_case())
                            .default_response("ok")
                            .build();

                        warning_dialog.add_responses(&[("ok", "_Ok")]);

                        warning_dialog.present(Some(&window));
                    }
                }

                // Set focus on package view
                imp.package_view.view().grab_focus();
            }
        ));
    }

    //---------------------------------------
    // Setup alpm: get package updates
    //---------------------------------------
    fn get_package_updates(&self) {
        let imp = self.imp();

        let update_row = imp.update_row.borrow().clone();
        update_row.set_state(FilterRowState::Checking);

        // Spawn async process to check for updates
        glib::spawn_future_local(clone!(
            #[weak] imp,
            async move {
                let mut update_str = String::new();
                let mut error_msg: Option<String> = None;

                // Check for pacman updates async
                let pacman_handle = async_command::run("/usr/bin/checkupdates", &[]);

                let (pacman_res, aur_res) = if let Ok(paru_path) = PARU_PATH.as_ref() {
                    // Check for AUR updates async
                    let aur_handle = async_command::run(paru_path, &["-Qu", "--mode=ap"]);

                    join!(pacman_handle, aur_handle)
                } else {
                    (pacman_handle.await, Ok((None, String::new())))
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
                update_row.set_state(FilterRowState::Updates(error_msg, update_map.len() as u32));

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
        if let Ok(mut debouncer) = new_debouncer(
            Duration::from_secs(2),
            None,
            move |result: DebounceEventResult| {
                if result.unwrap_or_default().iter()
                    .map(|event| event.kind)
                    .any(|kind| kind.is_create() || kind.is_modify() || kind.is_remove()) {
                        sender.send_blocking(())
                            .expect("Failed to send through channel");
                    }
            }
        ) {
            // Watch pacman local db path
            let path = Path::new(&PACMAN_CONFIG.db_path).join("local");

            if debouncer.watch(&path, RecursiveMode::Recursive).is_ok() {
                // Store debouncer
                imp.notify_debouncer.set(debouncer).unwrap();

                // Attach receiver for async channel
                glib::spawn_future_local(clone!(
                    #[weak(rename_to = window)] self,
                    async move {
                        while receiver.recv().await == Ok(()) {
                            if window.imp().prefs_dialog.borrow().auto_refresh() {
                                gtk::prelude::WidgetExt::activate_action(&window, "win.refresh", None).unwrap();
                            }
                        }
                    }
                ));
            }
        };
    }
}
