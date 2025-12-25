use gtk::subclass::prelude::*;
use gtk::prelude::{GObjectPropertyExpressionExt, WidgetExt};
use gtk::glib;
use glib::closure;

use crate::pkg_data::PkgFlags;
use crate::pkg_object::PkgObject;

//------------------------------------------------------------------------------
// MODULE: PackageItem
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/package_item.ui")]
    pub struct PackageItem {
        #[template_child]
        pub(super) name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) version_image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) version_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) status_image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) repository_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) size_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) groups_image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) groups_label: TemplateChild<gtk::Label>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PackageItem {
        const NAME: &'static str = "PackageItem";
        type Type = super::PackageItem;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PackageItem {}
    impl WidgetImpl for PackageItem {}
    impl BoxImpl for PackageItem {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PackageItem
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PackageItem(ObjectSubclass<imp::PackageItem>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PackageItem {
    //---------------------------------------
    // Bind function
    //---------------------------------------
    pub fn bind(&self, pkg: &PkgObject) {
        let imp = self.imp();

        pkg.property_expression("flags")
            .chain_closure::<bool>(closure!(|_: Option<glib::Object>, flags: PkgFlags| {
                flags.intersects(PkgFlags::UPDATES)
            }))
            .bind(&imp.version_image.get(), "visible", gtk::Widget::NONE);

        pkg.property_expression("version")
            .bind(&imp.version_label.get(), "label", gtk::Widget::NONE);

        imp.name_label.set_label(&pkg.name());
        imp.status_image.set_visible(pkg.flags().intersects(PkgFlags::INSTALLED));
        imp.status_image.set_icon_name(Some(pkg.status_icon_symbolic()));
        imp.status_label.set_label(pkg.status());
        imp.repository_label.set_label(&pkg.repository());
        imp.size_label.set_label(&pkg.install_size_string());
        imp.groups_image.set_visible(!pkg.groups().is_empty());
        imp.groups_label.set_label(pkg.groups());
    }
}

impl Default for PackageItem {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
