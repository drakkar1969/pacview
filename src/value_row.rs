use std::cell::RefCell;

use gtk::glib::{self, SignalHandlerId};
use gtk::subclass::prelude::*;
use gtk::prelude::*;

use crate::prop_object::PropObject;

//------------------------------------------------------------------------------
// MODULE: ValueRow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/value_row.ui")]
    pub struct ValueRow {
        #[template_child]
        pub image: TemplateChild<gtk::Image>,
        #[template_child]
        pub label: TemplateChild<gtk::Label>,

        pub bindings: RefCell<Vec<glib::Binding>>,
        pub signal: RefCell<Option<glib::SignalHandlerId>>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for ValueRow {
        const NAME: &'static str = "ValueRow";
        type Type = super::ValueRow;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ValueRow {}
    impl WidgetImpl for ValueRow {}
    impl BoxImpl for ValueRow {}
    impl ValueRow {}
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION: ValueRow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct ValueRow(ObjectSubclass<imp::ValueRow>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl ValueRow {
    //-----------------------------------
    // Public new function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // Public property binding functions
    //-----------------------------------
    pub fn bind_properties(&self, property: &PropObject) {
        let imp = self.imp();

        let image = imp.image.get();
        let label = imp.label.get();

        let mut bindings = imp.bindings.borrow_mut();

        // Bind PropObject properties to widget properties and save bindings
        let binding = property.bind_property("icon", &image, "visible")
            .transform_to(|_, icon: Option<&str>| {
                Some(icon.is_some())
            })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
        bindings.push(binding);

        let binding = property.bind_property("icon", &image, "icon-name")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
        bindings.push(binding);

        let binding = property.bind_property("value", &label, "label")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
        bindings.push(binding);
    }

    pub fn unbind_properties(&self) {
        // Unbind PropObject properties from widgets
        for binding in self.imp().bindings.borrow_mut().drain(..) {
            binding.unbind();
        }
    }

    //-----------------------------------
    // Public signal binding functions
    //-----------------------------------
    pub fn add_label_signal(&self, signal: SignalHandlerId) {
        // Save label signal
        self.imp().signal.replace(Some(signal));
    }

    pub fn drop_label_signal(&self) {
        // Disconnect label signal
        let imp = self.imp();

        if let Some(signal) = imp.signal.take() {
            imp.label.disconnect(signal);
        }
    }
}
