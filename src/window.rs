use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::thread;
use std::collections::HashMap;
use std::time::Duration;

use gtk::{gio, glib, gdk, graphene::Point};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, closure_local};
use glib::once_cell::sync::OnceCell;

use pacmanconf;
use alpm;
use titlecase::titlecase;
use regex::Regex;
use lazy_static::lazy_static;
use raur::blocking::Raur;
use notify_debouncer_full::{notify::*, new_debouncer, DebounceEventResult};

use crate::APP_ID;
use crate::PacViewApplication;
use crate::pkg_object::{PkgObject, PkgData, PkgFlags};
use crate::search_header::{SearchHeader, SearchMode, SearchFlags};
use crate::package_view::PackageView;
use crate::info_pane::{InfoPane, PropID};
use crate::filter_row::FilterRow;
use crate::stats_window::StatsWindow;
use crate::preferences_window::PreferencesWindow;
use crate::details_window::DetailsWindow;
use crate::utils::Utils;

//------------------------------------------------------------------------------
// STRUCT: PacmanConfig
//------------------------------------------------------------------------------
#[derive(Default, Clone)]
pub struct PacmanConfig {
    pub pacman_repos: Vec<String>,
    pub root_dir: String,
    pub db_path: String,
    pub log_file: String,
    pub cache_dirs: Vec<String>,
}

//------------------------------------------------------------------------------
// MODULE: PacViewWindow
//------------------------------------------------------------------------------
mod imp {
    use notify_debouncer_full::{Debouncer, FileIdMap};

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

        pub pacman_config: RefCell<PacmanConfig>,

        pub update_row: RefCell<FilterRow>,

        pub notify_watcher: OnceCell<Debouncer<INotifyWatcher, FileIdMap>>,

        pub package_view_popup: OnceCell<gtk::PopoverMenu>,
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

            obj.setup_widgets();
            obj.setup_actions();
            obj.setup_shortcuts();
            obj.setup_signals();

            obj.setup_alpm();

            obj.setup_inotify();
        }

        //-----------------------------------
        // Destructor
        //-----------------------------------
        fn dispose(&self) {
            self.package_view_popup.get().unwrap().unparent();
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

        // Bind search button state to search header active state
        imp.search_button.bind_property("active", &imp.search_header.get(), "active")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();

        // Create package view popover menu
        let builder = gtk::Builder::from_resource("/com/github/PacView/ui/package_view_menu.ui");
        let menu: gio::MenuModel = builder.object("popup_menu").unwrap();

        let popover_menu = gtk::PopoverMenu::from_model(Some(&menu));
        popover_menu.set_parent(self);
        popover_menu.set_has_arrow(false);
        popover_menu.set_halign(gtk::Align::Start);

        imp.package_view_popup.set(popover_menu).unwrap();

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
        let start_action = gio::ActionEntry::<PacViewWindow>::builder("toggle-search")
            .activate(clone!(@weak imp => move |_, _, _| {
                imp.search_header.set_active(!imp.search_header.active());
            }))
            .build();

        let stop_action = gio::ActionEntry::<PacViewWindow>::builder("stop-search")
            .activate(clone!(@weak imp => move |_, _, _| {
                imp.search_header.set_active(false);
            }))
            .build();

        // Add search actions to window
        self.add_action_entries([start_action, stop_action]);

        // Add infopane visibility property action
        let show_infopane_action = gio::PropertyAction::new("show-infopane", &imp.info_pane.get(), "visible");

        self.add_action(&show_infopane_action);

        // Add package view refresh action
        let refresh_action = gio::ActionEntry::<PacViewWindow>::builder("refresh")
            .activate(clone!(@weak self as obj, @weak imp => move |_, _, _| {
                imp.search_header.set_active(false);

                obj.setup_alpm();
            }))
            .build();

        // Add package view show stats action
        let stats_action = gio::ActionEntry::<PacViewWindow>::builder("show-stats")
            .activate(clone!(@weak self as obj, @weak imp => move |_, _, _| {
                let pacman_config = imp.pacman_config.borrow();
                
                let stats_window = StatsWindow::new(
                    &obj.upcast(),
                    &pacman_config.pacman_repos,
                    &imp.package_view.imp().model
                );

                stats_window.present();
            }))
            .build();

        // Add package view copy list action
        let copy_action = gio::ActionEntry::<PacViewWindow>::builder("copy-list")
            .activate(clone!(@weak self as obj, @weak imp => move |_, _, _| {
                let copy_text = imp.package_view.imp().selection.iter::<glib::Object>()
                    .flatten()
                    .map(|item| {
                        let pkg = item
                            .downcast::<PkgObject>()
                            .expect("Must be a 'PkgObject'");

                        format!("{repo}/{name}-{version}", repo=pkg.repo_show(), name=pkg.name(), version=pkg.version())
                    })
                    .collect::<Vec<String>>()
                    .join("\n");

                obj.clipboard().set_text(&copy_text);
            }))
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
            .activate(clone!(@weak imp => move |_, _, _| {
                imp.info_pane.display_prev();
            }))
            .build();
        
        let next_action = gio::ActionEntry::<PacViewWindow>::builder("next")
            .activate(clone!(@weak imp => move |_, _, _| {
                imp.info_pane.display_next();
            }))
            .build();

        // Add info pane show details action
        let details_action = gio::ActionEntry::<PacViewWindow>::builder("show-details")
            .activate(clone!(@weak self as obj, @weak imp => move |_, _, _| {
                if let Some(pkg) = imp.info_pane.pkg() {
                    let pacman_config = imp.pacman_config.borrow();

                    let details_window = DetailsWindow::new(
                        &obj.upcast(),
                        &pkg,
                        imp.prefs_window.custom_font(),
                        &imp.prefs_window.monospace_font(),
                        &pacman_config.log_file,
                        &pacman_config.cache_dirs,
                        &imp.package_view.imp().model
                    );

                    details_window.present();
                }
            }))
            .build();

        // Add info pane actions to window
        self.add_action_entries([prev_action, next_action, details_action]);

        // Add show preferences action
        let prefs_action = gio::ActionEntry::<PacViewWindow>::builder("show-preferences")
            .activate(clone!(@weak imp => move |_, _, _| {
                imp.prefs_window.present();
            }))
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
            Some(gtk::NamedAction::new("win.toggle-search"))
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

        // Search header activated signal
        imp.search_header.connect_closure("activated", false, closure_local!(@watch self as obj => move |_: SearchHeader, active: bool| {
            if active == false {
                obj.imp().package_view.imp().view.grab_focus();
            }
        }));

        // Search header changed signal
        imp.search_header.connect_closure("changed", false, closure_local!(@watch self as obj => move |_: SearchHeader, search_term: &str, flags: SearchFlags, mode: SearchMode| {
            let imp = obj.imp();

            if search_term == "" {
                imp.package_view.imp().search_filter.unset_filter_func();
            } else {
                if mode == SearchMode::Exact {
                    let term = search_term.to_string();

                    imp.package_view.imp().search_filter.set_filter_func(move |item| {
                        let pkg: &PkgObject = item
                            .downcast_ref::<PkgObject>()
                            .expect("Must be a 'PkgObject'");

                        let results = [
                            flags.contains(SearchFlags::NAME) &&
                                pkg.name().eq_ignore_ascii_case(&term),
                            flags.contains(SearchFlags::DESC) &&
                                pkg.description().eq_ignore_ascii_case(&term),
                            flags.contains(SearchFlags::GROUP) &&
                                pkg.groups().eq_ignore_ascii_case(&term),
                            flags.contains(SearchFlags::DEPS) &&
                                pkg.depends().iter().any(|s| s.eq_ignore_ascii_case(&term)),
                            flags.contains(SearchFlags::OPTDEPS) &&
                                pkg.optdepends().iter().any(|s| s.eq_ignore_ascii_case(&term)),
                            flags.contains(SearchFlags::PROVIDES) &&
                                pkg.provides().iter().any(|s| s.eq_ignore_ascii_case(&term)),
                            flags.contains(SearchFlags::FILES) &&
                                pkg.files().iter().any(|s| s.eq_ignore_ascii_case(&term)),
                        ];

                        results.iter().any(|&x| x)
                    });
                } else {
                    let term = search_term.to_ascii_lowercase();

                    imp.package_view.imp().search_filter.set_filter_func(move |item| {
                        let pkg: &PkgObject = item
                            .downcast_ref::<PkgObject>()
                            .expect("Must be a 'PkgObject'");

                        let mut results = vec![];

                        for t in term.split_whitespace() {
                            let t_results = [
                                flags.contains(SearchFlags::NAME) &&
                                    pkg.name().to_ascii_lowercase().contains(&t),
                                flags.contains(SearchFlags::DESC) &&
                                    pkg.description().to_ascii_lowercase().contains(&t),
                                flags.contains(SearchFlags::GROUP) &&
                                    pkg.groups().to_ascii_lowercase().contains(&t),
                                flags.contains(SearchFlags::DEPS) &&
                                    pkg.depends().iter().any(|s| s.to_ascii_lowercase().contains(&t)),
                                flags.contains(SearchFlags::OPTDEPS) &&
                                    pkg.optdepends().iter().any(|s| s.to_ascii_lowercase().contains(&t)),
                                flags.contains(SearchFlags::PROVIDES) &&
                                    pkg.provides().iter().any(|s| s.to_ascii_lowercase().contains(&t)),
                                flags.contains(SearchFlags::FILES) &&
                                    pkg.files().iter().any(|s| s.to_ascii_lowercase().contains(&t)),
                            ];

                            results.push(t_results.iter().any(|&x| x));
                        }

                        if mode == SearchMode::All {
                            results.iter().all(|&x| x)
                        } else {
                            results.iter().any(|&x| x)
                        }
                    });
                }
            }
        }));

        // Repo listbox row selected signal
        imp.repo_listbox.connect_row_selected(clone!(@weak imp => move |_, row| {
            if let Some(row) = row {
                let repo_id = row
                    .downcast_ref::<FilterRow>()
                    .expect("Must be a 'FilterRow'")
                    .repo_id();

                imp.package_view.imp().repo_filter.set_search(repo_id.as_deref());
            }
        }));

        // Status listbox row selected signal
        imp.status_listbox.connect_row_selected(clone!(@weak imp => move |_, row| {
            if let Some(row) = row {
                let status_id = row
                    .downcast_ref::<FilterRow>()
                    .expect("Must be a 'FilterRow'")
                    .status_id();

                imp.package_view.imp().status_filter.set_filter_func(move |item| {
                    let pkg: &PkgObject = item
                        .downcast_ref::<PkgObject>()
                        .expect("Must be a 'PkgObject'");

                    pkg.flags().intersects(status_id)
                });
            }
        }));

        // Package view selected signal
        imp.package_view.connect_closure("selected", false, closure_local!(@watch self as obj => move |_: PackageView, pkg: Option<PkgObject>| {
            obj.imp().info_pane.set_pkg(pkg.as_ref());
        }));

        // Package view pressed signal
        imp.package_view.connect_closure("pressed", false, closure_local!(@watch self as obj => move |_: PackageView, button: u32, x: f32, y: f32| {
            if button == gdk::BUTTON_SECONDARY {
                let imp = obj.imp();

                if let Some(point) = imp.package_view.compute_point(obj, &Point::new(x, y)) {
                    let rect = gdk::Rectangle::new(point.x() as i32, point.y() as i32, 0, 0);

                    let popover_menu = imp.package_view_popup.get().unwrap();

                    popover_menu.set_pointing_to(Some(&rect));
                    popover_menu.popup();
                }
            }
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
        let mut pacman_repos: Vec<String> = pacman_config.repos.iter()
            .map(|r| r.name.to_string())
            .collect();
        
        // Add 'local' to pacman repositories
        pacman_repos.push(String::from("local"));

        // Store pacman config
        self.imp().pacman_config.replace(PacmanConfig{
            pacman_repos,
            root_dir: pacman_config.root_dir,
            db_path: pacman_config.db_path,
            log_file: pacman_config.log_file,
            cache_dirs: pacman_config.cache_dir,
        });
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

        imp.repo_listbox.select_row(Some(&row));

        for repo in &imp.pacman_config.borrow().pacman_repos {
            let row = FilterRow::new("repository-symbolic", &titlecase(&repo), Some(&repo), PkgFlags::empty());

            imp.repo_listbox.append(&row);
        }

        // Add package status rows (enumerate PkgStatusFlags)
        let flags = glib::FlagsClass::new::<PkgFlags>();

        for f in flags.values() {
            let flag = PkgFlags::from_bits_truncate(f.value());

            let row = FilterRow::new(&format!("status-{}-symbolic", f.nick()), f.name(), None, flag);

            imp.status_listbox.append(&row);

            if flag == PkgFlags::INSTALLED {
                imp.status_listbox.select_row(Some(&row));
            }

            if flag == PkgFlags::UPDATES {
                row.set_spinning(true);
                row.set_sensitive(false);

                imp.update_row.replace(row);
            }
        }
    }

    //-----------------------------------
    // Setup alpm: load alpm packages
    //-----------------------------------
    fn load_packages_async(&self) {
        let imp = self.imp();

        let pacman_config = imp.pacman_config.borrow().clone();

        // Spawn thread to load packages
        let (sender, receiver) = glib::MainContext::channel::<(alpm::Alpm, Vec<PkgData>)>(glib::Priority::DEFAULT);

        thread::spawn(move || {
            let handle = alpm::Alpm::new(pacman_config.root_dir, pacman_config.db_path).unwrap();

            let localdb = handle.localdb();

            let mut data_list: Vec<PkgData> = vec![];

            for repo in pacman_config.pacman_repos {
                if let Ok(db) = handle.register_syncdb(repo, alpm::SigLevel::DATABASE_OPTIONAL) {
                    data_list.extend(db.pkgs().iter()
                        .map(|syncpkg| {
                            let localpkg = localdb.pkg(syncpkg.name());

                            PkgData::new(syncpkg, localpkg)
                        })
                    );
                }
            }

            data_list.extend(localdb.pkgs().iter()
                .filter_map(|pkg| {
                    handle.syncdbs().find_satisfier(pkg.name()).map_or_else(
                        || Some(PkgData::new(pkg, Ok(pkg))),
                        |_| None
                )})
            );

            sender.send((handle, data_list)).expect("Could not send through channel");
        });

        // Attach thread receiver
        receiver.attach(
            None,
            clone!(@weak self as win, @weak imp => @default-return glib::ControlFlow::Break, move |(handle, data_list)| {
                let handle_ref = Rc::new(handle);

                let pkg_list: Vec<PkgObject> = data_list.into_iter()
                    .map(|data| PkgObject::new(handle_ref.clone(), data))
                    .collect();

                imp.package_view.imp().model.splice(0, imp.package_view.imp().model.n_items(), &pkg_list);

                imp.package_view.imp().stack.set_visible_child_name("view");

                win.check_aur_packages_async();
                win.get_package_updates_async();

                glib::ControlFlow::Break
            }),
        );
    }

    //-----------------------------------
    // Setup alpm: check AUR packages
    //-----------------------------------
    fn check_aur_packages_async(&self) {
        let imp = self.imp();

        // Get list of local packages (not in sync DBs)
        let local_pkgs = imp.package_view.imp().model.iter::<PkgObject>()
            .flatten()
            .filter_map(|pkg| if pkg.repository() == "local" {Some(pkg.name())} else {None})
            .collect::<Vec<String>>();

        // Spawn thread to check AUR packages
        let (sender, receiver) = glib::MainContext::channel::<Vec<String>>(glib::Priority::DEFAULT);

        thread::spawn(move || {
            let mut aur_list: Vec<String> = vec![];

            // Check if local packages are in AUR
            let handle = raur::blocking::Handle::new();

            if let Ok(aur_pkgs) = handle.info(&local_pkgs) {
                aur_list.extend(aur_pkgs.iter().map(|pkg| pkg.name.to_string()));
            }

            // Return thread result
            sender.send(aur_list).expect("Could not send through channel");
        });

        // Attach thread receiver
        receiver.attach(
            None,
            clone!(@weak imp => @default-return glib::ControlFlow::Break, move |aur_list| {
                // Update repository for AUR packages
                for pkg in imp.package_view.imp().model.iter::<PkgObject>().flatten()
                    .filter(|pkg| aur_list.contains(&pkg.name()))
                {
                    pkg.set_repo_show("aur");

                    // Update info pane if currently displayed package is in AUR
                    let info_pkg = imp.info_pane.pkg();

                    if info_pkg.is_some() && info_pkg.unwrap() == pkg {
                        imp.info_pane.update_property_value(PropID::PackageUrl, &imp.info_pane.prop_to_package_url(&pkg), None);

                        imp.info_pane.update_property_value(PropID::Repository, &pkg.repo_show(), None);
                    }
                }

                glib::ControlFlow::Break
            }),
        );
    }

    //-----------------------------------
    // Setup alpm: get package updates
    //-----------------------------------
    fn get_package_updates_async(&self) {
        let imp = self.imp();

        // Get custom command for AUR updates
        let aur_command = imp.prefs_window.aur_command();

        // Spawn thread to check for updates
        let (sender, receiver) = glib::MainContext::channel::<(bool, HashMap<String, String>)>(glib::Priority::DEFAULT);

        thread::spawn(move || {
            let mut update_map = HashMap::new();
            let mut update_str = String::from("");

            // Check for pacman updates
            let (code, stdout) = Utils::run_command("/usr/bin/checkupdates");

            if code == Some(0) {
                update_str += &stdout;
            }

            let success = code == Some(0) || code == Some(2);

            // If no error on pacman updates
            if success {
                // Check for AUR updates
                let (code, stdout) = Utils::run_command(&aur_command);

                if code == Some(0) {
                    update_str += &stdout;
                }

                lazy_static! {
                    static ref EXPR: Regex = Regex::new("([a-zA-Z0-9@._+-]+?)[ \\t]+?([a-zA-Z0-9@._+-:]+?)[ \\t]+?->[ \\t]+?([a-zA-Z0-9@._+-:]+)").unwrap();
                }

                // Build update map (package name, version)
                update_map = update_str.lines()
                    .filter_map(|s|
                        EXPR.captures(s)
                            .filter(|caps| caps.len() == 4)
                            .map(|caps| {
                                let pkg_name = caps[1].to_string();
                                let version = format!("{} \u{2192} {}", caps[2].to_string(), caps[3].to_string());

                                (pkg_name, version)
                            })
                    )
                    .collect::<HashMap<String, String>>();
            }

            // Return thread result
            sender.send((success, update_map)).expect("Could not send through channel");
        });

        // Attach thread receiver
        receiver.attach(
            None,
            clone!(@weak imp => @default-return glib::ControlFlow::Break, move |(success, update_map)| {
                let mut update_list: Vec<PkgObject> = vec![];

                // If updates found
                if update_map.len() > 0 {
                    // Get list of packages with updates
                    update_list = imp.package_view.imp().model.iter::<PkgObject>()
                        .flatten()
                        .filter(|pkg| update_map.contains_key(&pkg.name()))
                        .collect();

                    // Update status of packages with updates
                    for pkg in update_list.iter() {
                        pkg.set_version(update_map[&pkg.name()].to_string());

                        pkg.set_flags(pkg.flags() | PkgFlags::UPDATES);

                        pkg.set_has_update(true);

                        // Update info pane if currently displayed package has update
                        let info_pkg = imp.info_pane.pkg();

                        if info_pkg.is_some() && info_pkg.unwrap() == *pkg {
                            imp.info_pane.update_property_value(PropID::Version, &pkg.version(), Some("pkg-update"));
                        }
                    }
                }

                // Show update status/count in sidebar
                let update_row = imp.update_row.borrow();

                update_row.set_spinning(false);
                update_row.set_icon(if success {"status-updates-symbolic"} else {"status-updates-error-symbolic"});
                update_row.set_count(if success && update_list.len() > 0 {update_list.len().to_string()} else {String::from("")});

                update_row.set_tooltip_text(if success {Some("")} else {Some("Update Error")});

                update_row.set_sensitive(success);

                glib::ControlFlow::Break
            }),
        );
    }

    //-----------------------------------
    // Setup INotify
    //-----------------------------------
    fn setup_inotify(&self) {
        let imp = self.imp();

        // Create glib channel
        let (sender, receiver) = glib::MainContext::channel::<()>(glib::Priority::DEFAULT);

        // Create new watcher
        let mut watcher = new_debouncer(Duration::from_secs(1), None, move |result: DebounceEventResult| {
            if let Ok(events) = result {
                for event in events {
                    if event.kind.is_create() || event.kind.is_modify() || event.kind.is_remove() {
                        sender.send(()).expect("Could not send through channel");

                        break;
                    }
                }
            }
        }).unwrap();

        // Watch pacman local db path
        let pacman_config = imp.pacman_config.borrow();

        let watch_path = format!("{}/local/", pacman_config.db_path);

        watcher.watcher().watch(Path::new(&watch_path), RecursiveMode::Recursive).unwrap();
        watcher.cache().add_root(Path::new(&watch_path), RecursiveMode::Recursive);

        // Store watcher
        imp.notify_watcher.set(watcher).unwrap();

        // Attach receiver for glib channel
        receiver.attach(
            None,
            clone!(@weak self as obj, @weak imp => @default-return glib::ControlFlow::Break, move |_| {
                if imp.prefs_window.auto_refresh() == true {
                    imp.search_header.set_active(false);

                    obj.setup_alpm();
                }

                glib::ControlFlow::Continue
            }),
        );
    }
}
