use gtk::{gio, glib};
use adw::subclass::prelude::*;
use gtk::prelude::StaticType;

use alpm;

use crate::PacViewApplication;
use crate::pkgobject::PkgObject;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/window.ui")]
    pub struct PacViewWindow {
        #[template_child]
        pub pkgview_model: TemplateChild<gio::ListStore>,
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
            obj.load_packages();
        }
    }

    impl ObjectImpl for PacViewWindow {}
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
