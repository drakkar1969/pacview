use std::cell::{Cell, RefCell};

use gtk::{glib, gdk};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
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
        pub(super) prop_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) text_widget: TemplateChild<TextWidget>,

        #[property(get, set, builder(PropType::default()))]
        ptype: Cell<PropType>,
        #[property(get, set, nullable)]
        icon: RefCell<Option<String>>,
        #[property(get, set)]
        label: RefCell<String>,
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
        type ParentType = gtk::ListBoxRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
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
            obj.setup_controllers();
        }

        //-----------------------------------
        // Dispose function
        //-----------------------------------
        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for PropertyValue {}
    impl ListBoxRowImpl for PropertyValue {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PropertyValue
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PropertyValue(ObjectSubclass<imp::PropertyValue>)
    @extends gtk::ListBoxRow, gtk::Widget,
    @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl PropertyValue {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(ptype: PropType) -> Self {
        glib::Object::builder()
            .property("ptype", ptype)
            .build()
    }

    //-----------------------------------
    // Set package link handler function
    //-----------------------------------
    pub fn set_pkg_link_handler(&self, handler: RustClosure) {
        let imp = self.imp();

        imp.text_widget.connect_closure("package-link", false, handler);
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind properties to widgets
        self.bind_property("label", &imp.prop_label.get(), "label")
            .sync_create()
            .build();

        self.bind_property("icon", &imp.image.get(), "visible")
            .transform_to(|_, icon: Option<String>| Some(icon.is_some()))
            .sync_create()
            .build();

        self.bind_property("icon", &imp.image.get(), "icon-name")
            .sync_create()
            .build();

        self.bind_property("ptype", &imp.text_widget.get(), "ptype")
            .sync_create()
            .build();

        self.bind_property("text", &imp.text_widget.get(), "text")
            .sync_create()
            .build();
    }

    //-----------------------------------
    // Setup controllers
    //-----------------------------------
    fn setup_controllers(&self) {
        let imp = self.imp();

        // Add mouse click controller
        let click_gesture = gtk::GestureClick::builder()
            .button(gdk::BUTTON_PRIMARY)
            .build();

        click_gesture.connect_pressed(clone!(
            #[weak] imp,
            move |_, _, _, _| {
                imp.text_widget.grab_focus();
            }
        ));

        self.add_controller(click_gesture);
    }
}
