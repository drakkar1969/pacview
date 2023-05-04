use gtk::{gio, glib};
use adw::subclass::prelude::*;
use gtk::prelude::StaticType;

use alpm::{Alpm, SigLevel, PackageReason};
use bytesize;

use crate::PacViewApplication;
use crate::pkgobject::{PkgStatusFlags, PkgObject};

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
        let mut obj_list: Vec<PkgObject> = Vec::new();

        let handle = Alpm::new("/", "/var/lib/pacman/").unwrap();

        handle.register_syncdb("core", SigLevel::DATABASE_OPTIONAL).unwrap();
        handle.register_syncdb("extra", SigLevel::DATABASE_OPTIONAL).unwrap();
        handle.register_syncdb("community", SigLevel::DATABASE_OPTIONAL).unwrap();
        handle.register_syncdb("custom", SigLevel::DATABASE_OPTIONAL).unwrap();

        let localdb = handle.localdb();

        for db in handle.syncdbs() {
            for pkg in db.pkgs() {
                let obj = PkgObject::new();

                if let Some(localpkg) = localdb.pkgs().find_satisfier(pkg.name()) {
                    if localpkg.reason() == PackageReason::Explicit {
                        obj.set_flags(PkgStatusFlags::EXPLICIT);
                        obj.set_status("explicit");
                        obj.set_status_icon("pkg-explicit");
                    } else {
                        if !localpkg.required_by().is_empty() {
                            obj.set_flags(PkgStatusFlags::DEPENDENCY);
                            obj.set_status("dependency");
                            obj.set_status_icon("pkg-dependency");
                        } else {
                            if !localpkg.optional_for().is_empty() {
                                obj.set_flags(PkgStatusFlags::OPTIONAL);
                                obj.set_status("optional");
                                obj.set_status_icon("pkg-optional");
                            } else {
                                obj.set_flags(PkgStatusFlags::ORPHAN);
                                obj.set_status("orphan");
                                obj.set_status_icon("pkg-orphan");
                            }
                        }
                    }

                    if let Some(install_date) = localpkg.install_date() {
                        let datetime = glib::DateTime::from_unix_local(install_date).expect("error");

                        let s = datetime.format("%Y/%m/%d %H:%M").expect("error");
    
                        obj.set_install_date(install_date);
                        obj.set_install_date_short(s);
                    }
                }

                obj.set_name(pkg.name());
                obj.set_version(pkg.version().as_str());
                obj.set_repository(db.name());

                obj.set_install_size(pkg.isize());

                let s = bytesize::to_string(pkg.isize() as u64, true);

                obj.set_install_size_string(s);

                let mut groups: Vec<&str> = pkg.groups().iter().collect();

                groups.sort_unstable();

                obj.set_groups(groups.join(", "));

                obj_list.push(obj);
            }
        }

        let model = &self.imp().pkgview_model;
        model.extend_from_slice(&obj_list);
    }
}
