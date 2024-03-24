use std::cell::{Cell, RefCell, OnceCell};
use std::path::Path;
use std::rc::Rc;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use std::env;
use std::io::prelude::*;

use gtk::{gio, glib};
use adw::subclass::prelude::*;
use adw::prelude::AdwDialogExt;
use gtk::prelude::*;
use glib::{clone, closure_local};

use alpm_utils::DbListExt;
use titlecase::titlecase;
use fancy_regex::Regex;
use lazy_static::lazy_static;
use flate2::read::GzDecoder;
use notify_debouncer_full::{notify::*, new_debouncer, Debouncer, DebounceEventResult, FileIdMap};

use crate::APP_ID;
use crate::PacViewApplication;
use crate::pkg_object::{PkgObject, PkgData, PkgFlags};
use crate::search_header::{SearchHeader, SearchMode, SearchProp};
use crate::package_view::PackageView;
use crate::info_pane::{InfoPane, PropID};
use crate::filter_row::FilterRow;
use crate::stats_window::StatsWindow;
use crate::backup_window::BackupWindow;
use crate::preferences_dialog::PreferencesDialog;
use crate::details_window::DetailsWindow;
use crate::utils::Utils;

//------------------------------------------------------------------------------
// ENUM: UpdateResult
//------------------------------------------------------------------------------
enum UpdateResult {
    Map(HashMap<String, String>),
    Error(String)
}

//------------------------------------------------------------------------------
// MODULE: PacViewWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PacViewWindow)]
    #[template(resource = "/com/github/PacView/ui/window.ui")]
    pub struct PacViewWindow {
        #[template_child]
        pub search_header: TemplateChild<SearchHeader>,
        #[template_child]
        pub search_button: TemplateChild<gtk::ToggleButton>,

        #[template_child]
        pub repo_listbox: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub status_listbox: TemplateChild<gtk::ListBox>,

        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub pane: TemplateChild<gtk::Paned>,

        #[template_child]
        pub package_view: TemplateChild<PackageView>,
        #[template_child]
        pub info_pane: TemplateChild<InfoPane>,

        #[property(get, set)]
        auto_refresh: Cell<bool>,
        #[property(get, set)]
        aur_command: RefCell<String>,
        #[property(get, set)]
        search_delay: Cell<f64>,
        #[property(get, set)]
        remember_columns: Cell<bool>,
        #[property(get, set)]
        remember_sort: Cell<bool>,

        pub gsettings: OnceCell<gio::Settings>,

        pub cache_dir: RefCell<Option<String>>,

        pub pacman_config: RefCell<pacmanconf::Config>,
        pub pacman_repos: RefCell<Vec<String>>,

        pub saved_repo_id: RefCell<Option<String>>,
        pub saved_status_id: Cell<PkgFlags>,

        pub all_repo_row: RefCell<FilterRow>,
        pub all_status_row: RefCell<FilterRow>,
        pub update_row: RefCell<FilterRow>,

        pub pkg_snapshot: RefCell<Vec<PkgObject>>,

        pub notify_watcher: OnceCell<Debouncer<INotifyWatcher, FileIdMap>>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
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

    #[glib::derived_properties]
    impl ObjectImpl for PacViewWindow {
        //-----------------------------------
        // Constructor
        //-----------------------------------
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
        //-----------------------------------
        // Window close handler
        //-----------------------------------
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
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(app: &PacViewApplication) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    //-----------------------------------
    // Init gsettings
    //-----------------------------------
    fn init_gsettings(&self) {
        let gsettings = gio::Settings::new(APP_ID);

        gsettings.delay();

        self.imp().gsettings.set(gsettings).unwrap();
    }

    //-----------------------------------
    // Load gsettings
    //-----------------------------------
    fn load_gsettings(&self) {
        let imp = self.imp();

        let gsettings = imp.gsettings.get().unwrap();

        // Bind gsettings
        gsettings.bind("window-width", self, "default-width").build();
        gsettings.bind("window-height", self, "default-height").build();
        gsettings.bind("window-maximized", self, "maximized").build();

        gsettings.bind("show-infopane", &imp.info_pane.get(), "visible").build();
        gsettings.bind("infopane-position", &imp.pane.get(), "position").build();

        gsettings.bind("auto-refresh", self, "auto-refresh").build();
        gsettings.bind("aur-update-command", self, "aur-command").build();
        gsettings.bind("search-delay", self, "search-delay").build();
        gsettings.bind("remember-columns", self, "remember-columns").build();
        gsettings.bind("remember-sorting", self, "remember-sort").build();

        // Restore package view columns if setting active
        if self.remember_columns() {
            imp.package_view.set_columns(&gsettings.strv("view-columns"));
        }

        // Restore package view sort column/sort order
        imp.package_view.set_sorting(&gsettings.string("sort-column"), gsettings.boolean("sort-ascending"));
    }

    //-----------------------------------
    // Save gsettings
    //-----------------------------------
    fn save_gsettings(&self) {
        let imp = self.imp();

        let gsettings = imp.gsettings.get().unwrap();

        // Save package view column order if setting active
        if self.remember_columns() {
            gsettings.set_strv("view-columns", imp.package_view.columns()).unwrap();
        } else {
            gsettings.reset("view-columns");
        }

        // Save package view sort column/order if setting active
        if self.remember_sort() {
            let (sort_col, sort_asc) = imp.package_view.sorting();

            gsettings.set_string("sort-column", &sort_col).unwrap();
            gsettings.set_boolean("sort-ascending", sort_asc).unwrap();
        } else {
            gsettings.reset("sort-column");
            gsettings.reset("sort-ascending");
        }

        // Save gsettings
        gsettings.apply();
    }

    //-----------------------------------
    // Init cache dir
    //-----------------------------------
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

        // Store cache dir
        self.imp().cache_dir.replace(cache_dir);
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind search header delay preference
        self.bind_property("search-delay", &imp.search_header.get(), "delay")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Set search header key capture widget
        imp.search_header.set_key_capture_widget(imp.package_view.imp().view.get().upcast());

        // Bind search button state to search header enabled state
        imp.search_button.bind_property("active", &imp.search_header.get(), "enabled")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();

        // Bind package view item count to status label text
        imp.package_view.imp().selection.bind_property("n-items", &imp.status_label.get(), "label")
            .transform_to(|_, n_items: u32| {
                Some(format!("{n_items} matching package{}", if n_items != 1 {"s"} else {""}))
            })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Set initial focus on package view
        imp.package_view.imp().view.grab_focus();
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        let imp = self.imp();

        // Add start/stop search actions
        let start_action = gio::ActionEntry::builder("start-search")
            .activate(|window: &Self, _, _| {
                window.imp().search_header.set_enabled(true);
            })
            .build();

        let stop_action = gio::ActionEntry::builder("stop-search")
            .activate(|window: &Self, _, _| {
                window.imp().search_header.set_enabled(false);
            })
            .build();

        // Add search actions to window
        self.add_action_entries([start_action, stop_action]);

        // Add infopane visibility property action
        let show_infopane_action = gio::PropertyAction::new("show-infopane", &imp.info_pane.get(), "visible");

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
                    .unwrap_or(PkgFlags::empty());

                imp.saved_status_id.set(status_id);

                window.setup_alpm(false);
            })
            .build();

        // Add show all packages action
        let all_pkgs_action = gio::ActionEntry::builder("show-all-packages")
            .activate(|window: &Self, _, _| {
                let imp = window.imp();

                imp.all_repo_row.borrow().activate();
                imp.all_status_row.borrow().activate();
            })
            .build();

        // Add package view show stats action
        let stats_action = gio::ActionEntry::builder("show-stats")
            .activate(|window: &Self, _, _| {
                let imp = window.imp();

                let stats_window = StatsWindow::new(
                    window,
                    &imp.pacman_repos.borrow(),
                    &imp.pkg_snapshot.borrow()
                );

                stats_window.present();
            })
            .build();

        // Add package view show backup files action
        let backup_action = gio::ActionEntry::builder("show-backup-files")
            .activate(|window: &Self, _, _| {
                let imp = window.imp();

                let stats_window = BackupWindow::new(
                    window,
                    &imp.pkg_snapshot.borrow()
                );

                stats_window.present();
            })
            .build();

        // Add package view copy list action
        let copy_action = gio::ActionEntry::builder("copy-list")
            .activate(|window: &Self, _, _| {
                let imp = window.imp();

                let copy_text = imp.package_view.imp().selection.iter::<glib::Object>()
                    .flatten()
                    .map(|item| {
                        let pkg = item
                            .downcast::<PkgObject>()
                            .expect("Could not downcast to 'PkgObject'");

                        format!("{repo}/{name}-{version}", repo=pkg.repository(), name=pkg.name(), version=pkg.version())
                    })
                    .collect::<Vec<String>>()
                    .join("\n");

                window.clipboard().set_text(&copy_text);
            })
            .build();

        // Add package view actions to window
        self.add_action_entries([refresh_action, all_pkgs_action, stats_action, backup_action, copy_action]);

        // Bind package view item count to copy list action enabled state
        let copy_action = self.lookup_action("copy-list").unwrap();

        imp.package_view.imp().selection.bind_property("n-items", &copy_action, "enabled")
            .transform_to(|_, n_items: u32| {
                Some(n_items > 0)
            })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Add info pane prev/next actions
        let prev_action = gio::ActionEntry::builder("previous")
            .activate(|window: &Self, _, _| {
                window.imp().info_pane.display_prev();
            })
            .build();

        let next_action = gio::ActionEntry::builder("next")
            .activate(|window: &Self, _, _| {
                window.imp().info_pane.display_next();
            })
            .build();

        // Add info pane show details action
        let details_action = gio::ActionEntry::builder("show-details")
            .activate(|window: &Self, _, _| {
                let imp = window.imp();

                if let Some(pkg) = imp.info_pane.pkg() {
                    let details_window = DetailsWindow::new(
                        window,
                        &pkg,
                        &imp.pacman_config.borrow(),
                        &imp.pkg_snapshot.borrow()
                    );

                    details_window.present();
                }
            })
            .build();

        // Add info pane actions to window
        self.add_action_entries([prev_action, next_action, details_action]);

        // Add show preferences action
        let prefs_action = gio::ActionEntry::builder("show-preferences")
            .activate(|window: &Self, _, _| {
                let prefs_dialog = PreferencesDialog::new();

                prefs_dialog.prepare(
                    window.auto_refresh(),
                    &window.aur_command(),
                    window.search_delay(),
                    window.remember_columns(),
                    window.remember_sort()
                );

                prefs_dialog.connect_closed(clone!(@weak window => move |prefs_dialog| {
                    window.set_auto_refresh(prefs_dialog.auto_refresh());
                    window.set_aur_command(prefs_dialog.aur_command());
                    window.set_search_delay(prefs_dialog.search_delay());
                    window.set_remember_columns(prefs_dialog.remember_columns());
                    window.set_remember_sort(prefs_dialog.remember_sort());
                }));

                prefs_dialog.present(window);
            })
            .build();

        // Add preference actions to window
        self.add_action_entries([prefs_action]);
    }

    //-----------------------------------
    // Setup shortcuts
    //-----------------------------------
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

        // Add view show all packages shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<alt>A"),
            Some(gtk::NamedAction::new("win.show-all-packages"))
        ));

        // Add view show stats shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<alt>S"),
            Some(gtk::NamedAction::new("win.show-stats"))
        ));

        // Add view show backup files shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<alt>B"),
            Some(gtk::NamedAction::new("win.show-backup-files"))
        ));

        // Add view copy list shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<alt>C"),
            Some(gtk::NamedAction::new("win.copy-list"))
        ));

        // Add infopane previous shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<alt>Left"),
            Some(gtk::NamedAction::new("win.previous"))
        ));

        // Add infopane next shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<alt>Right"),
            Some(gtk::NamedAction::new("win.next"))
        ));

        // Add infopane show details shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<alt>Return|<alt>KP_Enter"),
            Some(gtk::NamedAction::new("win.show-details"))
        ));

        // Add shortcut controller to window
        self.add_controller(controller);
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Search header enabled signal
        imp.search_header.connect_closure("enabled", false, closure_local!(@watch self as window => move |_: SearchHeader, enabled: bool| {
            if !enabled {
                window.imp().package_view.imp().view.grab_focus();
            }
        }));

        // Search header changed signal
        imp.search_header.connect_closure("changed", false, closure_local!(@watch self as window => move |_: SearchHeader, search_term: &str, mode: SearchMode, prop: SearchProp| {
            window.imp().package_view.set_search_filter(search_term, mode, prop);
        }));

        // Search header AUR Search signal
        imp.search_header.connect_closure("aur-search", false, closure_local!(@watch self as window => move |search_header: SearchHeader, search_term: &str, prop: SearchProp| {
            window.imp().package_view.search_in_aur(search_header, search_term, prop);
        }));

        // Repo listbox row activated signal
        imp.repo_listbox.connect_row_activated(clone!(@weak imp => move |_, row| {
            let repo_id = row
                .downcast_ref::<FilterRow>()
                .expect("Could not downcast to 'FilterRow'")
                .repo_id();

            imp.package_view.set_repo_filter(repo_id.as_deref());

            imp.package_view.imp().view.grab_focus();
        }));

        // Status listbox row activated signal
        imp.status_listbox.connect_row_activated(clone!(@weak imp => move |_, row| {
            let status_id = row
                .downcast_ref::<FilterRow>()
                .expect("Could not downcast to 'FilterRow'")
                .status_id();

            imp.package_view.set_status_filter(status_id);

            imp.package_view.imp().view.grab_focus();
        }));

        // Package view selected signal
        imp.package_view.connect_closure("selected", false, closure_local!(@watch self as window => move |_: PackageView, pkg: Option<PkgObject>| {
            window.imp().info_pane.set_pkg(pkg.as_ref());
        }));

        // Package view activate signal
        imp.package_view.connect_closure("activated", false, closure_local!(@watch self as window => move |_: PackageView, index: u32| {
            let imp = window.imp();

            if let Some(pkg) = imp.package_view.imp().selection.item(index)
                .and_downcast::<PkgObject>()
            {
                imp.info_pane.set_pkg(Some(&pkg));

                ActionGroupExt::activate_action(window, "show-details", None);
            }
        }));
    }

    //-----------------------------------
    // Setup alpm
    //-----------------------------------
    fn setup_alpm(&self, update_aur_file: bool) {
        self.get_pacman_config();
        self.populate_sidebar();
        self.load_packages_async(update_aur_file);
    }

    //-----------------------------------
    // Setup alpm: get pacman configuration
    //-----------------------------------
    fn get_pacman_config(&self) {
        // Get pacman config
        let pacman_config = pacmanconf::Config::new().unwrap();

        // Get pacman repositories
        let pacman_repos: Vec<String> = pacman_config.repos.iter()
            .map(|r| r.name.to_string())
            .chain([String::from("aur"), String::from("local")])
            .collect();

        // Store pacman config/repos
        self.imp().pacman_config.replace(pacman_config);
        self.imp().pacman_repos.replace(pacman_repos);
    }

    //-----------------------------------
    // Setup alpm: populate sidebar listboxes
    //-----------------------------------
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

            let row = FilterRow::new(&format!("status-{}-symbolic", f.nick()), f.name(), None, flag);

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

    //-----------------------------------
    // Download AUR names helper function
    //-----------------------------------
    pub fn download_aur_names(file: &gio::File) {
        let url = "https://aur.archlinux.org/packages.gz";

        if let Ok(bytes) = reqwest::blocking::get(url).and_then(|res| res.bytes()) {
            let mut decoder = GzDecoder::new(&bytes[..]);

            let mut gz_string = String::new();

            if decoder.read_to_string(&mut gz_string).is_ok() {
                file.replace_contents(gz_string.as_bytes(), None, false, gio::FileCreateFlags::REPLACE_DESTINATION, None::<&gio::Cancellable>).unwrap_or_default();
            }
        }
    }

    //-----------------------------------
    // Setup alpm: load alpm packages
    //-----------------------------------
    fn load_packages_async(&self, update_aur_file: bool) {
        let imp = self.imp();

        let cache_dir = imp.cache_dir.borrow();

        let pacman_config = imp.pacman_config.borrow();

        // Spawn thread to load packages
        let (sender, receiver) = async_channel::bounded(1);

        gio::spawn_blocking(clone!(@strong cache_dir, @strong pacman_config => move || {
            let mut aur_names: HashSet<String> = HashSet::new();

            // Get AUR package names from local file
            if let Some(cache_dir) = cache_dir {
                let aur_file = gio::File::for_path(Path::new(&cache_dir).join("aur_packages"));

                // If AUR package names file does not exist, download it
                if !aur_file.query_exists(None::<&gio::Cancellable>) {
                    Self::download_aur_names(&aur_file);
                }

                // Load AUR package names from file
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
                for db in handle.syncdbs() {
                    data_list.extend(db.pkgs().iter()
                        .map(|syncpkg| {
                            let localpkg = localdb.pkg(syncpkg.name());

                            PkgData::from_pkg(syncpkg, localpkg)
                        })
                    );
                }

                // Load local packages not in sync databases
                data_list.extend(localdb.pkgs().iter()
                    .filter(|pkg| handle.syncdbs().pkg(pkg.name()).is_err())
                    .map(|pkg| {
                        let mut data = PkgData::from_pkg(pkg, Ok(pkg));

                        if aur_names.contains(&data.name) {
                            data.repository = "aur".to_string();
                        }

                        data
                    })
                );
            }

            sender.send_blocking(data_list).expect("Could not send through channel");
        }));

        // Attach thread receiver
        glib::spawn_future_local(clone!(@weak self as window, @weak imp, @strong pacman_config => async move {
            while let Ok(data_list) = receiver.recv().await {
                if let Ok(handle) = alpm_utils::alpm_with_conf(&pacman_config) {
                    let handle_ref = Rc::new(handle);

                    let pkg_list: Vec<PkgObject> = data_list.into_iter()
                        .map(|data| PkgObject::new(Some(handle_ref.clone()), data))
                        .collect();

                    imp.package_view.imp().pkg_model.splice(0, imp.package_view.imp().pkg_model.n_items(), &pkg_list);

                    let local_pkg_names: HashSet<String> = pkg_list.iter()
                        .filter(|pkg| pkg.flags().intersects(PkgFlags::INSTALLED))
                        .map(|pkg| pkg.name())
                        .collect();

                    imp.package_view.imp().local_pkg_names.replace(local_pkg_names);

                    imp.info_pane.imp().pkg_snapshot.replace(pkg_list.clone());

                    imp.pkg_snapshot.replace(pkg_list);

                    imp.package_view.imp().stack.set_visible_child_name("view");

                    window.get_package_updates_async();

                    if update_aur_file {
                        window.update_aur_file_async();
                    }
                }
            }
        }));
    }

    //-----------------------------------
    // Pacman/AUR update helper functions
    //-----------------------------------
    fn get_pacman_updates(cache_dir: Option<String>, update_config: &mut pacmanconf::Config) -> UpdateResult {
        cache_dir
            .ok_or(alpm::Error::NotADir)
            .and_then(|cache_dir| {
                // Create link to local package database in cache dir
                let link_dest = Path::new(&update_config.db_path).join("local");

                gio::File::for_path(Path::new(&cache_dir).join("local"))
                    .make_symbolic_link(link_dest, None::<&gio::Cancellable>)
                    .or_else(|error| {
                        if error.matches(gio::IOErrorEnum::Exists) { Ok(()) } else { Err(alpm::Error::NotADir) }
                    })
                    .map(|_| cache_dir)
            })
            .and_then(|cache_dir| {
                // Change pacman config DB path to cache dir
                update_config.db_path = cache_dir;

                // Sync copy of remote databases in cache dir
                let mut handle = alpm_utils::alpm_with_conf(update_config)?;

                handle.syncdbs_mut().update(false)
                    .and_then(|_| {
                        handle.unlock()?;

                        handle.trans_init(alpm::TransFlag::NO_LOCK | alpm::TransFlag::DB_ONLY | alpm::TransFlag::NO_DEPS)?;

                        handle.sync_sysupgrade(false)?;

                        // Create map with pacman updates (name, version)
                        let pacman_map = handle.trans_add().iter()
                            .map(|pkg| (pkg.name().to_string(), pkg.version().to_string()))
                            .collect::<HashMap<String, String>>();

                        Ok(pacman_map)
                    })
                    .map_err(|error| {
                        handle.unlock().unwrap_or_default();

                        error
                    })

            })
            .map_or_else(
                |error| UpdateResult::Error(format!("Error Retrieving Pacman Updates ({})", error)),
                |pacman_map| UpdateResult::Map(pacman_map)
            )
    }

    fn get_aur_updates(aur_command: &str) -> UpdateResult {
        // Run AUR helper
        Utils::run_command(aur_command)
            .map(|stdout| {
                // Create map with AUR updates (name, version)
                lazy_static! {
                    static ref EXPR: Regex = Regex::new("([a-zA-Z0-9@._+-]+?)[ \\t]+?([a-zA-Z0-9@._+-:]+?)[ \\t]+?->[ \\t]+?([a-zA-Z0-9@._+-:]+)").unwrap();
                }

                let aur_map: HashMap<String, String> = stdout.lines()
                    .filter_map(|s|
                        EXPR.captures(s).ok().and_then(|caps| {
                            caps
                                .filter(|caps| caps.len() == 4)
                                .map(|caps| {
                                    (caps[1].to_string(), caps[3].to_string())
                                })
                        })
                    )
                    .collect();

                aur_map
            })
            .map_or_else(
                |error| UpdateResult::Error(format!("Error Retrieving AUR Updates ({})", error)),
                |aur_map| UpdateResult::Map(aur_map)
            )
    }

    //-----------------------------------
    // Setup alpm: get package updates
    //-----------------------------------
    fn get_package_updates_async(&self) {
        let imp = self.imp();

        let update_row = imp.update_row.borrow();
        update_row.set_spinning(true);

        let cache_dir = imp.cache_dir.borrow();

        // Need to clone pacman config to modify db path
        let mut update_config = imp.pacman_config.borrow().clone();

        // Get custom command for AUR updates
        let aur_command = self.aur_command();

        // Spawn threads to check for pacman/AUR updates
        let (sender, receiver) = async_channel::bounded(1);

        let sender1 = sender.clone();

        gio::spawn_blocking(clone!(@strong cache_dir => move || {
            sender1.send_blocking(Self::get_pacman_updates(cache_dir, &mut update_config))
                .expect("Could not send through channel");
        }));

        gio::spawn_blocking(move || {
            sender.send_blocking(Self::get_aur_updates(&aur_command))
                .expect("Could not send through channel");
        });

        // Attach thread receiver
        let mut update_map: HashMap<String, String> = HashMap::new();
        let mut error_msg: Option<String> = None;
        let mut n_threads = 0;

        glib::spawn_future_local(clone!(@weak imp, @strong update_row => async move {
            while let Ok(result) = receiver.recv().await {
                n_threads += 1;

                // Get update map and error message
                match result {
                    UpdateResult::Map(map) => update_map.extend(map),
                    UpdateResult::Error(error) => {
                        if error_msg.is_none() { error_msg = Some(error) }
                    }
                }

                // Update status of packages with updates
                if n_threads == 2 {
                    if !update_map.is_empty() {
                        imp.package_view.imp().pkg_model.iter::<PkgObject>()
                            .flatten()
                            .filter(|pkg| update_map.contains_key(&pkg.name()))
                            .for_each(|pkg| {
                                pkg.set_version(format!("{} \u{2192} {}", pkg.version(), update_map[&pkg.name()]));

                                pkg.set_flags(pkg.flags() | PkgFlags::UPDATES);

                                pkg.set_has_update(true);

                                // Update info pane if currently displayed package has update
                                let info_pkg = imp.info_pane.pkg();

                                if info_pkg.is_some_and(|info_pkg| info_pkg == pkg) {
                                    imp.info_pane.set_property_value(PropID::Version, true, &pkg.version(), Some("pkg-update"));
                                }
                            });
                    }

                    // Show update status/count in sidebar
                    update_row.set_spinning(false);
                    update_row.set_icon(if error_msg.is_some() {"status-updates-error-symbolic"} else {"status-updates-symbolic"});
                    update_row.set_count(update_map.len() as u32);
                    update_row.set_tooltip_text(error_msg.as_deref());

                    // If update row is selected, refresh package status filter
                    if update_row.is_selected() {
                        imp.package_view.set_status_filter(update_row.status_id());
                    }
                }
            }
        }));
    }

    //-----------------------------------
    // Setup alpm: update AUR file
    //-----------------------------------
    fn update_aur_file_async(&self) {
        let imp = self.imp();

        let cache_dir = imp.cache_dir.borrow();

        // Spawn thread to load AUR package names file
        gio::spawn_blocking(clone!(@strong cache_dir => move || {
            if let Some(cache_dir) = cache_dir {
                let aur_file = gio::File::for_path(Path::new(&cache_dir).join("aur_packages"));

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
                    Self::download_aur_names(&aur_file);
                }
            }
        }));
    }

    //-----------------------------------
    // Setup INotify
    //-----------------------------------
    fn setup_inotify(&self) {
        let imp = self.imp();

        // Create glib channel
        let (sender, receiver) = async_channel::bounded(1);

        // Create new watcher
        let mut watcher = new_debouncer(Duration::from_secs(1), None, move |result: DebounceEventResult| {
            if let Ok(events) = result {
                for event in events {
                    if event.kind.is_create() || event.kind.is_modify() || event.kind.is_remove() {
                        sender.send_blocking(()).expect("Could not send through channel");

                        break;
                    }
                }
            }
        }).unwrap();

        // Watch pacman local db path
        let pacman_config = imp.pacman_config.borrow();

        let watch_path = Path::new(&pacman_config.db_path).join("local");

        watcher.watcher().watch(&watch_path, RecursiveMode::Recursive).unwrap();
        watcher.cache().add_root(&watch_path, RecursiveMode::Recursive);

        // Store watcher
        imp.notify_watcher.set(watcher).unwrap();

        // Attach receiver for glib channel
        glib::spawn_future_local(clone!(@weak self as window, @weak imp => async move {
            while let Ok(()) = receiver.recv().await {
                if window.auto_refresh() {
                    ActionGroupExt::activate_action(&window, "refresh", None);
                }
            }
        }));
    }
}
