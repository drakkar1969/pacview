use gtk::subclass::prelude::*;
use gtk::prelude::{GObjectPropertyExpressionExt, WidgetExt};
use gtk::glib;
use glib::closure_local;

use crate::{
    pkg_object::PkgObject,
    tag_label::TagLabel
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
        pub(super) repository_tag: TemplateChild<TagLabel>,
        #[template_child]
        pub(super) version_tag: TemplateChild<TagLabel>,
        #[template_child]
        pub(super) update_tag: TemplateChild<TagLabel>,
        #[template_child]
        pub(super) groups_tag: TemplateChild<TagLabel>,
        #[template_child]
        pub(super) status_tag: TemplateChild<TagLabel>,
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

        // Get expressions for update version
        let update_expr = item.property_expression("item")
            .chain_property::<PkgObject>("update-version")
            .chain_closure::<String>(closure_local!(
                |_: Option<glib::Object>, update: Option<String>| update.unwrap_or_default()
            ));

        // Bind update version to update tag text
        update_expr.bind(&imp.update_tag.get(), "text", glib::Object::NONE);

        // Bind update version to update tag visibility
        let has_update_expr = update_expr.chain_closure::<bool>(closure_local!(
            |_: Option<glib::Object>, update: String| !update.is_empty()
        ));

        has_update_expr.bind(&imp.update_tag.get(), "visible", glib::Object::NONE);

        // Bind update version to version tag visibility
        let no_update_expr = has_update_expr.chain_closure::<bool>(closure_local!(
            |_: Option<glib::Object>, has_update: bool| !has_update
        ));

        no_update_expr.bind(&imp.version_tag.get(), "visible", glib::Object::NONE);
    }

    //---------------------------------------
    // Bind function
    //---------------------------------------
    pub fn bind(&self, pkg: &PkgObject) {
        let imp = self.imp();

        imp.name_label.set_label(&pkg.name());

        imp.repository_tag.set_text(pkg.repo_display());
        imp.version_tag.set_text(pkg.version());

        imp.groups_tag.set_visible(!pkg.groups().is_empty());
        imp.groups_tag.set_text(pkg.groups().join(" | "));

        imp.status_tag.set_visible(pkg.is_installed());
        imp.status_tag.set_tag_type(pkg.status_tag_type());
        imp.status_tag.set_text(pkg.status());
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
