use std::cell::{RefCell, OnceCell};
use std::sync::LazyLock;
use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use std::fs;

use gtk::{gio, glib, gdk};
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::{clone, Propagation};
use gdk::{Key, ModifierType};

use alpm_utils::DbListExt;
use heck::ToTitleCase;
use regex::Regex;
use futures::join;
use notify_debouncer_full::{notify::{INotifyWatcher, RecursiveMode}, new_debouncer, Debouncer, DebounceEventResult, NoCache};
use tokio_util::sync::CancellationToken;

use crate::{
    APP_ID,
    PacViewApplication,
    pkg_data::{PkgFlags, PkgData},
    pkg_object::PkgObject,
    package_view::{PackageView, PackageViewState},
    info_pane::InfoPane,
    repo_item::RepoItem,
    status_item::{StatusItem, StatusItemState},
    stats_window::StatsWindow,
    backup_window::BackupWindow,
    groups_window::GroupsWindow,
    log_window::LogWindow,
    cache_window::CacheWindow,
    config_dialog::ConfigDialog,
    preferences_dialog::PreferencesDialog,
    utils::{Paths, Pacman, ParuConf, AurDBFile, TokioUtils}
};

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
        pub(super) main_menu_button: TemplateChild<gtk::MenuButton>,

        #[template_child]
        pub(super) repo_sidebar: TemplateChild<adw::Sidebar>,
        #[template_child]
        pub(super) repo_section: TemplateChild<adw::SidebarSection>,
        #[template_child]
        pub(super) status_sidebar: TemplateChild<adw::Sidebar>,
        #[template_child]
        pub(super) status_section: TemplateChild<adw::SidebarSection>,

        #[template_child]
        pub(super) package_view: TemplateChild<PackageView>,
        #[template_child]
        pub(super) info_pane: TemplateChild<InfoPane>,

        pub(super) saved_repo_id: RefCell<Option<String>>,

        pub(super) all_repo_item: RefCell<RepoItem>,
        pub(super) all_status_item: RefCell<StatusItem>,
        pub(super) installed_item: RefCell<StatusItem>,
        pub(super) update_item: RefCell<StatusItem>,

        pub(super) update_cancel_token: RefCell<Option<CancellationToken>>,

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

    impl ObjectImpl for PacViewWindow {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
            obj.setup_widgets();
            obj.bind_gsettings();
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

                imp.package_view.count_label().set_label("");

                window.cancel_package_updates();

                window.setup_alpm(false);
            });

            // Check for updates action
            klass.install_action_async("win.check-updates", None, async |window, _, _| {
                window.cancel_package_updates();

                window.get_package_updates().await;
            });

            // Update AUR database action
            klass.install_action_async("win.update-aur-database", None, async |window, _, _| {
                let imp = window.imp();

                if AurDBFile::path().is_some() {
                    imp.update_item.borrow().set_state(StatusItemState::Reset);
                    imp.package_view.set_state(PackageViewState::AURDownload);
                    imp.info_pane.set_pkg(None::<PkgObject>);
                    imp.package_view.count_label().set_label("");

                    window.cancel_package_updates();

                    // Spawn tokio task to download AUR package names file
                    let _ = AurDBFile::download().await;

                    // Refresh packages
                    gtk::prelude::WidgetExt::activate_action(&window, "win.refresh", None)
                        .unwrap();
                }
            });

            // Package view copy list action
            klass.install_action("win.copy-package-list", None, |window, _, _| {
                 window.imp().package_view.copy_list();
            });

            // Show sidebar action
            klass.install_action("win.show-sidebar", None, |window, _, _| {
                window.imp().sidebar_split_view.set_show_sidebar(true);
            });

            // Show infopane action
            klass.install_action("win.show-infopane", None, |window, _, _| {
                window.imp().main_split_view.set_show_sidebar(true);
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
                window.imp().package_view.search_bar().set_enabled(true);

                Propagation::Stop
            });

            klass.add_binding(Key::Escape, ModifierType::NO_MODIFIER_MASK, |window| {
                let imp = window.imp();

                if (imp.sidebar_split_view.is_collapsed() && imp.sidebar_split_view.shows_sidebar()) || (imp.main_split_view.is_collapsed() && imp.main_split_view.shows_sidebar()) {
                    Propagation::Proceed
                } else {
                    window.imp().package_view.search_bar().set_enabled(false);

                    Propagation::Stop
                }
            });

            // Show sidebar key binding
            klass.add_binding_action(Key::B, ModifierType::CONTROL_MASK, "win.show-sidebar");

            // Show infopane key binding
            klass.add_binding_action(Key::I, ModifierType::CONTROL_MASK, "win.show-infopane");

            // Package view grouping key binding
            klass.add_binding(Key::G, ModifierType::ALT_MASK, |window| {
                let imp = window.imp();

                imp.package_view.set_grouping(!imp.package_view.grouping());

                Propagation::Stop
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
            klass.add_binding_action(Key::C, ModifierType::CONTROL_MASK | ModifierType::ALT_MASK, "win.copy-package-list");

            // View show all packages key binding
            klass.add_binding(Key::A, ModifierType::ALT_MASK, |window| {
                let imp = window.imp();

                imp.all_repo_item.borrow().activate();
                imp.all_status_item.borrow().activate();

                Propagation::Stop
            });

            // View show installed packages key binding
            klass.add_binding(Key::D, ModifierType::ALT_MASK, |window| {
                let imp = window.imp();

                imp.all_repo_item.borrow().activate();
                imp.installed_item.borrow().activate();

                Propagation::Stop
            });

            // View show updates key binding
            klass.add_binding(Key::U, ModifierType::ALT_MASK, |window| {
                let imp = window.imp();

                imp.all_repo_item.borrow().activate();
                imp.update_item.borrow().activate();

                Propagation::Stop
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
                window.imp().info_pane.set_active_tab("info");

                Propagation::Stop
            });

            klass.add_binding(Key::F, ModifierType::ALT_MASK, |window| {
                window.imp().info_pane.set_active_tab("files");

                Propagation::Stop
            });

            klass.add_binding(Key::L, ModifierType::ALT_MASK, |window| {
                window.imp().info_pane.set_active_tab("log");

                Propagation::Stop
            });

            // Infopane previous/next key bindings
            klass.add_binding(Key::Left, ModifierType::ALT_MASK, |window| {
                window.imp().info_pane.activate_action("info.previous", None).unwrap();

                Propagation::Stop
            });

            klass.add_binding(Key::Right, ModifierType::ALT_MASK, |window| {
                window.imp().info_pane.activate_action("info.next", None).unwrap();

                Propagation::Stop
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
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

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

        // Package view n_items property notify signal
        imp.package_view.selection().connect_items_changed(clone!(
            #[weak(rename_to = window)] self,
            move |selection, _, _, _| {
                window.action_set_enabled("win.copy-package-list", selection.n_items() != 0);
            }
        ));

        // Preferences infopane width property notify signal
        let prefs_dialog = imp.prefs_dialog.borrow();

        prefs_dialog.connect_infopane_width_notify(clone!(
            #[weak(rename_to = window)] self,
            move |prefs_dialog| {
                let imp = window.imp();

                let infopane_width = prefs_dialog.infopane_width();
                let sidebar_width = imp.sidebar_split_view.min_sidebar_width();
                let min_packageview_width = 400.0;

                let unit = imp.main_split_view.sidebar_width_unit();

                let main_condition = adw::BreakpointCondition::new_length(
                    adw::BreakpointConditionLengthType::MaxWidth,
                    sidebar_width + infopane_width + min_packageview_width,
                    adw::LengthUnit::Sp
                );

                let sidebar_condition = adw::BreakpointCondition::new_length(
                    adw::BreakpointConditionLengthType::MaxWidth,
                    sidebar_width + infopane_width,
                    adw::LengthUnit::Sp
                );

                window.set_width_request(unit.to_px(infopane_width, None) as i32);

                imp.main_split_view.set_min_sidebar_width(infopane_width);
                imp.main_split_view.set_max_sidebar_width(infopane_width*2.0);

                imp.main_breakpoint.set_condition(Some(&main_condition));
                imp.sidebar_breakpoint.set_condition(Some(&sidebar_condition));
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
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Add main breakpoint setters
        imp.main_breakpoint.add_setters(&[
            (&imp.main_split_view.get().upcast::<glib::Object>(), "collapsed", true),
            (&imp.package_view.infopane_button().upcast(), "visible", true)
        ]);

        // Add sidebar breakpoint setters
        imp.sidebar_breakpoint.add_setters(&[
            (&imp.main_split_view.get().upcast::<glib::Object>(), "collapsed", true),
            (&imp.sidebar_split_view.get().upcast(), "collapsed", true),
            (&imp.main_menu_button.get().upcast(), "visible", false),
            (&imp.package_view.main_menu_button().upcast(), "visible", true),
            (&imp.package_view.sidebar_button().upcast(), "visible", true),
            (&imp.package_view.infopane_button().upcast(), "visible", true)
        ]);

        // Set window parents
        imp.backup_window.borrow().set_transient_for(Some(self));
        imp.cache_window.borrow().set_transient_for(Some(self));
        imp.groups_window.borrow().set_transient_for(Some(self));
        imp.log_window.borrow().set_transient_for(Some(self));
        imp.stats_window.borrow().set_transient_for(Some(self));

        // Bind preferences dialog properties to search bar
        let prefs_dialog = imp.prefs_dialog.borrow();
        let search_bar = imp.package_view.search_bar();

        prefs_dialog.bind_property("search-prop", &search_bar, "default-prop")
            .sync_create()
            .build();

        prefs_dialog.bind_property("search-exact", &search_bar, "default-exact")
            .sync_create()
            .build();
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
        settings.bind("search-prop", &imp.package_view.search_bar(), "prop")
            .get()
            .get_no_changes()
            .build();

        settings.bind("search-exact", &imp.package_view.search_bar(), "exact")
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
        settings.bind("remember-grouping", prefs_dialog, "remember-grouping").build();
        settings.bind("search-prop", prefs_dialog, "search-prop").build();
        settings.bind("search-exact", prefs_dialog, "search-exact").build();
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

        // Load/save package view grouping property
        if prefs_dialog.remember_grouping() {
            settings.bind("grouping", &imp.package_view.get(), "grouping")
                .get()
                .get_no_changes()
                .build();
        }

        settings.bind("grouping", &imp.package_view.get(), "grouping")
            .set()
            .build();
    }

    //---------------------------------------
    // Setup alpm
    //---------------------------------------
    fn setup_alpm(&self, first_load: bool) {
        let imp = self.imp();

        let pacman_config = Pacman::config();

        // Load pacman log
        Pacman::set_log(fs::read_to_string(&pacman_config.log_file).ok());

        // Load pacman cache
        let mut cache_files: Vec<PathBuf> = pacman_config.cache_dir.iter()
            .flat_map(fs::read_dir)
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .collect();

        cache_files.sort_unstable();

        Pacman::set_cache(cache_files);

        // Init config dialog
        imp.config_dialog.borrow().init(pacman_config);

        // Create repo names list
        let repo_names: Vec<String> = pacman_config.repos.iter()
            .map(|r| r.name.clone())
            .chain(ParuConf::repo_names())
            .chain(["aur", "local"].map(ToOwned::to_owned))
            .collect();

        // Populate sidebar
        self.alpm_populate_sidebar(&repo_names, first_load);

        // If AUR database download is enabled and AUR file does not exist, download it
        if imp.prefs_dialog.borrow().aur_database_download() && AurDBFile::path().as_ref()
            .is_some_and(|aur_file| fs::metadata(aur_file).is_err()) {
                imp.package_view.set_state(PackageViewState::AURDownload);
                imp.info_pane.set_pkg(None::<PkgObject>);

                glib::spawn_future_local(clone!(
                    #[weak(rename_to = window)] self,
                    async move {
                        let _ = AurDBFile::download().await;

                        window.alpm_load_packages(repo_names);
                    }
                ));
            } else {
                self.alpm_load_packages(repo_names);
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
            let flags = glib::FlagsClass::new::<PkgFlags>();

            for f in flags.values() {
                let flag = PkgFlags::from_bits_truncate(f.value());
                let nick = f.nick();

                let item = StatusItem::new(&format!("status-{nick}-symbolic"), f.name(), flag);

                imp.status_section.append(item.clone());

                if flag == PkgFlags::INSTALLED {
                    item.activate();
                }

                match flag {
                    PkgFlags::ALL => { imp.all_status_item.replace(item); },
                    PkgFlags::INSTALLED => { imp.installed_item.replace(item); },
                    PkgFlags::UPDATES => { imp.update_item.replace(item); },
                    _ => {}
                }
            }
        }
    }

    //---------------------------------------
    // Setup alpm: load alpm packages
    //---------------------------------------
    fn alpm_load_packages(&self, repo_names: Vec<String>) {
        let imp = self.imp();

        // Get AUR download preference
        let aur_download = imp.prefs_dialog.borrow().aur_database_download();

        // Create task to load package data
        let (sender, receiver) = async_channel::bounded(1);

        let alpm_future = gio::spawn_blocking(move || {
            // Get alpm handle
            let pacman_config = Pacman::config();

            let alpm_handle = alpm_utils::alpm_with_conf(pacman_config)?;

            // Load AUR package names from file if AUR download is enabled in preferences
            let aur_file = if aur_download {
                AurDBFile::path()
                    .and_then(|aur_file| fs::read_to_string(aur_file).ok())
                    .unwrap_or_default()
            } else {
                String::new()
            };

            let n_lines = aur_file.lines().count();
            let mut aur_names: HashSet<&str> = HashSet::with_capacity(n_lines);
            aur_names.extend(aur_file.lines());

            // Get paru repo package map
            let paru_map = ParuConf::local_pkg_map();

            let syncdbs = alpm_handle.syncdbs();
            let localdb = alpm_handle.localdb();

            // Load pacman local packages
            let local_data: Vec<PkgData> = localdb.pkgs().iter()
                .map(|pkg| {
                    let repository = if let Some(repo) = paru_map.get(pkg.name()) {
                        repo.as_str()
                    } else if aur_names.contains(pkg.name()) {
                        "aur"
                    } else {
                        syncdbs.pkg(pkg.name()).ok()
                            .and_then(|sync_pkg| sync_pkg.db())
                            .map_or("local", alpm::Db::name)
                    };

                    PkgData::from_alpm(pkg, true, repository)
                })
                .collect();

            sender.send_blocking((local_data, true))
                .expect("Failed to send through channel");

            // Load pacman sync packages
            for db in syncdbs {
                let mut sync_data: Vec<PkgData> = Vec::with_capacity(db.pkgs().len());

                sync_data.extend(
                    db.pkgs().iter()
                        .filter(|pkg| localdb.pkg(pkg.name()).is_err())
                        .map(|pkg| PkgData::from_alpm(pkg, false, db.name()))
                );

                sender.send_blocking((sync_data, false))
                    .expect("Failed to send through channel");
            }

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
                PkgObject::init_alpm_handle();

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
                        // Get package updates
                        window.get_package_updates().await;

                        // Populate windows
                        glib::idle_add_local_once(clone!(
                            #[weak] imp,
                            move || {
                                let pkg_model = imp.package_view.pkg_model();

                                imp.backup_window.borrow().populate(&pkg_model);
                                imp.cache_window.borrow().populate();
                                imp.groups_window.borrow().populate(&pkg_model);
                                imp.log_window.borrow().populate();
                                imp.stats_window.borrow().populate(&repo_names, &pkg_model);
                            }
                        ));

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
    // Cancel package updates
    //---------------------------------------
    fn cancel_package_updates(&self) {
        if let Some(token) = self.imp().update_cancel_token.take() {
            token.cancel();
        }
    }

    //---------------------------------------
    // Setup alpm: get package updates
    //---------------------------------------
    #[allow(clippy::future_not_send)]
    async fn get_package_updates(&self) {
        let imp = self.imp();

        // Reset sidebar update count
        imp.update_item.borrow().set_state(StatusItemState::Checking);

        // Create and store update cancel token
        let cancel_token = CancellationToken::new();
        let alpm_token = cancel_token.clone();
        let aur_token = cancel_token.clone();

        imp.update_cancel_token.replace(Some(cancel_token));

        // Check for pacman updates
        let mut update_output = String::new();
        let mut error_msg: Option<String> = None;

        let alpm_task = TokioUtils::run("/usr/bin/checkupdates", &[""], Some(alpm_token));

        let (alpm_result, aur_result) = if let Ok(paru_path) = Paths::paru().as_ref() {
            // Check for AUR updates
            let aur_task = TokioUtils::run(paru_path, &["-Qu", "--mode=ap"], Some(aur_token));

            join!(alpm_task, aur_task)
        } else {
            (alpm_task.await, Ok((None, String::new())))
        };

        // Remove stored update cancel token
        imp.update_cancel_token.replace(None);

        // Get pacman update results
        match alpm_result {
            Ok((Some(0), stdout)) => {
                update_output.push_str(&stdout);
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
        match aur_result {
            Ok((Some(0), stdout)) => {
                update_output.push_str(&stdout);
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

        let update_map: HashMap<String, String> = update_output.lines()
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

        update_item.set_state(StatusItemState::Updates(update_map.len(), error_msg));

        // If update item is selected, refresh package status filter
        if imp.status_sidebar.selected() == update_item.index() {
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
