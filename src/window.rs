use std::cell::{Cell, RefCell, OnceCell};
use std::path::Path;
use std::rc::Rc;
use std::collections::HashMap;
use std::time::Duration;
use std::env;

use gtk::{gio, glib};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, closure_local};

use alpm_utils::DbListExt;
use titlecase::titlecase;
use regex::Regex;
use lazy_static::lazy_static;
use notify_debouncer_full::{notify::*, new_debouncer, Debouncer, DebounceEventResult, FileIdMap};

use crate::APP_ID;
use crate::PacViewApplication;
use crate::pkg_object::{PkgObject, PkgData, PkgFlags};
use crate::search_header::{SearchHeader, SearchMode, SearchProp};
use crate::package_view::PackageView;
use crate::info_pane::{InfoPane, PropID};
use crate::filter_row::FilterRow;
use crate::stats_window::StatsWindow;
use crate::preferences_window::PreferencesWindow;
use crate::details_window::DetailsWindow;
use crate::utils::Utils;

//------------------------------------------------------------------------------
// MODULE: PacViewWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
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

        #[template_child]
        pub prefs_window: TemplateChild<PreferencesWindow>,

        pub gsettings: OnceCell<gio::Settings>,

        pub config_dir: OnceCell<Option<String>>,

        pub pacman_config: RefCell<pacmanconf::Config>,
        pub pacman_repos: RefCell<Vec<String>>,

        pub saved_repo_id: RefCell<Option<String>>,
        pub saved_status_id: Cell<PkgFlags>,

        pub update_row: RefCell<FilterRow>,

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
            PkgObject::ensure_type();
            SearchHeader::ensure_type();
            PackageView::ensure_type();
            InfoPane::ensure_type();

            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PacViewWindow {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.init_gsettings();
            obj.load_gsettings();

            obj.init_config_dir();

            obj.setup_widgets();
            obj.setup_actions();
            obj.setup_shortcuts();
            obj.setup_signals();

            obj.setup_alpm();

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

        if let Some(gsettings) = imp.gsettings.get() {

            // Bind gsettings
            gsettings.bind("window-width", self, "default-width").build();
            gsettings.bind("window-height", self, "default-height").build();
            gsettings.bind("window-maximized", self, "maximized").build();

            gsettings.bind("search-include-aur", &imp.search_header.get(), "include-aur").build();

            gsettings.bind("show-infopane", &imp.info_pane.get(), "visible").build();
            gsettings.bind("infopane-position", &imp.pane.get(), "position").build();

            gsettings.bind("auto-refresh", &imp.prefs_window.get(), "auto-refresh").build();
            gsettings.bind("aur-update-command", &imp.prefs_window.get(), "aur-command").build();
            gsettings.bind("search-delay", &imp.prefs_window.get(), "search-delay").build();
            gsettings.bind("remember-columns", &imp.prefs_window.get(), "remember-columns").build();
            gsettings.bind("remember-sorting", &imp.prefs_window.get(), "remember-sort").build();
            gsettings.bind("custom-font", &imp.prefs_window.get(), "custom-font").build();
            gsettings.bind("monospace-font", &imp.prefs_window.get(), "monospace-font").build();

            // Get default value for monospace font
            let default_font = gsettings.default_value("monospace-font").unwrap().to_string().replace("'", "");

            imp.prefs_window.set_default_monospace_font(default_font);

            // Restore package view columns if setting active
            if imp.prefs_window.remember_columns() {
                imp.package_view.set_columns(&gsettings.strv("view-columns"));
            }

            // Restore package view sort column/sort order
            imp.package_view.set_sorting(&gsettings.string("sort-column"), gsettings.boolean("sort-ascending"));
        }
    }

    //-----------------------------------
    // Save gsettings
    //-----------------------------------
    fn save_gsettings(&self) {
        let imp = self.imp();

        if let Some(gsettings) = imp.gsettings.get() {
            // Save package view column order if setting active
            if imp.prefs_window.remember_columns() {
                gsettings.set_strv("view-columns", imp.package_view.columns()).unwrap();
            } else {
                gsettings.reset("view-columns");
            }

            // Save package view sort column/order if setting active
            if imp.prefs_window.remember_sort() {
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
    }

    //-----------------------------------
    // Init config dir
    //-----------------------------------
    fn init_config_dir(&self) {
        let config_dir = env::var("XDG_DATA_HOME")
            .or_else(|_| env::var("HOME")
                .and_then(|var| Ok(Path::new(&var).join(".local/share").display().to_string()))
            )
            .and_then(|var| Ok(Path::new(&var).join("pacview").display().to_string()))
            .ok();

        self.imp().config_dir.set(config_dir).unwrap();
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind search header delay preference
        imp.prefs_window.bind_property("search-delay", &imp.search_header.get(), "delay")
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

        // Bind package view model to info pane package model
        imp.package_view.imp().filter_model.bind_property("model", &imp.info_pane.get(), "pkg-model")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Set preferences window parent
        imp.prefs_window.set_transient_for(Some(self));

        // Set initial focus on package view
        imp.package_view.imp().view.grab_focus();
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        let imp = self.imp();

        // Add start/stop search actions
        let start_action = gio::ActionEntry::<PacViewWindow>::builder("start-search")
            .activate(|window, _, _| {
                window.imp().search_header.set_enabled(true);
            })
            .build();

        let stop_action = gio::ActionEntry::<PacViewWindow>::builder("stop-search")
            .activate(|window, _, _| {
                window.imp().search_header.set_enabled(false);
            })
            .build();

        // Add search actions to window
        self.add_action_entries([start_action, stop_action]);

        // Add infopane visibility property action
        let show_infopane_action = gio::PropertyAction::new("show-infopane", &imp.info_pane.get(), "visible");

        self.add_action(&show_infopane_action);

        // Add package view refresh action
        let refresh_action = gio::ActionEntry::<PacViewWindow>::builder("refresh")
            .activate(|window, _, _| {
                let imp = window.imp();

                let repo_id = imp.repo_listbox.selected_row()
                    .and_downcast::<FilterRow>()
                    .and_then(|row| row.repo_id());

                imp.saved_repo_id.replace(repo_id);

                let status_id = imp.status_listbox.selected_row()
                    .and_downcast::<FilterRow>()
                    .and_then(|row| Some(row.status_id()))
                    .unwrap_or(PkgFlags::empty());

                imp.saved_status_id.set(status_id);

                window.setup_alpm();
            })
            .build();

        // Add package view show stats action
        let stats_action = gio::ActionEntry::<PacViewWindow>::builder("show-stats")
            .activate(|window, _, _| {
                let imp = window.imp();

                let stats_window = StatsWindow::new(
                    window.upcast_ref(),
                    &imp.pacman_repos.borrow(),
                    &imp.package_view.imp().pkg_model
                );

                stats_window.present();
            })
            .build();

        // Add package view copy list action
        let copy_action = gio::ActionEntry::<PacViewWindow>::builder("copy-list")
            .activate(|window, _, _| {
                let imp = window.imp();

                let copy_text = imp.package_view.imp().selection.iter::<glib::Object>()
                    .flatten()
                    .map(|item| {
                        let pkg = item
                            .downcast::<PkgObject>()
                            .expect("Must be a 'PkgObject'");

                        format!("{repo}/{name}-{version}", repo=pkg.repository(), name=pkg.name(), version=pkg.version())
                    })
                    .collect::<Vec<String>>()
                    .join("\n");

                window.clipboard().set_text(&copy_text);
            })
            .build();

        // Add package view actions to window
        self.add_action_entries([refresh_action, stats_action, copy_action]);

        // Bind package view item count to copy list action enabled state
        if let Some(copy_action) = self.lookup_action("copy-list") {
            imp.package_view.imp().selection.bind_property("n-items", &copy_action, "enabled")
                .transform_to(|_, n_items: u32| {
                    Some(n_items > 0)
                })
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
        }

        // Add info pane prev/next actions
        let prev_action = gio::ActionEntry::<PacViewWindow>::builder("previous")
            .activate(|window, _, _| {
                window.imp().info_pane.display_prev();
            })
            .build();
        
        let next_action = gio::ActionEntry::<PacViewWindow>::builder("next")
            .activate(|window, _, _| {
                window.imp().info_pane.display_next();
            })
            .build();

        // Add info pane show details action
        let details_action = gio::ActionEntry::<PacViewWindow>::builder("show-details")
            .activate(|window, _, _| {
                let imp = window.imp();

                if let Some(pkg) = imp.info_pane.pkg() {
                    let details_window = DetailsWindow::new(
                        window.upcast_ref(),
                        &pkg,
                        imp.prefs_window.custom_font(),
                        &imp.prefs_window.monospace_font(),
                        &imp.pacman_config.borrow(),
                        &imp.package_view.imp().pkg_model
                    );

                    details_window.present();
                }
            })
            .build();

        // Add info pane actions to window
        self.add_action_entries([prev_action, next_action, details_action]);

        // Add show preferences action
        let prefs_action = gio::ActionEntry::<PacViewWindow>::builder("show-preferences")
            .activate(|window, _, _| {
                window.imp().prefs_window.present();
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

        // Add view show stats shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<alt>S"),
            Some(gtk::NamedAction::new("win.show-stats"))
        ));

        // Add view copy list shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<alt>L"),
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
        imp.search_header.connect_closure("enabled", false, closure_local!(@watch self as obj => move |_: SearchHeader, enabled: bool| {
            if enabled == false {
                obj.imp().package_view.imp().view.grab_focus();
            }
        }));

        // Search header changed signal
        imp.search_header.connect_closure("changed", false, closure_local!(@watch self as obj => move |search_header: SearchHeader, search_term: &str, mode: SearchMode, prop: SearchProp, include_aur: bool, aur_error: bool| {
            obj.imp().package_view.set_search_filter(search_header, search_term, mode, prop, include_aur, aur_error);
        }));

        // Repo listbox row activated signal
        imp.repo_listbox.connect_row_activated(clone!(@weak imp => move |_, row| {
            let repo_id = row
                .downcast_ref::<FilterRow>()
                .expect("Must be a 'FilterRow'")
                .repo_id();

            imp.package_view.set_repo_filter(repo_id.as_deref());

            imp.package_view.imp().view.grab_focus();
        }));

        // Status listbox row activated signal
        imp.status_listbox.connect_row_activated(clone!(@weak imp => move |_, row| {
            let status_id = row
                .downcast_ref::<FilterRow>()
                .expect("Must be a 'FilterRow'")
                .status_id();

            imp.package_view.set_status_filter(status_id);

            imp.package_view.imp().view.grab_focus();
        }));

        // Package view selected signal
        imp.package_view.connect_closure("selected", false, closure_local!(@watch self as obj => move |_: PackageView, pkg: Option<PkgObject>| {
            obj.imp().info_pane.set_pkg(pkg.as_ref());
        }));
    }

    //-----------------------------------
    // Setup alpm
    //-----------------------------------
    fn setup_alpm(&self) {
        self.get_pacman_config();
        self.populate_sidebar();
        self.load_packages_async();
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
        let row = FilterRow::new("repository-symbolic", "All", None, PkgFlags::empty());

        imp.repo_listbox.append(&row);

        let saved_repo_id = imp.saved_repo_id.take();

        if saved_repo_id.is_none() {
            row.activate();
        }

        imp.pacman_repos.borrow().iter()
            .for_each(|repo| {
                let display_label = if repo == "aur" { repo.to_uppercase() } else { titlecase(repo) };

                let row = FilterRow::new("repository-symbolic", &display_label, Some(repo), PkgFlags::empty());

                imp.repo_listbox.append(&row);

                if saved_repo_id.as_ref() == Some(repo) {
                    row.activate();
                }
            });

        // Add package status rows (enumerate PkgStatusFlags)
        let saved_status_id = imp.saved_status_id.get();

        let flags = glib::FlagsClass::new::<PkgFlags>();

        for f in flags.values() {
            let flag = PkgFlags::from_bits_truncate(f.value());

            let row = FilterRow::new(&format!("status-{}-symbolic", f.nick()), f.name(), None, flag);

            imp.status_listbox.append(&row);

            if saved_status_id == PkgFlags::empty() {
                if flag == PkgFlags::INSTALLED {
                    row.activate();
                }
            } else {
                if flag == saved_status_id {
                    row.activate();
                }
            }

            if flag == PkgFlags::UPDATES {
                row.set_spinning(true);
                row.set_sensitive(true);

                imp.update_row.replace(row);
            }
        }

        imp.saved_status_id.set(PkgFlags::empty());
    }

    //-----------------------------------
    // Setup alpm: load alpm packages
    //-----------------------------------
    fn load_packages_async(&self) {
        let imp = self.imp();

        let config_dir = imp.config_dir.get().unwrap().clone();

        let pacman_config = imp.pacman_config.borrow().clone();

        // Spawn thread to load packages
        let (sender, receiver) = async_channel::bounded(1);

        gio::spawn_blocking(move || {
            let mut aur_names: Vec<String> = vec![];

            if let Some(config_dir) = config_dir {
                let aur_file = gio::File::for_path(Path::new(&config_dir).join("aur_packages"));

                // If AUR package list file does not exist, download it
                if aur_file.query_exists(None::<&gio::Cancellable>) == false {
                    let res = gio::File::for_path(config_dir)
                        .make_directory_with_parents(None::<&gio::Cancellable>);

                    if res.is_ok() || res.unwrap_err().matches(gio::IOErrorEnum::Exists) {
                        Utils::download_unpack_gz_file(&aur_file, "https://aur.archlinux.org/packages.gz");
                    }
                }

                // Load packages from AUR package list file
                if let Ok((bytes, _)) = aur_file.load_contents(None::<&gio::Cancellable>) {
                    if let Ok(s) = String::from_utf8(bytes) {
                        aur_names = s.lines().into_iter()
                            .map(|line| line.to_string())
                            .collect::<Vec<String>>();
                    }
                };
            }

            // Load pacman database packages
            let handle = alpm_utils::alpm_with_conf(&pacman_config).unwrap();

            let localdb = handle.localdb();

            let mut data_list: Vec<PkgData> = vec![];

            handle.syncdbs().iter()
                .for_each(|db| {
                    data_list.extend(db.pkgs().iter()
                        .map(|syncpkg| {
                            let localpkg = localdb.pkg(syncpkg.name());

                            PkgData::from_pkg(syncpkg, localpkg)
                        })
                    );
                });

            data_list.extend(localdb.pkgs().iter()
                .filter_map(|pkg| {
                    handle.syncdbs().pkg(pkg.name()).map_or_else(
                        |_| {
                            let mut data = PkgData::from_pkg(pkg, Ok(pkg));

                            if aur_names.contains(&data.name) {
                                data.repository = "aur".to_string();
                            }

                            Some(data)
                        },
                        |_| None
                    )
                })
            );

            sender.send_blocking((handle, data_list)).expect("Could not send through channel");
        });

        // Attach thread receiver
        glib::spawn_future_local(clone!(@weak self as obj, @weak imp => async move {
            while let Ok((handle, data_list)) = receiver.recv().await {
                let handle_ref = Rc::new(handle);

                let pkg_list: Vec<PkgObject> = data_list.into_iter()
                    .map(|data| PkgObject::new(Some(handle_ref.clone()), data))
                    .collect();

                imp.package_view.imp().pkg_model.splice(0, imp.package_view.imp().pkg_model.n_items(), &pkg_list);

                imp.package_view.imp().stack.set_visible_child_name("view");

                obj.get_package_updates_async();
                obj.update_aur_file_async();
            }
        }));
    }

    //-----------------------------------
    // Setup alpm: get package updates
    //-----------------------------------
    fn get_package_updates_async(&self) {
        let imp = self.imp();

        // Get custom command for AUR updates
        let aur_command = imp.prefs_window.aur_command();

        // Spawn thread to check for updates
        let (sender, receiver) = async_channel::bounded(1);

        gio::spawn_blocking(move || {
            let mut update_str = String::from("");

            let mut error_msg: Option<&str> = None;

            // Check for pacman updates
            let (code, stdout) = Utils::run_command("/usr/bin/checkupdates");

            if code == Some(0) {
                update_str += &stdout;
            }

            if code != Some(0) && code != Some(2) {
                error_msg = Some("Error Retrieving Updates");
            }

            // Check for AUR updates
            let (code, stdout) = Utils::run_command(&aur_command);

            if code == Some(0) {
                update_str += &stdout;
            }

            // Build update map (package name, version)
            lazy_static! {
                static ref EXPR: Regex = Regex::new("([a-zA-Z0-9@._+-]+?)[ \\t]+?([a-zA-Z0-9@._+-:]+?)[ \\t]+?->[ \\t]+?([a-zA-Z0-9@._+-:]+)").unwrap();
            }

            let update_map: HashMap<String, String> = update_str.lines()
                .filter_map(|s|
                    EXPR.captures(s)
                        .filter(|caps| caps.len() == 4)
                        .map(|caps| {
                            let pkg_name = caps[1].to_string();
                            let version = format!("{} \u{2192} {}", caps[2].to_string(), caps[3].to_string());

                            (pkg_name, version)
                        })
                )
                .collect();

            // Return thread result
            sender.send_blocking((error_msg, update_map)).expect("Could not send through channel");
        });

        // Attach thread receiver
        glib::spawn_future_local(clone!(@weak imp => async move {
            while let Ok((error_msg, update_map)) = receiver.recv().await {
                // If updates found
                if update_map.len() > 0 {
                    // Update status of packages with updates
                    imp.package_view.imp().pkg_model.iter::<PkgObject>()
                        .flatten()
                        .filter(|pkg| update_map.contains_key(&pkg.name()))
                        .for_each(|pkg| {
                            pkg.set_version(update_map[&pkg.name()].to_string());

                            pkg.set_flags(pkg.flags() | PkgFlags::UPDATES);

                            pkg.set_has_update(true);

                            // Update info pane if currently displayed package has update
                            let info_pkg = imp.info_pane.pkg();

                            if info_pkg.is_some() && info_pkg.unwrap() == pkg {
                                imp.info_pane.set_property_value(PropID::Version, true, &pkg.version(), Some("pkg-update"));
                            }
                        });
                }

                // Show update status/count in sidebar
                let update_row = imp.update_row.borrow();

                update_row.set_spinning(false);
                update_row.set_icon(if error_msg.is_some() {"status-updates-error-symbolic"} else {"status-updates-symbolic"});
                update_row.set_count(update_map.len() as u32);
                update_row.set_tooltip_text(error_msg);

                // If update row is selected, refresh package status filter
                if update_row.is_selected() {
                    imp.package_view.set_status_filter(update_row.status_id());
                }
            }
        }));
    }

    //-----------------------------------
    // Setup alpm: update AUR file
    //-----------------------------------
    fn update_aur_file_async(&self) {
        let imp = self.imp();

        let config_dir = imp.config_dir.get().unwrap().clone();

        // Spawn thread to load AUR package list file
        gio::spawn_blocking(move || {
            if let Some(config_dir) = config_dir {
                let aur_file = gio::File::for_path(Path::new(&config_dir).join("aur_packages"));

                // Get AUR package list file age
                let file_days = aur_file.query_info("time::modified", gio::FileQueryInfoFlags::NONE, None::<&gio::Cancellable>)
                    .ok()
                    .and_then(|file_info| file_info.modification_date_time())
                    .and_then(|file_time| {
                        glib::DateTime::now_local().ok()
                            .and_then(|current_time| Some(current_time.difference(&file_time).as_days()))
                    });

                // Download AUR package list file if does not exist or older than 7 days
                if file_days.is_none() || file_days.unwrap() >= 7 {
                    Utils::download_unpack_gz_file(&aur_file, "https://aur.archlinux.org/packages.gz");
                }
            }
        });
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
        glib::spawn_future_local(clone!(@weak self as obj, @weak imp => async move {
            while let Ok(()) = receiver.recv().await {
                if imp.prefs_window.auto_refresh() == true {
                    if let Some(refresh_action) = obj.lookup_action("refresh") {
                        refresh_action.activate(None);
                    }
                }
            }
        }));
    }
}
