use std::cell::{Cell, RefCell};
use std::thread;
use std::collections::HashMap;

use gtk::{gio, glib, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, once_cell::sync::OnceCell};

use pacmanconf;
use alpm;
use titlecase;
use fancy_regex::Regex;
use lazy_static::lazy_static;
use url::Url;
use reqwest;

use crate::APP_ID;
use crate::PacViewApplication;
use crate::pkg_object::{PkgObject, PkgData, PkgFlags};
use crate::prop_object::PropObject;
use crate::aur::AurInfo;
use crate::search_header::{SearchHeader, SearchMode};
use crate::filter_row::FilterRow;
use crate::value_row::ValueRow;
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
        pub infopane_overlay: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub infopane_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub infopane_toolbar: TemplateChild<gtk::Box>,
        #[template_child]
        pub infopane_navbutton_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub infopane_prev_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub infopane_next_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub infopane_empty_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub prefs_window: TemplateChild<PreferencesWindow>,

        gsettings: OnceCell<gio::Settings>,

        update_row: RefCell<FilterRow>,

        alpm_handle: OnceCell<alpm::Alpm>,

        default_repo_names: RefCell<Vec<String>>,
        pacman_repo_names: RefCell<Vec<String>>,

        pacman_config: RefCell<pacmanconf::Config>,

        package_list: RefCell<Vec<PkgObject>>,

        history_list: RefCell<Vec<PkgObject>>,
        history_index: Cell<usize>,
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
            PropObject::static_type();
            SearchHeader::static_type();

            klass.bind_template();
            klass.bind_template_callbacks();
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
                self.infopane_overlay.set_visible(gsettings.boolean("show-infopane"));
                self.pane.set_position(gsettings.int("infopane-position"));

                self.prefs_window.set_aur_command(gsettings.string("aur-update-command"));
                self.prefs_window.set_remember_columns(gsettings.boolean("remember-columns"));
                self.prefs_window.set_remember_sort(gsettings.boolean("remember-sorting"));
                self.prefs_window.set_custom_font(gsettings.boolean("custom-font"));
                self.prefs_window.set_monospace_font(gsettings.string("monospace-font"));

                if self.prefs_window.remember_columns() {
                    let column_ids = gsettings.strv("view-columns");

                    let mut col_index = 0;

                    for id in &column_ids {
                        for col in self.pkgview.columns().iter::<gtk::ColumnViewColumn>() {
                            if let Ok(col) = col {
                                if col.id().unwrap() == *id {
                                    self.pkgview.insert_column(col_index, &col);
                                    col_index += 1;
                                }
                            }
                        }
                    }

                    for col in self.pkgview.columns().iter::<gtk::ColumnViewColumn>() {
                        if let Ok(col) = col {
                            if !column_ids.contains(col.id().unwrap()) {
                                col.set_visible(false);
                            }
                        }
                    }
                }

                let sort_asc = gsettings.boolean("sort-ascending");
                let sort_col = gsettings.string("sort-column");

                for col in self.pkgview.columns().iter::<gtk::ColumnViewColumn>() {
                    if let Ok(col) = col {
                        if col.id().unwrap() == sort_col {
                            self.pkgview.sort_by_column(Some(&col), if sort_asc {gtk::SortType::Ascending} else {gtk::SortType::Descending});
                        }
                    }
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
                gsettings.set_boolean("show-infopane", self.infopane_overlay.is_visible()).unwrap();
                gsettings.set_int("infopane-position", self.pane.position()).unwrap();

                gsettings.set_string("aur-update-command", &self.prefs_window.aur_command()).unwrap();
                gsettings.set_boolean("remember-columns", self.prefs_window.remember_columns()).unwrap();
                gsettings.set_boolean("remember-sorting", self.prefs_window.remember_sort()).unwrap();
                gsettings.set_boolean("custom-font", self.prefs_window.custom_font()).unwrap();
                gsettings.set_string("monospace-font", &self.prefs_window.monospace_font()).unwrap();

                if self.prefs_window.remember_columns() {
                    let column_ids: Vec<String> = self.pkgview.columns()
                        .iter::<gtk::ColumnViewColumn>()
                        .filter(|col| col.as_ref().unwrap().is_visible())
                        .map(|col| col.unwrap().id().unwrap().to_string())
                        .collect();

                    gsettings.set_strv("view-columns", column_ids).unwrap();
                } else {
                    gsettings.reset("view-columns");
                }

                if self.prefs_window.remember_sort() {
                    let mut sort_col = String::from("");
                    let mut sort_asc = gtk::SortType::Ascending;

                    if let Some(sorter) = self.pkgview.sorter().and_downcast_ref::<gtk::ColumnViewSorter>() {
                        if let Some(col) = sorter.primary_sort_column() {
                            sort_col = col.id().unwrap().to_string();
                        }

                        sort_asc = sorter.primary_sort_order();
                    }

                    gsettings.set_string("sort-column", &sort_col).unwrap();
                    gsettings.set_boolean("sort-ascending", if sort_asc == gtk::SortType::Ascending {true} else {false}).unwrap();
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

            // Add select all/reset search header search by property actions
            let prop_array = ["name", "desc", "group", "deps", "optdeps", "provides", "files"];

            let selectall_action = gio::ActionEntry::builder("selectall")
                .activate(clone!(@weak self as win => move |_, _, _| {
                    let header = &win.search_header;

                    header.set_block_notify(true);

                    for prop in prop_array {
                        header.set_property(&format!("by-{}", prop), true);
                    }

                    header.set_block_notify(false);

                    win.on_search_changed(&header.imp().search_entry.text().to_string(), header.by_name(), header.by_desc(), header.by_group(), header.by_deps(), header.by_optdeps(), header.by_provides(), header.by_files(), header.mode());
                }))
                .build();

            let reset_action = gio::ActionEntry::builder("reset")
                .activate(clone!(@weak self as win => move |_, _, _| {
                    let header = &win.search_header;

                    header.set_block_notify(true);

                    for prop in prop_array {
                        header.set_property(&format!("by-{}", prop), prop == "name");
                    }

                    header.set_block_notify(false);

                    win.on_search_changed(&header.imp().search_entry.text().to_string(), header.by_name(), header.by_desc(), header.by_group(), header.by_deps(), header.by_optdeps(), header.by_provides(), header.by_files(), header.mode());
                }))
                .build();

            // Add actions to search group
            let search_group = gio::SimpleActionGroup::new();

            self.obj().insert_action_group("search", Some(&search_group));

            search_group.add_action_entries([search_start_action, search_stop_action, selectall_action, reset_action]);

            // Add search header search by property actions
            for prop in prop_array {
                let action = gio::PropertyAction::new(&format!("toggle-{}", prop), &self.search_header.get(), &format!("by-{}", prop));
                search_group.add_action(&action);
            }

            // Add search header search mode stateful action
            let mode_action = gio::SimpleAction::new_stateful("toggle-mode", Some(&String::static_variant_type()), "all".to_variant());
            mode_action.connect_change_state(clone!(@weak self as win => move |action, param| {
                let param = param.unwrap().get::<String>().unwrap();

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
        }

        //-----------------------------------
        // Setup toolbar buttons
        //-----------------------------------
        fn setup_toolbar(&self) {
            let obj = self.obj();

            // Add sidebar/infopane visibility property actions
            let show_sidebar_action = gio::PropertyAction::new("show-sidebar", &self.flap.get(), "reveal-flap");
            obj.add_action(&show_sidebar_action);

            let show_infopane_action = gio::PropertyAction::new("show-infopane", &self.infopane_overlay.get(), "visible");
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
                    let stats_window = StatsWindow::new(
                        &win.pacman_repo_names.borrow(),
                        &win.package_list.borrow()
                    );

                    stats_window.set_transient_for(Some(&*win.obj()));

                    stats_window.present();
                }))
                .build();

            // Add pkgview copy list action
            let copy_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("copy-list")
                .activate(clone!(@weak self as win => move |_, _, _| {
                    let item_list: Vec<String> = IntoIterator::into_iter(0..win.pkgview_selection.n_items())
                    .map(|i| {
                        let pkg: PkgObject = win.pkgview_selection.item(i).and_downcast().expect("Must be a PkgObject");

                        format!("{repo}/{name}-{version}", repo=pkg.repo_show(), name=pkg.name(), version=pkg.version())
                    })
                    .collect();

                    let copy_text = item_list.join("\n");

                    let clipboard = win.obj().clipboard();

                    clipboard.set_text(&copy_text);
                }))
                .build();

            // Add pkgview reset columns action
            let columns_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("reset-columns")
                .activate(clone!(@weak self as win => move |_, _, _| {
                    let column_ids: Vec<String> = vec![String::from("package"), String::from("version"), String::from("repository"), String::from("status"), String::from("date"), String::from("size"), String::from("groups")];
                    let mut col_index = 0;

                    for id in &column_ids {
                        for col in win.pkgview.columns().iter::<gtk::ColumnViewColumn>() {
                            if let Ok(col) = col {
                                if col.id().unwrap() == *id {
                                    win.pkgview.insert_column(col_index, &col);
                                    col_index += 1;
                                }
                            }
                        }
                    }

                    for col in win.pkgview.columns().iter::<gtk::ColumnViewColumn>() {
                        if let Ok(col) = col {
                            col.set_visible(true);
                        }
                    }
                }))
                .build();

            // Add actions to view group
            let pkgview_group = gio::SimpleActionGroup::new();

            obj.insert_action_group("view", Some(&pkgview_group));

            pkgview_group.add_action_entries([refresh_action, stats_action, copy_action, columns_action]);

            // Add pkgview header menu property actions
            let col_action = gio::PropertyAction::new("show-column-version", &self.pkgview_version_column.get(), "visible");
            pkgview_group.add_action(&col_action);

            let col_action = gio::PropertyAction::new("show-column-repository", &self.pkgview_repository_column.get(), "visible");
            pkgview_group.add_action(&col_action);

            let col_action = gio::PropertyAction::new("show-column-status", &self.pkgview_status_column.get(), "visible");
            pkgview_group.add_action(&col_action);

            let col_action = gio::PropertyAction::new("show-column-date", &self.pkgview_date_column.get(), "visible");
            pkgview_group.add_action(&col_action);

            let col_action = gio::PropertyAction::new("show-column-size", &self.pkgview_size_column.get(), "visible");
            pkgview_group.add_action(&col_action);

            let col_action = gio::PropertyAction::new("show-column-groups", &self.pkgview_groups_column.get(), "visible");
            pkgview_group.add_action(&col_action);

            // Set initial focus on pkgview
            self.pkgview.grab_focus();
        }

        //-----------------------------------
        // Setup info pane
        //-----------------------------------
        fn setup_infopane(&self) {
            let obj = self.obj();

            // Add info pane prev/next actions
            let prev_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("previous")
                .activate(clone!(@weak self as win => move |_, _, _| {
                    win.infopane_display_prev();
                }))
                .build();
            
            let next_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("next")
                .activate(clone!(@weak self as win => move |_, _, _| {
                    win.infopane_display_next();
                }))
                .build();

            // Add info pane show details action
            let details_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("show-details")
                .activate(clone!(@weak self as win, @weak obj => move |_, _, _| {
                    let hlist = win.history_list.borrow();
                    let hindex = win.history_index.get();

                    let monospace_font = win.prefs_window.monospace_font();

                    let font: Option<String> = if win.prefs_window.custom_font() {Some(monospace_font)} else {None};

                    if let Some(pkg) = hlist.get(hindex) {
                        let details_window = DetailsWindow::new(
                            pkg,
                            font,
                            &win.pacman_config.borrow().log_file
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
            let pacman_config = pacmanconf::Config::new().unwrap();

            let mut repo_list: Vec<String> = pacman_config.repos.iter().map(|r| r.name.to_string()).collect();
            repo_list.push(String::from("foreign"));

            self.pacman_config.replace(pacman_config);

            self.pacman_repo_names.replace(repo_list);

            let default_repo_names: Vec<String> = vec![String::from("core"), String::from("extra"), String::from("multilib")];

            self.default_repo_names.replace(default_repo_names);
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

            // Add repository rows
            let row = FilterRow::new("repository-symbolic", "All", "", PkgFlags::default());

            self.repo_listbox.append(&row);

            self.repo_listbox.select_row(Some(&row));

            let repo_names = self.pacman_repo_names.borrow().to_vec();

            for repo in repo_names {
                let row = FilterRow::new("repository-symbolic", &titlecase::titlecase(&repo), &repo.to_lowercase(), PkgFlags::default());

                self.repo_listbox.append(&row);
            }

            // Add package status rows (enumerate PkgStatusFlags)
            let status_map = [
                ("all", PkgFlags::ALL),
                ("installed", PkgFlags::INSTALLED),
                ("explicit", PkgFlags::EXPLICIT),
                ("dependency", PkgFlags::DEPENDENCY),
                ("optional", PkgFlags::OPTIONAL),
                ("orphan", PkgFlags::ORPHAN),
                ("none", PkgFlags::NONE),
                ("updates", PkgFlags::UPDATES),
            ];

            for (text, flag) in status_map {
                let row = FilterRow::new(&format!("status-{}-symbolic", text), &titlecase::titlecase(text), "", flag);

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
            let (sender, receiver) = glib::MainContext::channel::<(alpm::Alpm, Vec<PkgData>)>(glib::PRIORITY_DEFAULT);

            let root_dir = self.pacman_config.borrow().root_dir.to_string();
            let db_path = self.pacman_config.borrow().db_path.to_string();

            let repo_names = self.pacman_repo_names.borrow().to_vec();

            thread::spawn(move || {
                let handle = alpm::Alpm::new(root_dir, db_path).unwrap();

                let localdb = handle.localdb();

                let mut data_list: Vec<PkgData> = vec![];

                for repo in repo_names {
                    let db = handle.register_syncdb(repo, alpm::SigLevel::DATABASE_OPTIONAL).unwrap();
                    
                    data_list.extend(db.pkgs().iter()
                        .map(|syncpkg| {
                            let localpkg = localdb.pkg(syncpkg.name());

                            PkgData::from_alpm_package(db.name(), syncpkg, localpkg)
                        })
                    );
                }

                data_list.extend(localdb.pkgs().iter()
                    .filter(|pkg| {
                        !handle.syncdbs().find_satisfier(pkg.name()).is_some()
                    })
                    .map(|pkg| {
                        PkgData::from_alpm_package("foreign", pkg, Ok(pkg))
                    })
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
                .filter(|pkg| pkg.repository() == "foreign")
                .map(|pkg| format!("&arg[]={name}", name=pkg.name()))
                .collect::<Vec<String>>()
                .concat();

            thread::spawn(move || {
                let mut aur_list: Vec<String> = vec![]; 

                let aur_url = String::from("https://aur.archlinux.org/rpc/?v=5&type=info") + &aur_params;

                if let Ok(response) = reqwest::blocking::get(aur_url) {
                    if response.status() == 200 {
                        if let Ok(data) = response.json::<AurInfo>() {
                            aur_list.extend(data.results.into_iter().map(|item| item.Name));
                        }
                    }
                }

                // Return thread result
                sender.send(aur_list).expect("Could not send through channel");
            });

            receiver.attach(
                None,
                clone!(@weak self as win => @default-return Continue(false), move |aur_list| {
                    let pkg_list = win.package_list.borrow();

                    for pkg in pkg_list.iter().filter(|pkg| aur_list.contains(&pkg.name())) {
                        pkg.set_repo_show("aur");

                        let hlist = win.history_list.borrow();
                        let hindex = win.history_index.get();
            
                        let infopane_model = win.infopane_model.get();
            
                        if let Some(info_pkg) = hlist.get(hindex) {
                            if info_pkg.name() == pkg.name() {
                                for i in IntoIterator::into_iter(0..infopane_model.n_items()) {
                                    let prop: PropObject = infopane_model.item(i).and_downcast().expect("Must be a PropObject");

                                    if prop.label() == "Repository" {
                                        prop.set_value(pkg.repo_show());
                                    }

                                    if prop.label() == "Description" {
                                        infopane_model.insert(i+1, &PropObject::new(
                                            "AUR URL", &win.prop_to_esc_url(&format!("https://aur.archlinux.org/packages/{name}", name=pkg.name())), None
                                        ));
                                    }
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
                        .filter(|s| EXPR.is_match(s).unwrap_or_default())
                        .map(|s| 
                            (EXPR.replace_all(s, "$1").to_string(), EXPR.replace_all(s, "$2").to_string())
                        )
                        .collect();
                }

                // Return thread result
                sender.send((success, update_map)).expect("Could not send through channel");
            });

            receiver.attach(
                None,
                clone!(@weak self as win => @default-return Continue(false), move |(success, update_map)| {
                    // If no error on pacman updates
                    if success == true {
                        // Update status of packages with updates
                        let pkg_list = win.package_list.borrow();

                        for (name, version) in update_map.iter() {
                            if let Some(pkg) = pkg_list.iter().find(|pkg| pkg.name().eq(name)) {
                                pkg.set_version(version.to_string());

                                let mut flags = pkg.flags();
                                flags.set(PkgFlags::UPDATES, true);

                                pkg.set_flags(flags);

                                pkg.set_has_update(true);

                                let hlist = win.history_list.borrow();
                                let hindex = win.history_index.get();

                                let infopane_model = win.infopane_model.get();

                                if let Some(info_pkg) = hlist.get(hindex) {
                                    if &info_pkg.name() == name {
                                        for i in IntoIterator::into_iter(0..infopane_model.n_items()) {
                                            let prop: PropObject = infopane_model.item(i).and_downcast().expect("Must be a PropObject");

                                            if prop.label() == "Version" {
                                                prop.set_value(pkg.version());
                                                prop.set_icon("pkg-update");
                                            }
                                        }
                                    }
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
                        .expect("Needs to be a PkgObject");

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
                }
            }
        }

        #[template_callback]
        fn on_search_changed(&self, term: &str, by_name: bool, by_desc: bool, by_group: bool, by_deps: bool, by_optdeps: bool, by_provides: bool, by_files: bool, mode: SearchMode) {
            let search_term = term.to_lowercase();

            if search_term == "" {
                self.pkgview_search_filter.unset_filter_func();
            } else {
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

                        results.into_iter().any(|x| x)
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

                            results.push(term_results.into_iter().any(|x| x));
                        }

                        if mode == SearchMode::All {
                            results.into_iter().all(|x| x)
                        } else {
                            results.into_iter().any(|x| x)
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
            if let Some(item) = self.pkgview_selection.selected_item() {
                let pkg = item.downcast::<PkgObject>().expect("Must be a PkgObject");

                self.history_list.replace(vec![pkg.clone()]);
                self.history_index.replace(0);

                self.infopane_display_package(Some(&pkg));
            } else {
                self.history_list.replace(vec![]);
                self.history_index.replace(0);

                self.infopane_display_package(None);
            }
        }

        #[template_callback]
        fn on_pkgview_clicked(&self, _n_press: i32, x: f64, y: f64) {
            let button = self.pkgview_click_gesture.current_button();

            if button == gdk::BUTTON_PRIMARY {
                self.on_package_selected();
            } else if button == gdk::BUTTON_SECONDARY {
                let rect = gdk::Rectangle::new(x as i32, y as i32, 0, 0);

                self.pkgview_popover_menu.set_pointing_to(Some(&rect));
                self.pkgview_popover_menu.popup();
            }
        }

        //-----------------------------------
        // Infopane package display functions
        //-----------------------------------
        fn infopane_display_package(&self, pkg: Option<&PkgObject>) {
            let hlist = self.history_list.borrow();
            let hindex = self.history_index.get();

            self.infopane_toolbar.set_visible(pkg.is_some());

            self.infopane_navbutton_box.set_visible(hlist.len() > 1);

            self.infopane_prev_button.set_sensitive(hindex > 0);
            self.infopane_next_button.set_sensitive(hlist.len() > 0 && hindex < hlist.len() - 1);

            self.infopane_model.remove_all();

            if let Some(pkg) = pkg {
                let handle = self.alpm_handle.get().unwrap();

                let (required_by, optional_for) = pkg.compute_requirements(handle);

                // Name
                self.infopane_model.append(&PropObject::new(
                    "Name", &format!("<b>{}</b>", pkg.name()), None
                ));
                // Version
                self.infopane_model.append(&PropObject::new(
                    "Version", &pkg.version(), if pkg.has_update() {Some("pkg-update")} else {None}
                ));
                // Description
                self.infopane_model.append(&PropObject::new(
                    "Description", &self.prop_to_esc_string(&pkg.description()), None
                ));
                // Package/AUR URL
                if self.default_repo_names.borrow().contains(&pkg.repo_show()) {
                    self.infopane_model.append(&PropObject::new(
                        "Package URL", &self.prop_to_esc_url(&format!("https://www.archlinux.org/packages/{repo}/{arch}/{name}", repo=pkg.repo_show(), arch=pkg.architecture(), name=pkg.name())), None
                    ));
                } else if &pkg.repo_show() == "aur" {
                    self.infopane_model.append(&PropObject::new(
                        "AUR URL", &self.prop_to_esc_url(&format!("https://aur.archlinux.org/packages/{name}", name=pkg.name())), None
                    ));
                }
                // URL
                if pkg.url() != "" {
                    self.infopane_model.append(&PropObject::new(
                        "URL", &self.prop_to_esc_url(&pkg.url()), None
                    ));
                }
                // Licenses
                if pkg.licenses() != "" {
                    self.infopane_model.append(&PropObject::new(
                        "Licenses", &self.prop_to_esc_string(&pkg.licenses()), None
                    ));
                }
                // Status
                let status = &pkg.status();
                self.infopane_model.append(&PropObject::new(
                    "Status", if pkg.flags().intersects(PkgFlags::INSTALLED) {&status} else {"not installed"}, Some(&pkg.status_icon())
                ));
                // Repository
                self.infopane_model.append(&PropObject::new(
                    "Repository", &pkg.repo_show(), None
                ));
                // Groups
                if pkg.groups() != "" {
                    self.infopane_model.append(&PropObject::new(
                        "Groups", &pkg.groups(), None
                    ));
                }
                // Provides
                if !pkg.provides().is_empty() {
                    self.infopane_model.append(&PropObject::new(
                        "Provides", &self.propvec_to_wrapstring(&pkg.provides()), None
                    ));
                }
                // Depends
                self.infopane_model.append(&PropObject::new(
                    "Dependencies", &self.propvec_to_linkstring(&pkg.depends()), None
                ));
                // Optdepends
                if !pkg.optdepends().is_empty() {
                    self.infopane_model.append(&PropObject::new(
                        "Optional", &self.propvec_to_linkstring(&pkg.optdepends()), None
                    ));
                }
                // Required by
                self.infopane_model.append(&PropObject::new(
                    "Required by", &self.propvec_to_linkstring(&required_by), None
                ));
                // Optional for
                if !optional_for.is_empty() {
                    self.infopane_model.append(&PropObject::new(
                        "Optional For", &self.propvec_to_linkstring(&optional_for), None
                    ));
                }
                // Conflicts
                if !pkg.conflicts().is_empty() {
                    self.infopane_model.append(&PropObject::new(
                        "Conflicts With", &self.propvec_to_linkstring(&pkg.conflicts()), None
                    ));
                }
                // Replaces
                if !pkg.replaces().is_empty() {
                    self.infopane_model.append(&PropObject::new(
                        "Replaces", &self.propvec_to_linkstring(&pkg.replaces()), None
                    ));
                }
                // Architecture
                if pkg.architecture() != "" {
                    self.infopane_model.append(&PropObject::new(
                        "Architecture", &pkg.architecture(), None
                    ));
                }
                // Packager
                if pkg.packager() != "" {
                    self.infopane_model.append(&PropObject::new(
                        "Packager", &self.prop_to_packager(&pkg.packager()), None
                    ));
                }
                // Build date
                self.infopane_model.append(&PropObject::new(
                    "Build Date", &pkg.build_date_long(), None
                ));
                // Install date
                if pkg.install_date() != 0 {
                    self.infopane_model.append(&PropObject::new(
                        "Install Date", &pkg.install_date_long(), None
                    ));
                }
                // Download size
                if pkg.download_size() != 0 {
                    self.infopane_model.append(&PropObject::new(
                        "Download Size", &pkg.download_size_string(), None
                    ));
                }
                // Installed size
                self.infopane_model.append(&PropObject::new(
                    "Installed Size", &pkg.install_size_string(), None
                ));
                // Has script
                self.infopane_model.append(&PropObject::new(
                    "Install Script", if pkg.has_script() {"Yes"} else {"No"}, None
                ));
                // SHA256 sum
                if pkg.sha256sum() != "" {
                    self.infopane_model.append(&PropObject::new(
                        "SHA256 Sum", &pkg.sha256sum(), None
                    ));
                }
                // MD5 sum
                if pkg.md5sum() != "" {
                    self.infopane_model.append(&PropObject::new(
                        "MD5 Sum", &pkg.md5sum(), None
                    ));
                }
            }

            self.infopane_empty_label.set_visible(!pkg.is_some());
        }

        fn infopane_display_prev(&self) {
            let hlist = self.history_list.borrow();
            let mut hindex = self.history_index.get();

            if hindex > 0 {
                hindex -= 1;

                if let Some(pkg) = hlist.get(hindex) {
                    self.history_index.replace(hindex);

                    self.infopane_display_package(Some(pkg));
                }
            }
        }

        fn infopane_display_next(&self) {
            let hlist = self.history_list.borrow();
            let mut hindex = self.history_index.get();

            if hlist.len() > 0 && hindex < hlist.len() - 1 {
                hindex += 1;

                if let Some(pkg) = hlist.get(hindex) {
                    self.history_index.replace(hindex);

                    self.infopane_display_package(Some(pkg));
                }
            }
        }

        //-----------------------------------
        // Infopane helper functions
        //-----------------------------------
        fn prop_to_esc_string(&self, prop: &str) -> String {
            glib::markup_escape_text(prop).to_string()
        }

        fn prop_to_esc_url(&self, prop: &str) -> String {
            format!("<a href=\"{url}\">{url}</a>", url=glib::markup_escape_text(prop).to_string())
        }

        fn prop_to_packager(&self, prop: &str) -> String {
            lazy_static! {
                static ref MATCH: Regex = Regex::new("^([^<]+)<([^>]+)>$").unwrap();
                static ref EXPR: Regex = Regex::new("([^<]+)<?([^>]+)?>?").unwrap();
            }

            if MATCH.is_match(prop).unwrap_or_default() {
                EXPR.replace_all(&prop, "$1&lt;<a href='mailto:$2'>$2</a>&gt;").to_string()
            } else {
                prop.to_string()
            }
        }

        fn propvec_to_wrapstring(&self, prop_vec: &Vec<String>) -> String {
            glib::markup_escape_text(&prop_vec.join("   ")).to_string()
        }

        fn propvec_to_linkstring(&self, prop_vec: &Vec<String>) -> String {
            if prop_vec.is_empty() {
                String::from("None")
            } else {
                lazy_static! {
                    static ref EXPR: Regex = Regex::new("(^|   |   \n)([a-zA-Z0-9@._+-]+)(?=&gt;|&lt;|<|>|=|:|   |\n|$)").unwrap();
                }

                let prop_str = self.propvec_to_wrapstring(prop_vec);

                EXPR.replace_all(&prop_str, "$1<a href='pkg://$2'>$2</a>").to_string()
            }
        }

        //-----------------------------------
        // Infopane value factory signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_infopane_setup_value(&self, item: glib::Object) {
            let value_row = ValueRow::new();

            item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .set_child(Some(&value_row));
        }

        #[template_callback]
        fn on_infopane_bind_value(&self, item: glib::Object) {
            let prop_obj = item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .item()
                .and_downcast::<PropObject>()
                .expect("The item has to be a `PropObject`.");

            let value_row = item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .child()
                .and_downcast::<ValueRow>()
                .expect("The child has to be a `Box`.");

            value_row.bind_properties(&prop_obj);

            let label = &value_row.imp().label;

            let signal = label.connect_activate_link(clone!(@weak self as win => @default-return gtk::Inhibit(true), move |_, link| win.infopane_link_handler(link)));

            value_row.add_label_signal(signal);
        }

        #[template_callback]
        fn on_infopane_unbind_value(&self, item: glib::Object) {
            let value_row = item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .child()
                .and_downcast::<ValueRow>()
                .expect("The child has to be a `Box`.");

            value_row.unbind_properties();
            value_row.drop_label_signal();
        }

        //-----------------------------------
        // Infopane value label link handler
        //-----------------------------------
        fn infopane_link_handler(&self, link: &str) -> gtk::Inhibit {
            if let Ok(url) = Url::parse(link) {
                if url.scheme() == "pkg" {
                    if let Some(pkg_name) = url.domain() {
                        let pkg_list = self.package_list.borrow();

                        let mut new_pkg = pkg_list.iter().find(|pkg| pkg.name() == pkg_name);

                        if !new_pkg.is_some() {
                            new_pkg = pkg_list.iter().find(|pkg| {
                                pkg.provides().iter().any(|s| s.contains(&pkg_name))
                            });
                        }

                        if let Some(new_pkg) = new_pkg {
                            let hlist = self.history_list.borrow().to_vec();
                            let hindex = self.history_index.get();

                            let i = hlist.iter().position(|pkg| pkg.name() == new_pkg.name());

                            if let Some(i) = i {
                                if i != hindex {
                                    self.history_index.replace(i);

                                    self.infopane_display_package(Some(new_pkg));
                                }
                            } else {
                                let j = if hlist.len() > 0 {hindex + 1} else {hindex};
                                let mut hslice = hlist[..j].to_vec();

                                hslice.push(new_pkg.clone());

                                self.history_list.replace(hslice);
                                self.history_index.replace(j);

                                self.infopane_display_package(Some(new_pkg));
                            }
                        }
                    }

                    gtk::Inhibit(true)
                } else {
                    gtk::Inhibit(false)
                }
            } else {
                gtk::Inhibit(true)
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
