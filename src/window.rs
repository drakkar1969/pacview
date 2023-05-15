use std::cell::{Cell, RefCell};
use std::thread;
use std::process::Command;
use std::collections::HashMap;
use std::borrow::Borrow;

use gtk::{gio, glib};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use pacmanconf;
use alpm;
use titlecase;
use fancy_regex::Regex;
use lazy_static::lazy_static;
use url::Url;

use crate::PacViewApplication;
use crate::pkg_object::{PkgObject, PkgData, PkgFlags};
use crate::prop_object::PropObject;
use crate::search_header::SearchHeader;
use crate::filter_row::FilterRow;
use crate::value_row::ValueRow;

//------------------------------------------------------------------------------
// MODULE: PACVIEWWINDOW
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
        pub pkgview_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub pkgview: TemplateChild<gtk::ColumnView>,
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
        pub infopane_overlay: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub infopane_model: TemplateChild<gio::ListStore>,

        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,

        #[property(get, set)]
        update_row: RefCell<FilterRow>,

        #[property(get, set)]
        pacman_root_dir: RefCell<String>,
        #[property(get, set)]
        pacman_db_path: RefCell<String>,
        #[property(get, set)]
        pacman_repo_names: RefCell<Vec<String>>,
        #[property(get, set)]
        default_repo_names: RefCell<Vec<String>>,

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

            self.setup_search();
            self.setup_toolbar();
            self.setup_pkgview();
            self.setup_infopane();
        }
    }

    impl WidgetImpl for PacViewWindow {}
    impl WindowImpl for PacViewWindow {}
    impl ApplicationWindowImpl for PacViewWindow {}
    impl AdwApplicationWindowImpl for PacViewWindow {}

    #[gtk::template_callbacks]
    impl PacViewWindow {
        //-----------------------------------
        // Setup search header
        //-----------------------------------
        fn setup_search(&self) {
            let obj = self.obj();

            // Create search action group
            let search_group = gio::SimpleActionGroup::new();

            obj.insert_action_group("search", Some(&search_group));

            // Create actions to start/stop search
            let search_start_action = gio::SimpleAction::new("start", None);
            search_start_action.connect_activate(clone!(@weak self as window => move |_, _| {
                window.search_header.set_active(true);
            }));
            search_group.add_action(&search_start_action);

            let search_stop_action = gio::SimpleAction::new("stop", None);
            search_stop_action.connect_activate(clone!(@weak self as window => move |_, _| {
                window.search_header.set_active(false);
            }));
            search_group.add_action(&search_stop_action);

            // Create actions for search header search by properties
            let prop_array = ["name", "desc", "group", "deps", "optdeps", "provides", "files"];

            for prop in prop_array {
                let action_name = format!("toggle-{}", prop);
                let prop_name = format!("by-{}", prop);

                let action = gio::PropertyAction::new(&action_name, &self.search_header.get(), &prop_name);
                search_group.add_action(&action);
            }

            // Create actions to select all/reset search header search by properties
            let selectall_action = gio::SimpleAction::new("selectall", None);
            selectall_action.connect_activate(clone!(@weak self as window => move |_, _| {
                for prop in prop_array {
                    let prop_name = format!("by-{}", prop);

                    window.search_header.set_property(&prop_name, true);
                }
            }));
            search_group.add_action(&selectall_action);

            let reset_action = gio::SimpleAction::new("reset", None);
            reset_action.connect_activate(clone!(@weak self as window => move |_, _| {
                for prop in prop_array {
                    let prop_name = format!("by-{}", prop);

                    window.search_header.set_property(&prop_name, prop == "name");
                }
            }));
            search_group.add_action(&reset_action);

            // Create action for search header search exact property
            let action = gio::PropertyAction::new("toggle-exact", &self.search_header.get(), "exact");
            search_group.add_action(&action);
        }

        //-----------------------------------
        // Setup toolbar buttons
        //-----------------------------------
        fn setup_toolbar(&self) {
            let obj = self.obj();

            // Add sidebar/infopane visibility actions
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

            // Create view action group
            let pkgview_group = gio::SimpleActionGroup::new();

            obj.insert_action_group("view", Some(&pkgview_group));

            // Create pkgview refresh action
            let refresh_action = gio::SimpleAction::new("refresh", None);
            refresh_action.connect_activate(clone!(@weak self as window => move |_, _| {
                window.search_header.set_active(false);
                
                window.on_show_window();
            }));
            pkgview_group.add_action(&refresh_action);

            // Create pkgview copy list action
            let window = self;

            let copy_action = gio::SimpleAction::new("copy-list", None);
            copy_action.connect_activate(clone!(@weak window => move |_, _| {
                let item_list: Vec<String> = IntoIterator::into_iter(0..window.pkgview_selection.n_items())
                    .map(|i| {
                        let obj: PkgObject = window.pkgview_selection.item(i).and_downcast().expect("Must be a PkgObject");

                        format!("{repo}/{name}-{version}", repo=obj.repository(), name=obj.name(), version=obj.version())
                    }
                ).collect();

                let copy_text = item_list.join("\n");

                let clipboard = window.obj().clipboard();

                clipboard.set_text(&copy_text);
            }));
            pkgview_group.add_action(&copy_action);

            // Set pkgview sorting
            let sort_column = self.pkgview.columns().item(0);

            self.pkgview.sort_by_column(sort_column.and_downcast_ref(), gtk::SortType::Ascending);

            // Set initial focus on pkgview
            self.pkgview.grab_focus();
        }

        //-----------------------------------
        // Setup info pane
        //-----------------------------------
        fn setup_infopane(&self) {
            let obj = self.obj();

            // Create info pane action group
            let infopane_group = gio::SimpleActionGroup::new();

            obj.insert_action_group("info", Some(&infopane_group));

            // Create info pane prev/next actions
            let prev_action = gio::SimpleAction::new("previous", None);
            prev_action.connect_activate(clone!(@weak self as window => move |_, _| {
                window.infopane_display_prev();
            }));
            infopane_group.add_action(&prev_action);

            let next_action = gio::SimpleAction::new("next", None);
            next_action.connect_activate(clone!(@weak self as window => move |_, _| {
                window.infopane_display_next();
            }));
            infopane_group.add_action(&next_action);
        }

        //-----------------------------------
        // Show window signal handler
        //-----------------------------------
        #[template_callback]
        fn on_show_window(&self) {
            self.get_pacman_config();
            self.populate_sidebar();
            self.load_packages_async();
        }

        //-----------------------------------
        // On show: get pacman configuration
        //-----------------------------------
        fn get_pacman_config(&self) {
            let pacman_config = pacmanconf::Config::new().unwrap();

            let mut repo_list: Vec<String> = pacman_config.repos.iter().map(|r| r.name.to_string()).collect();
            repo_list.push(String::from("foreign"));

            let obj = self.obj();

            obj.set_pacman_root_dir(pacman_config.root_dir);
            obj.set_pacman_db_path(pacman_config.db_path);
            obj.set_pacman_repo_names(repo_list);

            let default_repo_names: Vec<String> = vec![String::from("core"), String::from("extra"), String::from("community"), String::from("multilib")];

            obj.set_default_repo_names(&default_repo_names);
        }

        //-----------------------------------
        // On show: populate sidebar listboxes
        //-----------------------------------
        fn populate_sidebar(&self) {
            let obj = self.obj();

            // Clear sidebar rows
            while let Some(row) = self.repo_listbox.row_at_index(0) {
                self.repo_listbox.remove(&row);
            }

            while let Some(row) = self.status_listbox.row_at_index(0) {
                self.status_listbox.remove(&row);
            }

            // Repository rows
            let row = FilterRow::new("repository-symbolic", "All");
            row.set_repo_id("");

            self.repo_listbox.append(&row);

            self.repo_listbox.select_row(Some(&row));

            for repo in obj.pacman_repo_names() {
                let row = FilterRow::new("repository-symbolic", &titlecase::titlecase(&repo));
                row.set_repo_id(&repo.to_lowercase());

                self.repo_listbox.append(&row);
            }

            // Package status rows (enumerate PkgStatusFlags)
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

            for status in status_map {
                let row = FilterRow::new(&format!("status-{}-symbolic", status.0), &titlecase::titlecase(status.0));
                row.set_status_id(status.1);

                self.status_listbox.append(&row);

                if status.1 == PkgFlags::INSTALLED {
                    self.status_listbox.select_row(Some(&row));
                }

                if status.1 == PkgFlags::UPDATES {
                    row.set_spinning(true);
                    row.set_sensitive(false);

                    obj.set_update_row(row);
                }
            }
        }

        //-----------------------------------
        // On show: load alpm packages
        //-----------------------------------
        fn load_packages_async(&self) {
            let obj = self.obj();

            let (sender, receiver) = glib::MainContext::channel::<Vec<PkgData>>(glib::PRIORITY_DEFAULT);

            let root_dir = obj.pacman_root_dir();
            let db_path = obj.pacman_db_path();
            let repo_names = obj.pacman_repo_names();

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

                sender.send(data_list).expect("Could not send through channel");
            });

            let window = self;

            receiver.attach(
                None,
                clone!(@weak window => @default-return Continue(false), move |data_list| {
                    let obj_list: Vec<PkgObject> = data_list.into_iter().map(|data| {
                        let obj = PkgObject::new();
                        obj.set_data(data);
                        obj
                    }).collect();

                    window.package_list.replace(obj_list.clone());

                    window.pkgview_model.splice(0, window.pkgview_model.n_items(), &obj_list);

                    window.pkgview_stack.set_visible_child_name("view");

                    window.get_package_updates_async();

                    Continue(false)
                }),
            );
        }

        //-----------------------------------
        // On show: get package updates
        //-----------------------------------
        fn get_package_updates_async(&self) {
            pub struct UpdateResult {
                success: bool,
                map: HashMap<String, String>,
            }
    
            let (sender, receiver) = glib::MainContext::channel::<UpdateResult>(glib::PRIORITY_DEFAULT);

            thread::spawn(move || {
                let mut update_result = UpdateResult { success: false, map: HashMap::new() };

                if let Ok(output) = Command::new("checkupdates").output() {
                    if output.status.code() == Some(0) || output.status.code() == Some(2) {
                        update_result.success = true;
                    }

                    if update_result.success {
                        lazy_static! {
                            static ref EXPR: Regex = Regex::new("(\\S+) (\\S+ -> \\S+)").unwrap();
                        }

                        let stdout = String::from_utf8(output.stdout).unwrap_or_default();

                        for update in stdout.split_terminator("\n") {
                            if EXPR.is_match(update).unwrap_or_default() {
                                update_result.map.insert(EXPR.replace_all(&update, "$1").to_string(), EXPR.replace_all(&update, "$2").to_string());
                            }
                        }
                    }
                }

                sender.send(update_result).expect("Could not send through channel");
            });

            let obj_list = self.package_list.borrow().to_vec();
            let update_row: FilterRow = self.obj().update_row().clone();

            receiver.attach(
                None,
                clone!(@strong obj_list, @strong update_row => @default-return Continue(false), move |result| {
                    if result.success == true && result.map.len() > 0 {
                        let update_list = obj_list.iter()
                            .filter(|obj| result.map.contains_key(&obj.name()));

                        for obj in update_list {
                            let version = result.map.get(&obj.name());
    
                            if let Some(version) = version {
                                obj.set_version(version.borrow());
    
                                let mut flags = obj.flags();
                                flags.set(PkgFlags::UPDATES, true);
    
                                obj.set_flags(flags);

                                obj.set_has_update(true);
                            }
                        }
                    }

                    update_row.set_spinning(false);
                    update_row.set_icon(if result.success {"status-updates-symbolic"} else {"status-updates-error-symbolic"});
                    update_row.set_count(if result.success && result.map.len() > 0 {result.map.len().to_string()} else {String::from("")});

                    update_row.set_tooltip_text(if result.success {Some("")} else {Some("Update error")});

                    update_row.set_sensitive(result.success);

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
                    let obj: &PkgObject = item
                        .downcast_ref::<PkgObject>()
                        .expect("Needs to be a PkgObject");

                    obj.flags().intersects(row.status_id())
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
                    app.set_accels_for_action("search.toggle-exact", &["<ctrl>E"]);

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
                    app.set_accels_for_action("search.toggle-exact", &[]);

                    app.set_accels_for_action("search.selectall", &[]);
                    app.set_accels_for_action("search.reset", &[]);
                }
            }
        }

        #[template_callback]
        fn on_search_changed(&self, term: &str, by_name: bool, by_desc: bool, by_group: bool, by_deps: bool, by_optdeps: bool, by_provides: bool, by_files: bool, exact: bool) {
            let search_term = term.to_lowercase();

            if search_term == "" {
                self.pkgview_search_filter.unset_filter_func();
            } else {
                if exact {
                    self.pkgview_search_filter.set_filter_func(move |item| {
                        let obj: &PkgObject = item
                            .downcast_ref::<PkgObject>()
                            .expect("Needs to be a PkgObject");

                        let results = [
                            by_name && obj.name().to_lowercase().eq(&search_term),
                            by_desc && obj.description().to_lowercase().eq(&search_term),
                            by_group && obj.groups().to_lowercase().eq(&search_term),
                            by_deps && obj.depends().iter().any(|s| s.to_lowercase().eq(&search_term)),
                            by_optdeps && obj.optdepends().iter().any(|s| s.to_lowercase().eq(&search_term)),
                            by_provides && obj.provides().iter().any(|s| s.to_lowercase().eq(&search_term)),
                            by_files && obj.files().iter().any(|s| s.to_lowercase().eq(&search_term)),
                        ];

                        results.into_iter().any(|x| x)
                    });
                } else {
                    self.pkgview_search_filter.set_filter_func(move |item| {
                        let obj: &PkgObject = item
                            .downcast_ref::<PkgObject>()
                            .expect("Needs to be a PkgObject");

                        let mut results = vec![];

                        for term in search_term.split_whitespace() {
                            let term_results = [
                                by_name && obj.name().to_lowercase().contains(&term),
                                by_desc && obj.description().to_lowercase().contains(&term),
                                by_group && obj.groups().to_lowercase().contains(&term),
                                by_deps && obj.depends().iter().any(|s| s.to_lowercase().contains(&term)),
                                by_optdeps && obj.optdepends().iter().any(|s| s.to_lowercase().contains(&term)),
                                by_provides && obj.provides().iter().any(|s| s.to_lowercase().contains(&term)),
                                by_files && obj.files().iter().any(|s| s.to_lowercase().contains(&term)),
                            ];

                            results.push(term_results.into_iter().any(|x| x));
                        }

                        results.into_iter().all(|x| x)
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
                let obj = item.downcast::<PkgObject>().expect("Must be a PkgObject");
                self.infopane_display_package(Some(&obj));

                self.history_list.replace(vec![obj]);
                self.history_index.replace(0);
            } else {
                self.infopane_display_package(None);

                self.history_list.replace(vec![]);
                self.history_index.replace(0);
            }
        }

        //-----------------------------------
        // Infopane package display functions
        //-----------------------------------
        fn infopane_display_package(&self, obj: Option<&PkgObject>) {
            self.infopane_model.remove_all();

            if let Some(obj) = obj {
                // Name
                self.infopane_model.append(&PropObject::new(
                    "Name", &format!("<b>{}</b>", obj.name()), None
                ));
                // Version
                self.infopane_model.append(&PropObject::new(
                    "Version", &obj.version(), if obj.has_update() {Some("pkg-update")} else {None}
                ));
                // Description
                self.infopane_model.append(&PropObject::new(
                    "Description", &self.prop_to_esc_string(&obj.description()), None
                ));
                // Package/AUR URL
                if self.obj().default_repo_names().contains(&obj.repository()) {
                    self.infopane_model.append(&PropObject::new(
                        "Package URL", &self.prop_to_esc_url(&format!("https://www.archlinux.org/packages/{repo}/{arch}/{name}", repo=obj.repository(), arch=obj.architecture(), name=obj.name())), None
                    ));
                }
                // URL
                if obj.url() != "" {
                    self.infopane_model.append(&PropObject::new(
                        "URL", &self.prop_to_esc_url(&obj.url()), None
                    ));
                }
                // Licenses
                if obj.licenses() != "" {
                    self.infopane_model.append(&PropObject::new(
                        "Licenses", &self.prop_to_esc_string(&obj.licenses()), None
                    ));
                }
                // Status
                let status = &obj.status();
                self.infopane_model.append(&PropObject::new(
                    "Status", if obj.flags().intersects(PkgFlags::INSTALLED) {&status} else {"not installed"}, Some(&obj.status_icon())
                ));
                // Repository
                self.infopane_model.append(&PropObject::new(
                    "Repository", &obj.repository(), None
                ));
                // Groups
                if obj.groups() != "" {
                    self.infopane_model.append(&PropObject::new(
                        "Groups", &obj.groups(), None
                    ));
                }
                // Provides
                if !obj.provides().is_empty() {
                    self.infopane_model.append(&PropObject::new(
                        "Provides", &self.propvec_to_wrapstring(&obj.provides()), None
                    ));
                }
                // Depends
                self.infopane_model.append(&PropObject::new(
                    "Dependencies", &self.propvec_to_linkstring(&obj.depends()), None
                ));
                // Optdepends
                if !obj.optdepends().is_empty() {
                    self.infopane_model.append(&PropObject::new(
                        "Optional", &self.propvec_to_linkstring(&obj.optdepends()), None
                    ));
                }
                // Conflicts
                if !obj.conflicts().is_empty() {
                    self.infopane_model.append(&PropObject::new(
                        "Conflicts With", &self.propvec_to_linkstring(&obj.conflicts()), None
                    ));
                }
                // Replaces
                if !obj.replaces().is_empty() {
                    self.infopane_model.append(&PropObject::new(
                        "Replaces", &self.propvec_to_linkstring(&obj.replaces()), None
                    ));
                }
                // Architecture
                if obj.architecture() != "" {
                    self.infopane_model.append(&PropObject::new(
                        "Architecture", &obj.architecture(), None
                    ));
                }
                // Packager
                if obj.packager() != "" {
                    self.infopane_model.append(&PropObject::new(
                        "Packager", &self.prop_to_packager(&obj.packager()), None
                    ));
                }
                // Build date
                self.infopane_model.append(&PropObject::new(
                    "Build Date", &obj.build_date_long(), None
                ));
                // Install date
                if obj.install_date() != 0 {
                    self.infopane_model.append(&PropObject::new(
                        "Install Date", &obj.install_date_long(), None
                    ));
                }
                // Download size
                if obj.download_size() != 0 {
                    self.infopane_model.append(&PropObject::new(
                        "Download Size", &obj.download_size_string(), None
                    ));
                }
                // Installed size
                self.infopane_model.append(&PropObject::new(
                    "Installed Size", &obj.install_size_string(), None
                ));
                // Has script
                self.infopane_model.append(&PropObject::new(
                    "Install Script", if obj.has_script() {"Yes"} else {"No"}, None
                ));
                // SHA256 sum
                if obj.sha256sum() != "" {
                    self.infopane_model.append(&PropObject::new(
                        "SHA256 Sum", &obj.sha256sum(), None
                    ));
                }
                // MD5 sum
                if obj.md5sum() != "" {
                    self.infopane_model.append(&PropObject::new(
                        "MD5 Sum", &obj.md5sum(), None
                    ));
                }
            }
        }

        fn infopane_display_prev(&self) {
            let hlist = self.history_list.borrow().to_vec();
            let mut hindex = self.history_index.get();

            if hindex > 0 {
                hindex -= 1;

                if let Some(obj) = hlist.get(hindex) {
                    self.history_index.replace(hindex);

                    self.infopane_display_package(Some(obj));
                }
            }
        }

        fn infopane_display_next(&self) {
            let hlist = self.history_list.borrow().to_vec();
            let mut hindex = self.history_index.get();

            if hindex < hlist.len() - 1 {
                hindex += 1;

                if let Some(obj) = hlist.get(hindex) {
                    self.history_index.replace(hindex);

                    self.infopane_display_package(Some(obj));
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
            format!("<a href=\"{0}\">{0}</a>", glib::markup_escape_text(prop).to_string())
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

            let signal = label.connect_activate_link(clone!(@weak self as window => @default-return gtk::Inhibit(true), move |_, link| window.infopane_link_handler(link)));

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
            value_row.drop_label_signals();
        }

        //-----------------------------------
        // Infopane value label link handler
        //-----------------------------------
        fn infopane_link_handler(&self, link: &str) -> gtk::Inhibit {
            if let Ok(url) = Url::parse(link) {
                if url.scheme() == "pkg" {
                    if let Some(pkg_name) = url.domain() {
                        let mut new_obj: Option<&PkgObject> = None;

                        let obj_list = self.package_list.borrow().to_vec();

                        let new_obj_list: Vec<&PkgObject> = obj_list.iter()
                            .filter(|obj| obj.name() == pkg_name)
                            .collect();

                        if new_obj_list.len() > 0 {
                            new_obj = Some(new_obj_list[0]);
                        } else {
                            let new_obj_list: Vec<&PkgObject> = obj_list.iter()
                                .filter(|obj| obj.provides().iter().any(|s| s.to_lowercase().contains(&pkg_name)))
                                .collect();

                            if new_obj_list.len() > 0 {
                                new_obj = Some(new_obj_list[0]);
                            }
                        }

                        if let Some(new_obj) = new_obj {
                            let hlist = self.history_list.borrow().to_vec();
                            let hindex = self.history_index.get();

                            let i = hlist.iter().position(|obj| obj.name() == new_obj.name());

                            if let Some(i) = i {
                                if i != hindex {
                                    self.history_index.replace(i);

                                    self.infopane_display_package(Some(new_obj));
                                }
                            } else {
                                let j = if hlist.len() > 0 {hindex + 1} else {hindex};
                                let mut hslice = hlist[..j].to_vec();

                                hslice.push(new_obj.clone());

                                self.history_list.replace(hslice);
                                self.history_index.replace(j);

                                self.infopane_display_package(Some(new_obj));
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
// PUBLIC IMPLEMENTATION
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PacViewWindow(ObjectSubclass<imp::PacViewWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl PacViewWindow {
    pub fn new(app: &PacViewApplication) -> Self {
        glib::Object::builder().property("application", app).build()
    }
}
