use gtk::subclass::prelude::*;
use gtk::prelude::{GObjectPropertyExpressionExt, WidgetExt};
use gtk::glib;
use glib::closure_local;

use crate::{
    pkg_object::PkgObject,
    tag_label::{TagLabel, TagType}
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

        let update_expr = item.property_expression("item")
            .chain_property::<PkgObject>("update-version");

        // Bind update version to version label tag style
        let update_type_expr = update_expr.chain_closure::<TagType>(closure_local!(
            |_: Option<glib::Object>, update_version: Option<String>| {
                if update_version.is_some() {
                    TagType::Warning
                } else {
                    TagType::Accent
                }
            }
        ));

        update_type_expr.bind(&imp.version_tag.get(), "tag-type", glib::Object::NONE);

        // Bind update version to version label text
        let update_text_expr = update_expr.chain_closure::<String>(closure_local!(
            |_: Option<glib::Object>, update_version: Option<String>| {
                update_version.unwrap_or_default()
            }
        ));

        update_text_expr.bind(&imp.version_tag.get(), "text", glib::Object::NONE);
    }

    //---------------------------------------
    // Bind function
    //---------------------------------------
    pub fn bind(&self, pkg: &PkgObject) {
        let imp = self.imp();

        imp.name_label.set_label(&pkg.name());

        imp.repository_tag.set_text(pkg.repo_display());
        imp.version_tag.set_text(pkg.version());

        imp.status_tag.set_visible(pkg.is_installed());
        imp.status_tag.set_tag_type(pkg.status_tag_type());
        imp.status_tag.set_text(pkg.status());

        imp.groups_tag.set_visible(!pkg.groups().is_empty());
        imp.groups_tag.set_text(pkg.groups().join(" | "));
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
