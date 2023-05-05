use gtk::{gio, glib};
use adw::subclass::prelude::*;
use gtk::prelude::*;

use alpm;

use crate::PacViewApplication;
use crate::pkgobject::{PkgObject, PkgStatusFlags};
use crate::filter_row::FilterRow;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/window.ui")]
    pub struct PacViewWindow {
        #[template_child]
        pub repo_listbox: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub status_listbox: TemplateChild<gtk::ListBox>,

        #[template_child]
        pub pkgview: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub pkgview_filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub pkgview_repo_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub pkgview_status_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub pkgview_model: TemplateChild<gio::ListStore>,

        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PacViewWindow {
        const NAME: &'static str = "PacViewWindow";
        type Type = super::PacViewWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            PkgObject::static_type();
            
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl PacViewWindow {
        #[template_callback]
        fn on_show_window(&self) {
            let obj = self.obj();

            obj.populate_sidebar();
            obj.load_packages();
        }

        #[template_callback]
        fn on_repo_selected(&self, row: Option<FilterRow>) {
            if let Some(r) = row {
                let obj = self.obj();

                if let Some(repo_id) = &r.repo_id() {
                    obj.set_pkg_repo_filter(repo_id);
                }
            }
        }

        #[template_callback]
        fn on_status_selected(&self, row: Option<FilterRow>) {
            if let Some(r) = row {
                let obj = self.obj();

                obj.set_pkg_status_filter(r.status_id());
            }
        }
    }

    impl ObjectImpl for PacViewWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_pkgview();
        }
    }

    impl WidgetImpl for PacViewWindow {}
    impl WindowImpl for PacViewWindow {}
    impl ApplicationWindowImpl for PacViewWindow {}
    impl AdwApplicationWindowImpl for PacViewWindow {}
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

    fn set_pkg_repo_filter(&self, repo: &str) {
        let imp = &self.imp();

        imp.pkgview_repo_filter.set_search(Some(repo));
    }

    fn set_pkg_status_filter(&self, status: PkgStatusFlags) {
        let imp = &self.imp();

        imp.pkgview_status_filter.set_filter_func(move |item| {
            let pkg: &PkgObject = item
                .downcast_ref::<PkgObject>()
                .expect("Needs to be a PkgObject");

            pkg.flags().intersects(status)
        });
    }

    fn setup_pkgview(&self) {
        let imp = &self.imp();

        imp.pkgview_filter_model.bind_property("n-items", &imp.status_label.get(), "label")
            .transform_to(|_, n_items: u32| {
                Some(format!("{} matching package{}", n_items, if n_items != 1 {"s"} else {""}))
            })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        let sort_column = imp.pkgview.columns().item(0);

        imp.pkgview.sort_by_column(sort_column.and_downcast_ref(), gtk::SortType::Ascending);
    }

    fn populate_sidebar(&self) {
        let imp = &self.imp();

        let row = FilterRow::new("repository-symbolic", "All");
        imp.repo_listbox.append(&row);

        imp.repo_listbox.select_row(Some(&row));

        for s in ["Core", "Extra", "Community", "Custom"] {
            let row = FilterRow::new("repository-symbolic", s);
            row.set_repo_id(s.to_lowercase());
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
        let handle = alpm::Alpm::new("/", "/var/lib/pacman/").unwrap();

        handle.register_syncdb("core", alpm::SigLevel::DATABASE_OPTIONAL).unwrap();
        handle.register_syncdb("extra", alpm::SigLevel::DATABASE_OPTIONAL).unwrap();
        handle.register_syncdb("community", alpm::SigLevel::DATABASE_OPTIONAL).unwrap();
        handle.register_syncdb("custom", alpm::SigLevel::DATABASE_OPTIONAL).unwrap();

        let localdb = handle.localdb();

        let mut obj_list: Vec<PkgObject> = Vec::new();

        for db in handle.syncdbs() {
            for syncpkg in db.pkgs() {
                let localpkg = localdb.pkgs().find_satisfier(syncpkg.name());

                let obj = PkgObject::new(db.name(), syncpkg, localpkg);

                obj_list.push(obj);
            }
        }

        let model = &self.imp().pkgview_model;
        model.extend_from_slice(&obj_list);
    }
}
