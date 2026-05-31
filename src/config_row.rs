use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

//------------------------------------------------------------------------------
// MODULE: ConfigRow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/config_row.ui")]
    pub struct ConfigRow {
        #[template_child]
        pub(super) suffix_image: TemplateChild<gtk::Image>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for ConfigRow {
        const NAME: &'static str = "ConfigRow";
        type Type = super::ConfigRow;
        type ParentType = adw::ActionRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ConfigRow {}
    impl WidgetImpl for ConfigRow {}
    impl ListBoxRowImpl for ConfigRow {}
    impl PreferencesRowImpl for ConfigRow {}
    impl ActionRowImpl for ConfigRow {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: ConfigRow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct ConfigRow(ObjectSubclass<imp::ConfigRow>)
    @extends adw::ActionRow, adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
    @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl ConfigRow {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(label: &str, property: &str, action_name: Option<&str>) -> Self {
        let obj: Self = glib::Object::builder()
            .property("title", label)
            .property("subtitle", property)
            .property("action-name", action_name)
            .build();

        let imp = obj.imp();

        if action_name.is_some() && !property.is_empty() {
            obj.set_activatable(true);
            obj.set_action_target(Some(&property.to_variant()));

            imp.suffix_image.set_icon_name(Some("go-next-symbolic"));
        }

        obj
    }
}
