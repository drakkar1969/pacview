use gtk::subclass::prelude::*;
use gtk::prelude::{GObjectPropertyExpressionExt, WidgetExt};
use gtk::glib;
use glib::closure;

use crate::{
    pkg_data::PkgFlags,
    pkg_object::PkgObject
};

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
        pub(super) version_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) repository_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) groups_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) update_label: TemplateChild<gtk::Label>,
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
    // Setup function
    //---------------------------------------
    pub fn setup(&self, item: &gtk::ListItem) {
        let imp = self.imp();

        let expression = item.property_expression("item");
        let update_expr = expression.chain_property::<PkgObject>("update-version");

        expression
            .chain_property::<PkgObject>("version")
            .bind(&imp.version_label.get(), "label", gtk::Widget::NONE);

        update_expr
            .chain_closure::<bool>(closure!(|_: Option<glib::Object>, update: Option<String>| {
                update.is_none()
            }))
            .bind(&imp.version_label.get(), "visible", gtk::Widget::NONE);

        update_expr
            .chain_closure::<bool>(closure!(|_: Option<glib::Object>, update: Option<String>| {
                update.is_some()
            }))
            .bind(&imp.update_label.get(), "visible", gtk::Widget::NONE);

        update_expr
            .bind(&imp.update_label.get(), "label", gtk::Widget::NONE);
    }

    //---------------------------------------
    // Bind function
    //---------------------------------------
    pub fn bind(&self, pkg: &PkgObject) {
        let imp = self.imp();

        imp.name_label.set_label(&pkg.name());

        imp.repository_label.set_label(&pkg.repository());

        imp.status_label.set_visible(pkg.flags().intersects(PkgFlags::INSTALLED));
        imp.status_label.set_css_classes(&pkg.status_css_classes());
        imp.status_label.set_label(pkg.status());

        imp.groups_label.set_visible(!pkg.groups().is_empty());
        imp.groups_label.set_label(&pkg.groups().join(" | "));
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
