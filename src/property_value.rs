use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::RustClosure;

use crate::text_widget::{TextWidget, PropType};

//------------------------------------------------------------------------------
// MODULE: PropertyValue
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PropertyValue)]
    #[template(resource = "/com/github/PacView/ui/property_value.ui")]
    pub struct PropertyValue {
        #[template_child]
        pub(super) image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) text_widget: TemplateChild<TextWidget>,

        #[property(get, set, builder(PropType::default()))]
        ptype: Cell<PropType>,
        #[property(get, set, nullable)]
        icon: RefCell<Option<String>>,
        #[property(get, set)]
        text: RefCell<String>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PropertyValue {
        const NAME: &'static str = "PropertyValue";
        type Type = super::PropertyValue;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_layout_manager_type::<gtk::BoxLayout>();
            klass.set_css_name("property-value");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PropertyValue {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
        }

        //-----------------------------------
        // Dispose function
        //-----------------------------------
        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for PropertyValue {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PropertyValue
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PropertyValue(ObjectSubclass<imp::PropertyValue>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PropertyValue {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(ptype: PropType, link_handler: RustClosure) -> Self {
        let widget: Self = glib::Object::builder()
            .property("ptype", ptype)
            .build();

        let imp = widget.imp();

        imp.text_widget.connect_closure("package-link", false, link_handler);

        widget
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind properties to widgets
        self.bind_property("ptype", &imp.text_widget.get(), "ptype")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        self.bind_property("text", &imp.text_widget.get(), "text")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        self.bind_property("icon", &imp.image.get(), "visible")
            .transform_to(|_, icon: Option<String>| Some(icon.is_some()))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        self.bind_property("icon", &imp.image.get(), "icon-name")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
    }
}
