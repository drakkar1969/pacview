use std::cell::RefCell;
use std::rc::Rc;
use std::thread;
use std::collections::HashMap;

use gtk::{gio, glib};
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

use crate::APP_ID;
use crate::PacViewApplication;
use crate::pkg_object::{PkgObject, PkgData, PkgFlags};
use crate::search_header::{SearchHeader, SearchMode, SearchFlags};
use crate::package_view::PackageView;
use crate::info_pane::InfoPane;
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
        pub flap: TemplateChild<adw::Flap>,

        #[template_child]
        pub repo_listbox: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub status_listbox: TemplateChild<gtk::ListBox>,

        #[template_child]
        pub pane: TemplateChild<gtk::Paned>,

        #[template_child]
        pub package_view: TemplateChild<PackageView>,

        #[template_child]
        pub info_pane: TemplateChild<InfoPane>,

        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub prefs_window: TemplateChild<PreferencesWindow>,

        pub gsettings: OnceCell<gio::Settings>,

        pub pacman_config: RefCell<PacmanConfig>,

        pub update_row: RefCell<FilterRow>,
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

            obj.setup_search();
            obj.setup_toolbar();
            obj.setup_packageview();
            obj.setup_infopane();
            obj.setup_preferences();

            obj.setup_signals();

            obj.setup_alpm();
        }
    }

    impl WidgetImpl for PacViewWindow {}
    impl WindowImpl for PacViewWindow {
        //-----------------------------------
        // Window close handler
        //-----------------------------------
        fn close_request(&self) -> glib::signal::Inhibit {
            self.obj().save_gsettings();

            glib::signal::Inhibit(false)
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

            gsettings.bind("show-sidebar", &imp.flap.get(), "reveal-flap").build();
            gsettings.bind("show-infopane", &imp.info_pane.get(), "visible").build();
            gsettings.bind("infopane-position", &imp.pane.get(), "position").build();

            gsettings.bind("aur-update-command", &imp.prefs_window.get(), "aur-command").build();
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
    // Setup search header
    //-----------------------------------
    fn setup_search(&self) {
        let imp = self.imp();

        // Set key capture widget
        imp.search_header.set_key_capture_widget(&imp.package_view.imp().view.upcast_ref());

        // Add toggle/stop search actions
        let toggle_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("toggle")
            .activate(clone!(@weak imp => move |_, _, _| {
                imp.search_header.set_active(!imp.search_header.active());
            }))
            .build();

        let stop_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("stop")
            .activate(clone!(@weak imp => move |_, _, _| {
                imp.search_header.set_active(false);
            }))
            .build();

        // Add search header set search mode stateful action
        let mode_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("set-mode")
            .parameter_type(Some(&String::static_variant_type()))
            .state("all".to_variant())
            .change_state(clone!(@weak imp => move |_, action, param| {
                let param = param
                    .expect("Must be a 'Variant'")
                    .get::<String>()
                    .expect("Must be a 'String'");

                match param.as_str() {
                    "all" => {
                        imp.search_header.set_mode(SearchMode::All);
                        action.set_state(param.to_variant());
                    },
                    "any" => {
                        imp.search_header.set_mode(SearchMode::Any);
                        action.set_state(param.to_variant());
                    },
                    "exact" => {
                        imp.search_header.set_mode(SearchMode::Exact);
                        action.set_state(param.to_variant());
                    },
                    _ => unreachable!()
                }
            }))
            .build();

        // Add search header cycle search mode action
        let cycle_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("cycle-mode")
            .activate(|group, _, _| {
                if let Some(mode_action) = group.lookup_action("set-mode") {
                    let state = mode_action.state()
                        .expect("Must be a 'Variant'")
                        .get::<String>()
                        .expect("Must be a 'String'");

                    match state.as_str() {
                        "all" => mode_action.change_state(&"any".to_variant()),
                        "any" => mode_action.change_state(&"exact".to_variant()),
                        "exact" => mode_action.change_state(&"all".to_variant()),
                        _ => unreachable!()
                    };
                }
            })
            .build();

        // Add select all/reset search flags actions
        let all_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("all-flags")
            .activate(clone!(@weak imp => move |_, _, _| {
                imp.search_header.set_flags(SearchFlags::all());
            }))
            .build();

        let reset_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("reset-flags")
            .activate(clone!(@weak imp => move |_, _, _| {
                imp.search_header.set_flags(SearchFlags::NAME);
            }))
            .build();

        // Add actions to search group
        let search_group = gio::SimpleActionGroup::new();

        self.insert_action_group("search", Some(&search_group));

        search_group.add_action_entries([toggle_action, stop_action, mode_action, cycle_action, all_action, reset_action]);

        // Add search header search flags stateful actions
        let flags_class = glib::FlagsClass::new(SearchFlags::static_type()).unwrap();

        for f in flags_class.values() {
            let flag = SearchFlags::from_bits_truncate(f.value());

            let flag_action = gio::SimpleAction::new_stateful(&format!("flag-{}", f.nick()), None, (flag == SearchFlags::NAME).to_variant());

            flag_action.connect_activate(clone!(@weak imp, @strong flag => move |_, _| {
                imp.search_header.set_flags(imp.search_header.flags() ^ flag);
            }));

            search_group.add_action(&flag_action);

            // Bind search header flags property to action state
            imp.search_header.bind_property("flags", &flag_action, "state")
                .transform_to(move |_, flags: SearchFlags| Some(flags.contains(flag).to_variant()))
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
        }
    }

    //-----------------------------------
    // Setup toolbar buttons
    //-----------------------------------
    fn setup_toolbar(&self) {
        let imp = self.imp();

        // Add sidebar/infopane visibility property actions
        let show_sidebar_action = gio::PropertyAction::new("show-sidebar", &imp.flap.get(), "reveal-flap");
        self.add_action(&show_sidebar_action);

        let show_infopane_action = gio::PropertyAction::new("show-infopane", &imp.info_pane.get(), "visible");
        self.add_action(&show_infopane_action);

        // Bind search button state to search header active state
        imp.search_button.bind_property("active", &imp.search_header.get(), "active")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();
    }

    //-----------------------------------
    // Setup package column view
    //-----------------------------------
    fn setup_packageview(&self) {
        let imp = self.imp();

        // Bind package view item count to status label text
        imp.package_view.imp().filter_model.bind_property("n-items", &imp.status_label.get(), "label")
            .transform_to(|_, n_items: u32| {
                Some(format!("{} matching package{}", n_items, if n_items != 1 {"s"} else {""}))
            })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Add package view check for updates action
        let updates_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("check-updates")
            .activate(clone!(@weak self as obj => move |_, _, _| {
                obj.get_package_updates_async();
            }))
            .build();

        // Add package view refresh action
        let refresh_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("refresh")
            .activate(clone!(@weak self as obj, @weak imp => move |_, _, _| {
                imp.search_header.set_active(false);

                obj.setup_alpm();
            }))
            .build();

        // Add package view show stats action
        let stats_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("show-stats")
            .activate(clone!(@weak self as obj, @weak imp => move |_, _, _| {
                let pacman_config = imp.pacman_config.borrow();
                
                let stats_window = StatsWindow::new(
                    &obj.upcast_ref(),
                    &pacman_config.pacman_repos,
                    &imp.package_view.imp().model
                );

                stats_window.present();
            }))
            .build();

        // Add package view copy list action
        let copy_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("copy-list")
            .activate(clone!(@weak self as obj, @weak imp => move |_, _, _| {
                let copy_text = imp.package_view.imp().filter_model.iter::<glib::Object>()
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

        // Add package view reset columns action
        let columns_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("reset-columns")
            .activate(clone!(@weak imp => move |_, _, _| {
                imp.package_view.reset_columns();
            }))
            .build();

        // Add actions to view group
        let view_group = gio::SimpleActionGroup::new();

        self.insert_action_group("view", Some(&view_group));

        view_group.add_action_entries([refresh_action, updates_action, stats_action, copy_action, columns_action]);

        // Bind package view item count to copy list action enabled state
        if let Some(copy_action) = view_group.lookup_action("copy-list") {
            imp.package_view.imp().filter_model.bind_property("n-items", &copy_action, "enabled")
                .transform_to(|_, n_items: u32| {
                    Some(n_items > 0)
                })
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
        }

        // Add package view header menu property actions
        let columns = imp.package_view.imp().view.columns();

        for col in columns.iter::<gtk::ColumnViewColumn>().flatten() {
            let col_action = gio::PropertyAction::new(&format!("show-column-{}", col.id().unwrap()), &col, "visible");
            view_group.add_action(&col_action);
        }

        // Set initial focus on package view
        imp.package_view.imp().view.grab_focus();
    }

    //-----------------------------------
    // Setup info pane
    //-----------------------------------
    fn setup_infopane(&self) {
        let imp = self.imp();

        // Bind package view model to info pane package model
        imp.package_view.imp().filter_model.bind_property("model", &imp.info_pane.get(), "pkg-model")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Add info pane prev/next actions
        let prev_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("previous")
            .activate(clone!(@weak imp => move |_, _, _| {
                imp.info_pane.display_prev();
            }))
            .build();
        
        let next_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("next")
            .activate(clone!(@weak imp => move |_, _, _| {
                imp.info_pane.display_next();
            }))
            .build();

        // Add info pane show details action
        let details_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("show-details")
            .activate(clone!(@weak self as obj, @weak imp => move |_, _, _| {
                if let Some(pkg) = imp.info_pane.pkg() {
                    let pacman_config = imp.pacman_config.borrow();

                    let details_window = DetailsWindow::new(
                        &obj.upcast_ref(),
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

        // Add actions to info pane group
        let infopane_group = gio::SimpleActionGroup::new();

        self.insert_action_group("info", Some(&infopane_group));

        infopane_group.add_action_entries([prev_action, next_action, details_action]);
    }

    //-----------------------------------
    // Setup preferences
    //-----------------------------------
    fn setup_preferences(&self) {
        let imp = self.imp();

        // Set preferences window parent
        imp.prefs_window.set_transient_for(Some(self));

        // Add show preferences action
        let prefs_action = gio::SimpleAction::new("show-preferences", None);
        prefs_action.connect_activate(clone!(@weak imp => move |_, _| {
            imp.prefs_window.present();
        }));
        self.add_action(&prefs_action);
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Search header activated signal
        imp.search_header.connect_closure("activated", false, closure_local!(@watch self as obj => move |_: SearchHeader, active: bool| {
            if active {
                if let Some(app) = obj.application() {
                    app.set_accels_for_action("search.flag-name", &["<ctrl>1"]);
                    app.set_accels_for_action("search.flag-desc", &["<ctrl>2"]);
                    app.set_accels_for_action("search.flag-group", &["<ctrl>3"]);
                    app.set_accels_for_action("search.flag-deps", &["<ctrl>4"]);
                    app.set_accels_for_action("search.flag-optdeps", &["<ctrl>5"]);
                    app.set_accels_for_action("search.flag-provides", &["<ctrl>6"]);
                    app.set_accels_for_action("search.flag-files", &["<ctrl>7"]);

                    app.set_accels_for_action("search.all-flags", &["<ctrl>L"]);
                    app.set_accels_for_action("search.reset-flags", &["<ctrl>R"]);

                    app.set_accels_for_action("search.cycle-mode", &["<ctrl>M"]);
                }

            } else {
                obj.imp().package_view.imp().view.grab_focus();

                if let Some(app) = obj.application() {
                    app.set_accels_for_action("search.flag-name", &[]);
                    app.set_accels_for_action("search.flag-desc", &[]);
                    app.set_accels_for_action("search.flag-group", &[]);
                    app.set_accels_for_action("search.flag-deps", &[]);
                    app.set_accels_for_action("search.flag-optdeps", &[]);
                    app.set_accels_for_action("search.flag-provides", &[]);
                    app.set_accels_for_action("search.flag-files", &[]);

                    app.set_accels_for_action("search.all-flags", &[]);
                    app.set_accels_for_action("search.reset-flags", &[]);

                    app.set_accels_for_action("search.cycle-mode", &[]);
                }
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

                imp.package_view.imp().repo_filter.set_search(Some(&repo_id));
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
        while let Some(row) = imp.repo_listbox.row_at_index(0) {
            imp.repo_listbox.remove(&row);
        }

        while let Some(row) = imp.status_listbox.row_at_index(0) {
            imp.status_listbox.remove(&row);
        }

        // Add repository rows (enumerate pacman repositories)
        let row = FilterRow::new("repository-symbolic", "All", "", PkgFlags::default());

        imp.repo_listbox.append(&row);

        imp.repo_listbox.select_row(Some(&row));

        for repo in &imp.pacman_config.borrow().pacman_repos {
            let row = FilterRow::new("repository-symbolic", &titlecase(&repo), &repo, PkgFlags::default());

            imp.repo_listbox.append(&row);
        }

        // Add package status rows (enumerate PkgStatusFlags)
        let flags = glib::FlagsClass::new(PkgFlags::static_type()).unwrap();

        for f in flags.values() {
            let flag = PkgFlags::from_bits_truncate(f.value());

            let row = FilterRow::new(&format!("status-{}-symbolic", f.nick()), f.name(), "", flag);

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

        let (sender, receiver) = glib::MainContext::channel::<(alpm::Alpm, Vec<PkgData>)>(glib::PRIORITY_DEFAULT);

        let pacman_config = imp.pacman_config.borrow().clone();

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

        receiver.attach(
            None,
            clone!(@weak self as win, @weak imp => @default-return Continue(false), move |(handle, data_list)| {
                let handle_ref = Rc::new(Some(handle));

                let pkg_list: Vec<PkgObject> = data_list.into_iter()
                    .map(|data| PkgObject::new(handle_ref.clone(), data))
                    .collect();

                imp.package_view.imp().model.splice(0, imp.package_view.imp().model.n_items(), &pkg_list);

                imp.package_view.imp().stack.set_visible_child_name("view");

                win.check_aur_packages_async();
                win.get_package_updates_async();

                Continue(false)
            }),
        );
    }

    //-----------------------------------
    // Setup alpm: check AUR packages
    //-----------------------------------
    fn check_aur_packages_async(&self) {
        let imp = self.imp();

        let (sender, receiver) = glib::MainContext::channel::<Vec<String>>(glib::PRIORITY_DEFAULT);

        // Get list of local packages (not in sync DBs)
        let local_pkgs = imp.package_view.imp().model.iter::<PkgObject>()
            .flatten()
            .filter_map(|pkg| if pkg.repository() == "local" {Some(pkg.name())} else {None})
            .collect::<Vec<String>>();

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

        receiver.attach(
            None,
            clone!(@weak imp => @default-return Continue(false), move |aur_list| {
                // Update repository for AUR packages
                for pkg in imp.package_view.imp().model.iter::<PkgObject>().flatten()
                    .filter(|pkg| aur_list.contains(&pkg.name()))
                {
                    pkg.set_repo_show("aur");

                    // Update info pane if currently displayed package is in AUR
                    if imp.info_pane.pkg().unwrap_or_default() == pkg {
                        imp.info_pane.update_prop("Package URL", &imp.info_pane.prop_to_package_url(&pkg), None);

                        imp.info_pane.update_prop("Repository", &pkg.repo_show(), None);
                    }
                }

                Continue(false)
            }),
        );
    }

    //-----------------------------------
    // Setup alpm: get package updates
    //-----------------------------------
    fn get_package_updates_async(&self) {
        let imp = self.imp();

        let update_row = imp.update_row.borrow();

        update_row.set_spinning(true);
        update_row.set_count("");
        update_row.set_sensitive(false);

        let (sender, receiver) = glib::MainContext::channel::<(bool, HashMap<String, String>)>(glib::PRIORITY_DEFAULT);

        // Get custom command for AUR updates
        let aur_command = imp.prefs_window.aur_command();

        // Spawn thread to check for updates
        thread::spawn(move || {
            let mut update_map = HashMap::new();
            let mut update_str = String::from("");

            // Check for pacman updates
            let (code, stdout) = Utils::run_command("/usr/bin/checkupdates");

            if code == Some(0) {
                update_str += &stdout;
            }

            let success = code == Some(0) || code == Some(2);

            // If no error on pacman updates, check for AUR updates
            if success {
                let (code, stdout) = Utils::run_command(&aur_command);

                if code == Some(0) {
                    update_str += &stdout;
                }

                lazy_static! {
                    static ref EXPR: Regex = Regex::new("(\\S+)\\s+(\\S+)\\s+->\\s+(\\S+)").unwrap();
                }

                // Build update map (package name, version)
                update_map = update_str.lines()
                    .filter_map(|s|
                        EXPR.captures(s)
                            .filter(|caps| caps.len() == 4)
                            .map(|caps| (caps[1].to_string(), format!("{} -> {}", caps[2].to_string(), caps[3].to_string())))
                    )
                    .collect::<HashMap<String, String>>();
            }

            // Return thread result
            sender.send((success, update_map)).expect("Could not send through channel");
        });

        receiver.attach(
            None,
            clone!(@weak imp => @default-return Continue(false), move |(success, update_map)| {
                // Update status of packages with updates
                if update_map.len() > 0 {
                    for pkg in imp.package_view.imp().model.iter::<PkgObject>().flatten()
                        .filter(|pkg| update_map.contains_key(&pkg.name()))
                    {
                        pkg.set_version(update_map[&pkg.name()].to_string());

                        pkg.set_flags(pkg.flags() | PkgFlags::UPDATES);

                        pkg.set_has_update(true);

                        // Update package view if update view is selected
                        if imp.update_row.borrow().is_selected() {
                            imp.package_view.imp().status_filter.changed(gtk::FilterChange::Different);
                        }

                        // Update info pane if currently displayed package has update
                        if imp.info_pane.pkg().unwrap_or_default() == pkg {
                            imp.info_pane.update_prop("Version", &pkg.version(), Some("pkg-update"));
                        }
                    }
                }

                // Show update status/count in sidebar
                let update_row = imp.update_row.borrow();

                update_row.set_spinning(false);
                update_row.set_icon(if success {"status-updates-symbolic"} else {"status-updates-error-symbolic"});
                update_row.set_count(if success && update_map.len() > 0 {update_map.len().to_string()} else {String::from("")});

                update_row.set_tooltip_text(if success {Some("")} else {Some("Update error")});

                update_row.set_sensitive(success);

                Continue(false)
            }),
        );
    }
}
