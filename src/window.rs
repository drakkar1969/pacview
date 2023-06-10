use std::cell::RefCell;
use std::thread;
use std::collections::HashMap;

use gtk::{gio, glib, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, once_cell::sync::OnceCell};

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
    pub default_repos: Vec<String>,
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
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PacViewWindow)]
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
        pub pkgview_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub pkgview: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub pkgview_click_gesture: TemplateChild<gtk::GestureClick>,
        #[template_child]
        pub pkgview_popover_menu: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        pub pkgview_selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub pkgview_repo_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub pkgview_status_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub pkgview_search_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub pkgview_filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub pkgview_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub pkgview_empty_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub pkgview_version_column: TemplateChild<gtk::ColumnViewColumn>,
        #[template_child]
        pub pkgview_repository_column: TemplateChild<gtk::ColumnViewColumn>,
        #[template_child]
        pub pkgview_status_column: TemplateChild<gtk::ColumnViewColumn>,
        #[template_child]
        pub pkgview_date_column: TemplateChild<gtk::ColumnViewColumn>,
        #[template_child]
        pub pkgview_size_column: TemplateChild<gtk::ColumnViewColumn>,
        #[template_child]
        pub pkgview_groups_column: TemplateChild<gtk::ColumnViewColumn>,

        #[template_child]
        pub info_pane: TemplateChild<InfoPane>,

        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub prefs_window: TemplateChild<PreferencesWindow>,

        gsettings: OnceCell<gio::Settings>,

        update_row: RefCell<FilterRow>,

        pub alpm_handle: OnceCell<Alpm>,

        pub package_list: RefCell<Vec<PkgObject>>,

        #[property(get, set)]
        pacman_config: RefCell<PacmanConfig>,
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
            PkgObject::static_type();
            SearchHeader::static_type();
            InfoPane::static_type();

            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PacViewWindow {
        //-----------------------------------
        // Default property functions
        //-----------------------------------
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            self.init_gsettings();
            self.load_gsettings();

            self.setup_search();
            self.setup_toolbar();
            self.setup_pkgview();
            self.setup_infopane();
            self.setup_preferences();

            self.setup_alpm();
        }
    }

    impl WidgetImpl for PacViewWindow {}
    impl WindowImpl for PacViewWindow {
        //-----------------------------------
        // Window close handler
        //-----------------------------------
        fn close_request(&self) -> glib::signal::Inhibit {
            self.save_gsettings();

            glib::signal::Inhibit(false)
        }
    }
    impl ApplicationWindowImpl for PacViewWindow {}
    impl AdwApplicationWindowImpl for PacViewWindow {}

    #[gtk::template_callbacks]
    impl PacViewWindow {
        //-----------------------------------
        // Init gsettings
        //-----------------------------------
        fn init_gsettings(&self) {
            let gsettings = gio::Settings::new(APP_ID);

            self.gsettings.set(gsettings).unwrap();
        }

        //-----------------------------------
        // Load gsettings
        //-----------------------------------
        fn load_gsettings(&self) {
            if let Some(gsettings) = self.gsettings.get() {
                let obj = self.obj();

                obj.set_default_size(gsettings.int("window-width"), gsettings.int("window-height"));
                obj.set_maximized(gsettings.boolean("window-maximized"));

                self.flap.set_reveal_flap(gsettings.boolean("show-sidebar"));
                self.info_pane.set_visible(gsettings.boolean("show-infopane"));
                self.pane.set_position(gsettings.int("infopane-position"));

                self.prefs_window.set_aur_command(gsettings.string("aur-update-command"));
                self.prefs_window.set_remember_columns(gsettings.boolean("remember-columns"));
                self.prefs_window.set_remember_sort(gsettings.boolean("remember-sorting"));
                self.prefs_window.set_custom_font(gsettings.boolean("custom-font"));
                self.prefs_window.set_monospace_font(gsettings.string("monospace-font"));

                let default_font = gsettings.default_value("monospace-font").unwrap().to_string().replace("'", "");

                self.prefs_window.set_default_monospace_font(default_font);

                // Restore pkgview columns only if setting active
                if self.prefs_window.remember_columns() {
                    // Get saved column IDs
                    let column_ids = gsettings.strv("view-columns");

                    // Iterate through column IDs
                    for (i, id) in column_ids.iter().enumerate() {
                        // If column exists with given ID, insert it at position
                        if let Some(col) = self.pkgview.columns().iter::<gtk::ColumnViewColumn>().flatten().find(|col| col.id().unwrap() == *id) {
                            self.pkgview.insert_column(i as u32, &col);
                        }
                    }

                    // Hide columns that are not in saved column IDs
                    for col in self.pkgview.columns().iter::<gtk::ColumnViewColumn>().flatten() {
                        if !column_ids.contains(col.id().unwrap()) {
                            col.set_visible(false);
                        }
                    }
                }

                // Get saved pkgview sort column/sort order
                let sort_asc = gsettings.boolean("sort-ascending");
                let sort_col = gsettings.string("sort-column");

                // Find and set sort column
                if let Some(col) = self.pkgview.columns().iter::<gtk::ColumnViewColumn>().flatten().find(|col| col.id().unwrap() == sort_col) {
                    self.pkgview.sort_by_column(Some(&col), if sort_asc {gtk::SortType::Ascending} else {gtk::SortType::Descending});
                }
            }
        }

        //-----------------------------------
        // Save gsettings
        //-----------------------------------
        fn save_gsettings(&self) {
            if let Some(gsettings) = self.gsettings.get() {
                let obj = self.obj();

                let (width, height) = obj.default_size();

                gsettings.set_int("window-width", width).unwrap();
                gsettings.set_int("window-height", height).unwrap();
                gsettings.set_boolean("window-maximized", obj.is_maximized()).unwrap();

                gsettings.set_boolean("show-sidebar", self.flap.reveals_flap()).unwrap();
                gsettings.set_boolean("show-infopane", self.info_pane.is_visible()).unwrap();
                gsettings.set_int("infopane-position", self.pane.position()).unwrap();

                gsettings.set_string("aur-update-command", &self.prefs_window.aur_command()).unwrap();
                gsettings.set_boolean("remember-columns", self.prefs_window.remember_columns()).unwrap();
                gsettings.set_boolean("remember-sorting", self.prefs_window.remember_sort()).unwrap();
                gsettings.set_boolean("custom-font", self.prefs_window.custom_font()).unwrap();
                gsettings.set_string("monospace-font", &self.prefs_window.monospace_font()).unwrap();

                // Save pkgview column order if setting active
                if self.prefs_window.remember_columns() {
                    let column_ids: Vec<glib::GString> = self.pkgview.columns()
                        .iter::<gtk::ColumnViewColumn>()
                        .flatten()
                        .filter_map(|col| if col.is_visible() {Some(col.id().unwrap())} else {None})
                        .collect();

                    gsettings.set_strv("view-columns", column_ids).unwrap();
                } else {
                    gsettings.reset("view-columns");
                }

                // Save pkgview sort column/order if setting active
                if self.prefs_window.remember_sort() {
                    // Get pkgview sorter
                    let sorter = self.pkgview.sorter()
                        .and_downcast::<gtk::ColumnViewSorter>()
                        .expect("Must be a 'ColumnViewSorter'");

                    // Get sort column
                    let sort_col = sorter.primary_sort_column().map_or(
                        glib::GString::from(""),
                        |col| col.id().unwrap()
                    );

                    // Get sort order
                    let sort_asc = sorter.primary_sort_order() == gtk::SortType::Ascending;

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
            // Set key capture widget
            self.search_header.set_key_capture_widget(&self.pkgview.upcast_ref());

            // Add start/stop search actions
            let search_start_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("start")
                .activate(clone!(@weak self as win => move |_, _, _| {
                    win.search_header.set_active(true)
                }))
                .build();

            let search_stop_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("stop")
                .activate(clone!(@weak self as win => move |_, _, _| {
                    win.search_header.set_active(false)
                }))
                .build();

            // Get list of search header by-* properties
            let by_prop_array: Vec<String> = self.search_header.list_properties().iter()
                .filter_map(|p| if p.name().contains("by-") {Some(p.name().to_string())} else {None})
                .collect();

            // Add select all/reset search header search by property actions
            let selectall_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("selectall")
                .activate(clone!(@weak self as win, @strong by_prop_array => move |_, _, _| {
                    let header = &win.search_header;

                    header.set_block_notify(true);

                    for prop in &by_prop_array {
                        header.set_property(prop, true);
                    }

                    header.set_block_notify(false);
                }))
                .build();

            let reset_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("reset")
                .activate(clone!(@weak self as win, @strong by_prop_array => move |_, _, _| {
                    let header = &win.search_header;

                    header.set_block_notify(true);

                    for prop in &by_prop_array {
                        header.set_property(prop, prop == &by_prop_array[0]);
                    }

                    header.set_block_notify(false);
                }))
                .build();

            // Add actions to search group
            let search_group = gio::SimpleActionGroup::new();

            self.obj().insert_action_group("search", Some(&search_group));

            search_group.add_action_entries([search_start_action, search_stop_action, selectall_action, reset_action]);

            // Add search header set search mode stateful action
            let mode_action = gio::SimpleAction::new_stateful("set-mode", Some(&String::static_variant_type()), "all".to_variant());
            mode_action.connect_change_state(clone!(@weak self as win => move |action, param| {
                let param = param
                    .expect("Must be a 'Variant'")
                    .get::<String>()
                    .expect("Must be a 'String'");

                win.search_header.set_mode(
                    match param.as_str() {
                        "all" => SearchMode::All,
                        "any" => SearchMode::Any,
                        "exact" => SearchMode::Exact,
                        _ => unreachable!()
                    }
                );

                action.set_state(param.to_variant());
            }));
            search_group.add_action(&mode_action);

            // Add search header cycle search mode action
            let cycle_mode_action = gio::SimpleAction::new("cycle-mode", None);
            cycle_mode_action.connect_activate(clone!(@weak self as win, @weak search_group => move |_, _| {
                if let Some(mode_action) = search_group.lookup_action("set-mode") {
                    let state = mode_action.state()
                        .expect("Must be a 'Variant'")
                        .get::<String>()
                        .expect("Must be a 'String'");

                    let new_state = match state.as_str() {
                        "all" => "any",
                        "any" => "exact",
                        "exact" => "all",
                        _ => unreachable!()
                    };

                    mode_action.change_state(&new_state.to_variant());
                }
            }));
            search_group.add_action(&cycle_mode_action);

            // Add search header search by property actions
            for prop in &by_prop_array {
                let action = gio::PropertyAction::new(&prop.replace("by-", "toggle-"), &self.search_header.get(), prop);
                search_group.add_action(&action);
            }
        }

        //-----------------------------------
        // Setup toolbar buttons
        //-----------------------------------
        fn setup_toolbar(&self) {
            let obj = self.obj();

            // Add sidebar/infopane visibility property actions
            let show_sidebar_action = gio::PropertyAction::new("show-sidebar", &self.flap.get(), "reveal-flap");
            obj.add_action(&show_sidebar_action);

            let show_infopane_action = gio::PropertyAction::new("show-infopane", &self.info_pane.get(), "visible");
            obj.add_action(&show_infopane_action);

            // Bind search button state to search header active state
            self.search_button.bind_property("active", &self.search_header.get(), "active")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                .build();
        }

        //-----------------------------------
        // Setup package column view
        //-----------------------------------
        fn setup_pkgview(&self) {
            let obj = self.obj();

            // Bind pkgview item count to empty label visisility
            self.pkgview_filter_model.bind_property("n-items", &self.pkgview_empty_label.get(), "visible")
                .transform_to(|_, n_items: u32| {
                    Some(n_items == 0)
                })
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();

            // Bind pkgview item count to status label text
            self.pkgview_filter_model.bind_property("n-items", &self.status_label.get(), "label")
                .transform_to(|_, n_items: u32| {
                    Some(format!("{} matching package{}", n_items, if n_items != 1 {"s"} else {""}))
                })
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();

            // Add pkgview refresh action
            let refresh_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("refresh")
                .activate(clone!(@weak self as win => move |_, _, _| {
                    win.search_header.set_active(false);

                    win.setup_alpm();
                }))
                .build();

            // Add pkgview show stats action
            let stats_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("show-stats")
                .activate(clone!(@weak self as win => move |_, _, _| {
                    let pacman_config = win.obj().pacman_config();
                    
                    let stats_window = StatsWindow::new(
                        &pacman_config.pacman_repos,
                        &win.package_list.borrow()
                    );

                    stats_window.set_transient_for(Some(&*win.obj()));

                    stats_window.present();
                }))
                .build();

            // Add pkgview copy list action
            let copy_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("copy-list")
                .activate(clone!(@weak self as win => move |_, _, _| {
                    let copy_text = (0..win.pkgview_selection.n_items()).into_iter()
                        .map(|i| {
                            let pkg = win.pkgview_selection.item(i)
                                .and_downcast::<PkgObject>()
                                .expect("Must be a 'PkgObject'");

                            format!("{repo}/{name}-{version}", repo=pkg.repo_show(), name=pkg.name(), version=pkg.version())
                        })
                        .collect::<Vec<String>>()
                        .join("\n");

                    win.obj().clipboard().set_text(&copy_text);
                }))
                .build();

            // Add pkgview reset columns action
            let columns_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("reset-columns")
                .activate(clone!(@weak self as win => move |_, _, _| {
                    // Get default column IDs
                    let column_ids = ["package", "version", "repository", "status", "date", "size"];

                    // Iterate through column IDs
                    for (i, id) in column_ids.iter().enumerate() {
                        // If column exists with given ID, insert it at position
                        if let Some(col) = win.pkgview.columns().iter::<gtk::ColumnViewColumn>().flatten().find(|col| col.id().unwrap() == *id) {
                            win.pkgview.insert_column(i as u32, &col);
                        }
                    }

                    // Show/hide columns
                    for col in win.pkgview.columns().iter::<gtk::ColumnViewColumn>().flatten() {
                        col.set_visible(column_ids.contains(&col.id().unwrap().as_str()));
                    }
                }))
                .build();

            // Add actions to view group
            let pkgview_group = gio::SimpleActionGroup::new();

            obj.insert_action_group("view", Some(&pkgview_group));

            pkgview_group.add_action_entries([refresh_action, stats_action, copy_action, columns_action]);

            // Add pkgview header menu property actions
            for col in self.pkgview.columns().iter::<gtk::ColumnViewColumn>().flatten() {
                let col_action = gio::PropertyAction::new(&format!("show-column-{}", col.id().unwrap()), &col, "visible");
                pkgview_group.add_action(&col_action);
            }

            // Set initial focus on pkgview
            self.pkgview.grab_focus();
        }

        //-----------------------------------
        // Setup info pane
        //-----------------------------------
        fn setup_infopane(&self) {
            let obj = self.obj();

            // Set info pane main window
            self.info_pane.set_main_window(&*obj);

            // Add info pane prev/next actions
            let prev_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("previous")
                .activate(clone!(@weak self as win => move |_, _, _| {
                    win.info_pane.display_prev();
                }))
                .build();
            
            let next_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("next")
                .activate(clone!(@weak self as win => move |_, _, _| {
                    win.info_pane.display_next();
                }))
                .build();

            // Add info pane show details action
            let details_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("show-details")
                .activate(clone!(@weak self as win, @weak obj => move |_, _, _| {
                    if let Some(pkg) = win.info_pane.pkg() {
                        let pacman_config = obj.pacman_config();

                        let details_window = DetailsWindow::new(
                            &pkg,
                            win.prefs_window.custom_font(),
                            &win.prefs_window.monospace_font(),
                            &pacman_config.log_file,
                            &pacman_config.cache_dir
                        );

                        details_window.set_transient_for(Some(&*win.obj()));

                        details_window.present();
                    }
                }))
                .build();

            // Add actions to info pane group
            let infopane_group = gio::SimpleActionGroup::new();

            obj.insert_action_group("info", Some(&infopane_group));

            infopane_group.add_action_entries([prev_action, next_action, details_action]);
        }

        //-----------------------------------
        // Setup preferences
        //-----------------------------------
        fn setup_preferences(&self) {
            let obj = self.obj();

            // Set preferences window parent
            self.prefs_window.set_transient_for(Some(&*obj));

            // Add show preferences action
            let prefs_action = gio::SimpleAction::new("show-preferences", None);
            prefs_action.connect_activate(clone!(@weak self as win, @weak obj => move |_, _| {
                win.prefs_window.present();
            }));
            obj.add_action(&prefs_action);
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
            // Get standard repository names
            let default_repos: Vec<String> = ["core", "extra", "multilib"].map(String::from).to_vec();

            // Get pacman config
            let pacman_config = pacmanconf::Config::new().unwrap();

            // Get pacman repositories
            let mut pacman_repos: Vec<String> = pacman_config.repos.iter()
                .map(|r| r.name.to_string())
                .collect();
            
            // Add 'local' to pacman repositories
            pacman_repos.push(String::from("local"));

            // Store pacman config
            self.obj().set_pacman_config(PacmanConfig{
                default_repos,
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
            // Clear sidebar rows
            while let Some(row) = self.repo_listbox.row_at_index(0) {
                self.repo_listbox.remove(&row);
            }

            while let Some(row) = self.status_listbox.row_at_index(0) {
                self.status_listbox.remove(&row);
            }

            // Add repository rows (enumerate pacman repositories)
            let row = FilterRow::new("repository-symbolic", "All", "", PkgFlags::default());

            self.repo_listbox.append(&row);

            self.repo_listbox.select_row(Some(&row));

            for repo in self.obj().pacman_config().pacman_repos {
                let row = FilterRow::new("repository-symbolic", &titlecase(&repo), &repo.to_lowercase(), PkgFlags::default());

                self.repo_listbox.append(&row);
            }

            // Add package status rows (enumerate PkgStatusFlags)
            let flags = glib::FlagsClass::new(PkgFlags::static_type()).unwrap();

            for f in flags.values() {
                let flag = PkgFlags::from_bits_truncate(f.value());

                let row = FilterRow::new(&format!("status-{}-symbolic", f.nick()), f.name(), "", flag);

                self.status_listbox.append(&row);

                if flag == PkgFlags::INSTALLED {
                    self.status_listbox.select_row(Some(&row));
                }

                if flag == PkgFlags::UPDATES {
                    row.set_spinning(true);
                    row.set_sensitive(false);

                    self.update_row.replace(row);
                }
            }
        }

        //-----------------------------------
        // Setup alpm: load alpm packages
        //-----------------------------------
        fn load_packages_async(&self) {
            let (sender, receiver) = glib::MainContext::channel::<(Alpm, Vec<PkgData>)>(glib::PRIORITY_DEFAULT);

            let pacman_config = self.obj().pacman_config();

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
                clone!(@weak self as win => @default-return Continue(false), move |(handle, data_list)| {
                    let pkg_list: Vec<PkgObject> = data_list.into_iter().map(|data| {
                        PkgObject::new(data)
                    }).collect();

                    win.alpm_handle.set(handle).unwrap_or_default();

                    win.package_list.replace(pkg_list);

                    win.pkgview_model.splice(0, win.pkgview_model.n_items(), &win.package_list.borrow());

                    win.pkgview_stack.set_visible_child_name("view");

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
            let (sender, receiver) = glib::MainContext::channel::<Vec<String>>(glib::PRIORITY_DEFAULT);

            let aur_params = self.package_list.borrow().iter()
                .filter(|&pkg| pkg.repository() == "local")
                .map(|pkg| pkg.name())
                .collect::<Vec<String>>();

            thread::spawn(move || {
                let mut aur_list: Vec<String> = vec![];

                let handle = raur::blocking::Handle::new();

                if let Ok(aur_pkgs) = handle.info(&aur_params) {
                    aur_list.extend(aur_pkgs.iter().map(|pkg| pkg.name.clone()));
                }

                // Return thread result
                sender.send(aur_list).expect("Could not send through channel");
            });

            receiver.attach(
                None,
                clone!(@weak self as win => @default-return Continue(false), move |aur_list| {
                    let pkg_list = win.package_list.borrow();

                    for pkg in pkg_list.iter().filter(|&pkg| aur_list.contains(&pkg.name())) {
                        pkg.set_repo_show("aur");

                        let infopane_model = win.info_pane.imp().model.get();

                        let infopane_pkg = win.info_pane.pkg();

                        if infopane_pkg.is_some() && infopane_pkg.unwrap() == *pkg {
                            for prop in infopane_model.iter::<PropObject>().flatten() {
                                if prop.label() == "Package URL" {
                                    prop.set_value(win.info_pane.prop_to_esc_url(&format!("https://aur.archlinux.org/packages/{name}", name=pkg.name())));
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
            let (sender, receiver) = glib::MainContext::channel::<(bool, HashMap<String, String>)>(glib::PRIORITY_DEFAULT);

            // Get custom command for AUR updates
            let aur_command = self.prefs_window.aur_command();

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
                clone!(@weak self as win => @default-return Continue(false), move |(success, update_map)| {
                    // Update status of packages with updates
                    let pkg_list = win.package_list.borrow();

                    for pkg in pkg_list.iter().filter(|&pkg| update_map.contains_key(&pkg.name())) {
                        pkg.set_version(update_map[&pkg.name()].to_string());

                        let mut flags = pkg.flags();
                        flags.set(PkgFlags::UPDATES, true);

                        pkg.set_flags(flags);

                        pkg.set_has_update(true);

                        let infopane_model = win.info_pane.imp().model.get();

                        let infopane_pkg = win.info_pane.pkg();

                        if infopane_pkg.is_some() && infopane_pkg.unwrap() == *pkg {
                            for prop in infopane_model.iter::<PropObject>().flatten() {
                                if prop.label() == "Version" {
                                    prop.set_value(pkg.version());
                                    prop.set_icon("pkg-update");
                                }
                            }
                        }
                    }

                    // Show update status/count in sidebar
                    let update_row = win.update_row.borrow();

                    update_row.set_spinning(false);
                    update_row.set_icon(if success {"status-updates-symbolic"} else {"status-updates-error-symbolic"});
                    update_row.set_count(if success && update_map.len() > 0 {update_map.len().to_string()} else {String::from("")});

                    update_row.set_tooltip_text(if success {Some("")} else {Some("Update error")});

                    update_row.set_sensitive(success);

                    Continue(false)
                }),
            );
        }

        //-----------------------------------
        // Sidebar signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_repo_selected(&self, row: Option<FilterRow>) {
            if let Some(row) = row {
                self.pkgview_repo_filter.set_search(Some(&row.repo_id()));
            }
        }

        #[template_callback]
        fn on_status_selected(&self, row: Option<FilterRow>) {
            if let Some(row) = row {
                self.pkgview_status_filter.set_filter_func(move |item| {
                    let pkg: &PkgObject = item
                        .downcast_ref::<PkgObject>()
                        .expect("Must be a 'PkgObject'");

                    pkg.flags().intersects(row.status_id())
                });
            }
        }

        //-----------------------------------
        // Search header signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_search_activated(&self, active: bool) {
            let obj = self.obj();

            if active {
                if let Some(app) = &obj.application() {
                    app.set_accels_for_action("search.toggle-name", &["<ctrl>1"]);
                    app.set_accels_for_action("search.toggle-desc", &["<ctrl>2"]);
                    app.set_accels_for_action("search.toggle-group", &["<ctrl>3"]);
                    app.set_accels_for_action("search.toggle-deps", &["<ctrl>4"]);
                    app.set_accels_for_action("search.toggle-optdeps", &["<ctrl>5"]);
                    app.set_accels_for_action("search.toggle-provides", &["<ctrl>6"]);
                    app.set_accels_for_action("search.toggle-files", &["<ctrl>7"]);

                    app.set_accels_for_action("search.selectall", &["<ctrl>L"]);
                    app.set_accels_for_action("search.reset", &["<ctrl>R"]);

                    app.set_accels_for_action("search.cycle-mode", &["<ctrl>M"]);
                }

            } else {
                self.pkgview.grab_focus();

                if let Some(app) = &obj.application() {
                    app.set_accels_for_action("search.toggle-name", &[]);
                    app.set_accels_for_action("search.toggle-desc", &[]);
                    app.set_accels_for_action("search.toggle-group", &[]);
                    app.set_accels_for_action("search.toggle-deps", &[]);
                    app.set_accels_for_action("search.toggle-optdeps", &[]);
                    app.set_accels_for_action("search.toggle-provides", &[]);
                    app.set_accels_for_action("search.toggle-files", &[]);

                    app.set_accels_for_action("search.selectall", &[]);
                    app.set_accels_for_action("search.reset", &[]);

                    app.set_accels_for_action("search.cycle-mode", &[]);
                }
            }
        }

        #[template_callback]
        fn on_search_changed(&self, term: &str, by_name: bool, by_desc: bool, by_group: bool, by_deps: bool, by_optdeps: bool, by_provides: bool, by_files: bool, mode: SearchMode) {
            if term == "" {
                self.pkgview_search_filter.unset_filter_func();
            } else {
                let search_term = term.to_lowercase();

                if mode == SearchMode::Exact {
                    self.pkgview_search_filter.set_filter_func(move |item| {
                        let pkg: &PkgObject = item
                            .downcast_ref::<PkgObject>()
                            .expect("Needs to be a PkgObject");

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
                    self.pkgview_search_filter.set_filter_func(move |item| {
                        let pkg: &PkgObject = item
                            .downcast_ref::<PkgObject>()
                            .expect("Needs to be a PkgObject");

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
        }

        //-----------------------------------
        // Pkgview signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_package_selected(&self) {
            let hist_model = self.info_pane.history_model();

            hist_model.remove_all();

            let pkg = self.pkgview_selection.selected_item()
                .and_downcast::<PkgObject>();

            self.info_pane.display_package(pkg.as_ref());

            if let Some(pkg) = pkg {
                hist_model.append(&pkg);
            }
        }

        #[template_callback]
        fn on_pkgview_clicked(&self, _n_press: i32, x: f64, y: f64) {
            let button = self.pkgview_click_gesture.current_button();

            if button == gdk::BUTTON_SECONDARY {
                let rect = gdk::Rectangle::new(x as i32, y as i32, 0, 0);

                self.pkgview_popover_menu.set_pointing_to(Some(&rect));
                self.pkgview_popover_menu.popup();
            }
        }
    }
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION: PacViewWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PacViewWindow(ObjectSubclass<imp::PacViewWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl PacViewWindow {
    //-----------------------------------
    // Public new function
    //-----------------------------------
    pub fn new(app: &PacViewApplication) -> Self {
        glib::Object::builder().property("application", app).build()
    }
}
