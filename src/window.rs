use std::cell::{Cell, RefCell, OnceCell};
use std::sync::LazyLock;
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
use regex::Regex;
use futures::join;
use notify_debouncer_full::{notify::{INotifyWatcher, RecursiveMode}, new_debouncer, Debouncer, DebounceEventResult, NoCache};

use crate::{
    APP_ID,
    PacViewApplication,
    pkg_data::{PkgFlags, PkgData},
    pkg_object::PkgObject,
    search_bar::SearchBar,
    package_view::{PackageView, PackageViewState, SortProp},
    info_pane::InfoPane,
    repo_item::RepoItem,
    status_item::StatusItem,
    status_item_indicator::StatusItemState,
    stats_window::StatsWindow,
    backup_window::BackupWindow,
    groups_window::GroupsWindow,
    log_window::LogWindow,
    cache_window::CacheWindow,
    config_dialog::ConfigDialog,
    preferences_dialog::PreferencesDialog,
    utils::{Paths, Pacman, AurDBFile, AsyncCommand}
};

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
        pub(super) sidebar_show_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) sort_button: TemplateChild<adw::SplitButton>,
        #[template_child]
        pub(super) search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) infopane_show_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) repo_sidebar: TemplateChild<adw::Sidebar>,
        #[template_child]
        pub(super) repo_section: TemplateChild<adw::SidebarSection>,
        #[template_child]
        pub(super) status_sidebar: TemplateChild<adw::Sidebar>,
        #[template_child]
        pub(super) status_section: TemplateChild<adw::SidebarSection>,

        #[template_child]
        pub(super) search_bar: TemplateChild<SearchBar>,
        #[template_child]
        pub(super) package_view: TemplateChild<PackageView>,
        #[template_child]
        pub(super) package_count_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub(super) info_pane: TemplateChild<InfoPane>,

        #[property(get, set, builder(SortProp::default()))]
        package_sort_prop: Cell<SortProp>,

        pub(super) repo_names: RefCell<Vec<String>>,

        pub(super) saved_repo_id: RefCell<Option<String>>,
        pub(super) saved_status_id: Cell<PkgFlags>,

        pub(super) all_repo_item: RefCell<RepoItem>,
        pub(super) all_status_item: RefCell<StatusItem>,
        pub(super) update_item: RefCell<StatusItem>,

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
            // Refresh action
            klass.install_action("win.refresh", None, |window, _, _| {
                let imp = window.imp();

                let repo_id = imp.repo_sidebar.selected_item()
                    .and_downcast::<RepoItem>()
                    .and_then(|item| item.id());

                imp.saved_repo_id.replace(repo_id);

                let status_id = imp.status_sidebar.selected_item()
                    .and_downcast::<StatusItem>()
                    .map(|item| item.id())
                    .unwrap_or_default();

                imp.saved_status_id.set(status_id);

                imp.package_count_label.set_label("");

                window.setup_alpm(false);
            });

            // Check for updates action
            klass.install_action_async("win.check-updates", None, async |window, _, _| {
                window.get_package_updates().await;
            });

            // Update AUR database action
            klass.install_action_async("win.update-aur-database", None, async |window, _, _| {
                let imp = window.imp();

                if AurDBFile::path().is_some() {
                    imp.update_item.borrow().set_state(StatusItemState::Reset);
                    imp.package_view.set_state(PackageViewState::AURDownload);
                    imp.info_pane.set_pkg(None::<PkgObject>);
                    imp.package_count_label.set_label("");

                    // Spawn tokio task to download AUR package names file
                    let _ = AurDBFile::download().await;

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
                let imp = window.imp();

                imp.backup_window.borrow().present();
            });

            klass.install_action("win.show-pacman-cache", None, |window, _, _| {
                window.imp().cache_window.borrow().present();
            });

            klass.install_action("win.show-pacman-groups", None, |window, _, _| {
                let imp = window.imp();

                imp.groups_window.borrow().present();
            });

            klass.install_action("win.show-pacman-log", None, |window, _, _| {
                window.imp().log_window.borrow().present();
            });

            klass.install_action("win.show-stats", None, |window, _, _| {
                let imp = window.imp();

                imp.stats_window.borrow().present();
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
            klass.add_binding(Key::B, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.sidebar_show_button.is_visible() {
                    imp.sidebar_show_button.emit_clicked();
                }

                glib::Propagation::Stop
            });

            // Show infopane key binding
            klass.add_binding(Key::I, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.infopane_show_button.is_visible() {
                    imp.infopane_show_button.emit_clicked();
                }

                glib::Propagation::Stop
            });

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

                imp.all_repo_item.borrow().activate();
                imp.all_status_item.borrow().activate();

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
            klass.add_binding(Key::P, ModifierType::ALT_MASK, |window| {
                window.imp().info_pane.show_pkgbuild();

                glib::Propagation::Stop
            });

            // Infopane show hashes key bindings
            klass.add_binding(Key::H, ModifierType::ALT_MASK, |window| {
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

        let sidebar_width = imp.sidebar_split_view.min_sidebar_width();
        let infopane_width = prefs_dialog.infopane_width();

        // Helper closure to convert sp to px
        let to_px = |sp: f64| -> f64 {
            imp.main_split_view.sidebar_width_unit().to_px(sp, None)
        };

        let min_packageview_width = 400.0;

        self.set_width_request(to_px(infopane_width) as i32);

        imp.main_split_view.set_min_sidebar_width(infopane_width);
        imp.main_split_view.set_max_sidebar_width(infopane_width*2.0);

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

        // Repo sidebar activated signal
        imp.repo_sidebar.connect_activated(clone!(
            #[weak] imp,
            move |sidebar, index| {
                let id = sidebar.items().item(index)
                    .and_downcast::<RepoItem>()
                    .expect("Failed to downcast to 'RepoItem'")
                    .id();

                imp.package_view.repo_filter_changed(id.as_deref());

                if imp.sidebar_split_view.is_collapsed() {
                    imp.sidebar_split_view.set_show_sidebar(false);
                }

                imp.package_view.view().grab_focus();
            }
        ));

        // Status sidebar activated signal
        imp.status_sidebar.connect_activated(clone!(
            #[weak] imp,
            move |sidebar, index| {
                let id = sidebar.items().item(index)
                    .and_downcast::<StatusItem>()
                    .expect("Failed to downcast to 'StatusItem'")
                    .id();

                imp.package_view.status_filter_changed(id);

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
                    Some(if sort_asc { "Sort Descending" } else { "Sort Ascending" })
                );
            }
        ));

        // Package view n_items property notify signal
        imp.package_view.selection().connect_items_changed(clone!(
            #[weak(rename_to = window)] self,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                window.imp().package_count_label.set_label(&format!(
                    "{n_items} matching package{}",
                    if n_items == 1 { "" } else { "s" }
                ));

                window.action_set_enabled("view.copy-list", n_items != 0);
            }
        ));

        let prefs_dialog = imp.prefs_dialog.borrow();

        // Sidebar show button clicked signal
        imp.sidebar_show_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                imp.sidebar_split_view.set_show_sidebar(true);
            }
        ));

        // Infopane show button clicked signal
        imp.infopane_show_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                imp.main_split_view.set_show_sidebar(true);
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

        // Bind search button state to search bar enabled state
        imp.search_button.bind_property("active", &imp.search_bar.get(), "enabled")
            .sync_create()
            .bidirectional()
            .build();

        // Bind property to package view sort prop
        self.bind_property("package-sort-prop", &imp.package_view.get(), "sort-prop")
            .sync_create()
            .bidirectional()
            .build();

        // Set window parents
        imp.backup_window.borrow().set_transient_for(Some(self));
        imp.cache_window.borrow().set_transient_for(Some(self));
        imp.groups_window.borrow().set_transient_for(Some(self));
        imp.log_window.borrow().set_transient_for(Some(self));
        imp.stats_window.borrow().set_transient_for(Some(self));
    }

    //---------------------------------------
    // Setup alpm
    //---------------------------------------
    fn setup_alpm(&self, first_load: bool) {
        let imp = self.imp();

        let pacman_config = Pacman::config();
        let user_cache_dir = glib::user_cache_dir();

        // Load pacman log
        Pacman::set_log(fs::read_to_string(&pacman_config.log_file).ok());

        // Load pacman cache
        let mut cache_files: Vec<PathBuf> = vec![];

        for dir in &pacman_config.cache_dir {
            if let Ok(read_dir) = fs::read_dir(dir) {
                let files = read_dir.into_iter().flatten().map(|entry| entry.path());

                cache_files.extend(files);
            }
        }

        cache_files.sort_unstable();

        Pacman::set_cache(cache_files);

        // Init config dialog
        imp.config_dialog.borrow().init(pacman_config);

        // Clear windows
        imp.backup_window.borrow().clear();
        imp.log_window.borrow().clear();
        imp.cache_window.borrow().clear();
        imp.groups_window.borrow().clear();
        imp.stats_window.borrow().clear();

        // Get paru repos
        let mut paru_repos: Vec<(String, PathBuf)> = vec![];

        if Paths::paru().is_ok()
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

        // If AUR database download is enabled and AUR file does not exist, download it
        if imp.prefs_dialog.borrow().aur_database_download() && AurDBFile::path().as_ref()
            .is_some_and(|aur_file| fs::metadata(aur_file).is_err()) {
                imp.package_view.set_state(PackageViewState::AURDownload);
                imp.info_pane.set_pkg(None::<PkgObject>);

                glib::spawn_future_local(clone!(
                    #[weak(rename_to = window)] self,
                    async move {
                        let _ = AurDBFile::download().await;

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

        // Add repository items (enumerate pacman repositories)
        imp.repo_section.remove_all();

        let saved_repo_id = imp.saved_repo_id.take();

        let all_item = RepoItem::new("repository-symbolic", "All", None);

        imp.repo_section.append(all_item.clone());

        if saved_repo_id.is_none() {
            all_item.activate();
        }

        imp.all_repo_item.replace(all_item);

        for repo in repo_names {
            let label = if repo == "aur" { repo.to_uppercase() } else { repo.to_title_case() };

            let item = RepoItem::new("repository-symbolic", &label, Some(repo));

            imp.repo_section.append(item.clone());

            if saved_repo_id.as_ref() == Some(repo) {
                item.activate();
            }
        }

        // If first load, add package status items (enumerate PkgStatusFlags)
        if first_load {
            let saved_status_id = imp.saved_status_id.replace(PkgFlags::empty());

            let flags = glib::FlagsClass::new::<PkgFlags>();

            for f in flags.values() {
                let flag = PkgFlags::from_bits_truncate(f.value());
                let nick = f.nick();

                let item = StatusItem::new(&format!("status-{nick}-symbolic"), f.name(), flag);

                imp.status_section.append(item.clone());

                if (saved_status_id == PkgFlags::empty() && flag == PkgFlags::INSTALLED)
                    || saved_status_id == flag {
                        item.activate();
                    }

                match flag {
                    PkgFlags::ALL => { imp.all_status_item.replace(item); },
                    PkgFlags::UPDATES => { imp.update_item.replace(item); },
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
        let pacman_config = Pacman::config();

        // Get AUR package names file
        let aur_download = imp.prefs_dialog.borrow().aur_database_download();

        // Create async channel
        let (sender, receiver) = async_channel::bounded(1);

        // Create task to load package data
        let alpm_future = gio::spawn_blocking(move || {
            // Get alpm handle
            let alpm_handle = alpm_utils::alpm_with_conf(pacman_config)?;

            // Load AUR package names from file if AUR download is enabled in preferences
            let aur_names: HashSet<String> = if aur_download {
                AurDBFile::path().as_ref()
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

            let syncdbs = alpm_handle.syncdbs();
            let localdb = alpm_handle.localdb();

            // Load pacman local packages
            let local_data: Vec<PkgData> = localdb.pkgs().iter()
                .map(|pkg| {
                    let repository = paru_map.get(pkg.name())
                        .map_or_else(|| {
                            if aur_names.contains(pkg.name()) {
                                "aur"
                            } else {
                                syncdbs.pkg(pkg.name()).ok()
                                    .and_then(|sync_pkg| sync_pkg.db())
                                    .map_or("local", alpm::Db::name)
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

        // Attach package load task receiver
        glib::spawn_future_local(clone!(
            #[weak(rename_to = window)] self,
            async move {
                let imp = window.imp();

                // Hide update count in sidebar
                imp.update_item.borrow().set_state(StatusItemState::Reset);

                // Show package view loading spinner
                imp.package_view.set_state(PackageViewState::PackageLoad);

                // Clear info pane package
                imp.info_pane.set_pkg(None::<PkgObject>);

                // Initialize PkgObject alpm handle
                PkgObject::with_alpm_handle(|handle| {
                    let alpm_handle = alpm_utils::alpm_with_conf(pacman_config).ok();

                    handle.replace(alpm_handle);
                });

                // Process task messages
                while let Ok((pkg_data, is_local_data)) = receiver.recv().await {
                    // Add packages to package view
                    let pkg_chunk: Vec<PkgObject> = pkg_data.into_iter()
                        .map(PkgObject::new)
                        .collect();

                    imp.package_view.splice_packages(&pkg_chunk, is_local_data);

                    // Hide loading spinner and focus package view
                    if is_local_data {
                        imp.package_view.set_state(PackageViewState::Normal);

                        imp.package_view.view().grab_focus();
                    }
                }

                // Await package load task
                let result: alpm::Result<()> = alpm_future.await
                    .expect("Failed to complete task");

                match result {
                    Ok(()) => {
                        // Populate windows
                        glib::idle_add_local_once(clone!(
                            #[weak] imp,
                            move || {
                                imp.backup_window.borrow()
                                    .populate(&imp.package_view.pkg_model());

                                imp.cache_window.borrow().populate();

                                imp.groups_window.borrow()
                                    .populate(&imp.package_view.pkg_model());

                                imp.log_window.borrow().populate();

                                imp.stats_window.borrow()
                                    .populate(&imp.repo_names.borrow(), &imp.package_view.pkg_model());
                            }
                        ));

                        // Get package updates
                        window.get_package_updates().await;

                        // Check AUR package names file age
                        let (max_age, aur_download) = {
                            let prefs_dialog = imp.prefs_dialog.borrow();

                            (prefs_dialog.aur_database_age() as u64, prefs_dialog.aur_database_download())
                        };

                        if aur_download && AurDBFile::out_of_date(max_age) {
                            let _ = AurDBFile::download().await;
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
            }
        ));
    }

    //---------------------------------------
    // Setup alpm: get package updates
    //---------------------------------------
    #[allow(clippy::future_not_send)]
    async fn get_package_updates(&self) {
        let imp = self.imp();

        // Reset sidebar update count
        imp.update_item.borrow().set_state(StatusItemState::Checking);

        // Check for pacman updates
        let mut update_str = String::new();
        let mut error_msg: Option<String> = None;

        let pacman_handle = AsyncCommand::run("/usr/bin/checkupdates", &[""]);

        let (pacman_res, aur_res) = if let Ok(paru_path) = Paths::paru().as_ref() {
            // Check for AUR updates
            let aur_handle = AsyncCommand::run(paru_path, &["-Qu", "--mode=ap"]);

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
        let update_item = imp.update_item.borrow();

        update_item.set_state(StatusItemState::Updates(error_msg, update_map.len() as u32));

        // If update item is selected, refresh package status filter
        if imp.status_sidebar.selected_item()
            .and_downcast::<StatusItem>().as_ref() == Some(&*update_item) {
                imp.package_view.status_filter_changed(update_item.id());
            }
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
            let path = Path::new(&Pacman::config().db_path).join("local");

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
        }
    }
}
