use std::cell::RefCell;
use std::thread;
use std::collections::HashMap;

use gtk::{gio, glib};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, closure_local, once_cell::sync::OnceCell};

use pacmanconf;
use alpm::{Alpm, SigLevel};
use titlecase::titlecase;
use fancy_regex::Regex;
use lazy_static::lazy_static;
use raur::blocking::Raur;

use crate::APP_ID;
use crate::PacViewApplication;
use crate::pkg_object::{PkgObject, PkgData, PkgFlags};
use crate::prop_object::PropObject;
use crate::search_header::{SearchHeader, SearchMode};
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
#[derive(Default, Clone, Debug, PartialEq, Eq, glib::Boxed)]
#[boxed_type(name = "PacmanConfig")]
pub struct PacmanConfig {
    pub pacman_repos: Vec<String>,
    pub root_dir: String,
    pub db_path: String,
    pub log_file: String,
    pub cache_dir: String
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
            // Save bound gsettings
            gsettings.apply();

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

        // Get list of search header by-* properties
        let by_prop_array: Vec<String> = imp.search_header.list_properties().iter()
            .filter_map(|p| if p.name().contains("by-") {Some(p.name().to_string())} else {None})
            .collect();

        // Add select all/reset search header search by-* property actions
        let all_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("selectall")
            .activate(clone!(@weak imp, @strong by_prop_array => move |_, _, _| {
                let header = &imp.search_header;

                header.set_block_notify(true);

                for prop in &by_prop_array {
                    header.set_property(prop, true);
                }

                header.set_block_notify(false);
            }))
            .build();

        let reset_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("reset")
            .activate(clone!(@weak imp, @strong by_prop_array => move |_, _, _| {
                let header = &imp.search_header;

                header.set_block_notify(true);

                for prop in &by_prop_array {
                    header.set_property(prop, prop == &by_prop_array[0]);
                }

                header.set_block_notify(false);
            }))
            .build();

        // Add actions to search group
        let search_group = gio::SimpleActionGroup::new();

        self.insert_action_group("search", Some(&search_group));

        search_group.add_action_entries([toggle_action, stop_action, mode_action, cycle_action, all_action, reset_action]);

        // Add search header search by-* property actions
        for prop in &by_prop_array {
            let action = gio::PropertyAction::new(&prop, &imp.search_header.get(), prop);
            search_group.add_action(&action);
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
                    &pacman_config.pacman_repos,
                    &imp.package_view.imp().model
                );

                stats_window.set_transient_for(Some(&obj));

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

        view_group.add_action_entries([refresh_action, stats_action, copy_action, columns_action]);

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

        // Set info pane main window
        imp.info_pane.set_main_window(self);

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
                        &pkg,
                        imp.prefs_window.custom_font(),
                        &imp.prefs_window.monospace_font(),
                        &pacman_config.log_file,
                        &pacman_config.cache_dir
                    );

                    details_window.set_transient_for(Some(&obj));

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
                    app.set_accels_for_action("search.by-name", &["<ctrl>1"]);
                    app.set_accels_for_action("search.by-desc", &["<ctrl>2"]);
                    app.set_accels_for_action("search.by-group", &["<ctrl>3"]);
                    app.set_accels_for_action("search.by-deps", &["<ctrl>4"]);
                    app.set_accels_for_action("search.by-optdeps", &["<ctrl>5"]);
                    app.set_accels_for_action("search.by-provides", &["<ctrl>6"]);
                    app.set_accels_for_action("search.by-files", &["<ctrl>7"]);

                    app.set_accels_for_action("search.selectall", &["<ctrl>L"]);
                    app.set_accels_for_action("search.reset", &["<ctrl>R"]);

                    app.set_accels_for_action("search.cycle-mode", &["<ctrl>M"]);
                }

            } else {
                obj.imp().package_view.imp().view.grab_focus();

                if let Some(app) = obj.application() {
                    app.set_accels_for_action("search.by-name", &[]);
                    app.set_accels_for_action("search.by-desc", &[]);
                    app.set_accels_for_action("search.by-group", &[]);
                    app.set_accels_for_action("search.by-deps", &[]);
                    app.set_accels_for_action("search.by-optdeps", &[]);
                    app.set_accels_for_action("search.by-provides", &[]);
                    app.set_accels_for_action("search.by-files", &[]);

                    app.set_accels_for_action("search.selectall", &[]);
                    app.set_accels_for_action("search.reset", &[]);

                    app.set_accels_for_action("search.cycle-mode", &[]);
                }
            }
        }));

        // Search header changed signal
        imp.search_header.connect_closure("changed", false, closure_local!(@watch self as obj => move |_: SearchHeader, term: &str, by_name: bool, by_desc: bool, by_group: bool, by_deps: bool, by_optdeps: bool, by_provides: bool, by_files: bool, mode: SearchMode| {
            let imp = obj.imp();

            if term == "" {
                imp.package_view.imp().search_filter.unset_filter_func();
            } else {
                let search_term = term.to_lowercase();

                if mode == SearchMode::Exact {
                    imp.package_view.imp().search_filter.set_filter_func(move |item| {
                        let pkg: &PkgObject = item
                            .downcast_ref::<PkgObject>()
                            .expect("Must be a 'PkgObject'");

                        let results = [
                            by_name && pkg.name().to_lowercase().eq(&search_term),
                            by_desc && pkg.description().to_lowercase().eq(&search_term),
                            by_group && pkg.groups().to_lowercase().eq(&search_term),
                            by_deps && pkg.depends().iter().any(|s| s.to_lowercase().eq(&search_term)),
                            by_optdeps && pkg.optdepends().iter().any(|s| s.to_lowercase().eq(&search_term)),
                            by_provides && pkg.provides().iter().any(|s| s.to_lowercase().eq(&search_term)),
                            by_files && pkg.files().iter().any(|s| s.to_lowercase().eq(&search_term)),
                        ];

                        results.iter().any(|&x| x)
                    });
                } else {
                    imp.package_view.imp().search_filter.set_filter_func(move |item| {
                        let pkg: &PkgObject = item
                            .downcast_ref::<PkgObject>()
                            .expect("Must be a 'PkgObject'");

                        let mut results = vec![];

                        for term in search_term.split_whitespace() {
                            let term_results = [
                                by_name && pkg.name().to_lowercase().contains(&term),
                                by_desc && pkg.description().to_lowercase().contains(&term),
                                by_group && pkg.groups().to_lowercase().contains(&term),
                                by_deps && pkg.depends().iter().any(|s| s.to_lowercase().contains(&term)),
                                by_optdeps && pkg.optdepends().iter().any(|s| s.to_lowercase().contains(&term)),
                                by_provides && pkg.provides().iter().any(|s| s.to_lowercase().contains(&term)),
                                by_files && pkg.files().iter().any(|s| s.to_lowercase().contains(&term)),
                            ];

                            results.push(term_results.iter().any(|&x| x));
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
            let imp = obj.imp();

            let hist_model = imp.info_pane.history_model();

            hist_model.remove_all();

            imp.info_pane.display_package(pkg.as_ref());

            if let Some(pkg) = pkg {
                hist_model.append(&pkg);
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
            cache_dir: pacman_config.cache_dir[0].clone()
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
            let row = FilterRow::new("repository-symbolic", &titlecase(&repo), &repo.to_lowercase(), PkgFlags::default());

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

        let (sender, receiver) = glib::MainContext::channel::<(Alpm, Vec<PkgData>)>(glib::PRIORITY_DEFAULT);

        let pacman_config = imp.pacman_config.borrow().clone();

        thread::spawn(move || {
            let handle = Alpm::new(pacman_config.root_dir, pacman_config.db_path).unwrap();

            let localdb = handle.localdb();

            let mut data_list: Vec<PkgData> = vec![];

            for repo in pacman_config.pacman_repos {
                if let Ok(db) = handle.register_syncdb(repo, SigLevel::DATABASE_OPTIONAL) {
                    data_list.extend(db.pkgs().iter()
                        .map(|syncpkg| {
                            let localpkg = localdb.pkg(syncpkg.name());

                            PkgData::from_alpm_package(syncpkg, localpkg)
                        })
                    );
                }
            }

            data_list.extend(localdb.pkgs().iter()
                .filter_map(|pkg| {
                    handle.syncdbs().find_satisfier(pkg.name()).map_or_else(
                        || Some(PkgData::from_alpm_package(pkg, Ok(pkg))),
                        |_| None
                )})
            );

            sender.send((handle, data_list)).expect("Could not send through channel");
        });

        receiver.attach(
            None,
            clone!(@weak self as win, @weak imp => @default-return Continue(false), move |(handle, data_list)| {
                let pkg_list: Vec<PkgObject> = data_list.into_iter().map(|data| {
                    PkgObject::new(data)
                }).collect();

                imp.info_pane.set_alpm_handle(Some(handle));

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

        let local_pkgs = imp.package_view.imp().model.iter::<PkgObject>()
            .flatten()
            .filter_map(|pkg| if pkg.repository() == "local" {Some(pkg.name())} else {None})
            .collect::<Vec<String>>();

        thread::spawn(move || {
            let mut aur_list: Vec<String> = vec![];

            let handle = raur::blocking::Handle::new();

            if let Ok(aur_pkgs) = handle.info(&local_pkgs) {
                aur_list.extend(aur_pkgs.iter().map(|pkg| pkg.name.clone()));
            }

            // Return thread result
            sender.send(aur_list).expect("Could not send through channel");
        });

        receiver.attach(
            None,
            clone!(@weak imp => @default-return Continue(false), move |aur_list| {
                for pkg in imp.package_view.imp().model.iter::<PkgObject>().flatten().filter(|pkg| aur_list.contains(&pkg.name())) {
                    pkg.set_repo_show("aur");

                    let infopane_model = imp.info_pane.imp().model.get();

                    let infopane_pkg = imp.info_pane.pkg();

                    if infopane_pkg.is_some() && infopane_pkg.unwrap() == pkg {
                        for prop in infopane_model.iter::<PropObject>().flatten() {
                            if prop.label() == "Package URL" {
                                prop.set_value(imp.info_pane.prop_to_esc_url(&format!("https://aur.archlinux.org/packages/{name}", name=pkg.name())));
                            }

                            if prop.label() == "Repository" {
                                prop.set_value(pkg.repo_show());
                            }
                        }
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
                    static ref EXPR: Regex = Regex::new("(\\S+) (\\S+ -> \\S+)").unwrap();
                }

                // Build update map (package name, version)
                update_map = update_str.lines()
                    .filter_map(|s|
                        if EXPR.is_match(s).unwrap_or_default() {
                            Some((EXPR.replace_all(s, "$1").to_string(), EXPR.replace_all(s, "$2").to_string()))
                        } else {
                            None
                        }
                    )
                    .collect();
            }

            // Return thread result
            sender.send((success, update_map)).expect("Could not send through channel");
        });

        receiver.attach(
            None,
            clone!(@weak imp => @default-return Continue(false), move |(success, update_map)| {
                // Update status of packages with updates
                if update_map.len() > 0 {
                    for pkg in imp.package_view.imp().model.iter::<PkgObject>().flatten().filter(|pkg| update_map.contains_key(&pkg.name())) {
                        pkg.set_version(update_map[&pkg.name()].to_string());

                        let mut flags = pkg.flags();
                        flags.set(PkgFlags::UPDATES, true);

                        pkg.set_flags(flags);

                        pkg.set_has_update(true);

                        let infopane_model = imp.info_pane.imp().model.get();

                        let infopane_pkg = imp.info_pane.pkg();

                        if infopane_pkg.is_some() && infopane_pkg.unwrap() == pkg {
                            for prop in infopane_model.iter::<PropObject>().flatten() {
                                if prop.label() == "Version" {
                                    prop.set_value(pkg.version());
                                    prop.set_icon("pkg-update");
                                }
                            }
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
