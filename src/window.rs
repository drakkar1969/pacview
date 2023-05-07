use std::cell::RefCell;

use gtk::{gio, glib};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use pacmanconf;
use alpm;
use titlecase;

use crate::PacViewApplication;
use crate::pkgobject::{PkgObject, PkgStatusFlags};
use crate::search_header::SearchHeader;
use crate::filter_row::FilterRow;

mod imp {
    use super::*;

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
        pub pkgview: TemplateChild<gtk::ColumnView>,
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
        pub infopane_overlay: TemplateChild<gtk::Overlay>,

        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,

        #[property(get, set)]
        pacman_root_dir: RefCell<String>,
        #[property(get, set)]
        pacman_db_path: RefCell<String>,
        #[property(get, set)]
        pacman_repo_names: RefCell<Vec<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PacViewWindow {
        const NAME: &'static str = "PacViewWindow";
        type Type = super::PacViewWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            PkgObject::static_type();
            SearchHeader::static_type();
            
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PacViewWindow {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }
    
        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_gactions();
            obj.setup_toolbar_buttons();
            obj.setup_pkgview();
        }
    }

    impl WidgetImpl for PacViewWindow {}
    impl WindowImpl for PacViewWindow {}
    impl ApplicationWindowImpl for PacViewWindow {}
    impl AdwApplicationWindowImpl for PacViewWindow {}

    #[gtk::template_callbacks]
    impl PacViewWindow {
        #[template_callback]
        fn on_show_window(&self) {
            let obj = self.obj();

            obj.get_pacman_config();
            obj.populate_sidebar();
            obj.load_packages();
        }

        #[template_callback]
        fn on_repo_selected(&self, row: Option<FilterRow>) {
            if let Some(r) = row {
                let obj = self.obj();

                if let Some(repo_id) = &r.repo_id() {
                    obj.repo_selected_handler(repo_id);
                }
            }
        }

        #[template_callback]
        fn on_status_selected(&self, row: Option<FilterRow>) {
            if let Some(r) = row {
                let obj = self.obj();

                obj.status_selected_handler(r.status_id());
            }
        }

        #[template_callback]
        fn on_search_changed(&self, term: &str) {
            let obj = self.obj();

            obj.search_changed_handler(term);
        }
    }
}

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

    fn setup_gactions(&self) {
        let search_group = gio::SimpleActionGroup::new();

        self.insert_action_group("search", Some(&search_group));

        let win = self;

        let search_start_action = gio::SimpleAction::new("start-search", None);
        search_start_action.connect_activate(clone!(@weak win => move |_, _| {
            let imp = win.imp();
    
            imp.search_header.set_search_active(true);
        }));
        search_group.add_action(&search_start_action);

        let search_stop_action = gio::SimpleAction::new("stop-search", None);
        search_stop_action.connect_activate(clone!(@weak win => move |_, _| {
            let imp = win.imp();
    
            imp.search_header.set_search_active(false);
        }));
        search_group.add_action(&search_stop_action);

        let prop_map = ["name", "group"];

        for prop in prop_map {
            let imp = self.imp();

            let action_name = format!("search-by-{}", prop);

            let action = gio::PropertyAction::new(&action_name, &imp.search_header.get(), &action_name);
            search_group.add_action(&action);
        }
    }

    fn setup_toolbar_buttons(&self) {
        let imp = self.imp();

        let show_sidebar_action = gio::PropertyAction::new("show-sidebar", &imp.flap.get(), "reveal-flap");
        self.add_action(&show_sidebar_action);

        let show_infopane_action = gio::PropertyAction::new("show-infopane", &imp.infopane_overlay.get(), "visible");
        self.add_action(&show_infopane_action);

        imp.search_button.bind_property("active", &imp.search_header.get(), "search-active")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();
    }

    fn setup_pkgview(&self) {
        let imp = self.imp();

        imp.pkgview_filter_model.bind_property("n-items", &imp.status_label.get(), "label")
            .transform_to(|_, n_items: u32| {
                Some(format!("{} matching package{}", n_items, if n_items != 1 {"s"} else {""}))
            })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        let sort_column = imp.pkgview.columns().item(0);

        imp.pkgview.sort_by_column(sort_column.and_downcast_ref(), gtk::SortType::Ascending);

        imp.pkgview.grab_focus();
    }

    fn get_pacman_config(&self) {
        let pacman_config = pacmanconf::Config::new().unwrap();

        let repo_list: Vec<String> = pacman_config.repos.iter().map(|r| r.name.to_string()).collect();

        self.set_pacman_root_dir(pacman_config.root_dir);
        self.set_pacman_db_path(pacman_config.db_path);
        self.set_pacman_repo_names(repo_list.clone());
    }

    fn populate_sidebar(&self) {
        let imp = self.imp();

        let row = FilterRow::new("repository-symbolic", "All");
        row.set_repo_id("");
        imp.repo_listbox.append(&row);

        imp.repo_listbox.select_row(Some(&row));

        for repo in self.pacman_repo_names() {
            let row = FilterRow::new("repository-symbolic", &titlecase::titlecase(&repo));
            row.set_repo_id(repo.to_lowercase());
            imp.repo_listbox.append(&row);
        }

        let status_map = [
            ("status-all-symbolic", "All", PkgStatusFlags::ALL),
            ("status-installed-symbolic", "Installed", PkgStatusFlags::INSTALLED),
            ("status-explicit-symbolic", "Explicit", PkgStatusFlags::EXPLICIT),
            ("status-dependency-symbolic", "Dependency", PkgStatusFlags::DEPENDENCY),
            ("status-optional-symbolic", "Optional", PkgStatusFlags::OPTIONAL),
            ("status-orphan-symbolic", "Orphan", PkgStatusFlags::ORPHAN),
            ("status-none-symbolic", "None", PkgStatusFlags::NONE),
            ("status-updates-symbolic", "Updates", PkgStatusFlags::UPDATES),
        ];

        for status in status_map {
            let row = FilterRow::new(status.0, status.1);
            row.set_status_id(status.2);
            imp.status_listbox.append(&row);

            if status.2 == PkgStatusFlags::INSTALLED {
                imp.status_listbox.select_row(Some(&row));
            }
        }
    }

    fn load_packages(&self) {
        let handle = alpm::Alpm::new(self.pacman_root_dir(), self.pacman_db_path()).unwrap();

        let mut obj_list: Vec<PkgObject> = Vec::new();

        let localdb = handle.localdb();

        for repo in self.pacman_repo_names() {
            let db = handle.register_syncdb(repo, alpm::SigLevel::DATABASE_OPTIONAL).unwrap();

            for syncpkg in db.pkgs() {
                let localpkg = localdb.pkgs().find_satisfier(syncpkg.name());

                let obj = PkgObject::new(db.name(), syncpkg, localpkg);

                obj_list.push(obj);
            }
        }

        let imp = self.imp();
        imp.pkgview_model.extend_from_slice(&obj_list);
    }

    fn repo_selected_handler(&self, repo: &str) {
        let imp = self.imp();

        imp.pkgview_repo_filter.set_search(Some(repo));
    }

    fn status_selected_handler(&self, status: PkgStatusFlags) {
        let imp = self.imp();

        imp.pkgview_status_filter.set_filter_func(move |item| {
            let obj: &PkgObject = item
                .downcast_ref::<PkgObject>()
                .expect("Needs to be a PkgObject");

            obj.flags().intersects(status)
        });
    }

    fn search_changed_handler(&self, term: &str) {
        let imp = self.imp();

        let search_term = String::from(term);

        if search_term == "" {
            imp.pkgview_search_filter.unset_filter_func();
        } else {
            let by_name = imp.search_header.search_by_name();
            let by_group = imp.search_header.search_by_group();

            imp.pkgview_search_filter.set_filter_func(move |item| {
                let obj: &PkgObject = item
                    .downcast_ref::<PkgObject>()
                    .expect("Needs to be a PkgObject");

                let mut name_ok = false;
                let mut group_ok = false;
    
                if by_name {
                    if let Some(name) = obj.name() {
                        name_ok = name.contains(&search_term);
                    }
                }

                if by_group {
                    if let Some(group) = obj.groups() {
                        group_ok = group.contains(&search_term);
                    }
                }
    
                name_ok | group_ok
            });
        }
    }
}
